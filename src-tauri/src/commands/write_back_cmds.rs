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
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT type, date, amount, is_fixed FROM \"transaction\" \
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
    for (t, date, amount, is_fixed) in rows {
        let mag = amount.abs();
        match t.as_str() {
            "income" => out.push(WriteBackTxn {
                date,
                kind: import::RowKind::Entrada,
                amount_cents: mag,
            }),
            "expense" => out.push(WriteBackTxn {
                date,
                kind: if is_fixed != 0 {
                    import::RowKind::Saida
                } else {
                    import::RowKind::Diario
                },
                amount_cents: mag,
            }),
            _ => {} // transfer (Economia) → aba Economia
        }
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
                });
            }
        }
    }
    Ok(out)
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

/// Re-verifica a frescura da planilha: compara o `modifiedTime` atual do Drive com o `preview_revision`
/// que o dono viu na prévia. Se AVANÇOU, aborta (Step 4). `preview_revision = None` ⇒ sem checagem
/// (compatibilidade: o frontend só passa o token a partir do PR-B; até lá o gate fica inerte, e o
/// envio real continua atrás de `WRITE_BACK_ENABLED`).
pub(crate) async fn guard_sheet_unchanged(
    client: &SheetsClient,
    spreadsheet_id: &str,
    preview_revision: Option<&str>,
) -> Result<(), String> {
    let Some(seen) = preview_revision.filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let current = client.get_file_modified_time(spreadsheet_id).await?;
    staleness_check(seen, &current)
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
    let (client, cells) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;
    let preview_revision = client.get_file_modified_time(&spreadsheet_id).await?;
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

/// Aplica o write-back: escreve as células DIVERGENTES de volta na aba. Trava-mestra: enquanto
/// `WRITE_BACK_ENABLED` estiver desligado, falha cedo e NÃO escreve nada. A UI já obteve o diff via
/// `preview_write_back` e o humano aprovou; aqui só replanejamos (a planilha pode ter mudado) e
/// escrevemos as células que ainda diferem. Retorna quantas células foram escritas.
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
) -> Result<usize, String> {
    write_back::ensure_write_back_enabled()?;
    // Gate de conflito (Step 3): nunca escrever sob conflitos de import pendentes — ANTES de tocar
    // o cliente do Sheets.
    guard_no_pending_conflicts(pool.inner()).await?;

    let resolved_secret = oauth::pkce::resolve_client_secret(client_secret.clone());
    // Escopo de escrita (Step 1): falha cedo com erro de re-consentimento se o token for readonly.
    oauth::token_store::ensure_write_scope(&app_dir.0, &client_id, resolved_secret.as_deref())
        .await?;

    let (client, plan) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;

    // Re-verifica a frescura (Step 4): aborta sem escrever se o `modifiedTime` avançou.
    guard_sheet_unchanged(&client, &spreadsheet_id, preview_revision.as_deref()).await?;

    // Só as células que MUDARAM; range com nome da aba ('2026'!E3); valor numérico em reais.
    let changed: Vec<&CellWrite> = plan.iter().filter(|c| c.changed).collect();
    let updates: Vec<(String, f64)> = changed
        .iter()
        .map(|c| {
            (
                format!("{}!{}", quote_sheet(&sheet_name), c.a1),
                c.value_cents as f64 / 100.0,
            )
        })
        .collect();

    let written = client
        .batch_update_values(&spreadsheet_id, &updates)
        .await?;

    // Auditoria pós-escrita (Step 7): realinha o `source_*` das transações escritas + registra a
    // escrita no `sync_log`, para que o próximo import reconheça os valores como a NOVA base (sem
    // conflito espúrio). Só roda em escrita bem-sucedida.
    if written > 0 {
        record_write_back_audit(pool.inner(), &sheet_name, &changed).await?;
    }

    Ok(written)
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
            "saida" => {
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
                     WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
                       AND (payment_method IS NULL OR payment_method <> 'credit')",
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
                // Economia é mensal: a célula carrega `date = "YYYY-MM"` e a transação-resumo tem
                // id determinístico `economia:YYYY-MM` (ver `store_economia_entries`). Realinha a
                // base desse mês para o valor escrito, igual aos demais kinds — assim o próximo
                // import não acusa conflito espúrio na Economia.
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
                     WHERE id = ?3 AND type = 'transfer'",
                )
                .bind(c.value_cents)
                .bind(&now)
                .bind(format!("economia:{}", c.date))
                .execute(&mut *tx)
                .await
            }
            _ => continue, // kind desconhecido: nada a realinhar nem a auditar
        }
        .map_err(|e| format!("realign source_amount: {e}"))?;
        realigned += updated.rows_affected() as usize;

        // Trilha no sync_log (best-effort de auditoria): só quando há um profile a referenciar (FK
        // NOT NULL). Id determinístico por (aba, célula) → idempotente entre escritas repetidas.
        if let Some((pid,)) = profile_id.as_ref() {
            let log_id = format!("writeback:{sheet_name}:{}", c.a1);
            sqlx::query(
                "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, source_sheet) \
                 VALUES (?1, 'write_back', 'cell', ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET timestamp = excluded.timestamp",
            )
            .bind(&log_id)
            .bind(&c.a1)
            .bind(pid)
            .bind(&now)
            .bind(sheet_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("sync_log write_back: {e}"))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| format!("commit audit: {e}"))?;
    Ok(realigned)
}

/// Economia REGISTRADA por mês (1..=12) do ano: soma dos transfers→reserva/ilíquido. É o numerador
/// do Economizado% do método e o que vai para a coluna `Economia` da aba homônima no write-back.
pub(crate) async fn load_economia_by_month(
    pool: &SqlitePool,
    year: i32,
) -> Result<[i64; 12], String> {
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
    let mut by = [0i64; 12];
    for (mm, cents) in rows {
        if let Ok(m) = mm.parse::<usize>()
            && (1..=12).contains(&m)
        {
            by[m - 1] = cents;
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
    let (client, cells) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;
    let preview_revision = client.get_file_modified_time(&spreadsheet_id).await?;
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

    let (client, plan) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;

    // Re-verifica a frescura (Step 4) antes de escrever.
    guard_sheet_unchanged(&client, &spreadsheet_id, preview_revision.as_deref()).await?;

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

/// Conta de RESERVA destino da Economia. Usa a primeira `liquidity='reserve'`; se não houver, cria
/// uma "Reserva" padrão (savings/reserve) do 1º titular — assim a Economia importada tem para onde ir.
pub(crate) async fn ensure_reserve_account(pool: &SqlitePool) -> Result<String, String> {
    if let Some((id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM account WHERE liquidity='reserve' ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve: {e}"))?
    {
        return Ok(id);
    }
    let owner: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
         VALUES (?1, 'Reserva', 'savings', ?2, 0, 'reserve')",
    )
    .bind(&id)
    .bind(owner.map(|(p,)| p))
    .execute(pool)
    .await
    .map_err(|e| format!("create reserve: {e}"))?;
    Ok(id)
}

pub(crate) async fn store_economia_entries(
    pool: &SqlitePool,
    entries: &[(i32, u32, i64)],
) -> Result<usize, String> {
    let today = chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339();

    // A conta de reserva é pré-requisito das linhas com economia > 0 — resolvida ANTES da transação
    // (assim um import só de zeros/deleções não cria uma reserva à toa). Os upserts/deletes correm
    // numa ÚNICA transação: uma falha no meio deixaria o Economizado%/ColchaoCard parcialmente errado.
    let needs_reserve = entries.iter().any(|(_, _, cents)| *cents > 0);
    let reserve_id = if needs_reserve {
        Some(ensure_reserve_account(pool).await?)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let mut count = 0usize;

    for (year, month, cents) in entries {
        let last = forecast::last_day_of_month(*year, *month);
        let date = last.format("%Y-%m-%d").to_string();
        let id = format!("economia:{year:04}-{month:02}");

        if *cents > 0 {
            let Some(reserve) = reserve_id.as_ref() else {
                return Err("conta de reserva não resolvida para a Economia".into());
            };
            let is_projection = (last > today) as i64;
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, description, date, to_account_id, is_projection, created_at, updated_at) \
                 VALUES (?1, 'transfer', ?2, 'Economia (importada da aba Economia)', ?3, ?4, ?5, ?6, ?6) \
                 ON CONFLICT(id) DO UPDATE SET amount=excluded.amount, date=excluded.date, \
                   is_projection=excluded.is_projection, updated_at=excluded.updated_at",
            )
            .bind(&id)
            .bind(cents)
            .bind(&date)
            .bind(reserve)
            .bind(is_projection)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("upsert economia: {e}"))?;
        } else {
            sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
                .bind(&id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("delete economia: {e}"))?;
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
