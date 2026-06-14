use crate::forecast::{self, CashflowEvent};
use crate::google_sheets::{self, SheetsClient, import, layout_detect};
use crate::oauth::{self, AppDataDir, OAuthStateStore};
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn start_oauth_flow(
    state: tauri::State<'_, OAuthStateStore>,
    app_dir: tauri::State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<String, String> {
    let config = oauth::pkce::OAuthConfig::google(client_id, client_secret);
    let port = find_available_port()?;
    let oauth_state = oauth::pkce::OAuthState::new(port);

    let app_dir_path = app_dir.0.clone();
    let config_for_bg =
        oauth::pkce::OAuthConfig::google(config.client_id.clone(), config.client_secret.clone());

    // Store state and spawn flow
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        let flow_state = oauth_state.clone();
        *guard = Some(oauth_state);

        tokio::spawn(async move {
            match oauth::run_oauth_flow(config_for_bg, flow_state, app_dir_path).await {
                Ok(_token) => {}
                Err(e) => eprintln!("OAuth flow error: {e}"),
            }
        });
    }

    // Clear state after spawn
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        *guard = None;
    }

    Ok("oauth_started".to_string())
}

#[tauri::command]
pub async fn check_auth_status(app_dir: tauri::State<'_, AppDataDir>) -> Result<String, String> {
    // DEBUG TEMPORÁRIO (dogfooding): visível no stderr do tauri dev.
    eprintln!("check_auth_status: app_dir={:?}", app_dir.0);
    let result = match crate::oauth::token_store::load_token(&app_dir.0) {
        Ok(Some(token)) => {
            // Access token expirado mas com refresh_token disponível segue "connected":
            // ensure_valid_token renova sob demanda no próximo uso (spec 010, slice 2).
            if crate::oauth::token_store::is_token_expired(&token) && token.refresh_token.is_empty()
            {
                Ok("expired".to_string())
            } else {
                Ok("connected".to_string())
            }
        }
        Ok(None) => Ok("disconnected".to_string()),
        Err(e) => Err(e),
    };
    eprintln!("check_auth_status -> {result:?}");
    result
}

#[tauri::command]
pub async fn disconnect_google(app_dir: tauri::State<'_, AppDataDir>) -> Result<(), String> {
    crate::oauth::token_store::delete_token(&app_dir.0)
}

fn find_available_port() -> Result<u16, String> {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    drop(listener);
    Ok(port)
}

#[derive(serde::Serialize)]
pub struct SheetInfo {
    pub title: String,
    pub sheet_id: i64,
}

#[tauri::command]
pub async fn list_sheet_names(
    app_dir: State<'_, AppDataDir>,
    spreadsheet_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<SheetInfo>, String> {
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let metadata = client.get_sheet_metadata(&spreadsheet_id).await?;
    let sheets = google_sheets::parse_sheet_names(&metadata);
    Ok(sheets
        .into_iter()
        .map(|s| SheetInfo {
            title: s.title,
            sheet_id: s.sheet_id,
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct SheetPreview {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

#[tauri::command]
pub async fn fetch_sheet_preview(
    app_dir: State<'_, AppDataDir>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<SheetPreview, String> {
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Grade inteira (não A1:Z21) — a planilha real vai até a coluna BO (~71 col); A:Z cortaria
    // JUNHO–DEZEMBRO no preview, como no import (auditoria vs planilha oficial, P1).
    let range = format!("'{}'", sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;

    let mut rows = values.values;
    let total_rows = rows.len();

    let headers = if rows.is_empty() {
        vec![]
    } else {
        rows.remove(0)
    };

    // O preview exibe só as primeiras linhas; o range completo garante todas as colunas/meses.
    rows.truncate(20);

    Ok(SheetPreview {
        headers,
        rows,
        total_rows: total_rows.saturating_sub(1),
    })
}

#[tauri::command]
pub async fn import_sheet_data(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    profile_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    if layout_detect::is_metric_tab(&sheet_name) {
        return Err(format!(
            "'{sheet_name}' é uma aba de métricas do método (não tem transações). \
             Importe as abas-ano; o import de métricas chega na spec 010."
        ));
    }
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Grade usada inteira: a planilha real tem 12 blocos mensais até a coluna BO (~71
    // colunas) — um range A:Z cortaria JUNHO–DEZEMBRO em silêncio (spec 010, slice 0).
    let range = format!("'{}'", sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;
    let rows = values.values;

    if rows.len() < 3 {
        return Ok(0);
    }

    let layout = match import::get_layout_for_sheet(&pool, &sheet_name).await? {
        Some(l) => l,
        None => {
            let detected = layout_detect::detect_layout(&rows, &sheet_name)?;
            sqlx::query(
                "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
            )
            .bind(&detected.id).bind(&detected.sheet_name).bind(detected.year)
            .bind(detected.month_names_row).bind(detected.header_row).bind(detected.data_start_row)
            .bind(detected.day_column).bind(detected.block_size).bind(&detected.date_direction)
            .execute(pool.inner())
            .await
            .map_err(|e| format!("save layout: {e}"))?;

            let mappings = layout_detect::generate_mappings(&detected);
            for m in &mappings {
                sqlx::query(
                    "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
                )
                .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
                .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
                .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
                .execute(pool.inner())
                .await
                .map_err(|e| format!("save mapping: {e}"))?;
            }
            detected
        }
    };

    let mappings = import::get_active_mappings_for_sheet(&pool, &sheet_name).await?;
    // Notas de célula = a descrição real de cada lançamento (quem/o quê/quanto por item). Sem
    // elas todo lançamento virava "Entrada/Saída 2026" e a trilha de auditoria se perdia
    // (auditoria vs planilha oficial). Falha de notas não bloqueia o import dos valores.
    let notes = client
        .get_sheet_notes(&spreadsheet_id, &sheet_name)
        .await
        .unwrap_or_default();
    let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &notes);
    let count = import::import_rows(&pool, &sheet_name, &imported_rows, &profile_id).await?;

    // Captura a coluna Saldo (o saldo corrente do método) → semente da projeção + visão
    // histórica. Sem isto a semente era 0 e o saldo de hoje aparecia zerado.
    let balance_offset = import::get_balance_offset_for_sheet(&pool, &sheet_name).await?;
    let balances = import::parse_balance_series(&rows, &layout, balance_offset);
    import::store_balance_series(&pool, &sheet_name, &balances).await?;

    Ok(count)
}

/// Células numéricas do calamine viram string decimal-com-ponto de 4 casas fixas: `123.456`
/// vira `123.4560`, que o `parse_number` nunca confunde com agrupamento de milhar
/// (spec 010, slice 0 — antes, `65.28` perdia o ponto e inflava 100×).
fn xlsx_cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Float(f) => format!("{f:.4}"),
        other => other.to_string().trim().to_string(),
    }
}

#[tauri::command]
pub async fn import_local_xlsx(
    pool: State<'_, SqlitePool>,
    file_path: String,
    profile_id: String,
) -> Result<String, String> {
    use calamine::{Reader, Xlsx, open_workbook};

    let mut workbook: Xlsx<_> =
        open_workbook(&file_path).map_err(|e| format!("open error: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut total = 0usize;
    let mut sheets_imported = Vec::new();

    for sheet_name in &sheet_names {
        if layout_detect::is_metric_tab(sheet_name) {
            continue;
        }
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| row.iter().map(xlsx_cell_to_string).collect())
                .collect();

            if rows.len() < 3 {
                continue;
            }

            let layout = match import::get_layout_for_sheet(&pool, sheet_name).await? {
                Some(l) => l,
                None => match layout_detect::detect_layout(&rows, sheet_name) {
                    Ok(detected) => {
                        sqlx::query(
                                "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
                            )
                            .bind(&detected.id).bind(&detected.sheet_name).bind(detected.year)
                            .bind(detected.month_names_row).bind(detected.header_row).bind(detected.data_start_row)
                            .bind(detected.day_column).bind(detected.block_size).bind(&detected.date_direction)
                            .execute(pool.inner())
                            .await
                            .map_err(|e| format!("save layout: {e}"))?;

                        let mappings = layout_detect::generate_mappings(&detected);
                        for m in &mappings {
                            sqlx::query(
                                    "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
                                )
                                .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
                                .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
                                .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
                                .execute(pool.inner())
                                .await
                                .map_err(|e| format!("save mapping: {e}"))?;
                        }
                        detected
                    }
                    Err(_) => continue,
                },
            };

            let mappings = import::get_active_mappings_for_sheet(&pool, sheet_name).await?;
            // xlsx (calamine) não expõe notas de célula → fallback "Entrada/Saída {data}". As
            // notas só vêm pelo caminho ao vivo (Sheets API). Limitação documentada na spec 010.
            let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &[]);
            if !imported_rows.is_empty() {
                let count =
                    import::import_rows(&pool, sheet_name, &imported_rows, &profile_id).await?;
                total += count;
                sheets_imported.push(format!("{sheet_name} ({count} rows)"));
            }

            // Série de Saldo da aba (semente da projeção + visão histórica do livro-razão).
            let balance_offset = import::get_balance_offset_for_sheet(&pool, sheet_name).await?;
            let balances = import::parse_balance_series(&rows, &layout, balance_offset);
            import::store_balance_series(&pool, sheet_name, &balances).await?;
        }
    }

    Ok(format!(
        "Imported {} total rows from: {}",
        total,
        sheets_imported.join(", ")
    ))
}

// --- App info ---

#[derive(serde::Serialize)]
pub struct AppInfo {
    pub version: String,
    pub db_path: String,
}

#[tauri::command]
pub async fn get_app_info(app_dir: State<'_, AppDataDir>) -> Result<AppInfo, String> {
    Ok(app_info_for_dir(&app_dir.0))
}

/// Pure helper so the command stays a thin adapter (testable without Tauri `State`).
fn app_info_for_dir(app_dir: &std::path::Path) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        db_path: app_dir.join("neko-finance.db").display().to_string(),
    }
}

// --- Forecast projection (spec 005) ---

/// Sum of liquid cash accounts — the projection seed (spec 003 US2).
/// Spec 007: only `liquidity = 'liquid'` pockets are cash; reserve/restricted/illiquid
/// money must not inflate the projected balance.
async fn liquid_seed(pool: &SqlitePool) -> Result<i64, String> {
    let seed: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'liquid'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;
    Ok(seed.0)
}

/// Semente da projeção — o saldo de partida do qual a engine encadeia o futuro.
///
/// Método da planilha: a coluna `Saldo` É o saldo que "bate com o banco". Quando há série
/// importada, a semente = `Saldo` do dia mais recente ≤ hoje; quaisquer lançamentos realizados
/// ENTRE esse dia e hoje são somados (cobre o caso de a planilha ainda não ter hoje preenchido),
/// de modo que o carregador de eventos pode seguir usando `date > today` sem perder o intervalo.
/// Sem planilha importada, cai nos Bolsos líquidos (spec 007). Precedência: planilha > bolsos —
/// quem importa a planilha quer que a projeção continue a própria linha dela.
async fn projection_seed(pool: &SqlitePool, today_naive: NaiveDate) -> Result<i64, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    let latest: Option<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date <= ?1 ORDER BY date DESC LIMIT 1",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("seed query: {e}"))?;

    let Some((seed_date, balance)) = latest else {
        return liquid_seed(pool).await;
    };

    let gap: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
         FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense')",
    )
    .bind(&seed_date)
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("seed gap: {e}"))?;

    Ok(balance + gap.0)
}

/// Meta de poupança do método: piso de 25% (faixa 20–30%, "média ANUAL" — "o ano tem que ser de
/// 20 a 30", aula "Como viver abaixo do que ganha"). Régua do guardrail "pode gastar".
const SAVINGS_TARGET_BPS: i64 = 2500;

/// Limiar de cobertura: um mês futuro com menos de 60% do gasto típico já lançado é tratado como
/// INCOMPLETO (projeção otimista demais — o "chá revelação" do método). Margem ampla porque o
/// método aceita variação mês a mês; abaixo disso é quase certo que falta fatura/variável.
const COVERAGE_COMPLETE_BPS: i64 = 6_000;

/// Renda e poupança REALIZADAS do ano corrente até hoje (`is_projection = 0`). Proxy de
/// Entradas/Economia da aba Economia (que ainda não é importada — slice 7): a poupança é o net
/// `renda − saída` realizado. Usada no guardrail anual: o ano PROJETADO mente quando os meses
/// futuros estão incompletos (só fixas/salário), então a régua olha só o que já aconteceu.
/// `transfer` é IGNORADO (não há linha Economia explícita ainda) — a poupança real virá do saldo
/// da reserva quando o slice de Economia existir; até lá o net é um proxy conservador (review P2).
///
/// Conta só **meses COMPLETOS** do ano (`substr(date) < mês corrente`), nunca o mês em andamento.
/// Meio do mês as contas já entraram (dias 10–12) mas o salário não (dia ~29), o que daria um
/// net negativo de timing e um "pode gastar R$ 0" de falso pânico (auditoria vs planilha oficial).
///
/// NÃO filtra `is_projection`: ele é congelado no import (data vs hoje DAQUELE dia) e fica STALE
/// quando o dono não re-importa por dias/meses. Um mês completo já passou — é realizado por
/// definição —, então a janela de DATA é a fonte de verdade, não o flag congelado.
async fn realized_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND substr(date,1,7) < ?2 \
           AND type IN ('income','expense')",
    )
    .bind(&year_start)
    .bind(&cur_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized annual: {e}"))?;
    Ok((row.0, row.0 - row.1)) // (renda, poupança=net) dos meses completos
}

/// Renda e net do ANO INTEIRO projetado (todas as linhas do ano). Exibido só como contraste com
/// o realizado — é OTIMISTA quando os meses futuros estão incompletos (não usar no guardrail).
async fn projected_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    // Range explícito (não `LIKE 'YYYY%'`) — consistente com o realizado e rejeita data
    // malformada que começa com o ano mas não é ISO válida (review P2).
    let start = format!("{}-01-01", today_naive.year());
    let end = format!("{}-12-31", today_naive.year());
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date <= ?2 AND type IN ('income','expense')",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("projected annual: {e}"))?;
    Ok((row.0, row.0 - row.1))
}

/// Gasto típico de um mês = MEDIANA da saída dos meses realizados COMPLETOS (anteriores ao mês
/// corrente), dos **últimos 6 meses** (recentes representam melhor o padrão atual que meses
/// antigos de anos anteriores — review ui-vs-planilha). Mediana para ser robusta a um mês atípico.
async fn realized_monthly_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT SUM(amount) FROM \"transaction\" \
         WHERE type='expense' AND substr(date,1,7) < ?1 \
         GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6",
    )
    .bind(&cur_ym)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("baseline: {e}"))?;
    let mut vals: Vec<i64> = rows.into_iter().map(|(s,)| s).collect();
    if vals.is_empty() {
        return Ok(0);
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    let median = if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2
    };
    Ok(median)
}

/// Piso de reserva = colchão intocável que a folga de caixa não pode comer. Por ora = soma dos
/// Bolsos marcados como reserva (spec 007, `liquidity = 'reserve'`); esses NÃO entram na semente
/// líquida, então subtraí-los aqui não dobra. O ideal metodológico (custo de vida × 12) fica
/// para quando a reserva for modelada como meta — ver limitações na spec 010. Hoje, sem reserva
/// configurada, retorna 0 e a régua de poupança é a que morde.
async fn reserve_floor(pool: &SqlitePool) -> Result<i64, String> {
    let floor: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor: {e}"))?;
    Ok(floor.0)
}

/// Fim do horizonte da projeção = o último dia com dado pré-lançado (transação futura ou Saldo
/// importado) ≥ hoje. A planilha do método já lança o ano inteiro à frente, então varremos ATÉ
/// O FIM DOS DADOS, não só o mês corrente — senão o "pode gastar" fica cego ao buraco do futuro
/// e às faturas dos meses seguintes (decisão do dono). Piso: fim do mês corrente.
async fn forecast_horizon_end(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<NaiveDate, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let max_txn: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM \"transaction\" WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon txn: {e}"))?;
    let max_bal: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM sheet_daily_balance WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon bal: {e}"))?;

    let mut horizon = forecast::last_day_of_month(today_naive.year(), today_naive.month());
    for (candidate,) in [max_txn, max_bal] {
        if let Some(date_str) = candidate
            && let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            && d > horizon
        {
            horizon = d;
        }
    }
    Ok(horizon)
}

/// Loads forward cashflow events for the projection window: future transactions (date > today,
/// avoiding double-counting today's already-realized spending baked into the balance snapshot)
/// plus credit-cycle lumps aggregated from `daily_checkin` at the card due date (Régua 2).
/// Single source of row→event mapping, shared by `dashboard_summary` and `forecast_dto`.
async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let horizon = horizon_end.format("%Y-%m-%d").to_string();

    // Liquidez da conta-destino entra no SELECT para classificar `transfer` → Economia (guardar
    // num bolso não-líquido) vs net-zero (entre contas líquidas).
    let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?1 AND t.date <= ?2",
    )
    .bind(&today)
    .bind(&horizon)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    let mut all_events: Vec<CashflowEvent> = txn_rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let to_liq = (!liq.is_empty()).then_some(liq);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
            Some(CashflowEvent {
                date,
                kind,
                amount_cents: amount,
                realized: is_proj == 0,
            })
        })
        .collect();

    let credit_cards: Vec<(i32, i32)> =
        sqlx::query_as("SELECT closing_day, due_day FROM account WHERE type = 'credit_card'")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("query credit cards: {e}"))?;

    if !credit_cards.is_empty() {
        // Use the first card's closing/due days (multi-card aggregation is a later slice)
        let (closing_day, due_day) = credit_cards[0];
        let closing_day = closing_day as u32;
        let due_day = due_day as u32;

        let checkins: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT date, daily_spend, credit_spend FROM daily_checkin WHERE date > ?1 AND date <= ?2",
        )
        .bind(&today)
        .bind(&horizon)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query checkins: {e}"))?;

        let mut credit_by_due: std::collections::HashMap<NaiveDate, i64> =
            std::collections::HashMap::new();

        for (date_str, daily_spend, credit_spend) in checkins {
            if let Ok(checkin_date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                // Daily spend (Régua 1) → Daily event on its day
                if daily_spend > 0 {
                    all_events.push(CashflowEvent {
                        date: checkin_date,
                        kind: forecast::EventKind::Daily,
                        amount_cents: daily_spend,
                        realized: true,
                    });
                }

                // Credit spend (Régua 2) → aggregate by due_date
                if credit_spend > 0 {
                    let due_date = forecast::cycle_due_date(checkin_date, closing_day, due_day);
                    *credit_by_due.entry(due_date).or_insert(0) += credit_spend;
                }
            }
        }

        for (due_date, total_credit) in credit_by_due {
            all_events.push(CashflowEvent {
                date: due_date,
                kind: forecast::EventKind::FixedOut,
                amount_cents: total_credit,
                realized: false, // future projection
            });
        }
    }

    Ok(all_events)
}

/// Eventos JÁ REALIZADOS do mês corrente (`month_start..=today`), classificados como os futuros.
/// O encadeamento de caixa não os usa (a semente já os embute), mas a performance do mês precisa
/// deles — senão o mês corrente aparece pela metade (review adversarial P0). Só transações; os
/// lumps de fatura realizados deste mês já estão na coluna Saída da planilha como transação.
async fn load_realized_month_events(
    pool: &SqlitePool,
    month_start: NaiveDate,
    today_naive: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let start = month_start.format("%Y-%m-%d").to_string();
    let today = today_naive.format("%Y-%m-%d").to_string();

    let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date <= ?2",
    )
    .bind(&start)
    .bind(&today)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query realized month: {e}"))?;

    Ok(rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let to_liq = (!liq.is_empty()).then_some(liq);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
            Some(CashflowEvent {
                date,
                kind,
                amount_cents: amount,
                realized: is_proj == 0,
            })
        })
        .collect())
}

/// Eventos para as MÉTRICAS por mês = futuros (encadeamento) + realizados do mês corrente.
/// Cobre o mês inteiro de hoje (realizado + projetado); meses à frente já são todos futuros.
async fn load_metric_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    future_events: &[CashflowEvent],
) -> Result<Vec<CashflowEvent>, String> {
    let month_start =
        NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1).expect("valid month");
    let mut metric = load_realized_month_events(pool, month_start, today_naive).await?;
    metric.extend_from_slice(future_events);
    Ok(metric)
}

#[derive(serde::Serialize)]
pub struct ForecastDayDto {
    pub date: String,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub economia_cents: i64,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct DayPointDto {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthEndDto {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthMetricDto {
    pub year: i32,
    pub month: u32,
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Diário médio do mês = Σ diário realizado ÷ dias decorridos (D/N). Antes morria no DTO.
    pub real_daily_avg_cents: i64,
    /// Economia lançada no mês (numerador do Economizado%).
    pub economia_cents: i64,
    pub savings_rate_bps: i64,
}

/// Poupança do ano: realizada (honesta) vs projetada (otimista quando o futuro está incompleto).
#[derive(serde::Serialize)]
pub struct AnnualSavingsDto {
    pub realized_income_cents: i64,
    pub realized_savings_cents: i64,
    pub realized_rate_bps: i64,
    pub projected_income_cents: i64,
    pub projected_savings_cents: i64,
    pub projected_rate_bps: i64,
    pub target_bps: i64,
}

/// Cobertura de um mês futuro (quanto do gasto típico já está lançado).
#[derive(serde::Serialize)]
pub struct MonthCoverageDto {
    pub year: i32,
    pub month: u32,
    pub projected_outflow_cents: i64,
    pub baseline_outflow_cents: i64,
    pub coverage_bps: i64,
    pub is_complete: bool,
    pub estimated_missing_cents: i64,
}

#[derive(serde::Serialize)]
pub struct ForecastDto {
    pub today: String,
    pub horizon_end: String,
    /// Poupança do ano — realizada vs projetada (previsibilidade).
    pub annual_savings: AnnualSavingsDto,
    /// Cobertura por mês futuro (vazio se a projeção está completa).
    pub coverage: Vec<MonthCoverageDto>,
    /// Gasto típico/mês (mediana realizada). `0` = sem histórico → previsibilidade indeterminada.
    pub baseline_outflow_cents: i64,
    /// Último mês cuja projeção é confiável ("YYYY-MM"); `null` se não há baseline para avaliar.
    pub trusted_through_month: Option<String>,
    /// Soma do que falta lançar nos meses incompletos (fatura + variáveis).
    pub total_missing_cents: i64,
    /// "Pode gastar hoje" honesto: o MAIS APERTADO de caixa × poupança (guardrail duplo).
    pub safe_to_spend_today_cents: i64,
    /// Folga de caixa (menor saldo projetado no horizonte − piso de reserva).
    pub cash_headroom_cents: i64,
    /// Folga da meta de poupança do mês corrente (negativa = já abaixo da meta). `null` quando a
    /// régua de poupança está inativa (mês sem renda) → só o caixa decide.
    pub savings_headroom_cents: Option<i64>,
    /// Qual régua limita: "cash" ou "savings".
    pub binding_guardrail: String,
    /// Meta de poupança em basis points (2500 = 25%).
    pub savings_target_bps: i64,
    pub deepest_deficit: Option<DayPointDto>,
    pub daily: Vec<ForecastDayDto>,
    pub month_end: Vec<MonthEndDto>,
    /// Performance/poupança por mês (Caixa ≠ Performance; expõe meses futuros magros).
    pub months: Vec<MonthMetricDto>,
}

#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String> {
    forecast_dto(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation with an injected `today` (deterministic, integration-testable).
/// Maps the pure engine output to ISO-8601-string DTOs; the core stays serde-free.
async fn forecast_dto(pool: &SqlitePool, today_naive: NaiveDate) -> Result<ForecastDto, String> {
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let seed = projection_seed(pool, today_naive).await?;
    let mut events = load_cashflow_events(pool, today_naive, horizon_end).await?;
    // Previsão de diário como DRIVER: injeta o teto/dia nos dias futuros do mês corrente, para o
    // saldo projetado e a Performance não nascerem otimistas (assumem o gasto típico até o fim do mês).
    let daily_ceiling: (i64,) = sqlx::query_as(
        "SELECT COALESCE(amount, 0) FROM daily_budget WHERE status='active' \
         ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query daily_budget: {e}"))?
    .unwrap_or((0,));
    let days_with_daily: std::collections::HashSet<NaiveDate> = events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    events.extend(forecast::project_daily_ceiling(
        daily_ceiling.0,
        today_naive,
        horizon_end,
        &days_with_daily,
    ));
    let metric_events = load_metric_events(pool, today_naive, &events).await?;
    let fc =
        forecast::project_with_metrics(seed, today_naive, &events, &metric_events, horizon_end);

    let reserve_floor_cents = reserve_floor(pool).await?;
    // Poupança ANUAL realizada (não o mês isolado, não o ano projetado-incompleto).
    let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
    let sts = forecast::safe_to_spend_today(
        &fc,
        annual_income,
        annual_savings_amt,
        SAVINGS_TARGET_BPS,
        reserve_floor_cents,
    );
    let binding_guardrail = match sts.binding {
        forecast::Guardrail::Cash => "cash",
        forecast::Guardrail::Savings => "savings",
    }
    .to_string();

    // Previsibilidade: poupança realizada vs projetada + cobertura dos meses futuros.
    let (proj_income, proj_savings) = projected_annual_savings(pool, today_naive).await?;
    // Taxa em bps para EXIBIÇÃO (round half-up, não trunca — senão 25,00% vira 2499/abaixo da
    // meta; review P3). Nunca usada em decisão (o guardrail compara centavos diretos).
    let rate_bps = |save: i64, inc: i64| {
        if inc > 0 {
            (save * 10_000 + inc / 2) / inc
        } else {
            0
        }
    };
    let annual_savings = AnnualSavingsDto {
        realized_income_cents: annual_income,
        realized_savings_cents: annual_savings_amt,
        realized_rate_bps: rate_bps(annual_savings_amt, annual_income),
        projected_income_cents: proj_income,
        projected_savings_cents: proj_savings,
        projected_rate_bps: rate_bps(proj_savings, proj_income),
        target_bps: SAVINGS_TARGET_BPS,
    };

    let baseline = realized_monthly_baseline(pool, today_naive).await?;
    let coverage_raw =
        forecast::month_coverage(&fc.months, today_naive, baseline, COVERAGE_COMPLETE_BPS);
    // Sem baseline (nenhum mês realizado) não dá para afirmar "confiável até X" → `None`. Com
    // baseline, o mês corrente é sempre confiável (tem o realizado) e estende pelos meses futuros
    // completos até o primeiro incompleto.
    let trusted_through_month = if baseline <= 0 {
        None
    } else {
        let mut trusted = format!("{:04}-{:02}", today_naive.year(), today_naive.month());
        for c in coverage_raw.iter() {
            if c.is_complete {
                trusted = format!("{:04}-{:02}", c.year, c.month);
            } else {
                break;
            }
        }
        Some(trusted)
    };
    let total_missing_cents = coverage_raw
        .iter()
        .filter(|c| !c.is_complete)
        .map(|c| c.estimated_missing_cents)
        .sum();
    let coverage: Vec<MonthCoverageDto> = coverage_raw
        .iter()
        .map(|c| MonthCoverageDto {
            year: c.year,
            month: c.month,
            projected_outflow_cents: c.projected_outflow_cents,
            baseline_outflow_cents: c.baseline_outflow_cents,
            coverage_bps: c.coverage_bps,
            is_complete: c.is_complete,
            estimated_missing_cents: c.estimated_missing_cents,
        })
        .collect();

    // Per-day flow sums (income, fixed out, daily out), keyed by the same dates the engine emits.
    let mut flows: std::collections::HashMap<NaiveDate, (i64, i64, i64, i64)> =
        std::collections::HashMap::new();
    for e in &events {
        let entry = flows.entry(e.date).or_default();
        match e.kind {
            forecast::EventKind::Income => entry.0 += e.amount_cents,
            forecast::EventKind::FixedOut => entry.1 += e.amount_cents,
            forecast::EventKind::Daily => entry.2 += e.amount_cents,
            forecast::EventKind::Economia => entry.3 += e.amount_cents,
        }
    }

    let daily = fc
        .daily
        .iter()
        .map(|p| {
            let (income, fixed_out, daily_out, economia) =
                flows.get(&p.date).copied().unwrap_or_default();
            ForecastDayDto {
                date: p.date.format("%Y-%m-%d").to_string(),
                income_cents: income,
                fixed_out_cents: fixed_out,
                daily_out_cents: daily_out,
                economia_cents: economia,
                balance_cents: p.balance_cents,
            }
        })
        .collect();

    Ok(ForecastDto {
        today: today_naive.format("%Y-%m-%d").to_string(),
        horizon_end: horizon_end.format("%Y-%m-%d").to_string(),
        annual_savings,
        coverage,
        baseline_outflow_cents: baseline,
        trusted_through_month,
        total_missing_cents,
        safe_to_spend_today_cents: sts.amount_cents,
        cash_headroom_cents: sts.cash_headroom_cents,
        savings_headroom_cents: sts.savings_headroom_cents,
        binding_guardrail,
        savings_target_bps: SAVINGS_TARGET_BPS,
        deepest_deficit: fc.deepest_deficit.as_ref().map(|p| DayPointDto {
            date: p.date.format("%Y-%m-%d").to_string(),
            balance_cents: p.balance_cents,
        }),
        daily,
        month_end: fc
            .month_end
            .iter()
            .map(|m| MonthEndDto {
                year: m.year,
                month: m.month,
                balance_cents: m.balance_cents,
            })
            .collect(),
        months: fc
            .months
            .iter()
            .map(|m| MonthMetricDto {
                year: m.year,
                month: m.month,
                income_cents: m.income_cents,
                performance_cents: m.performance_cents,
                cost_of_living_cents: m.cost_of_living_cents,
                real_daily_avg_cents: m.real_daily_avg_cents,
                economia_cents: m.economia_cents,
                savings_rate_bps: m.savings_rate_bps,
            })
            .collect(),
    })
}

// --- Dashboard query commands ---

#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub credit_spend_month: i64,
    /// Há rastreio de crédito (cartão ou gasto de crédito). `false` → a UI mostra "—" no tile,
    /// não um R$0 estrutural.
    pub has_credit: bool,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
}

#[tauri::command]
pub async fn get_dashboard_summary(
    pool: State<'_, SqlitePool>,
) -> Result<DashboardSummary, String> {
    dashboard_summary(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation: takes `&SqlitePool` and an injected `today`, so it is deterministic and
/// integration-testable without Tauri `State` or the ambient clock.
async fn dashboard_summary(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<DashboardSummary, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    // Seed + forward events: shared with `forecast_dto` (single source of event mapping).
    let seed = projection_seed(pool, today_naive).await?;
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let all_events = load_cashflow_events(pool, today_naive, horizon_end).await?;

    // `balance` is the projected end-of-current-month figure (the method's hero, spec 003 US8),
    // not the raw current account sum.
    let fc = forecast::project(seed, today_naive, &all_events, horizon_end);
    let projected_balance = fc
        .month_end
        .iter()
        .find(|m| m.year == today_naive.year() && m.month == today_naive.month())
        .map(|m| m.balance_cents)
        .or_else(|| fc.daily.last().map(|p| p.balance_cents))
        .unwrap_or(seed);

    // Active daily budget
    let daily_budget: (i64,) = sqlx::query_as(
        "SELECT COALESCE(amount, 0) FROM daily_budget WHERE status='active' ORDER BY start_date DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query: {e}"))?
    .unwrap_or((0,));

    // Diário de HOJE: check-ins (app) + transações Diário realizadas (planilha/import). Antes lia
    // só `daily_checkin` (nunca populada) → R$0 estrutural enganoso mesmo havendo Diário no dia.
    let daily_spend: (i64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(daily_spend) FROM daily_checkin WHERE date = ?1), 0) \
              + COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                          WHERE type='expense' AND is_fixed=0 AND date = ?1 \
                            AND (payment_method IS NULL OR payment_method <> 'credit')), 0)",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query daily spend: {e}"))?;

    // Crédito no mês (Régua 2): check-ins + transações de crédito do mês.
    let month_start = format!("{}-01", today_naive.format("%Y-%m"));
    let credit_spend: (i64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(credit_spend) FROM daily_checkin WHERE date >= ?1), 0) \
              + COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                          WHERE type='expense' AND payment_method='credit' AND date >= ?1), 0)",
    )
    .bind(&month_start)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query credit spend: {e}"))?;

    // Há rastreio de crédito? (cartão configurado ou algum gasto de crédito). Sem isso a UI mostra
    // "—" no tile de crédito, em vez de um R$0 estrutural enganoso.
    let has_credit: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM account WHERE type='credit_card') \
                  OR COALESCE((SELECT SUM(credit_spend) FROM daily_checkin), 0) > 0 \
                THEN 1 ELSE 0 END",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query has_credit: {e}"))?;

    // Reserve
    let reserve: (f64, String) = sqlx::query_as(
        "SELECT COALESCE(current_months, 0), COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query: {e}"))?
    .unwrap_or((0.0, "flat".to_string()));

    // Transações já realizadas: por DATA (≤ hoje), não pelo `is_projection` congelado (stale
    // quando o dono não re-importa por dias — auditoria de robustez a edições).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date <= ?1")
        .bind(&today)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("query: {e}"))?;

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget: daily_budget.0,
        daily_spend_today: daily_spend.0,
        credit_spend_month: credit_spend.0,
        has_credit: has_credit.0 != 0,
        reserve_months: reserve.0,
        reserve_trend: reserve.1,
        transaction_count: count.0,
    })
}

// --- Pockets & liquidity (spec 007) ---

#[derive(serde::Serialize)]
pub struct PocketAccount {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub liquidity: Option<String>,
    pub balance: i64,
    pub institution: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Pockets {
    pub liquid_cents: i64,
    pub reserve_cents: i64,
    pub restricted_cents: i64,
    pub illiquid_cents: i64,
    /// liquid + reserve + illiquid; restricted (vale) is tracked apart and the
    /// credit-card liability belongs to the invoice slice.
    pub net_worth_cents: i64,
    pub accounts: Vec<PocketAccount>,
}

/// Deterministic liquidity class per account type (spec 007 contract).
fn liquidity_for_type(account_type: &str) -> Option<&'static str> {
    match account_type {
        "bank" | "wallet" | "business" => Some("liquid"),
        "savings" => Some("reserve"),
        "meal_voucher" => Some("restricted"),
        "pension" | "fgts" => Some("illiquid"),
        _ => None, // credit_card: liability, not a pocket
    }
}

/// Pure aggregation over the account list (functional core, unit-tested).
fn aggregate_pockets(accounts: Vec<PocketAccount>) -> Pockets {
    let sum = |class: &str| -> i64 {
        accounts
            .iter()
            .filter(|a| a.liquidity.as_deref() == Some(class))
            .map(|a| a.balance)
            .sum()
    };
    let (liquid, reserve, restricted, illiquid) = (
        sum("liquid"),
        sum("reserve"),
        sum("restricted"),
        sum("illiquid"),
    );
    Pockets {
        liquid_cents: liquid,
        reserve_cents: reserve,
        restricted_cents: restricted,
        illiquid_cents: illiquid,
        net_worth_cents: liquid + reserve + illiquid,
        accounts,
    }
}

#[tauri::command]
pub async fn get_pockets(pool: State<'_, SqlitePool>) -> Result<Pockets, String> {
    pockets(pool.inner()).await
}

async fn pockets(pool: &SqlitePool) -> Result<Pockets, String> {
    type PocketRow = (String, String, String, Option<String>, i64, Option<String>);
    let rows: Vec<PocketRow> = sqlx::query_as(
        "SELECT id, name, type, liquidity, balance, institution FROM account \
         WHERE type != 'credit_card' ORDER BY created_at, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    Ok(aggregate_pockets(
        rows.into_iter()
            .map(
                |(id, name, t, liquidity, balance, institution)| PocketAccount {
                    id,
                    name,
                    r#type: t,
                    liquidity,
                    balance,
                    institution,
                },
            )
            .collect(),
    ))
}

#[tauri::command]
pub async fn create_account(
    pool: State<'_, SqlitePool>,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    create_account_inner(pool.inner(), name, account_type, balance_cents, institution).await
}

async fn create_account_inner(
    pool: &SqlitePool,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    let liquidity = liquidity_for_type(&account_type)
        .ok_or_else(|| format!("tipo inválido: {account_type}"))?;

    // Pockets exist before any sheet import; ensure the default owner person.
    // Atomic insert-if-empty so concurrent calls cannot both bootstrap an "Eu".
    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .map_err(|e| format!("create person: {e}"))?;
    let (owner_id,): (String,) =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, institution, balance, liquidity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&id)
    .bind(name)
    .bind(&account_type)
    .bind(&owner_id)
    .bind(&institution)
    .bind(balance_cents)
    .bind(liquidity)
    .execute(pool)
    .await
    .map_err(|e| format!("create account: {e}"))?;

    Ok(id)
}

#[derive(serde::Serialize)]
pub struct TransactionRow {
    pub id: String,
    pub r#type: String,
    pub amount: i64,
    pub description: String,
    pub date: String,
    pub payment_method: String,
    pub is_projection: bool,
}

#[tauri::command]
pub async fn get_recent_transactions(
    pool: State<'_, SqlitePool>,
    limit: i64,
) -> Result<Vec<TransactionRow>, String> {
    let rows: Vec<(String, String, i64, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, type, amount, COALESCE(description,''), date, COALESCE(payment_method,''), is_projection FROM \"transaction\" ORDER BY date DESC LIMIT ?1"
    )
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("query: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|(id, t, amount, desc, date, pm, is_proj)| TransactionRow {
            id,
            r#type: t,
            amount,
            description: desc,
            date,
            payment_method: pm,
            is_projection: is_proj != 0,
        })
        .collect())
}

#[tauri::command]
pub async fn detect_sheet_layout(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<layout_detect::SheetLayout, String> {
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Grade inteira — A1:Z10 podia cortar a linha de dados/cabeçalhos da detecção (P1).
    let range = format!("'{}'", sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;
    let rows = values.values;

    let layout = layout_detect::detect_layout(&rows, &sheet_name)?;

    sqlx::query(
        "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
    )
    .bind(&layout.id).bind(&layout.sheet_name).bind(layout.year)
    .bind(layout.month_names_row).bind(layout.header_row).bind(layout.data_start_row)
    .bind(layout.day_column).bind(layout.block_size).bind(&layout.date_direction)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("save layout: {e}"))?;

    let mappings = layout_detect::generate_mappings(&layout);
    for m in &mappings {
        sqlx::query(
            "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
        )
        .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
        .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
        .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("save mapping: {e}"))?;
    }

    Ok(layout)
}

#[tauri::command]
pub async fn save_sheet_mapping(
    pool: State<'_, SqlitePool>,
    mapping_id: String,
    block_offset: i32,
    is_active: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE sheet_mapping SET block_offset = ?1, is_active = ?2 WHERE id = ?3")
        .bind(block_offset)
        .bind(is_active as i32)
        .bind(&mapping_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("update mapping: {e}"))?;
    Ok(())
}

type MappingRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    i32,
    i32,
);

#[tauri::command]
pub async fn get_sheet_mappings(
    pool: State<'_, SqlitePool>,
    sheet_name: String,
) -> Result<Vec<layout_detect::SheetMappingEntry>, String> {
    let rows: Vec<MappingRow> = sqlx::query_as(
            "SELECT id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active FROM sheet_mapping WHERE sheet_name = ?1 ORDER BY block_offset"
        )
        .bind(&sheet_name)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("query: {e}"))?;

    Ok(rows
        .into_iter()
        .map(
            |(id, sn, cl, ch, tt, tf, dd, lid, bo, ia)| layout_detect::SheetMappingEntry {
                id,
                sheet_name: sn,
                column_letter: cl,
                column_header: ch,
                target_table: tt,
                target_field: tf,
                date_direction: dd,
                layout_id: lid,
                block_offset: bo,
                is_active: ia,
            },
        )
        .collect())
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
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;

    let url = "https://www.googleapis.com/drive/v3/files?q=mimeType%3D'application%2Fvnd.google-apps.spreadsheet'&fields=files(id,name,modifiedTime)&orderBy=modifiedTime%20desc&pageSize=50";

    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .bearer_auth(&token.access_token)
        .send()
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

    #[test]
    fn app_info_exposes_version_and_db_path() {
        let info = app_info_for_dir(std::path::Path::new("/tmp/neko-test"));
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.db_path.ends_with("neko-finance.db"));
        assert!(info.db_path.starts_with("/tmp/neko-test"));
    }

    #[test]
    fn test_parse_number_via_import() {
        use google_sheets::import::compute_checksum;
        let rows = vec![google_sheets::import::ImportedRow {
            date: "2025-01-01".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
            kind: google_sheets::import::RowKind::Entrada,
        }];
        let checksum = compute_checksum(&rows);
        assert!(!checksum.is_empty());
    }

    // Spec 010 slice 0: floats do calamine chegam ao parse_number sem ambiguidade de
    // separador — regressão do bug de 100× (65.28 → R$ 6.528,00).
    #[test]
    fn xlsx_float_cells_parse_to_correct_cents() {
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(65.28)),
            "65.2800"
        );
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(6012.73)),
            "6012.7300"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Int(1370)), "1370");
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::String(" Entrada ".into())),
            "Entrada"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Empty), "");

        use google_sheets::import::parse_number;
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(65.28))),
            6528
        );
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(10805.5048))),
            1080550
        );
        // float que parece milhar (3 dígitos após o ponto) — o {:.4} blinda o caso.
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(123.456))),
            12346
        );
    }

    // T7.6 — get_dashboard_summary returns the PROJECTED balance, not the raw account sum.
    #[tokio::test]
    async fn dashboard_balance_is_projected_not_raw_sum() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(&pool)
            .await
            .unwrap();

        // Liquid bank account with R$1000.00 → raw SUM(balance) = 100000.
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind("bank")
        .bind(&pid)
        .bind(100_000i64)
        .execute(&pool)
        .await
        .unwrap();

        // A FUTURE projected fixed expense of R$300.00 later this same month.
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) VALUES (?1,'expense',?2,?3,'debit',1,1)"
        )
        .bind(&tid).bind(30_000i64).bind("2026-03-20")
        .execute(&pool).await.unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();

        // Raw sum is 100000; the projected end-of-March is 100000 − 30000 = 70000.
        assert_eq!(summary.balance, 70_000);
    }

    // Bug: tiles do dashboard mostravam R$0 estrutural porque liam só `daily_checkin` (vazia).
    // Agora o Diário de hoje vem das transações realizadas; e `has_credit=false` sem cartão.
    #[tokio::test]
    async fn dashboard_daily_spend_comes_from_transactions_and_credit_flag() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Diário realizado de hoje (expense, is_fixed=0) — sem nenhum check-in.
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await;
        // Despesa de outro dia não conta no "hoje".
        insert_realized(&pool, "expense", 9_999, "2026-06-12").await;

        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "Diário de hoje vem das transações, não R$0 estrutural"
        );
        assert!(!s.has_credit, "sem cartão nem crédito → has_credit false");
    }

    // --- 005 get_forecast (TDD) ---

    async fn fixture_pool() -> sqlx::SqlitePool {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_liquid_account(pool: &sqlx::SqlitePool, balance: i64) {
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(pool)
            .await
            .unwrap();
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,'bank',?3,?4)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind(&pid)
        .bind(balance)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_projection(
        pool: &sqlx::SqlitePool,
        ttype: &str,
        amount: i64,
        date: &str,
        payment_method: &str,
        is_fixed: i64,
    ) {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) VALUES (?1,?2,?3,?4,?5,?6,1)"
        )
        .bind(&tid).bind(ttype).bind(amount).bind(date)
        .bind((!payment_method.is_empty()).then_some(payment_method))
        .bind(is_fixed)
        .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn forecast_dto_chains_daily_flows_and_safe_to_spend() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // R$ 1.000,00
        // Future fixed expense R$ 300,00 on the 20th; future income R$ 200,00 on the 25th.
        insert_projection(&pool, "expense", 30_000, "2026-03-20", "debit", 1).await;
        insert_projection(&pool, "income", 20_000, "2026-03-25", "", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.today, "2026-03-10");
        assert_eq!(fc.horizon_end, "2026-03-31");
        // One row per day, today through month-end inclusive.
        assert_eq!(fc.daily.len(), 22);
        assert_eq!(fc.daily[0].date, "2026-03-10");
        assert_eq!(fc.daily[0].balance_cents, 100_000);

        let d20 = fc.daily.iter().find(|d| d.date == "2026-03-20").unwrap();
        assert_eq!(d20.fixed_out_cents, 30_000);
        assert_eq!(d20.income_cents, 0);
        assert_eq!(d20.balance_cents, 70_000);

        let d25 = fc.daily.iter().find(|d| d.date == "2026-03-25").unwrap();
        assert_eq!(d25.income_cents, 20_000);
        assert_eq!(d25.balance_cents, 90_000);

        // Guardrail duplo: tudo aqui é projetado (nenhum realizado no ano) → a régua de poupança
        // ANUAL está inativa, manda o CAIXA: pode gastar = o vale de R$ 700,00. A régua de
        // poupança anual é exercitada no teste `forecast_dual_guardrail_savings_binds_for_joao`.
        assert_eq!(fc.cash_headroom_cents, 70_000);
        assert_eq!(fc.binding_guardrail, "cash");
        assert_eq!(fc.savings_headroom_cents, None);
        assert_eq!(fc.safe_to_spend_today_cents, 70_000);
        // Positive horizon: deepest point reported, but not negative.
        let trough = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(trough.balance_cents, 70_000);

        // Current month-end matches the dashboard hero.
        assert_eq!(fc.month_end.len(), 1);
        assert_eq!(fc.month_end[0].year, 2026);
        assert_eq!(fc.month_end[0].month, 3);
        assert_eq!(fc.month_end[0].balance_cents, 90_000);
    }

    #[tokio::test]
    async fn forecast_dto_reports_negative_deficit_and_zero_safe_to_spend() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 10_000).await; // R$ 100,00
        insert_projection(&pool, "expense", 50_000, "2026-03-15", "debit", 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        let deficit = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(deficit.balance_cents, -40_000);
        assert_eq!(deficit.date, "2026-03-15");
        assert_eq!(fc.safe_to_spend_today_cents, 0);
    }

    async fn insert_sheet_balance(pool: &sqlx::SqlitePool, sheet: &str, date: &str, cents: i64) {
        sqlx::query(
            "INSERT INTO sheet_daily_balance (sheet_name, date, balance_cents) VALUES (?1,?2,?3)",
        )
        .bind(sheet)
        .bind(date)
        .bind(cents)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_realized(pool: &sqlx::SqlitePool, ttype: &str, amount: i64, date: &str) {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES (?1,?2,?3,?4,0)",
        )
        .bind(&tid).bind(ttype).bind(amount).bind(date)
        .execute(pool).await.unwrap();
    }

    // Previsibilidade: meses futuros esparsos (só fixas) são detectados como incompletos, e a
    // poupança realizada (honesta) difere da projetada (otimista). O caso do João.
    #[tokio::test]
    async fn forecast_flags_incomplete_future_and_honest_annual_savings() {
        let pool = fixture_pool().await;
        // Meses realizados (jan–mai) com gasto típico R$ 1.000/mês e dissaving (renda 900 < 1000).
        for m in [1, 2, 3, 4, 5] {
            insert_realized(&pool, "income", 90_000, &format!("2026-{m:02}-05")).await;
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        // Junho corrente: realizado parcial.
        insert_realized(&pool, "income", 90_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 100_000, "2026-06-10").await;
        // Julho COMPLETO (salário + gasto típico) e agosto ESPARSO (salário + só fixa) — como na
        // planilha real: o futuro tem salário mas falta a fatura/variável → projeção otimista.
        insert_projection(&pool, "income", 90_000, "2026-07-05", "", 0).await;
        insert_projection(&pool, "expense", 100_000, "2026-07-10", "debit", 0).await;
        insert_projection(&pool, "income", 90_000, "2026-08-05", "", 0).await;
        insert_projection(&pool, "expense", 30_000, "2026-08-10", "debit", 1).await;
        insert_sheet_balance(&pool, "2026", "2026-12-31", 50_000).await; // estende horizonte

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Cobertura: julho completo (~100% do típico), agosto esparso (~30%).
        let jul = fc.coverage.iter().find(|c| c.month == 7).unwrap();
        let ago = fc.coverage.iter().find(|c| c.month == 8).unwrap();
        assert!(jul.is_complete);
        assert!(!ago.is_complete);
        assert!(ago.estimated_missing_cents > 0);
        assert_eq!(fc.trusted_through_month.as_deref(), Some("2026-07")); // confiável até julho
        assert!(fc.total_missing_cents > 0);

        // Poupança realizada (honesta) negativa; a projetada parece melhor (futuro esparso).
        assert!(fc.annual_savings.realized_rate_bps < fc.annual_savings.projected_rate_bps);
        assert_eq!(fc.annual_savings.target_bps, 2500);
    }

    // Staleness: um mês COMPLETO conta na poupança anual mesmo se as transações ainda têm
    // is_projection=1 congelado (dono importou quando era futuro e não re-importou). A janela
    // de DATA é a fonte de verdade, não o flag (auditoria: edições em dias passados).
    #[tokio::test]
    async fn realized_annual_ignores_stale_is_projection_flag() {
        let pool = fixture_pool().await;
        // Maio (mês completo) lançado como PROJEÇÃO (stale) — hoje é junho.
        insert_projection(&pool, "income", 500_000, "2026-05-05", "", 0).await;
        insert_projection(&pool, "expense", 480_000, "2026-05-10", "debit", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let (income, savings) = realized_annual_savings(&pool, today).await.unwrap();
        assert_eq!(income, 500_000); // maio conta apesar do is_projection=1
        assert_eq!(savings, 20_000); // 500.000 − 480.000
    }

    // Semente: o Saldo da planilha vence os Bolsos quando há série importada.
    #[tokio::test]
    async fn projection_seed_prefers_sheet_saldo_over_pockets() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // bolso de R$ 1.000,00
        insert_sheet_balance(&pool, "2026", "2026-06-12", 801_889).await; // Saldo de hoje

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // Saldo de hoje na planilha vence o bolso; sem lacuna (hoje está na série).
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 801_889);
    }

    // Sem planilha importada, a semente continua sendo os Bolsos líquidos (spec 007).
    #[tokio::test]
    async fn projection_seed_falls_back_to_pockets_without_sheet() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 250_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 250_000);
    }

    // Planilha sem o dia de hoje preenchido: semente = último Saldo ≤ hoje + lançamentos
    // realizados no intervalo (não pode perder o que aconteceu entre o último saldo e hoje).
    #[tokio::test]
    async fn projection_seed_folds_realized_gap_up_to_today() {
        let pool = fixture_pool().await;
        insert_sheet_balance(&pool, "2026", "2026-06-10", 500_000).await;
        insert_realized(&pool, "income", 20_000, "2026-06-11").await;
        insert_realized(&pool, "expense", 5_000, "2026-06-12").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // 500.000 + 20.000 − 5.000 = 515.000
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 515_000);
    }

    // Regressão do bug do 1º dogfooding: import OK (226 txns) mas saldo de hoje aparecia
    // ZERADO e surgia um déficit ("buraco") falso, porque a semente era 0 (nenhum bolso) e o
    // Saldo da planilha era descartado. Com a série de Saldo, o saldo de hoje bate com o Sheets.
    #[tokio::test]
    async fn forecast_seeds_from_sheet_saldo_no_false_deficit() {
        let pool = fixture_pool().await;
        // Saldo de hoje na planilha = R$ 8.018,89; uma saída futura pequena no dia 26.
        insert_sheet_balance(&pool, "2026", "2026-06-12", 801_889).await;
        insert_projection(&pool, "expense", 4_633, "2026-06-26", "debit", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Antes: daily[0] = 0 (saldo zerado). Agora bate com o Saldo de hoje da planilha.
        assert_eq!(fc.daily[0].date, "2026-06-12");
        assert_eq!(fc.daily[0].balance_cents, 801_889);
        // Antes: deepest_deficit = −R$ 46,33 (o "buraco que não existe"). Agora positivo.
        let trough = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(trough.balance_cents, 801_889 - 4_633);
        assert!(trough.balance_cents > 0);
        assert!(fc.safe_to_spend_today_cents > 0);
    }

    // Guardrail duplo end-to-end com os números do João: Saldo de hoje R$ 8.018,89 (caixa
    // cheio e crescendo), mas junho com performance negativa → "pode gastar" honesto = 0,
    // limitado pela poupança. O horizonte varre até o fim dos dados pré-lançados (dez).
    // Crava o P0 do review: a performance de junho inclui o REALIZADO antes de hoje (não só
    // os eventos futuros), senão o mês aparece com sinal trocado e o guardrail decide errado.
    #[tokio::test]
    async fn forecast_dual_guardrail_savings_binds_for_joao() {
        let pool = fixture_pool().await;
        insert_sheet_balance(&pool, "2026", "2026-06-13", 801_889).await; // Saldo de hoje
        insert_sheet_balance(&pool, "2026", "2026-12-31", 2_754_616).await; // estende horizonte
        // Meses COMPLETOS (jan–mai) abaixo da meta → a poupança ANUAL morde. Sem isto, o mês
        // corrente (junho, em andamento) NÃO conta — evita o falso pânico de meio de mês.
        for m in [1, 2, 3, 4, 5] {
            insert_realized(&pool, "income", 200_000, &format!("2026-{m:02}-05")).await;
            insert_realized(&pool, "expense", 220_000, &format!("2026-{m:02}-10")).await;
        }
        // Junho (corrente) — metade realizada antes de hoje, metade projetada: testa que a
        // PERFORMANCE do mês inclui o realizado (o P0), mesmo o mês não contando na poupança anual.
        insert_realized(&pool, "income", 400_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 700_000, "2026-06-10").await;
        insert_realized(&pool, "income", 583_712, "2026-06-29").await;
        insert_projection(&pool, "expense", 370_169, "2026-06-30", "debit", 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.horizon_end, "2026-12-31");
        // Performance de junho = MÊS INTEIRO (realizado + projetado): inclui os R$ 700k já
        // realizados em 10/jun, que o cálculo antigo (só futuros) ignorava (o P0).
        let jun = fc.months.iter().find(|m| m.month == 6).unwrap();
        assert_eq!(jun.performance_cents, 983_712 - 1_070_169); // −86.457
        // Poupança ANUAL (meses completos jan–mai, abaixo da meta) manda → pode gastar 0.
        assert_eq!(fc.binding_guardrail, "savings");
        assert_eq!(fc.safe_to_spend_today_cents, 0);
        assert!(fc.savings_headroom_cents.unwrap() < 0);
        assert!(fc.cash_headroom_cents > 0); // mas há caixa (é a reserva)
        assert_eq!(fc.savings_target_bps, 2500);
    }

    #[tokio::test]
    async fn forecast_dto_empty_db_is_flat_zero() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.daily.len(), 22);
        assert!(fc.daily.iter().all(|d| d.balance_cents == 0));
        assert!(
            fc.daily
                .iter()
                .all(|d| d.income_cents == 0 && d.fixed_out_cents == 0 && d.daily_out_cents == 0)
        );
        assert_eq!(fc.safe_to_spend_today_cents, 0);
    }

    // --- 007 pockets & liquidity (TDD) ---

    #[test]
    fn liquidity_is_derived_deterministically_per_type() {
        assert_eq!(liquidity_for_type("bank"), Some("liquid"));
        assert_eq!(liquidity_for_type("wallet"), Some("liquid"));
        assert_eq!(liquidity_for_type("business"), Some("liquid"));
        assert_eq!(liquidity_for_type("savings"), Some("reserve"));
        assert_eq!(liquidity_for_type("meal_voucher"), Some("restricted"));
        assert_eq!(liquidity_for_type("pension"), Some("illiquid"));
        assert_eq!(liquidity_for_type("fgts"), Some("illiquid"));
        assert_eq!(liquidity_for_type("credit_card"), None);
        assert_eq!(liquidity_for_type("nope"), None);
    }

    #[tokio::test]
    async fn savings_no_longer_inflates_the_projection_seed() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // bank → liquid
        // Savings R$ 5.000,00 — reserve money, must stay out of the cash seed.
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) \
             SELECT ?1, 'Poupança', 'savings', owner_person_id, 500000 FROM account LIMIT 1",
        )
        .bind(&aid)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(liquid_seed(&pool).await.unwrap(), 100_000);
    }

    #[tokio::test]
    async fn create_account_derives_liquidity_and_default_person() {
        let pool = fixture_pool().await;
        // No person yet — command must bootstrap "Eu".
        let id = create_account_inner(&pool, "Vale".into(), "meal_voucher".into(), 42_000, None)
            .await
            .unwrap();
        let (liq, owner): (String, String) =
            sqlx::query_as("SELECT liquidity, owner_person_id FROM account WHERE id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(liq, "restricted");
        let (owner_name,): (String,) = sqlx::query_as("SELECT name FROM person WHERE id = ?1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(owner_name, "Eu");

        // Invalid type and empty name are rejected at the boundary.
        assert!(
            create_account_inner(&pool, "X".into(), "credit_card".into(), 0, None)
                .await
                .is_err()
        );
        assert!(
            create_account_inner(&pool, "  ".into(), "bank".into(), 0, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn pockets_groups_and_net_worth_follow_the_contract() {
        let pool = fixture_pool().await;
        for (name, t, balance) in [
            ("Conta", "bank", 100_000i64),
            ("Poupança", "savings", 500_000),
            ("Vale", "meal_voucher", 42_000),
            ("Previdência", "pension", 900_000),
            ("FGTS", "fgts", 300_000),
        ] {
            create_account_inner(&pool, name.into(), t.into(), balance, None)
                .await
                .unwrap();
        }
        let p = pockets(&pool).await.unwrap();
        assert_eq!(p.liquid_cents, 100_000);
        assert_eq!(p.reserve_cents, 500_000);
        assert_eq!(p.restricted_cents, 42_000);
        assert_eq!(p.illiquid_cents, 1_200_000);
        // Net worth excludes the restricted vale ledger.
        assert_eq!(p.net_worth_cents, 1_800_000);
        assert_eq!(p.accounts.len(), 5);
    }

    #[tokio::test]
    async fn migration_trigger_backfills_liquidity_on_plain_inserts() {
        let pool = fixture_pool().await;
        // Mirrors legacy import paths that insert without the liquidity column.
        insert_liquid_account(&pool, 1).await;
        let (liq,): (Option<String>,) =
            sqlx::query_as("SELECT liquidity FROM account WHERE type = 'bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(liq.as_deref(), Some("liquid"));
    }

    // T7.2 — credit cycle aggregation: credit_spend from daily_checkin lands as a lump at due_day.
    #[tokio::test]
    async fn dashboard_credit_lump_at_due_day() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(&pool)
            .await
            .unwrap();

        // Liquid bank account with R$2000.00
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind("bank")
        .bind(&pid)
        .bind(200_000i64)
        .execute(&pool)
        .await
        .unwrap();

        // Credit card: closes on day 20, due on day 10
        let card_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, closing_day, due_day) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )
        .bind(&card_id)
        .bind("Cartão")
        .bind("credit_card")
        .bind(&pid)
        .bind(0i64)
        .bind(20i32)
        .bind(10i32)
        .execute(&pool)
        .await
        .unwrap();

        // Checkin on March 15 with credit_spend = R$500.00
        // Cycle closes March 20 → due April 10
        let checkin_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&checkin_id)
        .bind(&pid)
        .bind("2026-03-15")
        .bind(0i64)
        .bind(50_000i64)
        .execute(&pool)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();

        // Seed = 200000. Credit lump of 50000 lands on April 10 (outside March horizon).
        // Projected end-of-March = 200000 (no events in March after today).
        assert_eq!(summary.balance, 200_000);
    }
}
