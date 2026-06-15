#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod conflicts;
mod forecast;
mod google_sheets;
mod oauth;
mod recurrence;
mod splits;
mod tags;

use oauth::AppDataDir;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::start_oauth_flow,
            commands::check_auth_status,
            commands::disconnect_google,
            commands::list_sheet_names,
            commands::fetch_sheet_preview,
            commands::import_sheet_data,
            commands::import_economia_sheet,
            commands::import_local_xlsx,
            commands::get_dashboard_summary,
            commands::get_forecast,
            commands::get_annual_metrics,
            commands::get_recent_transactions,
            commands::create_transaction,
            commands::get_pockets,
            commands::create_account,
            commands::detect_sheet_layout,
            commands::preview_write_back,
            commands::write_back_enabled,
            commands::apply_write_back,
            commands::preview_economia_write_back,
            commands::apply_economia_write_back,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::save_sheet_mapping,
            commands::get_sheet_mappings,
            commands::list_user_spreadsheets,
            tags::create_tag_cmd,
            tags::list_tags_cmd,
            tags::set_transaction_tags_cmd,
            tags::tag_totals_for_month_cmd,
            recurrence::create_recurring_series_cmd,
            recurrence::delete_series_from_cmd,
            recurrence::delete_series_all_cmd,
            recurrence::update_series_from_cmd,
            recurrence::update_series_all_cmd,
            splits::splits_for_transaction_cmd,
            splits::owner_totals_for_month_cmd,
            conflicts::get_import_conflicts,
            conflicts::resolve_import_conflict,
        ])
        .setup(|app| {
            use tauri::Manager;
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir should exist");
            std::fs::create_dir_all(&app_dir)?;
            app.manage(AppDataDir(app_dir.clone()));

            let db_path = app_dir.join("neko-finance.db");
            let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

            let pool = tauri::async_runtime::block_on(async {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect(&db_url)
                    .await
                    .expect("failed to create database pool");

                sqlx::migrate!("./migrations")
                    .run(&pool)
                    .await
                    .expect("database migrations failed");

                pool
            });

            app.manage(pool);
            Ok(())
        })
        .manage(oauth::OAuthStateStore(Mutex::new(None)))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn test_migration_person_table() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let row: (String,) =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='person'")
                .fetch_one(&pool)
                .await
                .expect("person table not found");

        assert_eq!(row.0, "person");
    }

    #[tokio::test]
    async fn test_create_and_query_person() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&id)
            .bind("Alice Silva")
            .execute(&pool)
            .await
            .expect("insert failed");

        let (name,): (String,) = sqlx::query_as("SELECT name FROM person WHERE id = ?1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .expect("query failed");

        assert_eq!(name, "Alice Silva");
    }

    #[tokio::test]
    async fn test_full_schema_lifecycle() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let tables = vec![
            "person",
            "profile",
            "account",
            "category",
            "transaction",
            "split",
            "daily_budget",
            "daily_checkin",
            "reserve",
            "reserve_snapshot",
            "sheet_mapping",
            "sheet_layout",
            "sync_log",
            "transaction_fts",
            "app_setting",
            "import_conflict",
        ];
        for table_name in &tables {
            let (name,): (String,) =
                sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")
                    .bind(table_name)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or_else(|_| panic!("table {} not found", table_name));
            assert_eq!(name, *table_name);
        }

        let person_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&person_id)
            .bind("Alice Silva")
            .execute(&pool)
            .await
            .unwrap();

        let profile_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
            .bind(&profile_id)
            .bind(&person_id)
            .execute(&pool)
            .await
            .unwrap();

        let bank_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, institution, balance) VALUES (?1,?2,?3,?4,?5,?6)"
        )
        .bind(&bank_id).bind("Conta Corrente").bind("bank")
        .bind(&person_id).bind("Banco Exemplo").bind(500000)
        .execute(&pool).await.unwrap();

        let card_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, institution, credit_limit, closing_day, due_day) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"
        )
        .bind(&card_id).bind("Cartão de crédito").bind("credit_card")
        .bind(&person_id).bind("Banco Exemplo").bind(1000000).bind(5).bind(15)
        .execute(&pool).await.unwrap();

        // Spec 015 (WRONG #4): a árvore granular de categorias foi rebaixada para tags; só as
        // macro-naturezas (fixo/variável) + "Sem categoria" permanecem.
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM category")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count.0, 3,
            "só as macro-naturezas permanecem, got {}",
            count.0
        );
        let granular: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM category WHERE id = 'cat_var_alimentacao'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            granular.0, 0,
            "categoria granular removida (rebaixada para tag)"
        );

        let txn_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, payment_method, is_fixed, from_account_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"
        )
        .bind(&txn_id).bind("expense").bind(4300)
        .bind("Cafe + mercado").bind("2025-03-15").bind("debit").bind(1)
        .bind(&bank_id)
        .execute(&pool).await.unwrap();

        let split_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO split (id, transaction_id, amount, category_id, owner_person_id) VALUES (?1,?2,?3,?4,?5)"
        )
        .bind(&split_id).bind(&txn_id).bind(4300)
        .bind("cat_variable").bind(&person_id)
        .execute(&pool).await.unwrap();

        let checkin_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,?5)"
        )
        .bind(&checkin_id).bind(&person_id).bind("2025-03-15")
        .bind(4300).bind(0)
        .execute(&pool).await.unwrap();

        let budget_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status, free_income) VALUES (?1,?2,?3,?4,?5,?6)"
        )
        .bind(&budget_id).bind(&person_id).bind(4300)
        .bind("2025-03-01").bind("active").bind(129000)
        .execute(&pool).await.unwrap();

        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO reserve (id, person_id, target_months, current_months, trend) VALUES (?1,?2,?3,?4,?5)"
        )
        .bind(&reserve_id).bind(&person_id).bind(6).bind(4.5).bind("down")
        .execute(&pool).await.unwrap();

        let snap_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO reserve_snapshot (id, reserve_id, snapshot_date, current_months, total_reserve_amount) VALUES (?1,?2,?3,?4,?5)"
        )
        .bind(&snap_id).bind(&reserve_id).bind("2025-03-01").bind(4.5).bind(3000000)
        .execute(&pool).await.unwrap();

        let map_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7)"
        )
        .bind(&map_id).bind("2025").bind("C").bind("Entrada")
        .bind("transaction").bind("amount").bind("both")
        .execute(&pool).await.unwrap();

        let proj_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection) VALUES (?1,?2,?3,?4,?5,?6)"
        )
        .bind(&proj_id).bind("income").bind(350000)
        .bind("Salario projetado").bind("2025-06-25").bind(1)
        .execute(&pool).await.unwrap();

        let (proj_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE is_projection = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(proj_count, 1);

        sqlx::query("INSERT INTO transaction_fts (rowid, description) VALUES (?1, ?2)")
            .bind(1i64)
            .bind("Cafe da manha na padaria")
            .execute(&pool)
            .await
            .unwrap();

        let (fts_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transaction_fts WHERE description MATCH 'cafe'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(fts_count, 1);
    }

    #[tokio::test]
    async fn test_sheet_layout_crud() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let layout_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
        )
        .bind(&layout_id).bind("2025").bind(2025)
        .bind(0).bind(1).bind(2).bind(0).bind(6).bind("both")
        .execute(&pool).await.unwrap();

        let (name, year, block_size): (String, i64, i64) =
            sqlx::query_as("SELECT sheet_name, year, block_size FROM sheet_layout WHERE id = ?1")
                .bind(&layout_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "2025");
        assert_eq!(year, 2025);
        assert_eq!(block_size, 6);
    }

    #[tokio::test]
    async fn test_sheet_mapping_with_layout() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let layout_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sheet_layout (id, sheet_name, year, block_size, date_direction) VALUES (?1,?2,?3,?4,?5)"
        )
        .bind(&layout_id).bind("2025").bind(2025).bind(6).bind("both")
        .execute(&pool).await.unwrap();

        let map_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
        )
        .bind(&map_id).bind("2025").bind("C").bind("Entrada")
        .bind("transaction").bind("amount").bind("both")
        .bind(&layout_id).bind(1).bind(1)
        .execute(&pool).await.unwrap();

        let (header, offset, active): (String, i64, i64) = sqlx::query_as(
            "SELECT column_header, block_offset, is_active FROM sheet_mapping WHERE id = ?1",
        )
        .bind(&map_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(header, "Entrada");
        assert_eq!(offset, 1);
        assert_eq!(active, 1);
    }

    #[tokio::test]
    async fn test_sync_log_checksum() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

        let person_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&person_id)
            .bind("Test")
            .execute(&pool)
            .await
            .unwrap();

        let profile_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
            .bind(&profile_id)
            .bind(&person_id)
            .execute(&pool)
            .await
            .unwrap();

        let log_id = uuid::Uuid::new_v4().to_string();
        let checksum = "abc123def456";
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, source_sheet, checksum) VALUES (?1,?2,?3,?4,?5,?6,?7)"
        )
        .bind(&log_id).bind("import").bind("transaction").bind(&log_id)
        .bind(&profile_id).bind("2025").bind(checksum)
        .execute(&pool).await.unwrap();

        let (sheet, hash): (String, String) =
            sqlx::query_as("SELECT source_sheet, checksum FROM sync_log WHERE id = ?1")
                .bind(&log_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(sheet, "2025");
        assert_eq!(hash, checksum);

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1 AND checksum = ?2",
        )
        .bind("2025")
        .bind(checksum)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }
}
