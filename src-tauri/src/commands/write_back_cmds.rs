use super::*;

/// (id, account_id, due_date, closing_date, card_name, stated_total_cents, purchases_sum_cents)
type CardInvoiceRow = (String, String, String, String, String, Option<i64>, i64);

/// Lê as transações e as converte para candidatas de write-back da grade diária do `year`
/// (magnitude positiva; a coluna vem do tipo). Cada fatura de cartão é uma linha na seção
/// `CARTÕES:` da Saída do vencimento, preservando a conta que ela representa. A composição ocorre
/// antes do planejador porque a grade possui uma única célula por data×coluna e a agregação genérica
/// deliberadamente descarta notas em colisões. Sem cartão configurado, crédito continua na data da
/// compra. `transfer` (Economia) NÃO entra aqui — vai para a aba `Economia`.
pub(crate) async fn load_write_back_txns(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<WriteBackTxn>, String> {
    use std::collections::{BTreeMap, HashMap, HashSet};

    let today = chrono::Local::now().date_naive();
    let (has_card,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM account WHERE type = 'credit_card'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query card accounts: {e}"))?;
    // Alias → conta (não "alias conhecido" como HashSet): o discriminador de substituição é
    // "existe fatura para (conta, ciclo) da linha", e resolver a conta é o passo que torna essa
    // checagem possível por vencimento (ver `invoiced_accounts_by_due` abaixo).
    let alias_to_account: HashMap<String, String> = if has_card > 0 {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.id, a.name FROM account a WHERE a.type = 'credit_card' \
             UNION ALL \
             SELECT a.id, ca.alias FROM card_alias ca \
             JOIN account a ON a.id = ca.account_id \
             WHERE a.type = 'credit_card'",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query card aliases for write-back: {e}"))?;
        let mut map = HashMap::new();
        for (account_id, alias) in rows {
            let normalized = crate::cards::normalize_alias(&alias);
            if !normalized.is_empty() {
                map.entry(normalized).or_insert(account_id);
            }
        }
        map
    } else {
        HashMap::new()
    };

    // Faturas do ano são agrupadas primeiro por vencimento. A ordem das linhas dentro da nota é
    // estável por `created_at, id` da conta, para que um preview repetido não mude sem dado novo.
    let mut invoices_by_due: BTreeMap<String, Vec<write_back::TxnLineItem>> = BTreeMap::new();
    // Por vencimento, o conjunto de contas que TÊM fatura viva ali — a chave unificadora da
    // substituição: um alias conhecido cuja conta não aparece aqui (proposta aceita, fatura ainda
    // não materializada) é preservado como um desconhecido, nunca suprimido sem substituta.
    let mut invoiced_accounts_by_due: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    if has_card > 0 {
        let invoices: Vec<CardInvoiceRow> = sqlx::query_as(
            "SELECT i.id, i.account_id, i.due_date, i.closing_date, a.name, i.stated_total_cents, \
                    COALESCE(SUM(t.amount), 0) \
             FROM invoice i \
             JOIN account a ON a.id = i.account_id \
             LEFT JOIN \"transaction\" t ON t.invoice_id = i.id AND t.type = 'expense' \
             WHERE i.due_date >= ?1 AND i.due_date < ?2 AND i.due_date >= ?3 \
             GROUP BY i.id, i.account_id, i.due_date, i.closing_date, a.name, i.stated_total_cents, \
                      a.created_at, a.id \
             ORDER BY i.due_date, a.created_at, a.id",
        )
        .bind(format!("{year:04}-01-01"))
        .bind(format!("{}-01-01", year + 1))
        .bind(today.to_string())
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query card invoices: {e}"))?;

        for (
            _id,
            account_id,
            due_date,
            closing_date,
            card_name,
            stated_total_cents,
            purchases_sum_cents,
        ) in invoices
        {
            let closing = NaiveDate::parse_from_str(&closing_date, "%Y-%m-%d")
                .map_err(|_| format!("fechamento de fatura inválido: {closing_date}"))?;
            let due = NaiveDate::parse_from_str(&due_date, "%Y-%m-%d")
                .map_err(|_| format!("vencimento de fatura inválido: {due_date}"))?;
            if matches!(
                crate::cards::invoice_status(today, closing, due),
                crate::cards::InvoiceStatus::Paga
            ) {
                continue;
            }
            invoiced_accounts_by_due
                .entry(due_date.clone())
                .or_default()
                .insert(account_id);
            invoices_by_due
                .entry(due_date)
                .or_default()
                .push(write_back::TxnLineItem {
                    amount_cents: crate::cards::effective_total_cents(
                        stated_total_cents,
                        purchases_sum_cents,
                    )
                    .abs(),
                    description: card_name,
                    section: None,
                });
        }
    }

    // Entrada + Saída/Diário (expense não-crédito) do ano, cada um na sua data.
    let rows: Vec<(String, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT id, type, date, amount, is_fixed, COALESCE(description, '') FROM \"transaction\" \
         WHERE date >= ?1 AND date < ?2 \
           AND (type <> 'expense' OR payment_method IS NULL OR payment_method <> 'credit') \
           AND id NOT LIKE 'derived:%' \
           AND scenario_id IS NULL \
         ORDER BY date, id",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query txns: {e}"))?;

    let mut out = Vec::new();
    let mut saidas_by_date: BTreeMap<String, Vec<(String, i64, String)>> = BTreeMap::new();
    for (id, t, date, amount, is_fixed, description) in rows {
        let mag = amount.abs();
        let kind = match t.as_str() {
            "income" => import::RowKind::Entrada,
            "expense" => {
                if is_fixed != 0 {
                    import::RowKind::Saida
                } else {
                    import::RowKind::Diario
                }
            }
            _ => continue, // transfer (Economia) → aba Economia
        };
        if kind == import::RowKind::Saida {
            saidas_by_date
                .entry(date)
                .or_default()
                .push((id, mag, description));
            continue;
        }
        out.push(WriteBackTxn {
            date,
            kind,
            amount_cents: mag,
            items: load_txn_items(pool, &id).await?,
        });
    }

    // A grade tem uma única célula Saída por dia. Quando ela contém faturas, TODAS as Saídas
    // daquele dia precisam formar a mesma candidata itemizada; deixar uma segunda candidata solta
    // faria o agregador descartar a nota e apagaria a seção CARTÕES no write-back.
    for (date, parents) in saidas_by_date {
        if let Some(invoice_items) = invoices_by_due.remove(&date) {
            let mut card_section = None;
            let mut non_card_total = 0;
            let mut composed = Vec::new();
            for (id, amount_cents, description) in parents {
                let parent_items = load_all_txn_items(pool, &id).await?;
                if parent_items.is_empty() {
                    non_card_total += amount_cents;
                    composed.push(write_back::TxnLineItem {
                        amount_cents,
                        description,
                        section: None,
                    });
                    continue;
                }
                let invoiced_accounts = invoiced_accounts_by_due.get(&date);
                let replaced_total: i64 = parent_items
                    .iter()
                    .filter(|item| {
                        import::classify_line_item(item.section.as_deref(), &item.description)
                            == import::ItemKind::Cartao
                            && card_line_covered_by_invoice(
                                &item.description,
                                &alias_to_account,
                                invoiced_accounts,
                            )
                    })
                    .map(|item| item.amount_cents.abs())
                    .sum();
                non_card_total += amount_cents - replaced_total;
                for item in parent_items {
                    let is_card =
                        import::classify_line_item(item.section.as_deref(), &item.description)
                            == import::ItemKind::Cartao;
                    if is_card && card_section.is_none() {
                        card_section = item.section.clone();
                    }
                    let is_covered_by_invoice = is_card
                        && card_line_covered_by_invoice(
                            &item.description,
                            &alias_to_account,
                            invoiced_accounts,
                        );
                    if !is_covered_by_invoice {
                        composed.push(item);
                    }
                }
            }
            let card_section = card_section.unwrap_or_else(|| "CARTÕES:".to_string());
            let invoice_total: i64 = invoice_items
                .iter()
                .map(|item| item.amount_cents.abs())
                .sum();
            composed.extend(invoice_items.into_iter().map(|mut item| {
                item.section = Some(card_section.clone());
                item
            }));
            out.push(WriteBackTxn {
                date,
                kind: import::RowKind::Saida,
                amount_cents: non_card_total + invoice_total,
                items: Some(composed),
            });
        } else {
            for (id, amount_cents, _description) in parents {
                out.push(WriteBackTxn {
                    date: date.clone(),
                    kind: import::RowKind::Saida,
                    amount_cents,
                    items: load_txn_items(pool, &id).await?,
                });
            }
        }
    }

    // Células futuras ainda vazias não têm uma candidata-pai para compor: a fatura se torna a
    // candidata completa, com uma linha de nota por cartão.
    for (due_date, items) in invoices_by_due {
        let amount_cents = items.iter().map(|item| item.amount_cents.abs()).sum();
        out.push(WriteBackTxn {
            date: due_date,
            kind: import::RowKind::Saida,
            amount_cents,
            items: Some(
                items
                    .into_iter()
                    .map(|mut item| {
                        item.section = Some("CARTÕES:".to_string());
                        item
                    })
                    .collect(),
            ),
        });
    }

    if has_card == 0 {
        // Sem cartão: não há fatura para representar, então crédito do ano cai na própria data.
        let credit: Vec<(String, i64)> = sqlx::query_as(
            "SELECT date, amount FROM \"transaction\" \
             WHERE type='expense' AND payment_method='credit' AND date >= ?1 AND date < ?2 \
               AND scenario_id IS NULL ORDER BY date, id",
        )
        .bind(format!("{year:04}-01-01"))
        .bind(format!("{}-01-01", year + 1))
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query credit nocard: {e}"))?;
        for (date, amount) in credit {
            out.push(WriteBackTxn {
                date,
                kind: import::RowKind::Saida,
                amount_cents: amount.abs(),
                items: None,
            });
        }
    }
    Ok(out)
}

/// Casa uma linha de nota `Cartao` com uma fatura VIVA no mesmo vencimento — o discriminador de
/// substituição/supressão do write-back. "Alias conhecido" sozinho não basta: uma proposta aceita
/// cria conta+alias mas não materializa a fatura observada até o próximo import, e essa janela não
/// pode apagar a linha sem substituta. A descrição é casada ANTES do `#` (mesma regra do import em
/// `item.description.split('#').next()`), para que marcadores como `#reembolso:` não quebrem o
/// casamento do alias.
fn card_line_covered_by_invoice(
    description: &str,
    alias_to_account: &std::collections::HashMap<String, String>,
    invoiced_accounts: Option<&std::collections::HashSet<String>>,
) -> bool {
    let alias = crate::cards::normalize_alias(description.split('#').next().unwrap_or("").trim());
    let Some(account_id) = alias_to_account.get(&alias) else {
        return false;
    };
    invoiced_accounts.is_some_and(|accounts| accounts.contains(account_id))
}

/// Partes itemizadas de uma transação como `TxnLineItem` (valor + descrição), para o
/// write-back reconstruir `=SUM(...)` + nota. `None` quando há < 2 partes — uma única parte não é um
/// breakdown (não há fórmula a montar), então cai na escrita RAW numérica. Ordenado por
/// `position` para a fórmula/nota saírem na ordem do dono.
async fn load_txn_items(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Option<Vec<write_back::TxnLineItem>>, String> {
    let rows = load_all_txn_items(pool, transaction_id).await?;
    if rows.len() < 2 {
        return Ok(None);
    }
    Ok(Some(rows))
}

/// Partes preservadas mesmo quando há só uma linha: a composição de cartão precisa reconhecer e
/// substituir a seção inteira antes de decidir se a célula final merece fórmula.
async fn load_all_txn_items(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Vec<write_back::TxnLineItem>, String> {
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT amount_cents, description, section FROM line_item \
         WHERE transaction_id = ?1 ORDER BY position",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query line items: {e}"))?;
    Ok(rows
        .into_iter()
        .map(
            |(amount_cents, description, section)| write_back::TxnLineItem {
                amount_cents,
                description,
                section,
            },
        )
        .collect())
}

/// Mensagem (typed-error por string, como o resto deste módulo) quando há conflitos de import
/// pendentes: o write-back é BLOQUEADO até a fila ser resolvida (ADR-0003), senão escreveríamos por
/// cima de um valor que o dono ainda está conciliando.
pub(crate) const CONFLICTS_PENDING_MSG: &str =
    "Resolva os conflitos de importação antes de enviar.";

/// Erro quando a planilha mudou ENTRE o preview e o apply: a aprovação do dono vale para o que ele
/// VIU; uma edição concorrente exige re-revisão (não sobrescrever às cegas).
pub(crate) const SHEET_CHANGED_MSG: &str =
    "A planilha mudou desde a prévia — gere o preview de novo e revise antes de enviar.";

/// Conta conflitos de import ainda não resolvidos. > 0 ⇒ o write-back deve abortar.
pub(crate) async fn unresolved_conflict_count(pool: &SqlitePool) -> Result<i64, String> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("conflict count: {e}"))?;
    Ok(count)
}

/// Aborta o write-back se houver conflitos de import pendentes. Chamada ANTES de qualquer escrita —
/// inclusive antes de tocar o `SheetsClient` — para que um plano nunca seja enviado sob conflito.
pub(crate) async fn guard_no_pending_conflicts(pool: &SqlitePool) -> Result<(), String> {
    if unresolved_conflict_count(pool).await? > 0 {
        return Err(CONFLICTS_PENDING_MSG.to_string());
    }
    Ok(())
}

/// Decisão PURA da revalidação de frescura: a aprovação do dono vale para a revisão que
/// ele VIU (`seen`); se o `current` (modifiedTime relido no apply) for DIFERENTE, a planilha mudou
/// → aborta. Comparação por igualdade exata da string RFC-3339 do Drive (qualquer edição a avança).
pub(crate) fn staleness_check(seen: &str, current: &str) -> Result<(), String> {
    if current != seen {
        return Err(SHEET_CHANGED_MSG.to_string());
    }
    Ok(())
}

/// Cria um `SheetsClient` autenticado (mesmo caminho de token de `build_write_back_plan`/
/// `build_economia_plan`). Usado pelas prévias RICAS para buscar o `modifiedTime` ANTES de ler os
/// VALORES da aba — assim o `preview_revision` corresponde a um estado NÃO mais novo que o diff
/// (fecha o TOCTOU: uma edição concorrente após a foto do `modifiedTime` só pode tornar o estado de
/// apply MAIS novo, disparando o gate de frescura e forçando re-revisão; nunca aprova um diff velho).
async fn make_authenticated_client(
    app_dir: &std::path::Path,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<SheetsClient, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
            .await?;
    Ok(SheetsClient::new(token))
}

/// Núcleo compartilhado por `preview_write_back` (read-only) e `apply_write_back` (escreve): lê a
/// aba, resolve layout+mappings, carrega as transações do ano e planeja o diff célula a célula.
/// Devolve o `SheetsClient` autenticado (para o apply reusar na escrita) + o plano.
pub(crate) async fn build_write_back_plan(
    app_dir: &std::path::Path,
    pool: &SqlitePool,
    spreadsheet_id: &str,
    sheet_name: &str,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<(SheetsClient, Vec<CellWrite>), String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let range = quote_sheet(sheet_name);
    let values = client.get_sheet_values(spreadsheet_id, &range).await?;

    let layout = import::get_layout_for_sheet(pool, sheet_name)
        .await?
        .ok_or("layout não detectado para esta aba — rode a detecção primeiro")?;
    let mappings = import::get_active_mappings_for_sheet(pool, sheet_name).await?;
    // Ano não detectado → nada a planejar (nunca assume 2025).
    let Some(year) = layout.year else {
        return Ok((client, Vec::new()));
    };
    let txns = load_write_back_txns(pool, year).await?;
    let plan = write_back::plan_write_back(&values.values, &layout, &mappings, &txns);
    Ok((client, plan))
}

/// Pré-visualização do write-back: lê a planilha e produz o DIFF (transação → célula) para
/// aprovação. READ-ONLY — não escreve nada, então é seguro mesmo com a flag desligada.
#[tauri::command]
pub async fn preview_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<CellWrite>, String> {
    let (_client, plan) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;
    Ok(plan)
}

/// Resultado da prévia: o diff, o `preview_revision` capturado no Drive para detectar edição
/// concorrente no apply e o estado do gate de conflitos.
#[derive(serde::Serialize)]
pub struct WriteBackPreviewResult {
    pub cells: Vec<CellWrite>,
    /// `modifiedTime` RFC-3339 do Drive na hora da prévia (token de frescura para o apply).
    pub preview_revision: String,
    /// Há conflitos de import pendentes? A UI desabilita o envio (espelha o gate do backend).
    pub conflicts_pending: bool,
}

/// Prévia rica e read-only: mesmo plano do `preview_write_back`, mais
/// o `preview_revision` (re-revisão por edição concorrente) e o flag de conflitos pendentes.
/// Read-only — seguro com a flag desligada.
#[tauri::command]
pub async fn preview_write_back_status(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<WriteBackPreviewResult, String> {
    // Foto do `modifiedTime` ANTES de ler os VALORES (fecha o TOCTOU): o token de frescura passa a
    // corresponder a um estado NÃO mais novo que o diff. Uma edição entre esta foto (T1) e a leitura
    // dos valores (T2) só pode deixar o estado de apply mais novo → o gate de frescura dispara.
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let preview_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;
    let (_client, cells) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;
    let conflicts_pending = unresolved_conflict_count(pool.inner()).await? > 0;
    Ok(WriteBackPreviewResult {
        cells,
        preview_revision,
        conflicts_pending,
    })
}

/// Estado da flag de write-back (a UI usa para mostrar "desligado" e desabilitar o envio).
#[tauri::command]
pub fn write_back_enabled() -> bool {
    write_back::WRITE_BACK_ENABLED
}

/// Lê uma preferência local (KV). `None` quando a chave nunca foi gravada.
#[tauri::command]
pub async fn get_app_setting(
    pool: State<'_, SqlitePool>,
    key: String,
) -> Result<Option<String>, String> {
    app_setting_get(pool.inner(), &key).await
}

pub(crate) async fn app_setting_get(
    pool: &SqlitePool,
    key: &str,
) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_setting WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("get setting: {e}"))?;
    Ok(row.map(|(v,)| v))
}

/// Grava uma preferência local (KV), sobrescrevendo.
#[tauri::command]
pub async fn set_app_setting(
    pool: State<'_, SqlitePool>,
    key: String,
    value: String,
) -> Result<(), String> {
    app_setting_set(pool.inner(), &key, &value).await
}

pub(crate) async fn app_setting_set(
    pool: &SqlitePool,
    key: &str,
    value: &str,
) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_setting (key, value) VALUES (?1, ?2)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| format!("set setting: {e}"))?;
    Ok(())
}

/// Backup do banco local em `dest_path` (escolhido pelo usuário no save dialog). Usa `VACUUM INTO`,
/// que cria uma cópia CONSISTENTE mesmo com o banco em uso e em modo WAL — diferente de copiar o
/// arquivo `.db` cru, que poderia capturar um estado parcial (WAL não aplicado). Local-first: o dono
/// do dado precisa conseguir levar uma cópia íntegra. Retorna o caminho gravado.
#[tauri::command]
pub async fn backup_database(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    dest_path: String,
) -> Result<String, String> {
    backup_db(pool.inner(), &app_dir.0.join("neko-finance.db"), &dest_path).await
}

pub(crate) async fn backup_db(
    pool: &SqlitePool,
    active_db: &std::path::Path,
    dest_path: &str,
) -> Result<String, String> {
    use std::path::{Path, PathBuf};
    let dest = dest_path.trim();
    if dest.is_empty() {
        return Err("escolha um destino para o backup".into());
    }
    let dest_buf = PathBuf::from(dest);

    // a) Extensão obrigatória .db (case-insensitive): o save dialog da UI já filtra, mas o backend
    //    reforça a última linha de defesa contra destinos arbitrários vindos do renderer.
    if dest_buf
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| !e.eq_ignore_ascii_case("db"))
        .unwrap_or(true)
    {
        return Err("o backup deve ter extensão .db".into());
    }

    // b) Overwrite seguro: só permite substituir um arquivo pré-existente se ele for um banco SQLite
    //    (mesmo magic header). Isso impede que um renderer comprometido use o backup para sobrescrever
    //    um arquivo arbitrário do usuário; substituir um backup anterior continua permitido.
    if dest_buf.exists() {
        // read_exact de 16 bytes, não fs::read: o destino pode ser um backup de gigabytes e só o
        // magic header interessa.
        let mut header = [0u8; 16];
        let is_sqlite = std::fs::File::open(&dest_buf)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut header))
            .is_ok()
            && &header == b"SQLite format 3\0";
        if !is_sqlite {
            return Err(
                "o destino já existe e não é um backup SQLite — escolha outro nome.".into(),
            );
        }
    }

    // NUNCA fazer backup SOBRE o banco em uso: apagá-lo/escrevê-lo desvincularia o arquivo aberto
    // (Unix) e perderia escritas futuras, ou falharia travado (Windows). Só rejeita quando o destino
    // já existe E é o mesmo arquivo (canonicalize); um destino novo nunca pode ser o banco ativo.
    if let (Ok(d), Ok(a)) = (
        std::fs::canonicalize(&dest_buf),
        std::fs::canonicalize(active_db),
    ) && d == a
    {
        return Err("escolha um destino diferente do banco em uso.".into());
    }

    // Grava num TEMP no MESMO diretório do destino e só então faz `rename` (atômico no mesmo
    // filesystem): o backup ANTERIOR só é substituído quando o novo está completo. Se o VACUUM
    // falhar, o destino antigo permanece intacto. (`VACUUM INTO` recusa arquivo já existente, daí o
    // temp único; e roda como SQL BRUTO via `raw_sql` — um prepared statement o silenciaria.)
    let parent = dest_buf.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".neko-backup-{}.tmp", uuid::Uuid::new_v4()));
    let tmp_sql = tmp.to_string_lossy().replace('\'', "''");
    let stmt = format!("VACUUM INTO '{tmp_sql}'");
    if let Err(e) = sqlx::raw_sql(sqlx::AssertSqlSafe(stmt)).execute(pool).await {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("backup: {e}"));
    }
    std::fs::rename(&tmp, &dest_buf).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("finalizar backup: {e}")
    })?;
    Ok(dest.to_string())
}

/// Resultado do `apply_write_back`: nº de células escritas + um aviso NÃO-bloqueante
/// quando a escrita da NOTA de célula itemizada falhou (ex.: token sem escopo de escrita). O valor
/// (total/fórmula) já foi escrito com sucesso — a nota é enriquecimento, então sua falha não aborta.
#[derive(serde::Serialize)]
pub struct WriteBackResult {
    pub written: usize,
    pub note_warning: Option<String>,
}

/// Aplica o write-back: escreve as células DIVERGENTES de volta na aba. Trava-mestra: enquanto
/// `WRITE_BACK_ENABLED` estiver desligado, falha cedo e NÃO escreve nada. A UI já obteve o diff via
/// `preview_write_back` e o humano aprovou; aqui só replanejamos (a planilha pode ter mudado) e
/// escrevemos as células que ainda diferem.
///
/// Células ITEMIZADAS (≥2 partes) escrevem a FÓRMULA `=SUM(...)` via USER_ENTERED + a
/// NOTA por-parte; células normais seguem RAW numérico. A nota é best-effort:
/// se falhar (ex.: 403 readonly), devolvemos um `note_warning` em vez de abortar — o valor já está
/// gravado. Todos os gates (flag, conflito, escopo, frescura e blocklist de fórmula via
/// `plan_write_back`) permanecem intactos e ANTES de qualquer escrita.
#[tauri::command]
pub async fn apply_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
    // Token de frescura devolvido por `preview_write_back_status`. `None` no caminho sem token
    // da UI atual; quando presente, o apply ABORTA se a planilha mudou desde a prévia.
    preview_revision: Option<String>,
) -> Result<WriteBackResult, String> {
    write_back::ensure_write_back_enabled()?;
    // Gate de conflito: nunca escrever sob conflitos de import pendentes — ANTES de tocar
    // o cliente do Sheets.
    guard_no_pending_conflicts(pool.inner()).await?;

    let resolved_secret = oauth::pkce::resolve_client_secret(client_secret.clone());
    // Escopo de escrita: falha cedo com erro de re-consentimento se o token for readonly.
    oauth::token_store::ensure_write_scope(&app_dir.0, &client_id, resolved_secret.as_deref())
        .await?;

    // Foto do `modifiedTime` ANTES de ler os VALORES da aba (mesmo padrão de
    // `preview_write_back_status`). No caminho LEGADO (sem `preview_revision`), esta foto é o "estado
    // que o apply assumiu como base": comparada com a foto pós-plano, fecha o TOCTOU mesmo sem o
    // token de prévia da UI rica. Uma edição concorrente entre as duas fotos AVANÇA o `modifiedTime`
    // → o gate de frescura dispara e o apply aborta (nenhum diff velho chega à planilha).
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let early_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;

    let (client, plan) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;

    // Revalida a frescura SEMPRE; nenhum caminho de apply escapa do gate. A foto posterior do
    // `modifiedTime` é comparada com `preview_revision`, quando presente, ou com `early_revision`
    // no caminho sem token. Divergência aborta antes da escrita.
    let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;
    match preview_revision.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(seen) => staleness_check(seen, &post_plan_revision)?,
        None => staleness_check(&early_revision, &post_plan_revision)?,
    }

    // Só as células que MUDARAM; range com nome da aba ('2026'!E3).
    let changed: Vec<&CellWrite> = plan.iter().filter(|c| c.changed).collect();

    // Separa as células itemizadas (fórmula USER_ENTERED) das normais (RAW numérico).
    // Não itemizadas usam número cru, RAW e independente de locale.
    let raw_updates: Vec<(String, f64)> = changed
        .iter()
        .filter(|c| c.formula.is_none())
        .map(|c| {
            (
                format!("{}!{}", quote_sheet(&sheet_name), c.a1),
                c.value_cents as f64 / 100.0,
            )
        })
        .collect();
    let formula_updates: Vec<(String, String)> = changed
        .iter()
        .filter_map(|c| {
            c.formula
                .as_ref()
                .map(|f| (format!("{}!{}", quote_sheet(&sheet_name), c.a1), f.clone()))
        })
        .collect();

    let mut written = client
        .batch_update_values(&spreadsheet_id, &raw_updates)
        .await?;
    written += client
        .batch_update_formulas(&spreadsheet_id, &formula_updates)
        .await?;

    // Notas de célula das itemizadas são best-effort e não fatais: o valor já foi
    // escrito; a nota é enriquecimento. A1 SEM nome de aba (o `batch_update_notes` resolve a aba).
    let note_updates: Vec<(String, String)> = changed
        .iter()
        .filter_map(|c| c.note_text.as_ref().map(|n| (c.a1.clone(), n.clone())))
        .collect();
    let note_warning: Option<String> = if note_updates.is_empty() {
        None
    } else {
        match client
            .batch_update_notes(&spreadsheet_id, &sheet_name, &note_updates)
            .await
        {
            Ok(_) => None,
            Err(e) if e.starts_with("NOTE_WRITE_PERMISSION:") => Some(
                "Notas de célula não foram atualizadas: consentimento de escrita necessário."
                    .into(),
            ),
            Err(e) => Some(format!("Notas de célula: {e}")),
        }
    };
    let notes_written = note_updates.is_empty() || note_warning.is_none();

    // Auditoria pós-escrita: realinha o `source_*` das transações escritas + registra a
    // escrita no `sync_log`, para que o próximo import reconheça os valores como a NOVA base (sem
    // conflito espúrio). Só roda em escrita bem-sucedida.
    if written > 0 {
        record_write_back_audit_with_notes(pool.inner(), &sheet_name, &changed, notes_written)
            .await?;
    }

    Ok(WriteBackResult {
        written,
        note_warning,
    })
}

/// Auditoria pós-escrita do write-back. Faz DUAS coisas, atômicas:
///
/// 1) Realinha `source_amount` (a BASE do merge de 3 vias) das transações cujas células acabaram de
///    ser escritas ao valor que foi para a planilha. Sem isto, o próximo import veria `local ==
///    novo-valor`, `sheet == novo-valor`, mas `base == valor-antigo` → `Conflict` espúrio (ambos
///    "mudaram"). Com a base realinhada, `base == sheet` → sem conflito.
/// 2) Registra a escrita no `sync_log` (event_type `write_back`, com `source_sheet`) como trilha de
///    auditoria de que aquele estado da aba veio do app.
///
/// `cells` são as células efetivamente ESCRITAS (já filtradas por `changed`). Mapeamos cada uma para
/// as transações por `(date, type, is_fixed)` derivados do `kind` — o mesmo critério de
/// `load_write_back_txns`. Nas Saídas, as linhas de nota de cartões realinham também as faturas que
/// a célula representa.
pub(crate) async fn record_write_back_audit(
    pool: &SqlitePool,
    sheet_name: &str,
    cells: &[&CellWrite],
) -> Result<usize, String> {
    record_write_back_audit_with_notes(pool, sheet_name, cells, true).await
}

/// Variante da auditoria que preserva a base de merge das faturas quando a nota não chegou à
/// planilha. Totais e fórmulas podem ter sido escritos, mas são as linhas da nota que identificam
/// os totais por cartão; avançar essa base sem a nota faria o próximo import sobrescrever o local.
pub(crate) async fn record_write_back_audit_with_notes(
    pool: &SqlitePool,
    sheet_name: &str,
    cells: &[&CellWrite],
    notes_written: bool,
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let profile_id: Option<(String,)> =
        sqlx::query_as("SELECT id FROM profile ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("query profile: {e}"))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("begin audit: {e}"))?;
    let mut realigned = 0usize;
    for c in cells {
        // Re-chaveia uma linha MANUAL (id UUID, fora do `sync_log`) escrita numa célula da grade
        // diária para o `row_id` determinístico de (aba, data, kind, slot 0) e registra o vínculo no
        // `sync_log`. Isso impede que o import crie um gêmeo com dupla contagem. A operação precede
        // o realinhamento de `source_amount` e nunca sobrescreve uma linha importada existente.
        rekey_manual_row_to_deterministic(&mut tx, sheet_name, c, profile_id.as_ref(), &now)
            .await?;

        // Saída combina o realinhamento da linha fixa de débito com as faturas identificadas pelas
        // linhas da seção de cartões na nota escrita.
        if c.kind.as_str() == "saida" {
            realigned += realign_saida_cell(&mut tx, c, &now, notes_written).await?;
            record_write_back_log(&mut tx, sheet_name, &now, profile_id.as_ref(), c).await?;
            continue;
        }
        // kind (string da célula) → critério de seleção da(s) transação(ões) na data.
        let updated = match c.kind.as_str() {
            "entrada" => {
                // Exclui linhas `derived:%` (sintetizadas, ex.: reembolso) — espelha
                // `load_write_back_txns`. Sem isto, o realinho de entrada sobrescreveria o
                // `source_amount` de linhas derivadas que não vêm 1:1 da planilha.
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
                     WHERE date = ?3 AND type = 'income' AND id NOT LIKE 'derived:%' \
                       AND scenario_id IS NULL",
                )
                .bind(c.value_cents)
                .bind(&now)
                .bind(&c.date)
                .execute(&mut *tx)
                .await
            }
            "diario" => {
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
                     WHERE date = ?3 AND type = 'expense' AND is_fixed = 0 \
                       AND (payment_method IS NULL OR payment_method <> 'credit') \
                       AND scenario_id IS NULL",
                )
                .bind(c.value_cents)
                .bind(&now)
                .bind(&c.date)
                .execute(&mut *tx)
                .await
            }
            "economia" => {
                // Economia é mensal e vive como ANOTAÇÃO em `economia_annotation`, não como
                // transação `economia:YYYY-MM`. Depois de escrever na origem, realinhamos a anotação
                // local ao valor escrito para que o import seguinte veja origem == anotação, sem
                // conflito espúrio.
                let (yy, mm) = c
                    .date
                    .split_once('-')
                    .and_then(|(y, m)| Some((y.parse::<i64>().ok()?, m.parse::<i64>().ok()?)))
                    .ok_or_else(|| format!("data de economia inválida: {}", c.date))?;
                if c.value_cents > 0 {
                    sqlx::query(
                        "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
                         VALUES ('', ?1, ?2, ?3, ?4) \
                         ON CONFLICT(profile_id, year, month) DO UPDATE SET \
                           amount_cents=excluded.amount_cents, updated_at=excluded.updated_at",
                    )
                    .bind(yy)
                    .bind(mm)
                    .bind(c.value_cents)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                } else {
                    sqlx::query(
                        "DELETE FROM economia_annotation WHERE profile_id='' AND year=?1 AND month=?2",
                    )
                    .bind(yy)
                    .bind(mm)
                    .execute(&mut *tx)
                    .await
                }
            }
            _ => continue, // kind desconhecido: nada a realinhar nem a auditar
        }
        .map_err(|e| format!("realign source_amount: {e}"))?;
        realigned += updated.rows_affected() as usize;

        record_write_back_log(&mut tx, sheet_name, &now, profile_id.as_ref(), c).await?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("commit audit: {e}"))?;
    Ok(realigned)
}

/// Converte uma linha MANUAL escrita numa célula da grade diária em uma linha
/// SHEET-BACKED de primeira classe, re-chaveando-a para o `row_id` DETERMINÍSTICO de `(aba, data,
/// kind, slot 0)` e registrando-a no `sync_log`. Isto fecha a duplicata do round-trip
/// manual→write-back→re-import: sem o re-chaveamento, o import recomputa o id determinístico daquela
/// célula, não acha a linha (id-UUID), e INSERE um gêmeo (dupla contagem no Saldo/totais). Depois
/// dele, o import faz UPSERT na MESMA linha (idempotente).
///
/// SLOT: a grade canônica tem no máximo UMA linha por `(data, kind)` (uma célula por dia×coluna; ver
/// `parse_rows_with_layout`) e `plan_write_back` agrega tudo da mesma `(data, kind)` numa única
/// célula → o slot do import é sempre `0`. Por isso o alvo é `row_id(aba, data, kind, 0)`.
///
/// GARANTIA DE NÃO-COLISÃO: só re-chaveia quando
/// (1) o alvo determinístico AINDA NÃO EXISTE como linha (senão sobrescreveria uma linha importada);
/// (2) há EXATAMENTE UMA linha candidata manual em `(data, critério-do-kind)` que NÃO está no
///     `sync_log` (linha importada/derivada nunca conta como candidata) e cujo id DIFERE do alvo.
/// 0 ou 2+ candidatas → mapeamento 1:1 ambíguo → NÃO re-chaveia (deixa como está). O LUMP de cartão
/// (várias compras de crédito) é naturalmente excluído pelo critério `saida` (débito fixo não-crédito).
///
/// FKs: as tabelas-filhas (`split`, `transaction_tag`, `line_item`, `import_conflict`) referenciam
/// `transaction.id` como `ON DELETE CASCADE` SEM `ON UPDATE CASCADE`; com `foreign_keys=ON` (produção)
/// um `UPDATE` cru do id quebraria a FK das filhas. Por isso COPIAMOS o pai para o novo id, repontamos
/// as filhas e só então apagamos o pai antigo — ordem segura sob `NO ACTION` (o novo pai já existe ao
/// repontar; a antiga não tem mais filhas ao ser apagada). O alvo é garantidamente livre por (1).
async fn rekey_manual_row_to_deterministic(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    c: &CellWrite,
    profile_id: Option<&(String,)>,
    now: &str,
) -> Result<(), String> {
    // Economia é anotação (sem linha na grade diária / sem row_id determinístico) — fora de escopo.
    let kind = match c.kind.as_str() {
        "entrada" => import::RowKind::Entrada,
        "saida" => import::RowKind::Saida,
        "diario" => import::RowKind::Diario,
        _ => return Ok(()),
    };

    // (1) Alvo determinístico (slot 0). Se JÁ EXISTE uma linha com esse id, é uma linha importada
    //     daquela célula → NÃO re-chavear (sobrescreveria/colidiria). STOP silencioso e seguro.
    let target = import::row_id(sheet_name, &c.date, kind, 0);
    let (target_exists,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM \"transaction\" WHERE id = ?1 AND scenario_id IS NULL",
    )
    .bind(&target)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| format!("rekey target check: {e}"))?;
    if target_exists > 0 {
        return Ok(());
    }

    // (2) Candidatas manuais (mesmos critérios do realinho de `source_amount`, espelhando
    //     `load_write_back_txns`), restritas às que NÃO estão no `sync_log` (linha importada/derivada
    //     nunca é candidata, via `NOT IN`) e cujo id difere do alvo (`id <> ?2`). EXATAMENTE UMA →
    //     re-chaveia; senão → no-op. SQL literal por arm (sem string dinâmica): `?1`=data, `?2`=alvo.
    //     A cláusula `id NOT IN (SELECT entity_id FROM sync_log ...)` exclui linhas importadas.
    let select_sql: &'static str = match kind {
        import::RowKind::Entrada => concat!(
            "SELECT id FROM \"transaction\" WHERE date = ?1 AND type='income' \
             AND id NOT LIKE 'derived:%' AND scenario_id IS NULL AND ",
            "id NOT IN (SELECT entity_id FROM sync_log WHERE entity_type='transaction') AND id <> ?2"
        ),
        import::RowKind::Diario => concat!(
            "SELECT id FROM \"transaction\" WHERE date = ?1 AND type='expense' AND is_fixed = 0 \
             AND (payment_method IS NULL OR payment_method <> 'credit') AND scenario_id IS NULL AND ",
            "id NOT IN (SELECT entity_id FROM sync_log WHERE entity_type='transaction') AND id <> ?2"
        ),
        import::RowKind::Saida => concat!(
            "SELECT id FROM \"transaction\" WHERE date = ?1 AND type='expense' AND is_fixed = 1 \
             AND (payment_method IS NULL OR payment_method <> 'credit') AND scenario_id IS NULL AND ",
            "id NOT IN (SELECT entity_id FROM sync_log WHERE entity_type='transaction') AND id <> ?2"
        ),
    };
    let candidates: Vec<(String,)> = sqlx::query_as(select_sql)
        .bind(&c.date)
        .bind(&target)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| format!("rekey candidates: {e}"))?;
    let [(old_id,)] = candidates.as_slice() else {
        // 0 candidatas (já era determinística/importada) ou 2+ (ambíguo) → não re-chaveia.
        return Ok(());
    };
    let old_id = old_id.clone();

    // Copia o pai para o novo id (alvo livre por (1)), repointa as filhas, apaga o pai antigo.
    sqlx::query(
        "INSERT INTO \"transaction\" \
           (id, type, amount, description, date, payment_method, is_fixed, from_account_id, \
            to_account_id, is_projection, recurrence_id, source_amount, source_description, \
            source_note, due_date, invoice_id, card_series_id, refund_invoice_id, refund_txn_id, \
            refund_series_id, created_at, updated_at) \
         SELECT ?1, type, amount, description, date, payment_method, is_fixed, from_account_id, \
            to_account_id, is_projection, recurrence_id, source_amount, source_description, \
            source_note, due_date, invoice_id, card_series_id, refund_invoice_id, refund_txn_id, \
            refund_series_id, created_at, ?3 \
         FROM \"transaction\" WHERE id = ?2",
    )
    .bind(&target)
    .bind(&old_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("rekey copy parent: {e}"))?;

    // Reponta as filhas (literais por tabela; sqlx exige &'static str). Cobre TODA tabela que
    // referencia `transaction.id`: `split`/`transaction_tag`/`line_item` (FK ON DELETE CASCADE),
    // `import_conflict` (mesma coluna, sem FK) e o alvo de reembolso (FK ON DELETE SET NULL).
    // `?1`=novo id, `?2`=id antigo.
    for stmt in [
        "UPDATE split SET transaction_id = ?1 WHERE transaction_id = ?2",
        "UPDATE transaction_tag SET transaction_id = ?1 WHERE transaction_id = ?2",
        "UPDATE line_item SET transaction_id = ?1 WHERE transaction_id = ?2",
        "UPDATE import_conflict SET transaction_id = ?1 WHERE transaction_id = ?2",
        "UPDATE \"transaction\" SET refund_txn_id = ?1 WHERE refund_txn_id = ?2",
    ] {
        sqlx::query(stmt)
            .bind(&target)
            .bind(&old_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("rekey repoint child ({stmt}): {e}"))?;
    }

    sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
        .bind(&old_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("rekey delete old parent: {e}"))?;

    // Registra a linha re-chaveada no `sync_log` (event_type `import`, id determinístico `log:<id>`),
    // para que o PRÓXIMO import a reconheça como a MESMA linha (UPSERT) e o diff-delete não a remova.
    if let Some((pid,)) = profile_id {
        let log_id = format!("log:{target}");
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, source_sheet) \
             VALUES (?1, 'import', 'transaction', ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET timestamp = excluded.timestamp",
        )
        .bind(&log_id)
        .bind(&target)
        .bind(pid)
        .bind(now)
        .bind(sheet_name)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("rekey sync_log: {e}"))?;
    }

    Ok(())
}

/// Realinha a base (`source_amount`) das saídas fixas de débito e os totais declarados das faturas
/// representadas pela nota de uma célula Saída. Devolve o total de linhas realinhadas.
async fn realign_saida_cell(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    c: &CellWrite,
    now: &str,
    notes_written: bool,
) -> Result<usize, String> {
    // (a) Saída fixa de débito (linha 1:1). NÃO toca em linhas de crédito (tratadas em (b)).
    let debit = sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
           AND (payment_method IS NULL OR payment_method <> 'credit') \
           AND scenario_id IS NULL",
    )
    .bind(c.value_cents)
    .bind(now)
    .bind(&c.date)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("realign source_amount (saida débito): {e}"))?;

    let invoices = if notes_written {
        realign_card_invoices(tx, c).await?
    } else {
        0
    };
    Ok(debit.rows_affected() as usize + invoices)
}

/// Faz a planilha e o app convergirem por fatura, nunca por compra. A nota escrita contém uma linha
/// por conta, então a descrição é a chave humana que liga cada total ao registro `invoice` da mesma
/// data de vencimento. Faturas pagas ficam fora: uma Saída realizada não é proposta nem reescrita.
async fn realign_card_invoices(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    c: &CellWrite,
) -> Result<usize, String> {
    let Some(note) = c.note_text.as_deref() else {
        return Ok(0);
    };
    let card_lines = import::parse_itemized_note(note);
    let today = chrono::Local::now().date_naive();
    let invoices: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT i.id, a.name, i.closing_date, i.due_date \
         FROM invoice i JOIN account a ON a.id = i.account_id \
         WHERE i.due_date = ?1 ORDER BY a.created_at, a.id",
    )
    .bind(&c.date)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("query invoices for audit: {e}"))?;

    let mut updated = 0usize;
    for (invoice_id, card_name, closing_date, due_date) in invoices {
        let closing = NaiveDate::parse_from_str(&closing_date, "%Y-%m-%d")
            .map_err(|_| format!("fechamento de fatura inválido: {closing_date}"))?;
        let due = NaiveDate::parse_from_str(&due_date, "%Y-%m-%d")
            .map_err(|_| format!("vencimento de fatura inválido: {due_date}"))?;
        if matches!(
            crate::cards::invoice_status(today, closing, due),
            crate::cards::InvoiceStatus::Paga
        ) {
            continue;
        }
        let Some(line) = card_lines.iter().find(|line| {
            line.kind == import::ItemKind::Cartao
                && crate::cards::normalize_alias(&line.description)
                    == crate::cards::normalize_alias(&card_name)
        }) else {
            continue;
        };
        sqlx::query(
            "UPDATE invoice SET stated_total_cents = ?1, source_stated_total_cents = ?1 WHERE id = ?2",
        )
        .bind(line.amount_cents)
        .bind(&invoice_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("realign invoice stated_total: {e}"))?;
        updated += 1;
    }
    Ok(updated)
}

/// Trilha no sync_log (best-effort de auditoria): só quando há um profile a referenciar (FK NOT
/// NULL). Id determinístico por (aba, célula) → idempotente entre escritas repetidas.
async fn record_write_back_log(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    now: &str,
    profile_id: Option<&(String,)>,
    c: &CellWrite,
) -> Result<(), String> {
    if let Some((pid,)) = profile_id {
        let log_id = format!("writeback:{sheet_name}:{}", c.a1);
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, source_sheet) \
             VALUES (?1, 'write_back', 'cell', ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET timestamp = excluded.timestamp",
        )
        .bind(&log_id)
        .bind(&c.a1)
        .bind(pid)
        .bind(now)
        .bind(sheet_name)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("sync_log write_back: {e}"))?;
    }
    Ok(())
}

/// Economia AUTO-derivada por mês (1..=12) do ano para escrever na coluna `Economia` da aba
/// homônima. A proposta usa a MESMA definição derivada do motor mensal/anual: itens de nota
/// classificados por seção como `ItemKind::Economia` + transfers manuais → conta reserve com
/// data ≤ hoje. Ficam fora: `INVESTIMENTO`/Patrimônio, a anotação importada da própria aba
/// (evita eco/dobra no round-trip) e qualquer fallback por descrição/nome de banco.
pub(crate) async fn load_economia_by_month(
    pool: &SqlitePool,
    year: i32,
    today: chrono::NaiveDate,
) -> Result<[i64; 12], String> {
    let mut by = [0i64; 12];

    let rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT t.date, li.amount_cents, li.description, li.section \
         FROM line_item li \
         JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type = 'expense' AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_savings = 1 \
           ) \
         ORDER BY t.date, li.position",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query economia line items: {e}"))?;

    for (date, cents, description, section) in rows {
        let kind = import::classify_line_item(section.as_deref(), description.as_str());
        if kind != import::ItemKind::Economia {
            continue;
        }
        if let Some(mm) = date.get(5..7)
            && let Ok(m) = mm.parse::<usize>()
            && (1..=12).contains(&m)
        {
            by[m - 1] += cents.abs();
        }
    }

    // Transfers manuais → conta RESERVA também são economia do mês — MESMA definição
    // do motor mensal/anual: a aba que o app escreve tem que casar com o
    // Economizado% que o app exibe. Ilíquido (previdência) é patrimônio: fica fora. O corte é por
    // DATA (`<= hoje`), não pelo flag is_projection (que fica congelado quando a data passa):
    // a aba registra poupança FEITA — escrever ocorrências FUTURAS de série fabricaria economia
    // que ainda não aconteceu; os itens de nota seguem o ano inteiro porque
    // nascem das células da própria planilha (round-trip de lumps pré-lançados).
    let transfers: Vec<(String, i64)> = sqlx::query_as(
        "SELECT substr(t.date, 1, 7), COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.date <= ?3 \
           AND t.type='transfer' AND a.liquidity = 'reserve' AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_savings = 1 \
           ) \
         GROUP BY substr(t.date, 1, 7)",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .bind(today.format("%Y-%m-%d").to_string())
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query economia transfers: {e}"))?;
    for (ym, cents) in transfers {
        if let Some(mm) = ym.get(5..7)
            && let Ok(m) = mm.parse::<usize>()
            && (1..=12).contains(&m)
        {
            by[m - 1] += cents;
        }
    }

    Ok(by)
}

/// Núcleo compartilhado do write-back da Economia (aba `Economia`, separada da grade diária).
pub(crate) async fn build_economia_plan(
    app_dir: &std::path::Path,
    pool: &SqlitePool,
    spreadsheet_id: &str,
    year: i32,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<(SheetsClient, Vec<CellWrite>), String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let values = client
        .get_sheet_values(spreadsheet_id, "'Economia'")
        .await?;
    let by_month = load_economia_by_month(pool, year, chrono::Local::now().date_naive()).await?;
    let plan = write_back::plan_economia_write_back(&values.values, year, &by_month);
    Ok((client, plan))
}

/// Preview READ-ONLY do write-back da Economia (itens `ECONOMIA:` → coluna `Economia` por mês).
#[tauri::command]
pub async fn preview_economia_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    year: i32,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<CellWrite>, String> {
    let (_client, plan) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;
    Ok(plan)
}

/// Prévia RICA da Economia (read-only): plano + `preview_revision` (frescura) + conflitos pendentes.
/// Comando NOVO/aditivo; o `preview_economia_write_back` legado segue devolvendo `Vec<CellWrite>`.
#[tauri::command]
pub async fn preview_economia_write_back_status(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    year: i32,
    client_id: String,
    client_secret: Option<String>,
) -> Result<WriteBackPreviewResult, String> {
    // Foto do `modifiedTime` ANTES de ler os VALORES (fecha o TOCTOU; ver `preview_write_back_status`).
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let preview_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;
    let (_client, cells) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;
    let conflicts_pending = unresolved_conflict_count(pool.inner()).await? > 0;
    Ok(WriteBackPreviewResult {
        cells,
        preview_revision,
        conflicts_pending,
    })
}

/// Aplica o write-back da Economia. Atrás da MESMA flag `WRITE_BACK_ENABLED`. Retorna nº de células.
#[tauri::command]
pub async fn apply_economia_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    year: i32,
    client_id: String,
    client_secret: Option<String>,
    // Token de frescura; `None` no caminho sem token da UI.
    preview_revision: Option<String>,
) -> Result<usize, String> {
    write_back::ensure_write_back_enabled()?;
    // Gate de conflito antes de qualquer escrita ou chamada ao cliente.
    guard_no_pending_conflicts(pool.inner()).await?;

    let resolved_secret = oauth::pkce::resolve_client_secret(client_secret.clone());
    // Escopo de escrita: falha cedo com erro de re-consentimento se o token for readonly.
    oauth::token_store::ensure_write_scope(&app_dir.0, &client_id, resolved_secret.as_deref())
        .await?;

    // Gate de frescura sempre ligado, espelhando `apply_write_back`. A foto do
    // `modifiedTime` ANTES de ler os VALORES da aba para que o token corresponda a um estado NÃO mais
    // novo que o diff. No caminho LEGADO (sem `preview_revision`), `early_revision` é a base que o
    // apply assumiu; uma edição concorrente entre as duas fotos AVANÇA o `modifiedTime` → o gate
    // dispara → nenhum diff velho chega à planilha.
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let early_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;

    let (client, plan) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;

    // Revalida a frescura SEMPRE; nenhum caminho de apply escapa do gate. A foto posterior do
    // `modifiedTime` é comparada com `preview_revision`, quando presente, ou com `early_revision`
    // no caminho sem token. Divergência aborta antes da escrita.
    let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;
    match preview_revision.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(seen) => staleness_check(seen, &post_plan_revision)?,
        None => staleness_check(&early_revision, &post_plan_revision)?,
    }

    // Células efetivamente escritas (já filtradas por `changed`); reusadas para a auditoria.
    let written: Vec<&CellWrite> = plan.iter().filter(|c| c.changed).collect();
    let updates: Vec<(String, f64)> = written
        .iter()
        .map(|c| (format!("'Economia'!{}", c.a1), c.value_cents as f64 / 100.0))
        .collect();
    let n = client
        .batch_update_values(&spreadsheet_id, &updates)
        .await?;

    // Auditoria pós-escrita (paridade com `apply_write_back`): realinha a base (`source_amount`) das
    // linhas mensais de Economia + registra a escrita no `sync_log`. Só em escrita bem-sucedida.
    if n > 0 {
        record_write_back_audit(pool.inner(), "Economia", &written).await?;
    }
    Ok(n)
}

/// Persiste os valores da aba Economia como uma ANOTAÇÃO de métrica:
/// a poupança já é lançada como Saída no grid mensal (→ FixedOut/Daily → cost_of_living → Saldo UMA
/// vez). A aba Economia é a anotação manual do Economizado% (= Economia/Entradas), NÃO um segundo
/// movimento de caixa. Por isso gravamos em `economia_annotation` (fora do `transaction`), nunca como
/// `type='transfer'` — assim o valor NÃO entra na cadeia do Saldo (sem dupla contagem). É distinto do
/// transfer-de-reserva MANUAL, que continua um movimento real e entra no Saldo via
/// `EventKind::Economia`. Os upserts/deletes correm numa ÚNICA transação.
pub(crate) async fn store_economia_entries(
    pool: &SqlitePool,
    entries: &[(i32, u32, i64)],
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let mut count = 0usize;

    for (year, month, cents) in entries {
        if *cents > 0 {
            sqlx::query(
                "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
                 VALUES ('', ?1, ?2, ?3, ?4) \
                 ON CONFLICT(profile_id, year, month) DO UPDATE SET \
                   amount_cents=excluded.amount_cents, updated_at=excluded.updated_at",
            )
            .bind(year)
            .bind(*month as i64)
            .bind(cents)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("upsert annotation {year}-{month:02}: {e}"))?;
        } else {
            // Célula zerada/em branco = o dono removeu a anotação; apaga a linha.
            sqlx::query(
                "DELETE FROM economia_annotation WHERE profile_id='' AND year=?1 AND month=?2",
            )
            .bind(year)
            .bind(*month as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete annotation {year}-{month:02}: {e}"))?;
        }
        count += 1;
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(count)
}

#[derive(serde::Serialize)]
pub struct UserSpreadsheet {
    pub id: String,
    pub name: String,
    pub modified_time: String,
}

#[tauri::command]
pub async fn list_user_spreadsheets(
    app_dir: State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<UserSpreadsheet>, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;

    let url = "https://www.googleapis.com/drive/v3/files?q=mimeType%3D'application%2Fvnd.google-apps.spreadsheet'&fields=files(id,name,modifiedTime)&orderBy=modifiedTime%20desc&pageSize=50";

    let resp = crate::http::send_with_retry(
        crate::http::client()
            .get(url)
            .bearer_auth(&token.access_token),
    )
    .await
    .map_err(|e| format!("drive request: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Drive API error: {body}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    let files = json["files"].as_array().cloned().unwrap_or_default();

    Ok(files
        .into_iter()
        .filter_map(|f| {
            let id = f["id"].as_str()?.to_string();
            let name = f["name"].as_str().unwrap_or("").to_string();
            let modified = f["modifiedTime"].as_str().unwrap_or("").to_string();
            Some(UserSpreadsheet {
                id,
                name,
                modified_time: modified,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    #[tokio::test]
    async fn write_back_loads_each_card_invoice_at_its_own_due_date() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day, created_at) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10, '2026-01-01T00:00:00Z'), \
                    ('card-b', 'Cartão B', 'credit_card', 'person-1', 22, 15, '2026-01-02T00:00:00Z')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 11_100), \
                    ('invoice-b', 'card-b', ?4, ?5, ?6, 22_200)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(format!("{year}-03-10"))
        .bind(format!("{year}-04"))
        .bind(format!("{year}-03-22"))
        .bind(format!("{year}-04-15"))
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
               (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('purchase-a', 'expense', 100, ?1, 'credit', 0, 0, 'invoice-a'), \
                    ('purchase-b', 'expense', 200, ?2, 'credit', 0, 0, 'invoice-b')",
        )
        .bind(format!("{year}-02-05"))
        .bind(format!("{year}-03-05"))
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let saidas: Vec<&WriteBackTxn> = txns
            .iter()
            .filter(|txn| txn.kind == import::RowKind::Saida)
            .collect();

        assert_eq!(saidas.len(), 2, "uma candidata por vencimento de fatura");
        let first = saidas
            .iter()
            .find(|txn| txn.date == format!("{year}-03-10"))
            .unwrap();
        assert_eq!(first.amount_cents, 11_100, "stated é a autoridade");
        assert_eq!(
            first.items.as_ref().unwrap()[0].description,
            "Cartão A",
            "a nota identifica o cartão da fatura"
        );
        let second = saidas
            .iter()
            .find(|txn| txn.date == format!("{year}-04-15"))
            .unwrap();
        assert_eq!(second.amount_cents, 22_200, "stated é a autoridade");
        assert_eq!(second.items.as_ref().unwrap()[0].description, "Cartão B");
    }

    #[tokio::test]
    async fn write_back_composes_cards_that_share_a_due_date_into_one_candidate() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day, created_at) \
             VALUES ('card-a', 'Titular', 'credit_card', 'person-1', 20, 10, '2026-01-01T00:00:00Z'), \
                    ('card-b', 'Adicional', 'credit_card', 'person-1', 20, 10, '2026-01-02T00:00:00Z')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 10_000), \
                    ('invoice-b', 'card-b', ?1, ?2, ?3, 20_000)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(format!("{year}-03-10"))
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let saidas: Vec<&WriteBackTxn> = txns
            .iter()
            .filter(|txn| txn.date == format!("{year}-03-10") && txn.kind == import::RowKind::Saida)
            .collect();

        assert_eq!(saidas.len(), 1, "a célula recebe uma candidata composta");
        assert_eq!(saidas[0].amount_cents, 30_000);
        let items = saidas[0].items.as_ref().unwrap();
        assert_eq!(
            items
                .iter()
                .map(|item| (
                    item.amount_cents,
                    item.description.as_str(),
                    item.section.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                (10_000, "Titular", Some("CARTÕES:")),
                (20_000, "Adicional", Some("CARTÕES:")),
            ]
        );
    }

    #[tokio::test]
    async fn write_back_replaces_only_card_items_in_the_parent_candidate() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão Principal', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 25_000)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('parent', 'expense', 50_000, ?1, 1, 0)",
        )
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item \
               (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('conta', 'parent', 30_000, 'Condomínio', 0, 0, 'CONTAS:'), \
                    ('card-old', 'parent', 20_000, 'Cartão Principal', 1, 0, 'CARTOES:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let candidate = txns
            .iter()
            .find(|txn| txn.date == due && txn.kind == import::RowKind::Saida)
            .unwrap();
        assert_eq!(candidate.amount_cents, 55_000);
        assert_eq!(
            candidate
                .items
                .as_ref()
                .unwrap()
                .iter()
                .map(|item| (
                    item.amount_cents,
                    item.description.as_str(),
                    item.section.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                (30_000, "Condomínio", Some("CONTAS:")),
                (25_000, "Cartão Principal", Some("CARTOES:")),
            ],
            "itens fora de cartões, ordem e grafia do cabeçalho sobrevivem"
        );
    }

    #[tokio::test]
    async fn write_back_preserves_unknown_card_alias_while_replacing_known_invoice_line() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) VALUES ('visa', 'Visa', 'credit_card', 'person-1', 20, 10)")
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) VALUES ('visa-invoice', 'visa', ?1, ?2, ?3, 10_000)")
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) VALUES ('parent', 'expense', 30_000, ?1, 1, 0)")
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) VALUES ('visa-old', 'parent', 10_000, 'Visa', 0, 0, 'CARTÕES:'), ('nubank', 'parent', 20_000, 'Nubank', 1, 0, 'CARTÕES:')")
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let grid = vec![
            vec!["".into(), "MARÇO".into(), "".into(), "".into()],
            vec![
                "Dia".into(),
                "Saldo".into(),
                "Entrada".into(),
                "Saída".into(),
            ],
            vec!["10".into(), "".into(), "".into(), "".into()],
        ];
        let layout = crate::google_sheets::layout_detect::SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "both".into(),
        };
        let plan = write_back::plan_write_back(&grid, &layout, &[("amount_out".into(), 2)], &txns);

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].value_cents, 30_000);
        let note = plan[0].note_text.as_deref().expect("nota itemizada");
        assert!(note.contains("R$ 200,00 - Nubank"));
        assert!(note.contains("R$ 100,00 - Visa"));
    }

    /// O import casa o alias ANTES do `#` (`item.description.split('#').next()`, import.rs) —
    /// o write-back precisa da mesma regra. Sem ela, uma linha marcada com `#reembolso:` (uso real
    /// do dono nas linhas de cartão adicional) nunca casa o alias conhecido: o item some da lista de
    /// "conhecidos", o write-back preserva os R$ antigos da linha E ainda soma a fatura por cima —
    /// duplica o valor em vez de a fatura substituir a linha.
    #[tokio::test]
    async fn write_back_matches_card_alias_before_the_reembolso_marker_and_does_not_duplicate_the_invoice_line()
     {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) VALUES ('bia', 'Bradesco Bia', 'credit_card', 'person-1', 20, 10)")
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) VALUES ('bia-invoice', 'bia', ?1, ?2, ?3, 53_000)")
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) VALUES ('parent', 'expense', 53_000, ?1, 1, 0)")
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) VALUES ('bia-old', 'parent', 53_000, 'Bradesco Bia #reembolso:Bia', 0, 0, 'CARTÕES:')")
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let candidate = txns
            .iter()
            .find(|txn| txn.date == due && txn.kind == import::RowKind::Saida)
            .unwrap();

        assert_eq!(
            candidate.amount_cents, 53_000,
            "o marcador #reembolso: não pode fazer a fatura duplicar a linha antiga"
        );
        assert_eq!(
            candidate.items.as_ref().unwrap().len(),
            1,
            "a linha vira a fatura, não soma em cima dela"
        );
    }

    /// A CHAVE é "existe fatura para (conta, ciclo) da linha", não "o alias é conhecido": uma
    /// proposta recém-aceita cria conta+alias mas não materializa a fatura observada até o próximo
    /// import. Um item de cartão conhecido SEM fatura naquele vencimento tem de ser preservado —
    /// exatamente como um alias desconhecido — nunca suprimido nem substituído sem substituta.
    #[tokio::test]
    async fn write_back_preserves_known_card_item_without_its_own_invoice_even_when_sibling_card_has_one()
     {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('titular', 'Titular', 'credit_card', 'person-1', 20, 10), \
                    ('bia', 'Bia', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        // Só o titular tem fatura persistida neste vencimento — Bia é conta+alias recém-criados
        // (proposta aceita) sem fatura ainda materializada.
        sqlx::query("INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) VALUES ('titular-invoice', 'titular', ?1, ?2, ?3, 10_000)")
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) VALUES ('parent', 'expense', 15_000, ?1, 1, 0)")
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) VALUES ('titular-item', 'parent', 10_000, 'Titular', 0, 0, 'CARTÕES:'), ('bia-item', 'parent', 5_000, 'Bia', 1, 0, 'CARTÕES:')")
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let candidate = txns
            .iter()
            .find(|txn| txn.date == due && txn.kind == import::RowKind::Saida)
            .unwrap();

        assert_eq!(
            candidate.amount_cents, 15_000,
            "Bia (conhecido, sem fatura) preserva seus R$ 50 — a fatura do titular só substitui a linha dele"
        );
        assert_eq!(
            candidate
                .items
                .as_ref()
                .unwrap()
                .iter()
                .map(|item| (item.amount_cents, item.description.as_str()))
                .collect::<Vec<_>>(),
            vec![(5_000, "Bia"), (10_000, "Titular")],
            "item de Bia sobrevive intacto; o de Titular vira a linha da fatura"
        );
    }

    #[tokio::test]
    async fn write_back_plan_keeps_card_and_non_card_sections_when_two_saida_parents_share_due_date()
     {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Visa', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
             (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 15_000)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('a-card-parent', 'expense', 15_000, ?1, 1, 0), \
                    ('z-fixed-parent', 'expense', 5_000, ?1, 1, 0)",
        )
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item \
             (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('old-card', 'a-card-parent', 15_000, 'Visa', 0, 0, 'CARTÕES:'), \
                    ('rent', 'z-fixed-parent', 5_000, 'Aluguel', 0, 0, 'CONTAS:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let grid = vec![
            vec!["".into(), "MARÇO".into(), "".into(), "".into()],
            vec![
                "Dia".into(),
                "Saldo".into(),
                "Entrada".into(),
                "Saída".into(),
            ],
            vec!["10".into(), "".into(), "".into(), "".into()],
        ];
        let layout = crate::google_sheets::layout_detect::SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "both".into(),
        };
        let plan = write_back::plan_write_back(&grid, &layout, &[("amount_out".into(), 2)], &txns);

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].value_cents, 20_000);
        assert!(plan[0].formula.is_some());
        let note = plan[0].note_text.as_deref().expect("nota itemizada");
        assert!(note.contains("CONTAS:\nR$ 50,00 - Aluguel"));
        assert!(note.contains("CARTÕES:\nR$ 150,00 - Visa"));
    }

    #[tokio::test]
    async fn card_invoice_writeback_realigns_stated_total_to_its_note_line() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents, source_stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 10_000, 8_000)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        let cell = CellWrite {
            a1: "D3".into(),
            row: 2,
            col: 3,
            date: due,
            kind: "saida".into(),
            current: "100,00".into(),
            proposed: "123,45".into(),
            value_cents: 12_345,
            changed: true,
            formula: None,
            note_text: Some("CARTÕES:\nR$ 123,45 - Cartão A".into()),
        };

        let realigned = record_write_back_audit(&p, "2027", &[&cell]).await.unwrap();
        assert_eq!(realigned, 1, "a fatura escrita é realinhada");
        let totals: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT stated_total_cents, source_stated_total_cents FROM invoice WHERE id = 'invoice-a'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(totals, (Some(12_345), Some(12_345)));
    }

    #[tokio::test]
    async fn card_invoice_writeback_does_not_advance_the_note_merge_base_when_note_write_fails() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
             (id, account_id, cycle_month, closing_date, due_date, stated_total_cents, source_stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 15_000, 10_000)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        let cell = CellWrite {
            a1: "D3".into(),
            row: 2,
            col: 3,
            date: due,
            kind: "saida".into(),
            current: "100,00".into(),
            proposed: "150,00".into(),
            value_cents: 15_000,
            changed: true,
            formula: Some("=SUM(150.00)".into()),
            note_text: Some("CARTÕES:\nR$ 150,00 - Cartão A".into()),
        };

        let realigned = record_write_back_audit_with_notes(&p, &year.to_string(), &[&cell], false)
            .await
            .unwrap();
        assert_eq!(
            realigned, 0,
            "uma nota não escrita não pode realinhar a fatura"
        );
        let totals: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT stated_total_cents, source_stated_total_cents FROM invoice WHERE id = 'invoice-a'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(totals, (Some(15_000), Some(10_000)));
    }

    #[tokio::test]
    async fn card_invoice_writeback_realigns_every_card_line_in_a_shared_due_cell() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10), \
                    ('card-b', 'Cartão B', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents, source_stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 1, 1), \
                    ('invoice-b', 'card-b', ?1, ?2, ?3, 2, 2)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        let cell = CellWrite {
            a1: "D3".into(),
            row: 2,
            col: 3,
            date: due,
            kind: "saida".into(),
            current: "0,03".into(),
            proposed: "300,00".into(),
            value_cents: 30_000,
            changed: true,
            formula: Some("=SUM(100.00+200.00)".into()),
            note_text: Some("CARTÕES:\nR$ 100,00 - Cartão A\nR$ 200,00 - Cartão B".into()),
        };

        assert_eq!(
            record_write_back_audit(&p, "2027", &[&cell]).await.unwrap(),
            2
        );
        let totals: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, stated_total_cents, source_stated_total_cents FROM invoice ORDER BY id",
        )
        .fetch_all(&p)
        .await
        .unwrap();
        assert_eq!(
            totals,
            vec![
                ("invoice-a".into(), Some(10_000), Some(10_000)),
                ("invoice-b".into(), Some(20_000), Some(20_000)),
            ]
        );
    }

    #[tokio::test]
    async fn write_back_does_not_repropose_a_paid_card_invoice() {
        let p = pool().await;
        let due = chrono::Local::now().date_naive().pred_opt().unwrap();

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3, 12_345)",
        )
        .bind(crate::cards::cycle_month_of(due))
        .bind(due.pred_opt().unwrap().to_string())
        .bind(due.to_string())
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, due.year()).await.unwrap();
        assert!(
            !txns
                .iter()
                .any(|txn| txn.kind == import::RowKind::Saida && txn.date == due.to_string()),
            "fatura paga não reabre a célula realizada"
        );
    }

    #[tokio::test]
    async fn write_back_keeps_app_refund_expectation_and_excludes_import_derived_income() {
        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-a', 'Cartão A', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('invoice-a', 'card-a', ?1, ?2, ?3)",
        )
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        crate::commands::card_cmds::create_refund_expectation_inner(
            &p,
            "invoice-a",
            9_000,
            Some("Reembolso esperado"),
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
               (id, type, amount, date, is_fixed, is_projection, refund_invoice_id) \
             VALUES ('derived:reembolso:import', 'income', 8_000, ?1, 0, 0, 'invoice-a')",
        )
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();

        let txns = load_write_back_txns(&p, year).await.unwrap();
        let entradas: Vec<&WriteBackTxn> = txns
            .iter()
            .filter(|txn| txn.kind == import::RowKind::Entrada && txn.date == due)
            .collect();
        assert_eq!(entradas.len(), 1);
        assert_eq!(entradas[0].amount_cents, 9_000);
    }

    // A aba Economia é uma ANOTAÇÃO de métrica, não um movimento de caixa.
    // `store_economia_entries` grava em `economia_annotation` (fora do `transaction`) → nunca cria
    // uma linha `economia:YYYY-MM` nem entra na cadeia do Saldo. Sem isso, a mesma poupança (já
    // lançada como Saída no grid) seria descontada do Saldo uma segunda vez.
    #[tokio::test]
    async fn annotation_does_not_create_transaction_row() {
        let p = pool().await;

        let count = store_economia_entries(&p, &[(2026, 6, 50000)])
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Nenhuma linha de transação com o antigo id `economia:YYYY-MM` (esquema aposentado).
        let (txn_rows,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = 'economia:2026-06'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            txn_rows, 0,
            "a aba Economia NÃO cria linha em `transaction`"
        );

        // Nenhum transfer fantasma (a anotação não é um movimento de caixa).
        let (transfers,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE type='transfer'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            transfers, 0,
            "a anotação não vira transfer (sem dupla contagem no Saldo)"
        );

        // A anotação É persistida na tabela própria.
        let (amount,): (i64,) = sqlx::query_as(
            "SELECT amount_cents FROM economia_annotation WHERE year=2026 AND month=6",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(amount, 50000);
    }

    // A anotação não é carregada como evento de caixa, então o Saldo projetado
    // não a desconta (sem dupla contagem). `signed()`/Performance ficam intactos: só transfers REAIS
    // seguem como `EventKind::Economia` e tocam o Saldo.
    #[tokio::test]
    async fn annotation_not_loaded_as_cashflow_event() {
        let p = pool().await;
        store_economia_entries(&p, &[(2026, 6, 50000)])
            .await
            .unwrap();

        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let horizon = chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let events = crate::commands::forecast_cmds::load_cashflow_events(&p, today, horizon)
            .await
            .unwrap();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e.kind, forecast::EventKind::Economia)),
            "a anotação da aba Economia não aparece como EventKind::Economia (não toca o Saldo)"
        );
    }

    // O gate de frescura roda em todo apply, inclusive sem `preview_revision`. Como
    // `apply_write_back` depende de IO de rede, o teste cobre a decisão pura em `staleness_check`:
    // revisão igual passa; revisão diferente aborta.
    #[tokio::test]
    async fn staleness_check_rejects_different_revision() {
        // Mesma revisão → OK (a planilha não mudou desde a foto).
        staleness_check("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z").unwrap();

        // Revisão diferente → aborta (a planilha avançou; exige re-revisão).
        let err = staleness_check("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z").unwrap_err();
        assert_eq!(err, SHEET_CHANGED_MSG, "diff stale é rejeitado");
    }

    // O write-back da aba Economia vem dos itens `ECONOMIA:`. O filtro "Ignorar" continua
    // valendo no pai: se a transação foi marcada fora dos totais, nenhum item dela pode ir para a
    // coluna Economia da planilha.
    #[tokio::test]
    async fn economia_writeback_excludes_ignored_itemized_transaction() {
        let p = pool().await;

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('tx-ignored', 'expense', 50_000, '2026-03-20', 1, 0), \
                    ('tx-counted', 'expense', 30_000, '2026-03-21', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('li-ignored', 'tx-ignored', 50_000, 'Reserva ignorada', 0, 0, 'ECONOMIA:'), \
                    ('li-counted', 'tx-counted', 30_000, 'Reserva contada', 0, 0, 'ECONOMIA:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let tag = crate::tags::create_tag(&p, "Ignorar", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_rulers(&p, &tag, true, true, true, true)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&p, "tx-ignored", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let by_month = load_economia_by_month(
            &p,
            2026,
            chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        )
        .await
        .unwrap();
        // Março (índice 2) carrega só o item cujo pai NÃO tem tag excluída (30_000), não 80_000.
        assert_eq!(
            by_month[2], 30_000,
            "a coluna Economia omite itens de transações marcadas Ignorar"
        );
        // Demais meses zerados.
        assert_eq!(by_month.iter().sum::<i64>(), 30_000);
    }

    // A coluna Economia da aba homônima é proposta a partir da Economia
    // AUTO-derivada dos itens de nota. A fonte é seção `ECONOMIA:`; anotação importada antiga,
    // transfers manuais e `INVESTIMENTO:`/Patrimônio NÃO entram, e descrição/banco sem seção não
    // serve como fallback.
    #[tokio::test]
    async fn economia_writeback_uses_auto_line_items_not_annotations_transfers_or_patrimonio() {
        let p = pool().await;

        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('res-1', 'Reserva', 'savings', 'pe-1', 0, 'reserve'), \
                    ('ill-1', 'Previdência', 'pension', 'pe-1', 0, 'illiquid')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Fontes stale não devem alimentar a proposta de write-back.
        sqlx::query(
            "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
             VALUES ('', 2026, 3, 999_000, '2026-03-31T00:00:00Z')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('tr-reserve', 'transfer', 30_000, '2026-03-24', 'res-1', 0), \
                    ('tr-illiquid', 'transfer', 60_000, '2026-03-25', 'ill-1', 0), \
                    ('tr-reserve-proj', 'transfer', 80_000, '2026-08-10', 'res-1', 1)",
        )
        .execute(&p)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('tx-auto', 'expense', 150_000, '2026-03-20', 1, 0), \
                    ('tx-no-section', 'expense', 50_000, '2026-03-21', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('li-eco', 'tx-auto', 40_000, 'Reserva mensal', 0, 0, 'ECONOMIA:'), \
                    ('li-pat', 'tx-auto', 70_000, 'Previdência privada', 1, 0, 'INVESTIMENTO:'), \
                    ('li-saida', 'tx-auto', 40_000, 'Aluguel', 2, 0, 'CONTAS:'), \
                    ('li-bank-desc', 'tx-no-section', 50_000, 'Banco Exemplo - reserva', 0, 0, NULL)",
        )
        .execute(&p)
        .await
        .unwrap();

        let by_month = load_economia_by_month(
            &p,
            2026,
            chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        )
        .await
        .unwrap();
        // Definição canônica ÚNICA de economia — itens de seção ECONOMIA
        // + transfers→reserva (a aba escrita casa com o Economizado% exibido). Anotação antiga,
        // ilíquido (patrimônio) e fallback por descrição/banco seguem fora.
        assert_eq!(
            by_month[2], 70_000,
            "março = itens ECONOMIA (40.000) + transfer→reserva (30.000)"
        );
        assert_eq!(
            by_month.iter().sum::<i64>(),
            70_000,
            "sem anotação antiga, transfer ilíquido, ocorrência projetada de série \
             ou fallback por descrição/banco"
        );
    }

    // No round-trip, o apply realinha `economia_annotation` para que o import da aba
    // veja origem == app, mas a próxima proposta NÃO pode somar a anotação em cima dos itens
    // auto-derivados — senão cada write-back duplicaria a Economia local.
    #[tokio::test]
    async fn economia_writeback_round_trip_annotation_does_not_double_count_auto_items() {
        let p = pool().await;

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('tx-auto', 'expense', 40_000, '2026-01-20', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('li-eco', 'tx-auto', 40_000, 'Reserva mensal', 0, 0, 'ECONOMIA:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let before = load_economia_by_month(
            &p,
            2026,
            chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(before[0], 40_000);

        let cell = CellWrite {
            a1: "I5".into(),
            row: 4,
            col: 8,
            date: "2026-01".into(),
            kind: "economia".into(),
            current: "0,00".into(),
            proposed: "400,00".into(),
            value_cents: 40_000,
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&p, "Economia", &[&cell])
            .await
            .unwrap();
        assert_eq!(
            realigned, 1,
            "a anotação do mês é alinhada ao valor escrito"
        );

        let (annotation,): (i64,) = sqlx::query_as(
            "SELECT amount_cents FROM economia_annotation WHERE year=2026 AND month=1",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(annotation, 40_000);

        let after = load_economia_by_month(
            &p,
            2026,
            chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            after[0], 40_000,
            "a proposta continua igual aos itens auto-derivados, sem somar a anotação"
        );
        assert_eq!(after.iter().sum::<i64>(), 40_000);
    }

    // Uma transação MANUAL (id UUID, fora do `sync_log`) escrita de volta numa célula
    // antes VAZIA é re-chaveada para o `row_id` determinístico de `(aba, data, kind, 0)` e registrada
    // no `sync_log`. Sem isto, o re-import da planilha computaria o id determinístico, não acharia a
    // linha (id-UUID) e INSERIRIA um gêmeo → duplicata (dupla contagem). Com o re-chaveamento, o
    // re-import faz UPSERT na MESMA linha → EXATAMENTE UMA linha; o valor (totais/Saldo) não muda.
    #[tokio::test]
    async fn manual_writeback_then_reimport_yields_single_row() {
        use crate::google_sheets::import::{self, ImportedRow, RowKind};
        let p = pool().await;

        // Linha MANUAL criada no app: id-UUID, NÃO está no sync_log. Saída fixa de 120,00 em 06/JAN.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, source_amount) \
             VALUES ('manual-uuid-1', 'expense', 12_000, '2026-01-06', 1, 0, 12_000)",
        )
        .execute(&p)
        .await
        .unwrap();
        // Precisa de um profile para o sync_log da re-chaveada (FK NOT NULL).
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES ('pf-1', 'pe-1')")
            .execute(&p)
            .await
            .unwrap();

        // Write-back dessa linha para a célula (antes vazia) de Saída de 06/JAN/2026.
        let cell = CellWrite {
            a1: "E3".into(),
            row: 2,
            col: 4,
            date: "2026-01-06".into(),
            kind: "saida".into(),
            current: "".into(),
            proposed: "120,00".into(),
            value_cents: 12_000,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();

        // A linha UUID virou a determinística `row_id("2026","2026-01-06",Saida,0)`; nenhum gêmeo.
        let target = import::row_id("2026", "2026-01-06", RowKind::Saida, 0);
        let (uuid_gone,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = 'manual-uuid-1'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(uuid_gone, 0, "a linha UUID foi re-chaveada (não duplicada)");
        let (has_target,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = ?1")
                .bind(&target)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            has_target, 1,
            "a linha agora tem o id determinístico do import"
        );

        // RE-IMPORT da MESMA célula (a Saída de 06/JAN reaparece na planilha): faz UPSERT na MESMA
        // linha, não insere gêmeo. (`import_rows` calcula o mesmo `row_id` para slot 0.)
        let rows = vec![ImportedRow {
            date: "2026-01-06".into(),
            amount: -12_000,
            description: "Saída 2026-01-06".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: String::new(),
        }];
        import::import_rows(&p, "2026", &rows, "pf-1")
            .await
            .unwrap();

        // EXATAMENTE UMA linha (sem o gêmeo importado); valor preservado (totais/Saldo inalterados).
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date = '2026-01-06'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(count, 1, "manual→write-back→re-import = uma única linha");
        let (amount,): (i64,) = sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = ?1")
            .bind(&target)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(
            amount, 12_000,
            "o valor (totais/Saldo) não muda no round-trip"
        );
    }

    #[tokio::test]
    async fn rekeyed_refund_income_keeps_its_invoice_link_and_remains_in_invoice_detail() {
        use crate::commands::card_cmds::{
            create_card_account_inner, create_refund_expectation_inner, get_invoice_inner,
        };
        use crate::google_sheets::import::{self, RowKind};

        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let due = format!("{year}-03-10");
        let card =
            create_card_account_inner(&p, "Visa", None, Some(20), Some(10), None, None, None, &[])
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
             (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-refund', ?1, ?2, ?3, ?4, 10_000)",
        )
        .bind(&card)
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&due)
        .execute(&p)
        .await
        .unwrap();
        let refund =
            create_refund_expectation_inner(&p, "invoice-refund", 2_000, Some("Reembolso"))
                .await
                .unwrap();

        let cell = CellWrite {
            a1: "B3".into(),
            row: 2,
            col: 1,
            date: due.clone(),
            kind: "entrada".into(),
            current: "0,00".into(),
            proposed: "20,00".into(),
            value_cents: 2_000,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, &year.to_string(), &[&cell])
            .await
            .unwrap();

        let target = import::row_id(&year.to_string(), &due, RowKind::Entrada, 0);
        let (linked_invoice,): (Option<String>,) =
            sqlx::query_as("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(&target)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(linked_invoice.as_deref(), Some("invoice-refund"));
        let detail = get_invoice_inner(&p, "invoice-refund").await.unwrap();
        assert_eq!(detail.refunds.len(), 1);
        assert_eq!(detail.refunds[0].txn_id, target);
        let old_exists: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = ?1")
                .bind(refund)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(old_exists.0, 0);
    }

    #[tokio::test]
    async fn rekeyed_card_purchase_keeps_incoming_refund_transaction_link() {
        use crate::commands::card_cmds::create_card_account_inner;
        use crate::google_sheets::import::{self, RowKind};

        let p = pool().await;
        let year = chrono::Local::now().year() + 1;
        let date = format!("{year}-03-10");
        let card =
            create_card_account_inner(&p, "Visa", None, Some(20), Some(10), None, None, None, &[])
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
             (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-purchase', ?1, ?2, ?3, ?4, 10_000)",
        )
        .bind(&card)
        .bind(format!("{year}-03"))
        .bind(format!("{year}-02-20"))
        .bind(&date)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('purchase-old', 'expense', 10_000, ?1, 'debit', 1, 1, 'invoice-purchase'), \
                    ('refund-income', 'income', 2_000, ?1, NULL, 0, 1, NULL)",
        )
        .bind(&date)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE \"transaction\" SET refund_txn_id = 'purchase-old' WHERE id = 'refund-income'",
        )
        .execute(&p)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "B3".into(),
            row: 2,
            col: 1,
            date: date.clone(),
            kind: "saida".into(),
            current: "0,00".into(),
            proposed: "100,00".into(),
            value_cents: 10_000,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, &year.to_string(), &[&cell])
            .await
            .unwrap();

        let target = import::row_id(&year.to_string(), &date, RowKind::Saida, 0);
        let refund_target: Option<String> = sqlx::query_scalar(
            "SELECT refund_txn_id FROM \"transaction\" WHERE id = 'refund-income'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(refund_target.as_deref(), Some(target.as_str()));
    }

    // Garantia de não colisão: se a célula já tem uma linha importada (id
    // determinístico no `sync_log`), o re-chaveamento NÃO roda — nunca sobrescreve/colide com ela.
    // A linha manual separada permanece intacta (caso degenerado: 2 linhas na mesma célula).
    #[tokio::test]
    async fn rekey_never_overwrites_existing_imported_row() {
        use crate::google_sheets::import::{self, RowKind};
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES ('pf-1', 'pe-1')")
            .execute(&p)
            .await
            .unwrap();

        // Linha IMPORTADA já ocupa a célula determinística (id = row_id, registrada no sync_log).
        let target = import::row_id("2026", "2026-01-06", RowKind::Saida, 0);
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, source_amount) \
             VALUES (?1, 'expense', 12_000, '2026-01-06', 1, 0, 12_000)",
        )
        .bind(&target)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, source_sheet) \
             VALUES (?1, 'import', 'transaction', ?2, 'pf-1', '2026')",
        )
        .bind(format!("log:{target}"))
        .bind(&target)
        .execute(&p)
        .await
        .unwrap();

        // Uma linha manual SEPARADA na mesma data/kind (caso degenerado, não-canônico).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, source_amount) \
             VALUES ('manual-uuid-2', 'expense', 9_000, '2026-01-06', 1, 0, 9_000)",
        )
        .execute(&p)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "E3".into(),
            row: 2,
            col: 4,
            date: "2026-01-06".into(),
            kind: "saida".into(),
            current: "120,00".into(),
            proposed: "120,00".into(),
            value_cents: 12_000,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();

        // O alvo determinístico continua sendo a linha IMPORTADA original (valor intacto, não 9_000).
        let (amount,): (i64,) = sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = ?1")
            .bind(&target)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(
            amount, 12_000,
            "a linha importada existente não é sobrescrita"
        );
        // A manual NÃO foi re-chaveada (o alvo já existia → STOP seguro).
        let (manual_alive,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = 'manual-uuid-2'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            manual_alive, 1,
            "a manual não é re-chaveada quando o alvo já existe"
        );
    }

    // O braço `entrada` da auditoria precisa excluir linhas `derived:%`, que são sintetizadas e não
    // vêm 1:1 da planilha. `load_write_back_txns` aplica o mesmo filtro; sem ele, a base da linha
    // derivada seria sobrescrita e o import seguinte abriria conflito espúrio.
    #[tokio::test]
    async fn audit_entrada_skips_derived_rows() {
        use crate::google_sheets::import::{self, RowKind};
        let p = pool().await;

        // Renda IMPORTADA 1:1 (id determinístico + `sync_log`, realinhável e não re-chaveada) e linha
        // derivada não realinhável na mesma data.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES ('pf-1', 'pe-1')")
            .execute(&p)
            .await
            .unwrap();
        let inc_id = import::row_id("2026", "2026-03-01", RowKind::Entrada, 0);
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, source_amount) \
             VALUES (?1, 'income', 5000, '2026-03-01', 0, 0, 5000), \
                    ('derived:reimb-1', 'income', 1000, '2026-03-01', 0, 0, 1000)",
        )
        .bind(&inc_id)
        .execute(&p)
        .await
        .unwrap();
        // `sync_log` marca a renda 1:1 como importada e impede seu re-chaveamento.
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, source_sheet) \
             VALUES (?1, 'import', 'transaction', ?2, 'pf-1', '2026')",
        )
        .bind(format!("log:{inc_id}"))
        .bind(&inc_id)
        .execute(&p)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "B3".into(),
            row: 2,
            col: 1,
            date: "2026-03-01".into(),
            kind: "entrada".into(),
            current: "50,00".into(),
            proposed: "99,00".into(),
            value_cents: 9900,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();

        // A renda 1:1 é realinhada ao valor escrito.
        let (inc,): (Option<i64>,) =
            sqlx::query_as("SELECT source_amount FROM \"transaction\" WHERE id = ?1")
                .bind(&inc_id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(inc, Some(9900), "a renda importada 1:1 é realinhada");

        // A linha derivada NÃO é tocada.
        let (der,): (Option<i64>,) = sqlx::query_as(
            "SELECT source_amount FROM \"transaction\" WHERE id = 'derived:reimb-1'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(
            der,
            Some(1000),
            "a linha derivada não é realinhada pelo braço entrada"
        );
    }

    // Uma linha de CENÁRIO que compartilha (data, tipo) com uma
    // célula recém-escrita NUNCA deve ser re-chaveada nem auditada pelo write-back real — ela é
    // uma linha hipotética "e se", fora do livro-razão. Cobre os quatro pontos que liam
    // `"transaction"` sem filtro de cenário: `record_write_back_audit` (entrada), `realign_saida_cell`
    // (débito fixo) e `rekey_manual_row_to_deterministic` (candidata manual).
    #[tokio::test]
    async fn scenario_row_sharing_cell_is_not_rekeyed_or_audited() {
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO scenario (id, name, person_id) VALUES ('sc-1', 'E se', 'pe-1')")
            .execute(&p)
            .await
            .unwrap();

        // Linha REAL na célula (entrada, 2026-06-05) que a auditoria deve realinhar.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection, source_amount) \
             VALUES ('real-inc', 'income', 8000, '2026-06-05', 0, 8000)",
        )
        .execute(&p)
        .await
        .unwrap();
        // Linha de CENÁRIO com a MESMA (data, tipo) — não é uma linha real, nunca deve ser tocada.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection, source_amount, scenario_id) \
             VALUES ('scen-inc', 'income', 8000, '2026-06-05', 0, 8000, 'sc-1')",
        )
        .execute(&p)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "F5".into(),
            row: 4,
            col: 5,
            date: "2026-06-05".into(),
            kind: "entrada".into(),
            current: "80,00".into(),
            proposed: "90,00".into(),
            value_cents: 9000,
            changed: true,
            formula: None,
            note_text: None,
        };
        record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();

        // A linha real pode ter sido re-chaveada para o id determinístico; busca por conteúdo, não
        // pelo id original.
        let (real_amt,): (Option<i64>,) = sqlx::query_as(
            "SELECT source_amount FROM \"transaction\" \
             WHERE type='income' AND date='2026-06-05' AND scenario_id IS NULL",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(
            real_amt,
            Some(9000),
            "a linha real é realinhada normalmente"
        );

        // A linha de cenário mantém id e source_amount intactos — não foi vista pela auditoria.
        let scen_row: Option<(String, Option<i64>)> =
            sqlx::query_as("SELECT id, source_amount FROM \"transaction\" WHERE id = 'scen-inc'")
                .fetch_optional(&p)
                .await
                .unwrap();
        let (scen_id, scen_amt) =
            scen_row.expect("a linha de cenário continua existindo com o mesmo id");
        assert_eq!(scen_id, "scen-inc");
        assert_eq!(
            scen_amt,
            Some(8000),
            "a linha de cenário não é auditada/realinhada pelo write-back real"
        );
    }

    // Hardening do backup: o renderer pode enviar qualquer destino; a validação do backend é a
    // última linha de defesa contra sobrescrita de arquivos arbitrários.
    async fn test_pool() -> (SqlitePool, std::path::PathBuf, std::path::PathBuf) {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let dir = std::env::temp_dir().join(format!("neko-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("neko-src.db");
        let src_str = src.to_str().unwrap().to_string();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{src_str}"))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        (pool, src, dir)
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn backup_rejects_non_db_extension() {
        let (pool, src, dir) = test_pool().await;
        let dest = dir.join("neko-backup.txt");
        let err = backup_db(&pool, &src, dest.to_str().unwrap())
            .await
            .unwrap_err();
        assert!(
            err.contains("extensão .db"),
            "erro deve citar a extensão obrigatória: {err}"
        );
        drop(pool);
        cleanup(&dir);
    }

    #[tokio::test]
    async fn backup_rejects_overwriting_non_sqlite_file() {
        let (pool, src, dir) = test_pool().await;
        let dest = dir.join("neko-backup.db");
        std::fs::write(&dest, "nao sou sqlite").unwrap();

        let err = backup_db(&pool, &src, dest.to_str().unwrap())
            .await
            .unwrap_err();
        assert!(
            err.contains("não é um backup SQLite"),
            "erro deve avisar que o destino não é SQLite: {err}"
        );

        drop(pool);
        cleanup(&dir);
    }

    #[tokio::test]
    async fn backup_overwrites_existing_sqlite_backup() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let (pool, src, dir) = test_pool().await;
        let dest = dir.join("neko-backup.db");
        let dest_str = dest.to_str().unwrap();

        // Primeiro backup: cria o arquivo SQLite válido.
        backup_db(&pool, &src, dest_str).await.unwrap();
        let first = std::fs::read(&dest).unwrap();
        assert!(first.starts_with(b"SQLite format 3\0"));

        // Adiciona dados ao banco ativo para que o segundo backup seja diferente.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-x', 'Tester')")
            .execute(&pool)
            .await
            .unwrap();

        // Segundo backup: deve SUBSTITUIR o arquivo existente (também SQLite).
        backup_db(&pool, &src, dest_str).await.unwrap();
        let second = std::fs::read(&dest).unwrap();
        assert!(second.starts_with(b"SQLite format 3\0"));

        // Abre o backup e confere que a linha nova está lá (prova que foi substituído pelo estado
        // atual do banco, não apenas renomeado sem conteúdo novo).
        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::from_str(&format!("sqlite:{dest_str}")).unwrap())
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM person WHERE id = 'pe-x'")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "o backup substituído contém o dado novo");

        drop(backup_pool);
        drop(pool);
        cleanup(&dir);
    }
}
