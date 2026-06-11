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
) -> Result<String, String> {
    let config = oauth::pkce::OAuthConfig::google(client_id);
    let port = find_available_port()?;
    let oauth_state = oauth::pkce::OAuthState::new(port);

    let app_dir_path = app_dir.0.clone();
    let config_for_bg = oauth::pkce::OAuthConfig::google(config.client_id.clone());

    // Store state and spawn flow
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        let flow_state = oauth_state.clone();
        *guard = Some(oauth_state);

        tokio::spawn(async move {
            match oauth::run_oauth_flow(config_for_bg, flow_state, app_dir_path) {
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
    match crate::oauth::token_store::load_token(&app_dir.0) {
        Ok(Some(token)) => {
            if crate::oauth::token_store::is_token_expired(&token) {
                Ok("expired".to_string())
            } else {
                Ok("connected".to_string())
            }
        }
        Ok(None) => Ok("disconnected".to_string()),
        Err(e) => Err(e),
    }
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
) -> Result<Vec<SheetInfo>, String> {
    let token = oauth::token_store::ensure_valid_token(&app_dir.0, &client_id).await?;
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
) -> Result<SheetPreview, String> {
    let token = oauth::token_store::ensure_valid_token(&app_dir.0, &client_id).await?;
    let client = SheetsClient::new(token);

    let range = format!("'{}'!A1:Z21", sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;

    let mut rows = values.values;
    let total_rows = rows.len();

    let headers = if rows.is_empty() {
        vec![]
    } else {
        rows.remove(0)
    };

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
) -> Result<usize, String> {
    let token = oauth::token_store::ensure_valid_token(&app_dir.0, &client_id).await?;
    let client = SheetsClient::new(token);

    let range = format!("'{}'!A:Z", sheet_name);
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
    let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings);
    let count = import::import_rows(&pool, &sheet_name, &imported_rows, &profile_id).await?;
    Ok(count)
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
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.to_string().trim().to_string())
                        .collect()
                })
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
            let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings);
            if !imported_rows.is_empty() {
                let count =
                    import::import_rows(&pool, sheet_name, &imported_rows, &profile_id).await?;
                total += count;
                sheets_imported.push(format!("{sheet_name} ({count} rows)"));
            }
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
async fn liquid_seed(pool: &SqlitePool) -> Result<i64, String> {
    let seed: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(balance), 0) FROM account WHERE type IN ('bank','wallet','savings')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;
    Ok(seed.0)
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

    let txn_rows: Vec<(String, i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT type, amount, date, COALESCE(payment_method,''), is_fixed, is_projection \
         FROM \"transaction\" WHERE date > ?1 AND date <= ?2",
    )
    .bind(&today)
    .bind(&horizon)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    let mut all_events: Vec<CashflowEvent> = txn_rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref())?;
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

#[derive(serde::Serialize)]
pub struct ForecastDayDto {
    pub date: String,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
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
pub struct ForecastDto {
    pub today: String,
    pub horizon_end: String,
    pub safe_to_spend_today_cents: i64,
    pub deepest_deficit: Option<DayPointDto>,
    pub daily: Vec<ForecastDayDto>,
    pub month_end: Vec<MonthEndDto>,
}

#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String> {
    forecast_dto(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation with an injected `today` (deterministic, integration-testable).
/// Maps the pure engine output to ISO-8601-string DTOs; the core stays serde-free.
async fn forecast_dto(pool: &SqlitePool, today_naive: NaiveDate) -> Result<ForecastDto, String> {
    let horizon_end = forecast::last_day_of_month(today_naive.year(), today_naive.month());
    let seed = liquid_seed(pool).await?;
    let events = load_cashflow_events(pool, today_naive, horizon_end).await?;
    let fc = forecast::project(seed, today_naive, &events, horizon_end);

    // Per-day flow sums (income, fixed out, daily out), keyed by the same dates the engine emits.
    let mut flows: std::collections::HashMap<NaiveDate, (i64, i64, i64)> =
        std::collections::HashMap::new();
    for e in &events {
        let entry = flows.entry(e.date).or_default();
        match e.kind {
            forecast::EventKind::Income => entry.0 += e.amount_cents,
            forecast::EventKind::FixedOut => entry.1 += e.amount_cents,
            forecast::EventKind::Daily => entry.2 += e.amount_cents,
        }
    }

    let daily = fc
        .daily
        .iter()
        .map(|p| {
            let (income, fixed_out, daily_out) = flows.get(&p.date).copied().unwrap_or_default();
            ForecastDayDto {
                date: p.date.format("%Y-%m-%d").to_string(),
                income_cents: income,
                fixed_out_cents: fixed_out,
                daily_out_cents: daily_out,
                balance_cents: p.balance_cents,
            }
        })
        .collect();

    Ok(ForecastDto {
        today: today_naive.format("%Y-%m-%d").to_string(),
        horizon_end: horizon_end.format("%Y-%m-%d").to_string(),
        safe_to_spend_today_cents: fc.safe_to_spend_today_cents,
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
    })
}

// --- Dashboard query commands ---

#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub credit_spend_month: i64,
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
    let seed = liquid_seed(pool).await?;
    let horizon_end = forecast::last_day_of_month(today_naive.year(), today_naive.month());
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

    // Today's daily spend
    let daily_spend: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(daily_spend), 0) FROM daily_checkin WHERE date = ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;

    // Credit spend this month
    let month_start = format!("{}-01", today_naive.format("%Y-%m"));
    let credit_spend: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(credit_spend), 0) FROM daily_checkin WHERE date >= ?1")
            .bind(&month_start)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;

    // Reserve
    let reserve: (f64, String) = sqlx::query_as(
        "SELECT COALESCE(current_months, 0), COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query: {e}"))?
    .unwrap_or((0.0, "flat".to_string()));

    // Transaction count
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE is_projection = 0")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget: daily_budget.0,
        daily_spend_today: daily_spend.0,
        credit_spend_month: credit_spend.0,
        reserve_months: reserve.0,
        reserve_trend: reserve.1,
        transaction_count: count.0,
    })
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
) -> Result<layout_detect::SheetLayout, String> {
    let token = oauth::token_store::ensure_valid_token(&app_dir.0, &client_id).await?;
    let client = SheetsClient::new(token);

    let range = format!("'{}'!A1:Z10", sheet_name);
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
) -> Result<Vec<UserSpreadsheet>, String> {
    let token = oauth::token_store::ensure_valid_token(&app_dir.0, &client_id).await?;

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
        }];
        let checksum = compute_checksum(&rows);
        assert!(!checksum.is_empty());
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

        // Trough is R$ 700,00 (between the expense and the income) → safe to spend today.
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
