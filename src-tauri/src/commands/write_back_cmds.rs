use super::*;

/// Lê as transações e as converte para candidatas de write-back da grade diária do `year`
/// (magnitude positiva; a coluna vem do tipo). DECISÃO de método (a questão em aberto planilha↔
/// modelo): o CARTÃO **colapsa para um lump em Saída no VENCIMENTO** — formato canônico que o dono
/// edita à mão (a planilha crua não tem coluna Cartão). Por isso o crédito é carregado pela janela
/// de VENCIMENTO, não da compra: uma compra de DEZ/ano-1 vence em JAN/ano e tem que entrar no ano.
/// Sem cartão configurado, o crédito do ano cai na Saída da própria data. `transfer` (Economia) NÃO
/// entra aqui — vai para a aba `Economia` (ver `build_economia_plan`).
pub(crate) async fn load_write_back_txns(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<WriteBackTxn>, String> {
    // 1) Entrada + Saída/Diário (expense não-crédito) do ano, cada um na sua data.
    let rows: Vec<(String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, type, date, amount, is_fixed FROM \"transaction\" \
         WHERE date >= ?1 AND date < ?2 \
           AND NOT (type='expense' AND payment_method='credit') \
           AND id NOT LIKE 'derived:%'",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query txns: {e}"))?;

    let mut out = Vec::new();
    for (id, t, date, amount, is_fixed) in rows {
        let mag = amount.abs();
        // Plano 036: carrega as partes itemizadas desta linha (vazio = não itemizada → escrita RAW
        // de hoje). N+1 aceitável: write-back é manual e infrequente. Lump de cartão (seção 2) não
        // tem linha 1:1 importável e segue sem breakdown (items = None).
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
        let items = load_txn_items(pool, &id).await?;
        out.push(WriteBackTxn {
            date,
            kind,
            amount_cents: mag,
            items,
        });
    }

    // 2) Cartão → lump no vencimento. Com cartão configurado, junta as compras cujo VENCIMENTO cai
    //    no ano-alvo (janela DEZ/ano-1 .. DEZ/ano, pois a fatura vence ~1 mês após a compra).
    // Dias do cartão são NULL-áveis: FILTRA no SQL (não LIMIT 1 cego) — senão, se o 1º cartão
    // viesse sem ciclo mas existisse outro completo, o write-back ignoraria o ciclo válido e
    // lançaria crédito pela data da compra. Ordem determinística para escolher sempre o mesmo.
    let card: Option<(i64, i64)> = sqlx::query_as(
        "SELECT closing_day, due_day FROM account \
         WHERE type='credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query card: {e}"))?;

    match card {
        Some((closing, due)) => {
            let credit: Vec<(String, i64)> = sqlx::query_as(
                "SELECT date, amount FROM \"transaction\" \
                 WHERE type='expense' AND payment_method='credit' AND date >= ?1 AND date <= ?2",
            )
            .bind(format!("{:04}-12-01", year - 1))
            .bind(format!("{year:04}-12-31"))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("query credit: {e}"))?;

            let mut by_due: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (date, amount) in credit {
                if let Ok(d) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                    let due_date = forecast::cycle_due_date(d, closing as u32, due as u32);
                    if due_date.year() == year {
                        *by_due
                            .entry(due_date.format("%Y-%m-%d").to_string())
                            .or_insert(0) += amount.abs();
                    }
                }
            }
            for (due_date, cents) in by_due {
                out.push(WriteBackTxn {
                    date: due_date,
                    kind: import::RowKind::Saida,
                    amount_cents: cents,
                    // Lump de cartão = soma de compras; sem linha 1:1 → sem breakdown itemizado.
                    items: None,
                });
            }
        }
        None => {
            // Sem cartão: não há ciclo para colapsar — crédito do ano cai na Saída da própria data.
            let credit: Vec<(String, i64)> = sqlx::query_as(
                "SELECT date, amount FROM \"transaction\" \
                 WHERE type='expense' AND payment_method='credit' AND date >= ?1 AND date < ?2",
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
                    items: None, // crédito por compra (sem cartão) também não itemiza.
                });
            }
        }
    }
    Ok(out)
}

/// Plano 036: partes itemizadas de uma transação como `TxnLineItem` (valor + descrição), para o
/// write-back reconstruir `=SUM(...)` + nota. `None` quando há < 2 partes — uma única parte não é um
/// breakdown (não há fórmula a montar), então cai na escrita RAW numérica de hoje. Ordenado por
/// `position` para a fórmula/nota saírem na ordem do dono.
async fn load_txn_items(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Option<Vec<write_back::TxnLineItem>>, String> {
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT amount_cents, description, section FROM line_item \
         WHERE transaction_id = ?1 ORDER BY position",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query line items: {e}"))?;
    if rows.len() < 2 {
        return Ok(None);
    }
    Ok(Some(
        rows.into_iter()
            .map(
                |(amount_cents, description, section)| write_back::TxnLineItem {
                    amount_cents,
                    description,
                    section,
                },
            )
            .collect(),
    ))
}

/// Mensagem (typed-error por string, como o resto deste módulo) quando há conflitos de import
/// pendentes: o write-back é BLOQUEADO até a fila ser resolvida (ADR-0003), senão escreveríamos por
/// cima de um valor que o dono ainda está conciliando. Plano 028 Step 3.
pub(crate) const CONFLICTS_PENDING_MSG: &str =
    "Resolva os conflitos de importação antes de enviar.";

/// Erro quando a planilha mudou ENTRE o preview e o apply: a aprovação do dono vale para o que ele
/// VIU; uma edição concorrente exige re-revisão (não sobrescrever às cegas). Plano 028 Step 4.
pub(crate) const SHEET_CHANGED_MSG: &str =
    "A planilha mudou desde a prévia — gere o preview de novo e revise antes de enviar.";

/// Conta de conflitos de import ainda não resolvidos. > 0 ⇒ o write-back deve abortar (Step 3).
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

/// Decisão PURA da re-verificação de frescura (Step 4): a aprovação do dono vale para a revisão que
/// ele VIU (`seen`); se o `current` (modifiedTime relido no apply) for DIFERENTE, a planilha mudou
/// → aborta. Comparação por igualdade exata da string RFC-3339 do Drive (qualquer edição a avança).
pub(crate) fn staleness_check(seen: &str, current: &str) -> Result<(), String> {
    if current != seen {
        return Err(SHEET_CHANGED_MSG.to_string());
    }
    Ok(())
}

/// Aviso NÃO-bloqueante do write-back: o colapso do lump de cartão (`load_write_back_txns`) usa UM
/// cartão (o primeiro com ciclo completo). Se houver MAIS DE UM cartão com `closing_day`+`due_day`,
/// ou QUALQUER cartão SEM esses dias de ciclo, a data da fatura pode não bater com a intenção do
/// dono — então sinalizamos para a UI pedir conferência. NÃO altera o plano (suporte multi-cartão
/// está fora de escopo); apenas avisa. Plano 028 Step 8.
pub(crate) async fn multi_card_warning(pool: &SqlitePool) -> Result<bool, String> {
    // Cartões com ciclo COMPLETO (ambos os dias) e cartões com ciclo INCOMPLETO (algum dia ausente).
    let (with_cycle, without_cycle): (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN closing_day IS NOT NULL AND due_day IS NOT NULL THEN 1 ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN closing_day IS NULL OR due_day IS NULL THEN 1 ELSE 0 END), 0) \
         FROM account WHERE type = 'credit_card'",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query card cycle: {e}"))?;
    Ok(with_cycle > 1 || without_cycle > 0)
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

/// Resultado RICO da prévia (plano 028): o diff + o `preview_revision` (modifiedTime do Drive no
/// instante da prévia, que o apply re-verifica para forçar re-revisão em edição concorrente) + um
/// aviso não-bloqueante de multi-cartão. Comando NOVO, aditivo: o `preview_write_back` legado segue
/// devolvendo `Vec<CellWrite>` (a UI atual não muda); a UI passa a usar este no PR de hardening.
#[derive(serde::Serialize)]
pub struct WriteBackPreviewResult {
    pub cells: Vec<CellWrite>,
    /// `modifiedTime` RFC-3339 do Drive na hora da prévia (token de frescura para o apply).
    pub preview_revision: String,
    /// Há conflitos de import pendentes? A UI desabilita o envio (espelha o gate do backend).
    pub conflicts_pending: bool,
    /// Mais de um cartão com ciclo, ou um cartão sem ciclo → a data da fatura pode divergir.
    pub multi_card_warning: bool,
}

/// Prévia RICA (read-only) usada pela UI endurecida (PR-B): mesmo plano do `preview_write_back`, mais
/// o `preview_revision` (re-revisão por edição concorrente), o flag de conflitos pendentes e o aviso
/// de multi-cartão. Read-only — seguro com a flag desligada.
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
    let multi_card_warning = multi_card_warning(pool.inner()).await?;
    Ok(WriteBackPreviewResult {
        cells,
        preview_revision,
        conflicts_pending,
        multi_card_warning,
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

/// Resultado do `apply_write_back` (plano 036): nº de células escritas + um aviso NÃO-bloqueante
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
/// Plano 036: células ITEMIZADAS (≥2 partes) escrevem a FÓRMULA `=SUM(...)` via USER_ENTERED + a
/// NOTA por-parte; células normais seguem RAW numérico (inalterado). A nota é fase 2 best-effort:
/// se falhar (ex.: 403 readonly), devolvemos um `note_warning` em vez de abortar — o valor já está
/// gravado. TODOS os gates do plano 028 (flag, conflito, escopo, frescura, blocklist de fórmula via
/// `plan_write_back`) permanecem intactos e ANTES de qualquer escrita.
#[tauri::command]
pub async fn apply_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
    // Token de frescura devolvido por `preview_write_back_status` (Step 4). `None` no caminho legado
    // da UI atual; quando presente, o apply ABORTA se a planilha mudou desde a prévia.
    preview_revision: Option<String>,
) -> Result<WriteBackResult, String> {
    write_back::ensure_write_back_enabled()?;
    // Gate de conflito (Step 3): nunca escrever sob conflitos de import pendentes — ANTES de tocar
    // o cliente do Sheets.
    guard_no_pending_conflicts(pool.inner()).await?;

    let resolved_secret = oauth::pkce::resolve_client_secret(client_secret.clone());
    // Escopo de escrita (Step 1): falha cedo com erro de re-consentimento se o token for readonly.
    oauth::token_store::ensure_write_scope(&app_dir.0, &client_id, resolved_secret.as_deref())
        .await?;

    // Plano 047: foto do `modifiedTime` ANTES de ler os VALORES da aba (mesmo padrão de
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

    // Re-verifica a frescura (Step 4) SEMPRE — nenhum caminho de apply escapa do gate. Foto pós-plano
    // do `modifiedTime`; compara com o token da prévia rica (`preview_revision`) quando presente, ou
    // com a foto inicial (`early_revision`) no caminho legado. Aborta sem escrever se DIVERGIR.
    let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;
    match preview_revision.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(seen) => staleness_check(seen, &post_plan_revision)?,
        None => staleness_check(&early_revision, &post_plan_revision)?,
    }

    // Só as células que MUDARAM; range com nome da aba ('2026'!E3).
    let changed: Vec<&CellWrite> = plan.iter().filter(|c| c.changed).collect();

    // Plano 036: separa as células itemizadas (fórmula USER_ENTERED) das normais (RAW numérico).
    // Não-itemizadas seguem EXATAMENTE como hoje — número cru, RAW, locale-independente.
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

    // Fase 2 (plano 036): notas de célula das itemizadas. Best-effort, NÃO-FATAL — o valor já foi
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

    // Auditoria pós-escrita (Step 7): realinha o `source_*` das transações escritas + registra a
    // escrita no `sync_log`, para que o próximo import reconheça os valores como a NOVA base (sem
    // conflito espúrio). Só roda em escrita bem-sucedida.
    if written > 0 {
        record_write_back_audit(pool.inner(), &sheet_name, &changed).await?;
    }

    Ok(WriteBackResult {
        written,
        note_warning,
    })
}

/// Auditoria pós-escrita do write-back (plano 028 Step 7). Faz DUAS coisas, atômicas:
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
/// `load_write_back_txns`. Cartão (lump no vencimento) não tem linha 1:1 importável, então o realinho
/// foca nas linhas de movimento direto (income/expense em débito); o teste de round-trip cobre isto.
pub(crate) async fn record_write_back_audit(
    pool: &SqlitePool,
    sheet_name: &str,
    cells: &[&CellWrite],
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
        // O kind `saida` cobre DOIS casos físicos na mesma célula: (a) Saídas fixas de débito 1:1 e
        // (b) o LUMP de cartão no vencimento — soma de compras de crédito (`is_fixed=0`,
        // `payment_method='credit'`) agrupadas por `cycle_due_date`. O caso (b) não tem linha 1:1
        // importável, então o realinho do crédito é feito à parte (ver `realign_credit_lump`).
        if c.kind.as_str() == "saida" {
            realigned += realign_saida_cell(&mut tx, c, &now).await?;
            record_write_back_log(&mut tx, sheet_name, &now, profile_id.as_ref(), c).await?;
            continue;
        }
        // kind (string da célula) → critério de seleção da(s) transação(ões) na data.
        let updated = match c.kind.as_str() {
            "entrada" => {
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
                     WHERE date = ?3 AND type = 'income'",
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
                       AND (payment_method IS NULL OR payment_method <> 'credit')",
                )
                .bind(c.value_cents)
                .bind(&now)
                .bind(&c.date)
                .execute(&mut *tx)
                .await
            }
            "economia" => {
                // Economia é mensal e, desde o plano 052, é uma ANOTAÇÃO em `economia_annotation`
                // (não mais uma transação `economia:YYYY-MM`). A célula carrega `date = "YYYY-MM"`.
                // Após escrever na origem, alinhamos a anotação local ao valor escrito (upsert; ou
                // delete se zerado) — assim o próximo import vê origem == anotação, sem conflito
                // espúrio. É o análogo do realinho de `source_amount` dos demais kinds.
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

/// Realinha a base (`source_amount`) das transações de uma célula `saida`. Cobre os dois casos
/// físicos da coluna Saída: (a) Saída fixa de débito 1:1 — `source_amount = valor escrito`; (b) o
/// LUMP de cartão no vencimento — as compras de crédito cujo `cycle_due_date` cai na data da célula
/// (ver `realign_credit_lump`). Devolve o total de linhas realinhadas (débito + crédito).
async fn realign_saida_cell(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    c: &CellWrite,
    now: &str,
) -> Result<usize, String> {
    // (a) Saída fixa de débito (linha 1:1). NÃO toca em linhas de crédito (tratadas em (b)).
    let debit = sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
           AND (payment_method IS NULL OR payment_method <> 'credit')",
    )
    .bind(c.value_cents)
    .bind(now)
    .bind(&c.date)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("realign source_amount (saida débito): {e}"))?;

    // (b) Lump de cartão: realinha as compras de crédito cujo vencimento cai na data da célula.
    let credit = realign_credit_lump(tx, &c.date, now).await?;
    Ok(debit.rows_affected() as usize + credit)
}

/// Realinha a base das compras de crédito que compõem o LUMP de uma célula Saída escrita na data
/// `due_date`. O lump da planilha é `SUM(ABS(amount))` das compras cujo `cycle_due_date` é `due_date`
/// — não há coluna por-compra na planilha, então a base por-linha não tem snapshot rastreável. Ao
/// zerar `source_amount` (`NULL`) dessas compras, o merge de 3 vias do próximo import vê `base
/// ausente` (`reconcile` com `base = None` → `ApplySheet`, sem conflito): o lump agregado passa a
/// ser a nova base autoritativa e as compras individuais ficam abaixo da granularidade rastreada.
///
/// O vencimento é computado em Rust com `forecast::cycle_due_date` — a MESMA função que
/// `load_write_back_txns` usa para montar o lump —, então o agrupamento não diverge. App suporta um
/// cartão (`ORDER BY created_at, id LIMIT 1`, igual ao load); sem cartão configurado, nada a fazer.
async fn realign_credit_lump(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    due_date: &str,
    now: &str,
) -> Result<usize, String> {
    let card: Option<(i64, i64)> = sqlx::query_as(
        "SELECT closing_day, due_day FROM account \
         WHERE type='credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("query card for audit: {e}"))?;

    let Some((closing, due)) = card else {
        return Ok(0); // sem cartão com ciclo → crédito caiu na própria data; nada a colapsar.
    };

    // Plano 047: limita o scan ao período relevante. Sem bound, uma compra de ANOS atrás com o mesmo
    // dia-do-mês produziria o mesmo `cycle_due_date` calculado e seria realinhada por engano (base
    // zerada → no-conflito espúrio no próximo import). Um ciclo de fatura abrange ~2 meses, então
    // ir 2 anos para trás (1º de janeiro) é uma janela conservadora: larga o bastante para conter as
    // compras de qualquer ciclo único, mas exclui compras de anos anteriores.
    let cutoff = {
        let due = NaiveDate::parse_from_str(due_date, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());
        NaiveDate::from_ymd_opt(due.year() - 2, 1, 1)
            .unwrap_or(due)
            .format("%Y-%m-%d")
            .to_string()
    };

    let candidates: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, date FROM \"transaction\" \
         WHERE type='expense' AND payment_method='credit' \
           AND date >= ?1",
    )
    .bind(&cutoff)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("query credit candidates: {e}"))?;

    let matching_ids: Vec<String> = candidates
        .into_iter()
        .filter_map(|(id, date_str)| {
            let d = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let computed = forecast::cycle_due_date(d, closing as u32, due as u32);
            (computed.format("%Y-%m-%d").to_string() == due_date).then_some(id)
        })
        .collect();

    let mut n = 0usize;
    for id in &matching_ids {
        sqlx::query(
            "UPDATE \"transaction\" SET source_amount = NULL, updated_at = ?1 WHERE id = ?2",
        )
        .bind(now)
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("realign credit source_amount: {e}"))?;
        n += 1;
    }
    Ok(n)
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

/// Economia REGISTRADA por mês (1..=12) do ano: numerador do Economizado% do método e o que vai
/// para a coluna `Economia` da aba homônima no write-back. Soma DUAS fontes disjuntas (plano 052):
/// (A) a anotação manual da aba Economia (`economia_annotation`) e (B) os transfers→reserva/ilíquido
/// MANUAIS criados no Neko (plano 003). Nunca há sobreposição: a anotação só vem do import da aba;
/// o transfer manual nunca entra em `economia_annotation` — então somar não duplica.
pub(crate) async fn load_economia_by_month(
    pool: &SqlitePool,
    year: i32,
) -> Result<[i64; 12], String> {
    let mut by = [0i64; 12];

    // (A) Anotação da aba Economia.
    let annot: Vec<(i64, i64)> =
        sqlx::query_as("SELECT month, amount_cents FROM economia_annotation WHERE year = ?1")
            .bind(year as i64)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("query economia annotation: {e}"))?;
    for (m, cents) in annot {
        if (1..=12).contains(&m) {
            by[(m - 1) as usize] += cents;
        }
    }

    // (B) Transfers→reserva/ilíquido MANUAIS (plano 003), agregados por mês.
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT substr(t.date, 6, 2), COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.type = 'transfer' \
           AND a.liquidity IN ('reserve','illiquid') \
         GROUP BY substr(t.date, 6, 2)",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query economia: {e}"))?;
    for (mm, cents) in rows {
        if let Ok(m) = mm.parse::<usize>()
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
    let by_month = load_economia_by_month(pool, year).await?;
    let plan = write_back::plan_economia_write_back(&values.values, year, &by_month);
    Ok((client, plan))
}

/// Preview READ-ONLY do write-back da Economia (transfers→reserva → coluna `Economia` por mês).
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
        multi_card_warning: false, // a Economia não depende de ciclo de cartão
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
    // Token de frescura (Step 4); `None` no caminho legado da UI.
    preview_revision: Option<String>,
) -> Result<usize, String> {
    write_back::ensure_write_back_enabled()?;
    // Gate de conflito (Step 3) antes de qualquer escrita / chamada ao cliente.
    guard_no_pending_conflicts(pool.inner()).await?;

    let resolved_secret = oauth::pkce::resolve_client_secret(client_secret.clone());
    // Escopo de escrita (Step 1).
    oauth::token_store::ensure_write_scope(&app_dir.0, &client_id, resolved_secret.as_deref())
        .await?;

    // Gate de frescura SEMPRE-LIGADO (espelha `apply_write_back` / padrão do plano 047). Foto do
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

    // Re-verifica a frescura (Step 4) SEMPRE — nenhum caminho de apply escapa do gate. Foto pós-plano;
    // compara com o token da prévia rica (`preview_revision`) quando presente, ou com a foto inicial
    // (`early_revision`) no caminho legado. Aborta sem escrever se DIVERGIR.
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

/// Persiste os valores da aba Economia como uma ANOTAÇÃO de métrica (decisão do dono, plano 052):
/// a poupança já é lançada como Saída no grid mensal (→ FixedOut/Daily → cost_of_living → Saldo UMA
/// vez). A aba Economia é a anotação manual do Economizado% (= Economia/Entradas), NÃO um segundo
/// movimento de caixa. Por isso gravamos em `economia_annotation` (fora do `transaction`), nunca como
/// `type='transfer'` — assim o valor NÃO entra na cadeia do Saldo (sem dupla contagem). É distinto do
/// transfer-de-reserva MANUAL (plano 003), que continua um movimento real e entra no Saldo via
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

    // Bug 2 (plano 037): a fatura de cartão é escrita como um LUMP em Saída no vencimento, agregado
    // de compras de crédito individuais (is_fixed=0, payment_method='credit') agrupadas por
    // `cycle_due_date`. O braço `saida` da auditoria só realinhava linhas `is_fixed=1` não-crédito →
    // casava ZERO linhas para o lump → a base ficava STALE → conflito espúrio no próximo import.
    // O fix realinha a base (`source_amount = NULL`) das compras de crédito cujo vencimento cai na
    // data da célula. `base = None` faz o merge de 3 vias devolver `ApplySheet` (sem conflito).
    #[tokio::test]
    async fn credit_lump_writeback_realigns_source_amount() {
        let p = pool().await;

        // Cartão: fecha dia 25, vence dia 5. Compras de 20 e 22/MAI (≤ 25) vencem em 05/JUN.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-1', 'Cartão', 'credit_card', 'pe-1', 25, 5)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection, source_amount) \
             VALUES ('buy-1', 'expense', 3000, '2026-05-20', 'credit', 0, 0, 3000), \
                    ('buy-2', 'expense', 2000, '2026-05-22', 'credit', 0, 0, 2000)",
        )
        .execute(&p)
        .await
        .unwrap();

        // A compra abaixo vence em OUTRO ciclo (06/JUN > 25 → vence 05/JUL): NÃO pode ser realinhada.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection, source_amount) \
             VALUES ('buy-other', 'expense', 9000, '2026-06-06', 'credit', 0, 0, 9000)",
        )
        .execute(&p)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "F5".into(),
            row: 4,
            col: 5,
            date: "2026-06-05".into(),
            kind: "saida".into(),
            current: "0,00".into(),
            proposed: "50,00".into(),
            value_cents: 5000, // 3000 + 2000, o lump da fatura
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();
        assert_eq!(
            realigned, 2,
            "as duas compras do ciclo de 05/JUN são realinhadas"
        );

        // As compras do lump têm a base zerada (NULL) → o próximo import com sheet_value = 5000 vê
        // `base = None` → ApplySheet, sem conflito espúrio.
        let rows: Vec<(String, Option<i64>)> = sqlx::query_as(
            "SELECT id, source_amount FROM \"transaction\" \
             WHERE id IN ('buy-1', 'buy-2') ORDER BY id",
        )
        .fetch_all(&p)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        for (id, src) in &rows {
            assert!(
                src.is_none(),
                "{id}: a base do crédito é zerada (NULL) após o write-back do lump"
            );
        }

        // A compra de outro ciclo permanece intacta (base preservada).
        let (other,): (Option<i64>,) =
            sqlx::query_as("SELECT source_amount FROM \"transaction\" WHERE id = 'buy-other'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            other,
            Some(9000),
            "compra de outro vencimento não é tocada pelo realinho do lump"
        );
    }

    // Bug 1 (plano 052) — a aba Economia é uma ANOTAÇÃO de métrica, não um movimento de caixa.
    // `store_economia_entries` grava em `economia_annotation` (fora do `transaction`) → nunca cria
    // uma linha `economia:YYYY-MM` nem entra na cadeia do Saldo. Sem isso, a mesma poupança (já
    // lançada como Saída no grid) seria descontada do Saldo uma 2ª vez (dupla contagem do P0).
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

    // Bug 1 (plano 052) — a anotação não é carregada como evento de caixa, então o Saldo projetado
    // não a desconta (sem dupla contagem). `signed()`/Performance ficam intactos: só transfers REAIS
    // (plano 003) seguem como `EventKind::Economia` e tocam o Saldo.
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

    // Plano 047 (P2): `realign_credit_lump` antes scaneava TODAS as compras de crédito sem bound de
    // data. Uma compra de ANOS atrás com o mesmo dia-do-mês produz o mesmo `cycle_due_date` calculado
    // e era realinhada por engano (base zerada → no-conflito espúrio no próximo import). O fix limita
    // o scan a `date >= 1º/jan do ano-2`, excluindo compras de anos anteriores.
    #[tokio::test]
    async fn realign_credit_lump_ignores_purchases_from_prior_years() {
        let p = pool().await;

        // Mesmo cartão do teste de lump: fecha dia 25, vence dia 5.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-1', 'Cartão', 'credit_card', 'pe-1', 25, 5)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Compra RECENTE: 20/MAI/2026 (≤ 25) → vence 05/JUN/2026.
        // Compra ANTIGA: 20/MAI/2023 (mesmo dia-do-mês) → vence 05/JUN/2023 (mesmo padrão de dia).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection, source_amount) \
             VALUES ('buy-2026', 'expense', 3000, '2026-05-20', 'credit', 0, 0, 3000), \
                    ('buy-2023', 'expense', 7000, '2023-05-20', 'credit', 0, 0, 7000)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Write-back do lump da fatura de 05/JUN/2026.
        let cell = CellWrite {
            a1: "F5".into(),
            row: 4,
            col: 5,
            date: "2026-06-05".into(),
            kind: "saida".into(),
            current: "0,00".into(),
            proposed: "30,00".into(),
            value_cents: 3000,
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&p, "2026", &[&cell]).await.unwrap();
        assert_eq!(
            realigned, 1,
            "só a compra de 2026 entra no ciclo de 05/JUN/2026 (a de 2023 fica fora da janela)"
        );

        // A compra de 2026 teve a base zerada (NULL).
        let (recent,): (Option<i64>,) =
            sqlx::query_as("SELECT source_amount FROM \"transaction\" WHERE id = 'buy-2026'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert!(recent.is_none(), "a base da compra de 2026 é zerada");

        // A compra de 2023 permanece INTACTA (fora do bound de data → nunca avaliada).
        let (old,): (Option<i64>,) =
            sqlx::query_as("SELECT source_amount FROM \"transaction\" WHERE id = 'buy-2023'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            old,
            Some(7000),
            "a compra de ano anterior não é realinhada por engano"
        );
    }

    // Plano 047 (P2): o gate de frescura (Step 4 do plano 028) agora roda SEMPRE no apply, inclusive
    // no caminho legado (sem `preview_revision`). `apply_write_back` depende de IO de rede, então
    // testamos a decisão PURA (`staleness_check`): revisão igual passa, revisão diferente aborta.
    #[tokio::test]
    async fn staleness_check_rejects_different_revision() {
        // Mesma revisão → OK (a planilha não mudou desde a foto).
        staleness_check("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z").unwrap();

        // Revisão diferente → aborta (a planilha avançou; exige re-revisão).
        let err = staleness_check("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z").unwrap_err();
        assert_eq!(err, SHEET_CHANGED_MSG, "diff stale é rejeitado");
    }
}
