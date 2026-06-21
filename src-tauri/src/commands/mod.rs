//! Tauri command surface, split into cohesive submodules.
//!
//! Each `#[tauri::command]` keeps its exact name/signature; `mod.rs` re-exports
//! them via `pub use <submodule>::*` so `commands::<name>` resolves unchanged
//! for `lib.rs`'s `tauri::generate_handler![…]`. Submodules pull shared imports
//! and helpers (`quote_sheet`, `map_cashflow_row`) in via `use super::*`.

use crate::forecast::{self, CashflowEvent};
use crate::google_sheets::write_back::{self, CellWrite, WriteBackTxn};
use crate::google_sheets::{self, SheetsClient, import, layout_detect};
use crate::oauth::{self, AppDataDir, OAuthStateStore};
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

pub(crate) mod forecast_cmds;
pub(crate) mod oauth_cmds;
pub(crate) mod pockets;
pub(crate) mod reminder_cmds;
pub(crate) mod sheets_import;
pub(crate) mod transactions;
pub(crate) mod write_back_cmds;

pub use forecast_cmds::*;
pub use oauth_cmds::*;
pub use pockets::*;
pub use reminder_cmds::*;
pub use sheets_import::*;
pub use transactions::*;
pub use write_back_cmds::*;

/// Aba entre aspas simples para um range A1 do Sheets, com as aspas internas escapadas (`'` → `''`).
/// Sem isto, uma aba chamada `O'Brien` quebraria o range (`'O'Brien'`) e a chamada à API falharia.
pub(crate) fn quote_sheet(name: impl AsRef<str>) -> String {
    format!("'{}'", name.as_ref().replace('\'', "''"))
}

/// Mapeia uma linha crua do banco para um `CashflowEvent`. Retorna `None` para linhas que
/// `forecast::classify` não consegue classificar (ex.: tipo desconhecido — filtrado em silêncio,
/// consistente com o comportamento anterior dos três chamadores).
///
/// Invariante: `amount_cents` é sempre uma MAGNITUDE POSITIVA; o sinal vem de `kind`
/// (ver `forecast::signed`). O `.abs()` protege contra um negativo não-canônico gravado
/// por um escritor com bug.
pub(crate) fn map_cashflow_row(
    (ttype, amount, date_str, pm, is_fixed, is_proj, liq): (
        String,
        i64,
        String,
        String,
        i64,
        i64,
        String,
    ),
) -> Option<CashflowEvent> {
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    let pm = (!pm.is_empty()).then_some(pm);
    let to_liq = (!liq.is_empty()).then_some(liq);
    let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
    Some(CashflowEvent {
        date,
        kind,
        amount_cents: amount.abs(),
        realized: is_proj == 0,
    })
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
            raw_note: String::new(),
        }];
        let checksum = compute_checksum(&rows);
        assert!(!checksum.is_empty());
    }

    // Spec 010 slice 0: floats do calamine chegam ao parse_number sem ambiguidade de
    // separador — regressão do bug de 100× (12.34 → R$ 1.234,00).
    #[test]
    fn xlsx_float_cells_parse_to_correct_cents() {
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(12.34)),
            "12.3400"
        );
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(1234.56)),
            "1234.5600"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Int(1370)), "1370");
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::String(" Entrada ".into())),
            "Entrada"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Empty), "");

        use google_sheets::import::parse_number;
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(12.34))),
            1234
        );
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(5678.1234))),
            567812
        );
        // float que parece milhar (3 dígitos após o ponto) — o {:.4} blinda o caso.
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(123.456))),
            12346
        );
    }

    #[test]
    fn local_xlsx_path_validation_rejects_non_xlsx() {
        let path = std::env::temp_dir().join(format!("neko-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not a workbook").unwrap();

        let err = validate_local_xlsx_path(path.to_str().unwrap()).unwrap_err();

        std::fs::remove_file(&path).unwrap();
        assert!(err.contains(".xlsx"));
    }

    #[test]
    fn local_xlsx_path_validation_accepts_regular_xlsx_file() {
        let path = std::env::temp_dir().join(format!("neko-{}.xlsx", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"placeholder").unwrap();

        let got = validate_local_xlsx_path(path.to_str().unwrap()).unwrap();

        std::fs::remove_file(&path).unwrap();
        assert_eq!(got.extension().and_then(|e| e.to_str()), Some("xlsx"));
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

    // O Diário de hoje vem das transações realizadas (despesa variável não-crédito do dia).
    #[tokio::test]
    async fn dashboard_daily_spend_comes_from_transactions() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Diário realizado de hoje (expense, is_fixed=0).
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await;
        // Despesa de outro dia não conta no "hoje".
        insert_realized(&pool, "expense", 9_999, "2026-06-12").await;

        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "Diário de hoje vem das transações, não R$0 estrutural"
        );
    }

    // Defesa-em-profundidade (review adversarial): `amount` é magnitude positiva por convenção, mas
    // `daily_spend_today` usa ABS espelhando o forecast — então um amount negativo (não-canônico)
    // ainda rende magnitude positiva, jamais um "gasto negativo" que quebraria `teto - gasto`.
    #[tokio::test]
    async fn dashboard_daily_spend_is_positive_magnitude_even_if_amount_negative() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized(&pool, "expense", -4_271, "2026-06-13").await; // sinal não-canônico
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "ABS garante magnitude positiva mesmo com amount negativo"
        );
    }

    // Regressão (review adversarial): o gasto realizado de HOJE não pode contar ocorrências
    // PROJETADAS (is_projection=1) — ex.: uma recorrência futura cuja ocorrência cai hoje.
    #[tokio::test]
    async fn dashboard_daily_spend_excludes_projected() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await; // realizado → conta
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1,'expense',?2,'2026-06-13',0,1)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(9_999i64)
        .execute(&pool)
        .await
        .unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "projeção não entra no gasto realizado de hoje"
        );
    }

    // Visão anual: agrega as 4 métricas por mês a partir das transações do ano (realizado +
    // projetado), independente do horizonte do forecast.
    #[tokio::test]
    async fn annual_metrics_aggregates_each_month() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Março realizado: renda 700, diário 250.
        insert_realized(&pool, "income", 700_000, "2026-03-05").await;
        insert_realized(&pool, "expense", 250_000, "2026-03-10").await;
        // Julho projetado: renda 500.
        insert_projection(&pool, "income", 500_000, "2026-07-05", "", 0).await;

        let a = annual_metrics(&pool, 2026, today).await.unwrap();
        assert_eq!(a.year, 2026);
        assert_eq!(a.months.len(), 12);
        let mar = a.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.income_cents, 700_000);
        assert_eq!(mar.cost_of_living_cents, 250_000); // diário realizado
        assert_eq!(mar.performance_cents, 450_000); // 700 − 250
        let jul = a.months.iter().find(|m| m.month == 7).unwrap();
        assert_eq!(jul.income_cents, 500_000);
        let jan = a.months.iter().find(|m| m.month == 1).unwrap();
        assert_eq!(jan.income_cents, 0); // mês vazio
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

    #[tokio::test]
    async fn economia_import_zero_removes_stale_month() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;

        assert_eq!(
            store_economia_entries(&pool, &[(2026, 1, 100_000)])
                .await
                .unwrap(),
            1
        );
        let (stored,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id='economia:2026-01'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 100_000);

        assert_eq!(
            store_economia_entries(&pool, &[(2026, 1, 0)])
                .await
                .unwrap(),
            1
        );
        let (remaining,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id='economia:2026-01'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    // Regressão (review): store_economia_entries grava upsert+delete numa única transação. Uma
    // chamada com meses mistos (>0 e =0) deve aplicar TODOS atomicamente, criando a reserva 1×.
    #[tokio::test]
    async fn economia_mixed_entries_commit_in_one_transaction() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;

        let n = store_economia_entries(
            &pool,
            &[(2026, 1, 100_000), (2026, 2, 0), (2026, 3, 50_000)],
        )
        .await
        .unwrap();
        assert_eq!(n, 3);

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'economia:2026-%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2, "jan e mar gravados; fev (0) não cria linha");

        let (reserves,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM account WHERE liquidity='reserve'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reserves, 1, "a conta de reserva é criada uma única vez");
    }

    // M2: backup via VACUUM INTO grava um arquivo SQLite válido e não-vazio. Usa uma fonte em
    // ARQUIVO (como o app real), pois VACUUM INTO a partir de `:memory:` não materializa o arquivo.
    // Checa o cabeçalho mágico do SQLite — prova que VACUUM INTO produziu um DB de verdade.
    #[tokio::test]
    async fn backup_database_writes_valid_sqlite_file() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let src = std::env::temp_dir().join(format!("neko-src-{}.db", uuid::Uuid::new_v4()));
        let src_str = src.to_str().unwrap();
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
        insert_liquid_account(&pool, 123_456).await;

        let dest = std::env::temp_dir().join(format!("neko-backup-{}.db", uuid::Uuid::new_v4()));
        let dest_str = dest.to_str().unwrap();
        let written = backup_db(&pool, &src, dest_str).await.unwrap();
        assert_eq!(written, dest_str);

        let bytes = std::fs::read(&dest).unwrap();
        assert!(
            bytes.starts_with(b"SQLite format 3\0"),
            "o backup é um arquivo SQLite válido"
        );
        assert!(bytes.len() > 1000, "backup não-vazio");

        // Re-backup sobre um destino EXISTENTE: o swap atômico substitui sem deixar .tmp órfão.
        backup_db(&pool, &src, dest_str).await.unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".neko-backup-"))
            .collect();
        assert!(leftovers.is_empty(), "nenhum .tmp órfão após o rename");

        // Destino vazio é rejeitado; o próprio banco ativo é rejeitado (não desvincular o DB em uso).
        assert!(backup_db(&pool, &src, "   ").await.is_err());
        assert!(
            backup_db(&pool, &src, src_str).await.is_err(),
            "backup sobre o banco ativo é recusado"
        );

        drop(pool);
        std::fs::remove_file(&dest).ok();
        std::fs::remove_file(&src).ok();
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
        // poupança anual é exercitada no teste `forecast_dual_guardrail_savings_binds_for_owner`.
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

    // Spec 011: um `transfer` (magnitude positiva) para um bolso de POUPANÇA (reserve) vira
    // Economia — sai do saldo de gasto E conta no Economizado. Vale-refeição (restricted) NÃO.
    #[tokio::test]
    async fn forecast_dto_transfer_to_reserve_is_economia_and_leaves_balance() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // R$ 1.000,00 em caixa

        // Conta de reserva (poupança real).
        let pid: (String,) = sqlx::query_as("SELECT id FROM person LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Reserva','savings',?2,0,'reserve')",
        )
        .bind(&reserve_id)
        .bind(&pid.0)
        .execute(&pool)
        .await
        .unwrap();

        // Transfer FUTURO de R$ 300,00 (magnitude positiva) para a reserva.
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES (?1,'transfer',30_000,'2026-03-20',?2,1)",
        )
        .bind(&tid)
        .bind(&reserve_id)
        .execute(&pool)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Conta como Economia no mês.
        let mar = fc.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.economia_cents, 30_000, "guardar na reserva = Economia");
        // E sai do saldo de gasto (caixa cai de 100k para 70k).
        let d20 = fc.daily.iter().find(|d| d.date == "2026-03-20").unwrap();
        assert_eq!(d20.balance_cents, 70_000);
    }

    // Regressão (review adversarial): a Economia REGISTRADA anual (transfer→reserva) é distinta do
    // net superávit. O ColchaoCard mostrava "R$ 0" fixo; agora vem do DTO. Só transfer→reserva conta
    // — income/expense e transfer→líquido (net-zero entre contas) não.
    #[tokio::test]
    async fn annual_registered_economia_counts_only_reserve_transfers() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;
        let pid: (String,) = sqlx::query_as("SELECT id FROM person LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Reserva','savings',?2,0,'reserve')")
            .bind(&reserve_id).bind(&pid.0).execute(&pool).await.unwrap();
        let liquid2 = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Conta2','bank',?2,0,'liquid')")
            .bind(&liquid2).bind(&pid.0).execute(&pool).await.unwrap();
        // Mês COMPLETO (março; hoje = junho).
        insert_realized(&pool, "income", 500_000, "2026-03-05").await;
        insert_realized(&pool, "expense", 100_000, "2026-03-10").await;
        let mk_transfer = |amount: i64, date: &'static str, to: String| {
            let p = pool.clone();
            async move {
                sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES (?1,'transfer',?2,?3,?4,0)")
                    .bind(uuid::Uuid::new_v4().to_string()).bind(amount).bind(date).bind(to)
                    .execute(&p).await.unwrap();
            }
        };
        mk_transfer(30_000, "2026-03-20", reserve_id).await; // → reserva = Economia registrada
        mk_transfer(20_000, "2026-03-15", liquid2).await; // → líquido = net-zero, NÃO conta

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();
        assert_eq!(
            fc.annual_savings.registered_economia_cents, 30_000,
            "só transfer→reserva conta como Economia registrada"
        );
    }

    // REGRESSÃO: net superávit grande não satisfaz mais o guardrail de poupança — só a Economia
    // REGISTRADA (transfers→reserva) conta. Quem ganhou >> gastou mas NÃO transferiu para a reserva
    // tem Economizado = 0 pelo método, então a régua de poupança morde (não o proxy do net antigo).
    #[tokio::test]
    async fn guardrail_savings_uses_registered_economia_not_net_surplus() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 1_000_000).await; // R$ 10 000 de caixa → folga de caixa ampla
        // Mês COMPLETO (março; hoje = junho): renda R$ 5 000, saída R$ 1 000 → net R$ 4 000.
        // ZERO transfers para reserva → Economia registrada = 0.
        insert_realized(&pool, "income", 500_000, "2026-03-05").await;
        insert_realized(&pool, "expense", 100_000, "2026-03-10").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.annual_savings.realized_income_cents, 500_000);
        assert_eq!(
            fc.annual_savings.registered_economia_cents, 0,
            "sem transfer→reserva, a Economia registrada é zero"
        );
        // Folga de poupança = Economia(0) − 25% × renda(500_000) = −125_000 (NEGATIVA).
        // Sob o proxy antigo seria net(400_000) − 125_000 = +275_000 e binding="cash".
        assert_eq!(fc.savings_headroom_cents, Some(-125_000));
        assert_eq!(
            fc.binding_guardrail, "savings",
            "a régua de poupança morde apesar do net positivo"
        );
        assert_eq!(
            fc.safe_to_spend_today_cents, 0,
            "net superávit não conta como já-poupado; pode gastar 0"
        );
        assert!(
            fc.cash_headroom_cents > 0,
            "há caixa de sobra — o que morde é a poupança, não o caixa"
        );
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

    // KV de preferências: grava e lê; chave ausente → None.
    #[tokio::test]
    async fn app_setting_roundtrip() {
        let pool = fixture_pool().await;
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            None
        );
        app_setting_set(&pool, "onboarding_done", "true")
            .await
            .unwrap();
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            Some("true".to_string())
        );
        // Sobrescreve.
        app_setting_set(&pool, "onboarding_done", "false")
            .await
            .unwrap();
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            Some("false".to_string())
        );
    }

    // Multi-titular: get_recent_transactions traz os titulares distintos das parcelas (sem N+1),
    // ordenados por nome; transação sem split vem com `owners` vazio.
    #[tokio::test]
    async fn recent_transactions_carry_distinct_owners() {
        let pool = fixture_pool().await;
        for (id, name) in [("bruno", "Bruno"), ("ana", "Ana")] {
            sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                .bind(id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        // Lançamento dividido entre Bruno e Ana.
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES ('t1','expense',-30000,'2026-06-05',0)")
            .execute(&pool).await.unwrap();
        for (sid, owner, amt) in [("s1", "bruno", -20000), ("s2", "ana", -10000)] {
            sqlx::query("INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1,'t1',?2,?3)")
                .bind(sid).bind(amt).bind(owner)
                .execute(&pool).await.unwrap();
        }
        // Lançamento sem split → sem titulares.
        insert_realized(&pool, "expense", -5000, "2026-06-04").await;

        let rows = recent_transactions(&pool, 10).await.unwrap();
        let split = rows.iter().find(|r| r.id == "t1").unwrap();
        assert_eq!(split.owners, vec!["Ana".to_string(), "Bruno".to_string()]);
        let solo = rows.iter().find(|r| r.id != "t1").unwrap();
        assert!(solo.owners.is_empty(), "sem split → owners vazio");
    }

    // Os chips de tag do Livro-razão: recent_transactions devolve as tags anexadas.
    #[tokio::test]
    async fn recent_transactions_carry_attached_tags() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Viagem", "var(--cat-sky)", Some("✈"), false)
            .await
            .unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES ('t1','expense',-5000,'2026-06-05',0)")
            .execute(&pool).await.unwrap();
        insert_realized(&pool, "expense", -3000, "2026-06-04").await; // sem tag
        crate::tags::set_transaction_tags(&pool, "t1", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let rows = recent_transactions(&pool, 10).await.unwrap();
        let tagged = rows.iter().find(|r| r.id == "t1").unwrap();
        assert_eq!(tagged.tags.len(), 1);
        assert_eq!(tagged.tags[0].name, "Viagem");
        assert_eq!(tagged.tags[0].emoji.as_deref(), Some("✈"));
        let untagged = rows.iter().find(|r| r.id != "t1").unwrap();
        assert!(untagged.tags.is_empty(), "sem tag → tags vazio");
    }

    // Caminho de escrita: lançamento único realizado, com tags anexadas.
    #[tokio::test]
    async fn create_transaction_inserts_realized_with_tags() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Viagem", "#3aa", None, false)
            .await
            .unwrap();

        let id = create_transaction_inner(
            &pool,
            "expense",
            12_345,
            Some("Mercado".into()),
            "2026-06-14",
            Some("debit".into()),
            false,
            std::slice::from_ref(&tag),
            None,
            None,
        )
        .await
        .unwrap();

        let (amount, is_proj, desc): (i64, i64, String) = sqlx::query_as(
            "SELECT amount, is_projection, COALESCE(description,'') FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(amount, 12_345);
        assert_eq!(is_proj, 0, "lançamento manual é realizado");
        assert_eq!(desc, "Mercado");

        let (tag_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transaction_tag WHERE transaction_id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(tag_count, 1, "tag anexada ao lançamento");
    }

    // "Repetir": o caminho recorrente gera a série projetada inteira e propaga a tag.
    #[tokio::test]
    async fn create_transaction_with_recurrence_builds_tagged_series() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Aluguel", "#a33", None, false)
            .await
            .unwrap();

        let rec_id = create_transaction_inner(
            &pool,
            "expense",
            230_000,
            Some("Aluguel".into()),
            "2026-06-15",
            Some("debit".into()),
            true,
            std::slice::from_ref(&tag),
            Some(RecurrenceInput {
                frequency: "mensal".into(),
                repetitions: 3,
            }),
            None,
        )
        .await
        .unwrap();

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE recurrence_id = ?1 AND is_projection = 1",
        )
        .bind(&rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 3, "série projetada de 3 meses");

        let (tagged,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transaction_tag tt JOIN \"transaction\" t ON t.id = tt.transaction_id WHERE t.recurrence_id = ?1",
        )
        .bind(&rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tagged, 3, "tag propagada a cada ocorrência");
    }

    #[tokio::test]
    async fn create_transaction_rejects_bad_input() {
        let pool = fixture_pool().await;
        // transfer sem conta-destino → Err (Economia precisa de uma reserva explícita).
        assert!(
            create_transaction_inner(
                &pool,
                "transfer",
                100,
                None,
                "2026-06-14",
                None,
                false,
                &[],
                None,
                None,
            )
            .await
            .is_err(),
            "transfer sem to_account_id é rejeitado"
        );
        // tipo inexistente → Err.
        assert!(
            create_transaction_inner(
                &pool,
                "bogus",
                100,
                None,
                "2026-06-14",
                None,
                false,
                &[],
                None,
                None,
            )
            .await
            .is_err(),
            "tipo inválido é rejeitado"
        );
        assert!(
            create_transaction_inner(
                &pool,
                "expense",
                0,
                None,
                "2026-06-14",
                None,
                false,
                &[],
                None,
                None,
            )
            .await
            .is_err(),
            "valor zero/negativo é rejeitado"
        );
    }

    // Economia manual: transfer→reserva é aceito e gravado na MESMA forma do import (type='transfer',
    // amount positivo, to_account_id na reserva), que `classify()`/`forecast_dto` contam como Economia.
    #[tokio::test]
    async fn create_transaction_transfer_to_reserve_inserts_economia() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 0).await;
        let (reserve_id,): (String,) =
            sqlx::query_as("SELECT id FROM account WHERE liquidity='reserve' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let id = create_transaction_inner(
            &pool,
            "transfer",
            50_000,
            Some("Economia manual".into()),
            "2026-06-19",
            None,
            false,
            &[],
            None,
            Some(&reserve_id),
        )
        .await
        .expect("transfer para reserva deve ser aceito");

        let (r#type, amount, to_acct): (String, i64, Option<String>) =
            sqlx::query_as("SELECT type, amount, to_account_id FROM \"transaction\" WHERE id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(r#type, "transfer");
        assert_eq!(amount, 50_000);
        assert_eq!(to_acct.as_deref(), Some(reserve_id.as_str()));
    }

    // transfer→conta líquida é net-zero entre contas, não poupar: deve ser rejeitado (não é Economia).
    #[tokio::test]
    async fn create_transaction_transfer_to_liquid_account_is_rejected() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;
        let (liquid_id,): (String,) =
            sqlx::query_as("SELECT id FROM account WHERE liquidity='liquid' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let result = create_transaction_inner(
            &pool,
            "transfer",
            10_000,
            None,
            "2026-06-19",
            None,
            false,
            &[],
            None,
            Some(&liquid_id),
        )
        .await;
        assert!(
            result.is_err(),
            "transfer para conta líquida não é Economia — deve ser rejeitado"
        );
    }

    // Previsibilidade: meses futuros esparsos (só fixas) são detectados como incompletos, e a
    // poupança realizada (honesta) difere da projetada (otimista). O caso do usuário.
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
        insert_sheet_balance(&pool, "2026", "2026-06-12", 500_000).await; // Saldo de hoje

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // Saldo de hoje na planilha vence o bolso; sem lacuna (hoje está na série).
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 500_000);
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

    // Regressão: uma Economia (transfer→reserva) entre o último Saldo da planilha e hoje reduz o
    // saldo líquido. Antes era excluída do gap → a semente ficava superestimada pelo valor da
    // Economia. Agora o transfer entra como saída (−amount).
    #[tokio::test]
    async fn projection_seed_gap_includes_transfer_economia() {
        let pool = fixture_pool().await;
        insert_sheet_balance(&pool, "2026", "2026-06-10", 500_000).await;
        // Economia transferida entre a data da semente e hoje: deve reduzir a semente.
        insert_realized(&pool, "transfer", 50_000, "2026-06-11").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // 500.000 − 50.000 = 450.000
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 450_000);
    }

    // Regressão: sem bolso e com o Saldo da planilha sendo descartado, o saldo de hoje aparecia
    // ZERADO e surgia um déficit ("buraco") falso. Com a série de Saldo da planilha como semente,
    // o saldo de hoje passa a bater com a planilha e o déficit falso some.
    #[tokio::test]
    async fn forecast_seeds_from_sheet_saldo_no_false_deficit() {
        let pool = fixture_pool().await;
        // Saldo de hoje vindo da planilha; uma saída futura pequena no dia 26.
        insert_sheet_balance(&pool, "2026", "2026-06-12", 500_000).await;
        insert_projection(&pool, "expense", 7_890, "2026-06-26", "debit", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Antes: daily[0] = 0 (saldo zerado). Agora bate com o Saldo de hoje da planilha.
        assert_eq!(fc.daily[0].date, "2026-06-12");
        assert_eq!(fc.daily[0].balance_cents, 500_000);
        // Antes: deepest_deficit negativo (o "buraco que não existe"). Agora positivo.
        let trough = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(trough.balance_cents, 500_000 - 7_890);
        assert!(trough.balance_cents > 0);
        assert!(fc.safe_to_spend_today_cents > 0);
    }

    // Guardrail duplo end-to-end: caixa cheio (semente alta), mas o mês corrente com performance
    // negativa → "pode gastar" honesto = 0, limitado pela poupança ANUAL. O horizonte varre até o
    // fim dos dados pré-lançados (dez). Crava o P0 do review: a performance do mês corrente inclui o
    // REALIZADO antes de hoje (não só os eventos futuros), senão o mês aparece com sinal trocado e o
    // guardrail decide errado.
    #[tokio::test]
    async fn forecast_dual_guardrail_savings_binds_for_owner() {
        let pool = fixture_pool().await;
        // Caixa cheio: o Saldo de hoje fica bem acima do piso de reserva do método, de modo que a
        // régua de CAIXA tem folga e quem morde é a POUPANÇA anual (o ponto deste teste).
        insert_sheet_balance(&pool, "2026", "2026-06-13", 1_700_000).await; // Saldo de hoje
        insert_sheet_balance(&pool, "2026", "2026-12-31", 2_500_000).await; // estende horizonte
        // Meses COMPLETOS (jan–mai) abaixo da meta → a poupança ANUAL morde. Sem isto, o mês
        // corrente (junho, em andamento) NÃO conta — evita o falso pânico de meio de mês. Estes
        // mesmos meses definem o custo de vida (mediana = 220.000), logo o piso de reserva do
        // método = 220.000 × 6 = 1.320.000 (plano 033).
        for m in [1, 2, 3, 4, 5] {
            insert_realized(&pool, "income", 200_000, &format!("2026-{m:02}-05")).await;
            insert_realized(&pool, "expense", 220_000, &format!("2026-{m:02}-10")).await;
        }
        // Bolso de reserva configurado no exato piso do método (custo de vida × 6) — o caixa fica
        // acima dele, então a régua de caixa NÃO morde e a poupança anual é a que decide.
        insert_reserve_account(&pool, 1_320_000).await;
        // Junho (corrente) — metade realizada antes de hoje, metade projetada: testa que a
        // PERFORMANCE do mês inclui o realizado (o P0), mesmo o mês não contando na poupança anual.
        insert_realized(&pool, "income", 400_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 700_000, "2026-06-10").await;
        insert_realized(&pool, "income", 600_000, "2026-06-29").await;
        insert_projection(&pool, "expense", 400_000, "2026-06-30", "debit", 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.horizon_end, "2026-12-31");
        // Performance de junho = MÊS INTEIRO realizado: inclui os R$ 700k já realizados em 10/jun,
        // que o cálculo antigo (só futuros) ignorava (o P0). A PREVISÃO de diário restante reduz o
        // saldo de caixa projetado (correto para o forecast), mas NÃO desconta a Performance
        // (paridade com planilha — DECISÃO DO DONO 2026-06-20).
        let jun = fc.months.iter().find(|m| m.month == 6).unwrap();
        // Performance = Entradas − (Saídas + Diário realizado) = 1_000_000 − 1_100_000 = −100_000.
        assert_eq!(jun.performance_cents, -100_000);
        // Poupança ANUAL (meses completos jan–mai, abaixo da meta) manda → pode gastar 0.
        assert_eq!(fc.binding_guardrail, "savings");
        assert_eq!(fc.safe_to_spend_today_cents, 0);
        assert!(fc.savings_headroom_cents.unwrap() < 0);
        // Há caixa acima do piso de reserva (1.700.000 − fixos > 1.320.000) → a régua de caixa
        // tem folga e não é a que morde; a poupança anual (negativa) é a binding.
        assert!(fc.cash_headroom_cents > 0);
        assert_eq!(fc.savings_target_bps, 2500);
    }

    async fn insert_reserve_account(pool: &sqlx::SqlitePool, balance: i64) {
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(pool)
            .await
            .unwrap();
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES (?1,'Reserva','savings',?2,?3,'reserve')",
        )
        .bind(&aid)
        .bind(&pid)
        .bind(balance)
        .execute(pool)
        .await
        .unwrap();
    }

    // P1.1 (review): reserve_months derivado ao vivo = saldo das contas de reserva ÷ custo de vida
    // mensal (baseline), em vez do `reserve.current_months` que nunca tem writer de produção.
    #[tokio::test]
    async fn dashboard_reserve_months_derived_from_balance_and_baseline() {
        let pool = fixture_pool().await;
        // Custo de vida: meses completos (mar–mai) com saída 100.000/mês → mediana baseline = 100.000.
        for m in [3, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        insert_reserve_account(&pool, 600_000).await; // 600.000 ÷ 100.000 = 6,0 meses
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert!((s.reserve_months - 6.0).abs() < 1e-9);
    }

    // Sem nenhum mês completo (baseline 0), reserve_months é 0 (não divide por zero).
    #[tokio::test]
    async fn dashboard_reserve_months_zero_without_baseline() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 600_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.reserve_months, 0.0);
    }

    // --- reserve_floor tests (plano 033) -------------------------------------------------
    // O piso de reserva = max(saldo dos Bolsos de reserva, custo de vida mensal × meses do método).

    // Sem Bolso de reserva E sem histórico de custo de vida, o piso é 0 (não bloqueia usuário novo).
    #[tokio::test]
    async fn reserve_floor_zero_when_no_history_and_no_reserve_account() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let floor = reserve_floor(&pool, today).await.unwrap();
        assert_eq!(floor, 0);
    }

    // Sem Bolso de reserva mas COM histórico de custo de vida, o piso é
    // custo_de_vida_mensal × RESERVE_MIN_MONTHS (o mínimo calculado entra em cena — antes ficava 0
    // e o guardrail de caixa ficava desmontado).
    #[tokio::test]
    async fn reserve_floor_uses_computed_minimum_when_no_reserve_account() {
        let pool = fixture_pool().await;
        // 3 meses completos de saída a 100.000 cada → mediana do custo de vida = 100.000.
        for m in [3u32, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let floor = reserve_floor(&pool, today).await.unwrap();
        // 100.000 × 6 = 600.000
        assert_eq!(floor, 600_000);
    }

    // Com um Bolso de reserva acima do piso calculado, o saldo real vence (usamos o maior dos dois).
    #[tokio::test]
    async fn reserve_floor_uses_reserve_balance_when_above_computed_minimum() {
        let pool = fixture_pool().await;
        for m in [3u32, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        // Saldo de reserva 900.000 > piso calculado 600.000.
        insert_reserve_account(&pool, 900_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let floor = reserve_floor(&pool, today).await.unwrap();
        assert_eq!(floor, 900_000);
    }

    // Com um Bolso de reserva ABAIXO do piso calculado, o piso do método vence (é a restrição mais
    // forte — o objetivo do método é maior que o que o usuário guardou até agora).
    #[tokio::test]
    async fn reserve_floor_uses_computed_minimum_when_reserve_balance_is_low() {
        let pool = fixture_pool().await;
        for m in [3u32, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        // Saldo de reserva 200.000 < piso calculado 600.000.
        insert_reserve_account(&pool, 200_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let floor = reserve_floor(&pool, today).await.unwrap();
        assert_eq!(floor, 600_000);
    }

    // End-to-end (plano 033): SEM Bolso de reserva, o guardrail "pode gastar" deixa de ficar
    // desmontado — o piso calculado (custo de vida × meses) já protege o caixa via forecast_dto.
    // Antes do plano 033, reserve_floor = 0 aqui e cash_headroom = saldo cheio inteiro.
    #[tokio::test]
    async fn forecast_cash_guardrail_gated_by_computed_reserve_floor() {
        let pool = fixture_pool().await;
        // Saldo de hoje confortável; só fixas modestas adiante → o trough fica alto.
        insert_sheet_balance(&pool, "2026", "2026-06-13", 1_000_000).await;
        insert_sheet_balance(&pool, "2026", "2026-12-31", 1_200_000).await;
        // Custo de vida: 3 meses completos a 100.000 → mediana 100.000 → piso = 600.000.
        for m in [3u32, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        // Sem renda no ano → a régua de poupança fica INATIVA (None); resta só o caixa, que agora
        // está gated pelo piso de reserva calculado.
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();
        assert_eq!(fc.binding_guardrail, "cash");
        assert!(
            fc.savings_headroom_cents.is_none(),
            "sem renda, a régua de poupança está inativa"
        );
        // O piso (600.000) foi de fato subtraído: a folga de caixa é o trough menos 600.000, não o
        // trough inteiro. Confirma que o guardrail deixou de estar desmontado.
        let trough = fc.deepest_deficit.as_ref().unwrap().balance_cents;
        assert_eq!(fc.cash_headroom_cents, trough - 600_000);
    }

    // P1.3 (review): teto do diário cai para o Diário médio do mês anterior quando não há orçamento
    // explícito (antes era 0 fixo → tile "de R$0" e forecast otimista).
    #[tokio::test]
    async fn dashboard_daily_budget_falls_back_to_prior_month_avg() {
        let pool = fixture_pool().await;
        // Maio: diário (is_fixed=0) de 310.000 em 31 dias → média = 10.000/dia.
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.daily_budget, 310_000 / 31); // 10.000
    }

    // O saldo projetado do dashboard deve usar o mesmo driver de Diário futuro do forecast; senão o
    // tile "Saldo projetado" fica otimista e diverge do fim do mês exibido no herói.
    #[tokio::test]
    async fn dashboard_projected_balance_includes_future_daily_ceiling() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 300_000).await;
        // Maio: diário de 310.000 em 31 dias → teto típico = 10.000/dia.
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();
        let forecast = forecast_dto(&pool, today).await.unwrap();

        // Junho tem 17 dias após 13/jun (14..30). O saldo precisa reservar esse Diário restante.
        assert_eq!(summary.balance, 130_000);
        assert_eq!(forecast.month_end[0].balance_cents, summary.balance);
    }

    // Orçamento diário explícito ativo vence o fallback.
    #[tokio::test]
    async fn dashboard_daily_budget_prefers_explicit_active_budget() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await; // existiria fallback 10.000
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        let bid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status, free_income) \
             VALUES (?1,?2,?3,'2026-06-01','active',0)",
        )
        .bind(&bid)
        .bind(&pid)
        .bind(15_000_i64)
        .execute(&pool)
        .await
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.daily_budget, 15_000); // orçamento explícito, não o fallback de 10.000
    }

    // Grade do mês: 31 linhas (maio), fluxos agregados por dia e Saldo vindo da planilha.
    #[tokio::test]
    async fn month_grid_aggregates_flows_and_uses_sheet_saldo() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "income", 700_000, "2026-05-05").await;
        insert_realized(&pool, "expense", 12_000, "2026-05-05").await; // diário (is_fixed NULL → Diário)
        insert_realized(&pool, "expense", 4_000, "2026-05-09").await;
        insert_sheet_balance(&pool, "2026", "2026-05-05", 580_000).await;

        let grid = month_grid(&pool, 2026, 5).await.unwrap();
        assert_eq!(grid.len(), 31); // maio tem 31 dias
        assert_eq!(grid[0].day, 1);
        assert_eq!(grid[0].balance_cents, None); // dia 1 não importado

        let d5 = grid.iter().find(|g| g.day == 5).unwrap();
        assert_eq!(d5.income_cents, 700_000);
        assert_eq!(d5.daily_out_cents, 12_000);
        assert_eq!(d5.fixed_out_cents, 0);
        assert_eq!(d5.balance_cents, Some(580_000)); // Saldo da planilha

        let d9 = grid.iter().find(|g| g.day == 9).unwrap();
        assert_eq!(d9.daily_out_cents, 4_000);
        assert_eq!(d9.balance_cents, None);
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

    // Plan 009: o filtro de ano virou range `date >= 'YYYY-01-01' AND date < '(YYYY+1)-01-01'`.
    // O limite superior EXCLUSIVO `< '2027-01-01'` mantém o contrato: 2027-01-01 cai em 2027, não
    // em 2026 — byte-idêntico ao antigo `substr(date,1,4) = '2026'`.
    #[tokio::test]
    async fn load_year_events_year_boundary() {
        let pool = fixture_pool().await;
        let insert = "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
                      VALUES (?1,'income',?2,?3,0,0)";
        sqlx::query(insert)
            .bind("t1")
            .bind(100_000i64)
            .bind("2026-06-15")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(insert)
            .bind("t2")
            .bind(200_000i64)
            .bind("2027-01-01")
            .execute(&pool)
            .await
            .unwrap();

        let y2026 = load_year_events(&pool, 2026).await.unwrap();
        assert_eq!(y2026.len(), 1, "só 2026-06-15 cai em 2026");
        assert_eq!(y2026[0].amount_cents, 100_000);

        let y2027 = load_year_events(&pool, 2027).await.unwrap();
        assert_eq!(y2027.len(), 1, "2027-01-01 cai em 2027 (limite exclusivo)");
        assert_eq!(y2027[0].amount_cents, 200_000);
    }

    // ===================================================================================
    // Plan 010: characterization tests for money/forecast SQL helpers. These PIN the
    // current behavior of helpers that are otherwise only exercised through the
    // higher-level DTOs, so the commands.rs split (plan 011) has a safety net. They
    // assert what the code does TODAY; if a value looks surprising, it is pinned as-is.
    // ===================================================================================

    // Insere um transfer (Economia) com destino explícito e is_projection controlável. Não há helper
    // existente para transfers com to_account_id, então inserimos direto (centavos inteiros ≥ 0).
    async fn insert_transfer_to(
        pool: &sqlx::SqlitePool,
        amount: i64,
        date: &str,
        to_account_id: &str,
        is_projection: i64,
    ) {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES (?1,'transfer',?2,?3,?4,?5)",
        )
        .bind(&tid)
        .bind(amount)
        .bind(date)
        .bind(to_account_id)
        .bind(is_projection)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn reserve_account_id(pool: &sqlx::SqlitePool) -> String {
        let (id,): (String,) =
            sqlx::query_as("SELECT id FROM account WHERE liquidity='reserve' LIMIT 1")
                .fetch_one(pool)
                .await
                .unwrap();
        id
    }

    // --- realized_annual_economia ------------------------------------------------------

    // Só meses COMPLETOS contam (substr(date,1,7) < mês corrente); o mês corrente fica de fora.
    #[tokio::test]
    async fn economia_counts_complete_months_only() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 0).await;
        let reserve_id = reserve_account_id(&pool).await;
        // Março (mês completo) → conta.
        insert_transfer_to(&pool, 50_000, "2026-03-15", &reserve_id, 0).await;
        // Junho (mês corrente, incompleto) → NÃO conta.
        insert_transfer_to(&pool, 30_000, "2026-06-05", &reserve_id, 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let economia = realized_annual_economia(&pool, today).await.unwrap();
        assert_eq!(
            economia, 50_000,
            "só março (mês completo) conta; junho fica de fora"
        );
    }

    // Transfer para conta LÍQUIDA (não reserve/illiquid) não é Economia → não soma.
    #[tokio::test]
    async fn economia_skips_transfers_to_liquid_accounts() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 0).await;
        let (liquid_id,): (String,) =
            sqlx::query_as("SELECT id FROM account WHERE type='bank' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        // Transfer em mês completo, mas o destino é líquido → fora do filtro de liquidez.
        insert_transfer_to(&pool, 40_000, "2026-03-15", &liquid_id, 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let economia = realized_annual_economia(&pool, today).await.unwrap();
        assert_eq!(economia, 0, "transfer p/ conta líquida não é Economia");
    }

    // Mesma regra de staleness do savings: a janela de DATA vence o flag is_projection congelado.
    #[tokio::test]
    async fn economia_ignores_stale_is_projection_flag() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 0).await;
        let reserve_id = reserve_account_id(&pool).await;
        // Mês completo mas com is_projection=1 (stale): ainda conta.
        insert_transfer_to(&pool, 70_000, "2026-04-10", &reserve_id, 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let economia = realized_annual_economia(&pool, today).await.unwrap();
        assert_eq!(
            economia, 70_000,
            "data (mês completo) decide, não o flag stale"
        );
    }

    // --- realized_monthly_baseline -----------------------------------------------------

    // Mediana dos ÚLTIMOS 6 meses completos. Inserimos 7 meses; o LIMIT 6 descarta o mais antigo
    // (jan). Os 6 recentes (fev..jul) ordenados asc = [150k,180k,200k,250k,300k,400k] → (200k+250k)/2.
    #[tokio::test]
    async fn baseline_is_median_of_last_six_complete_months() {
        let pool = fixture_pool().await;
        // (mês, valor) do mais antigo ao mais recente; jan será descartado pelo LIMIT 6.
        let months = [
            (1, 100_000),
            (2, 200_000),
            (3, 300_000),
            (4, 150_000),
            (5, 250_000),
            (6, 180_000),
            (7, 400_000),
        ];
        for (m, v) in months {
            insert_realized(&pool, "expense", v, &format!("2026-{m:02}-10")).await;
        }
        // today = 1º dia do mês após o último mês de despesa (agosto) → todos jan..jul são completos.
        let today = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let baseline = realized_monthly_baseline(&pool, today).await.unwrap();
        assert_eq!(
            baseline, 225_000,
            "(200_000 + 250_000) / 2 dos 6 meses recentes"
        );
    }

    // Sem meses completos → 0 (não há padrão a inferir).
    #[tokio::test]
    async fn baseline_returns_zero_when_no_complete_months() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let baseline = realized_monthly_baseline(&pool, today).await.unwrap();
        assert_eq!(baseline, 0);
    }

    // Despesas só no mês corrente não contam (mês incompleto) → baseline 0.
    #[tokio::test]
    async fn baseline_ignores_current_month() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 100_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 200_000, "2026-06-20").await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let baseline = realized_monthly_baseline(&pool, today).await.unwrap();
        assert_eq!(
            baseline, 0,
            "junho (mês corrente) está fora da janela de meses completos"
        );
    }

    // Número ÍMPAR de meses → valor do meio. 3 meses [100k,200k,300k] → 200k.
    #[tokio::test]
    async fn baseline_odd_count_uses_middle_value() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 100_000, "2026-03-10").await;
        insert_realized(&pool, "expense", 200_000, "2026-04-10").await;
        insert_realized(&pool, "expense", 300_000, "2026-05-10").await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let baseline = realized_monthly_baseline(&pool, today).await.unwrap();
        assert_eq!(baseline, 200_000, "mediana de 3 valores = o do meio");
    }

    // --- effective_daily_ceiling -------------------------------------------------------

    // Sem orçamento explícito: teto = Σ diário (não-fixo, não-crédito, não-projeção) do mês anterior
    // ÷ dias do mês. Maio tem 31 dias; today = 13/jun → mês anterior = maio.
    #[tokio::test]
    async fn daily_ceiling_falls_back_to_prior_month_avg() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await; // diário (is_fixed NULL)
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let ceiling = effective_daily_ceiling(&pool, today).await.unwrap();
        assert_eq!(
            ceiling,
            310_000 / 31,
            "média do diário de maio = 10.000/dia"
        );
    }

    // Orçamento explícito ativo (> 0) VENCE o fallback calculado.
    #[tokio::test]
    async fn daily_ceiling_prefers_active_budget_over_fallback() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await; // existiria fallback 10.000
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        let bid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status, free_income) \
             VALUES (?1,?2,?3,'2026-06-01','active',0)",
        )
        .bind(&bid)
        .bind(&pid)
        .bind(15_000_i64)
        .execute(&pool)
        .await
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let ceiling = effective_daily_ceiling(&pool, today).await.unwrap();
        assert_eq!(
            ceiling, 15_000,
            "orçamento explícito vence o fallback de 10.000"
        );
    }

    // Usuário novo, pool vazio → 0 (nada a assumir).
    #[tokio::test]
    async fn daily_ceiling_zero_when_no_prior_month() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let ceiling = effective_daily_ceiling(&pool, today).await.unwrap();
        assert_eq!(ceiling, 0);
    }

    // Só o diário variável NÃO-crédito entra na média; fixo, crédito e projeção são excluídos.
    #[tokio::test]
    async fn daily_ceiling_excludes_fixed_and_credit_from_avg() {
        let pool = fixture_pool().await;
        // Variável, débito, realizado → CONTA (62.000).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES (?1,'expense',62_000,'2026-05-10','debit',0,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();
        // Fixo → excluído.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES (?1,'expense',999_000,'2026-05-12','debit',1,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();
        // Crédito → excluído.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES (?1,'expense',888_000,'2026-05-14','credit',0,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();
        // Projeção → excluída (is_projection=1).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES (?1,'expense',777_000,'2026-05-16','debit',0,1)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let ceiling = effective_daily_ceiling(&pool, today).await.unwrap();
        assert_eq!(
            ceiling,
            62_000 / 31,
            "só os 62.000 variáveis em débito contam"
        );
    }

    // --- upsert_daily_budget -----------------------------------------------------------

    // O teto gravado por upsert_daily_budget_inner é lido de volta por effective_daily_ceiling
    // (mesma fonte: daily_budget WHERE status='active'). Re-gravar depreca o anterior e mantém
    // exatamente UMA linha ativa. amount=0 desativa o teto (engine cai no fallback).
    #[tokio::test]
    async fn upsert_daily_budget_writes_active_budget_read_by_ceiling() {
        let pool = fixture_pool().await;
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();

        // Grava 5.000 → uma linha ativa, e effective_daily_ceiling a lê.
        upsert_daily_budget_inner(&pool, 5_000).await.unwrap();
        let active: Vec<(i64,)> =
            sqlx::query_as("SELECT amount FROM daily_budget WHERE status='active'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(active.len(), 1, "exatamente uma linha ativa");
        assert_eq!(active[0].0, 5_000);
        assert_eq!(
            effective_daily_ceiling(&pool, today).await.unwrap(),
            5_000,
            "o teto gravado vence o fallback de média"
        );

        // Re-grava 8.000 → o anterior é deprecado; segue havendo só UMA linha ativa = 8.000.
        upsert_daily_budget_inner(&pool, 8_000).await.unwrap();
        let active: Vec<(i64,)> =
            sqlx::query_as("SELECT amount FROM daily_budget WHERE status='active'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            active.len(),
            1,
            "deprecate-and-replace mantém uma linha ativa"
        );
        assert_eq!(active[0].0, 8_000);
        assert_eq!(effective_daily_ceiling(&pool, today).await.unwrap(), 8_000);

        // amount=0 desativa o teto explícito → não resta linha ativa; ceiling cai no fallback (0 aqui).
        upsert_daily_budget_inner(&pool, 0).await.unwrap();
        let active_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_budget WHERE status='active'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(active_count.0, 0, "zero desativa o teto explícito");
        assert_eq!(
            effective_daily_ceiling(&pool, today).await.unwrap(),
            0,
            "sem teto e sem mês anterior, cai no fallback (0)"
        );
    }

    // --- load_write_back_txns ----------------------------------------------------------

    // Insere uma despesa com payment_method/is_fixed explícitos, realizada (is_projection=0).
    async fn insert_expense_pm(
        pool: &sqlx::SqlitePool,
        amount: i64,
        date: &str,
        payment_method: &str,
        is_fixed: i64,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES (?1,'expense',?2,?3,?4,?5,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(amount)
        .bind(date)
        .bind(payment_method)
        .bind(is_fixed)
        .execute(pool)
        .await
        .unwrap();
    }

    // income → Entrada; expense variável (is_fixed=0, débito) → Diario.
    #[tokio::test]
    async fn write_back_txns_income_and_variable_expense() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "income", 500_000, "2026-04-05").await;
        insert_expense_pm(&pool, 12_000, "2026-04-10", "debit", 0).await;

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert_eq!(out.len(), 2);
        assert!(
            out.iter()
                .any(|t| t.kind == import::RowKind::Entrada && t.amount_cents == 500_000)
        );
        assert!(
            out.iter()
                .any(|t| t.kind == import::RowKind::Diario && t.amount_cents == 12_000)
        );
    }

    // expense fixo (is_fixed=1) → Saida.
    #[tokio::test]
    async fn write_back_txns_fixed_expense_maps_to_saida() {
        let pool = fixture_pool().await;
        insert_expense_pm(&pool, 80_000, "2026-04-10", "debit", 1).await;

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, import::RowKind::Saida);
        assert_eq!(out[0].amount_cents, 80_000);
        assert_eq!(out[0].date, "2026-04-10");
    }

    // Sem cartão configurado: o crédito cai como Saida na PRÓPRIA data (ramo no-card).
    #[tokio::test]
    async fn write_back_txns_credit_no_card_falls_to_own_date() {
        let pool = fixture_pool().await;
        insert_expense_pm(&pool, 45_000, "2026-04-20", "credit", 0).await;

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, import::RowKind::Saida);
        assert_eq!(out[0].amount_cents, 45_000);
        assert_eq!(
            out[0].date, "2026-04-20",
            "sem cartão, o crédito fica na própria data"
        );
    }

    // Transfers (Economia) NÃO entram na grade diária do write-back.
    #[tokio::test]
    async fn write_back_txns_transfer_excluded() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 0).await;
        let reserve_id = reserve_account_id(&pool).await;
        insert_transfer_to(&pool, 90_000, "2026-04-15", &reserve_id, 0).await;
        // Uma income p/ garantir que o filtro não esvazia tudo por engano.
        insert_realized(&pool, "income", 100_000, "2026-04-05").await;

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert_eq!(out.len(), 1, "só a income entra; o transfer fica de fora");
        assert_eq!(out[0].kind, import::RowKind::Entrada);
    }

    // Filtro de ano: income de 2025 não aparece ao pedir 2026.
    #[tokio::test]
    async fn write_back_txns_wrong_year_excluded() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "income", 500_000, "2025-12-31").await;

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert!(out.is_empty(), "transação de 2025 não entra no ano 2026");
    }

    // --- Plano 028: gates de segurança do write-back -----------------------------------------

    async fn seed_unresolved_conflict(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value) \
             VALUES ('c-test', 't-test', 'amount', '100', '150', '200')",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    // Step 3: com um conflito de import PENDENTE, o gate bloqueia o write-back ANTES de tocar o
    // cliente do Sheets (o teste exercita o guard que o apply chama primeiro).
    #[tokio::test]
    async fn write_back_blocked_when_import_conflicts_pending() {
        let pool = fixture_pool().await;
        assert_eq!(unresolved_conflict_count(&pool).await.unwrap(), 0);
        // Sem conflitos → o guard passa.
        assert!(guard_no_pending_conflicts(&pool).await.is_ok());

        seed_unresolved_conflict(&pool).await;
        assert_eq!(unresolved_conflict_count(&pool).await.unwrap(), 1);
        let err = guard_no_pending_conflicts(&pool).await.unwrap_err();
        assert_eq!(err, CONFLICTS_PENDING_MSG);
    }

    // Step 3: um conflito já RESOLVIDO não bloqueia (só os com resolved_at NULL contam).
    #[tokio::test]
    async fn write_back_not_blocked_by_resolved_conflict() {
        let pool = fixture_pool().await;
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, resolved_at) \
             VALUES ('c-done', 't', 'amount', '1', '2', '3', '2026-06-20')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(unresolved_conflict_count(&pool).await.unwrap(), 0);
        assert!(guard_no_pending_conflicts(&pool).await.is_ok());
    }

    // Step 4: se o modifiedTime AVANÇOU entre a prévia e o apply, a re-verificação aborta (e o apply
    // não escreve nada). Sem revisão (None) ou revisão idêntica → segue.
    #[test]
    fn staleness_aborts_when_sheet_modified_since_preview() {
        // Igual → ok (planilha intacta desde a prévia).
        assert!(staleness_check("2026-06-20T10:00:00Z", "2026-06-20T10:00:00Z").is_ok());
        // Avançou → erro de re-revisão (nada será escrito).
        let err = staleness_check("2026-06-20T10:00:00Z", "2026-06-20T10:05:00Z").unwrap_err();
        assert_eq!(err, SHEET_CHANGED_MSG);
    }

    // Step 8: dois cartões com ciclo completo (closing+due) → aviso não-bloqueante ligado.
    #[tokio::test]
    async fn multi_card_warning_set_with_two_cycle_cards() {
        let pool = fixture_pool().await;
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        let insert_card = |closing: Option<i64>, due: Option<i64>| {
            let pid = pid.clone();
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
                     VALUES (?1, 'Cartão', 'credit_card', ?2, ?3, ?4)",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&pid)
                .bind(closing)
                .bind(due)
                .execute(&pool)
                .await
                .unwrap();
            }
        };

        // Nenhum cartão → sem aviso.
        assert!(!multi_card_warning(&pool).await.unwrap());
        // Um cartão completo → ainda sem aviso (caso suportado).
        insert_card(Some(10), Some(20)).await;
        assert!(!multi_card_warning(&pool).await.unwrap());
        // Segundo cartão completo → aviso LIGADO (mais de um ciclo).
        insert_card(Some(5), Some(15)).await;
        assert!(multi_card_warning(&pool).await.unwrap());
    }

    // Step 8: um cartão SEM dias de ciclo também liga o aviso (a data da fatura é ambígua).
    #[tokio::test]
    async fn multi_card_warning_set_with_card_missing_cycle() {
        let pool = fixture_pool().await;
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES (?1, 'Cartão sem ciclo', 'credit_card', ?2, NULL, NULL)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();
        assert!(multi_card_warning(&pool).await.unwrap());
    }

    // Step 7: round-trip. O write-back grava o valor LOCAL na planilha; a auditoria realinha a BASE
    // (source_amount) a esse valor. Assim, uma edição local POSTERIOR não vira conflito espúrio: a
    // planilha guarda um valor que o PRÓPRIO app pôs lá (não uma edição independente do dono).
    //
    // Cenário: import (base=1000) → escrita do app leva a planilha a 1500 + auditoria (base→1500) →
    // dono edita local de novo para 1800 → re-import com a planilha ainda em 1500. Como base==sheet
    // (1500) e só o local mudou → KeepLocal, SEM conflito.
    #[tokio::test]
    async fn write_back_audit_prevents_spurious_conflict_on_reimport() {
        use google_sheets::import::{self, ImportedRow, RowKind};

        let pool = fixture_pool().await;
        let mk = |amount: i64| ImportedRow {
            date: "2026-03-01".into(),
            amount,
            description: "Salário".into(),
            is_projection: false,
            kind: RowKind::Entrada,
            raw_note: String::new(),
        };

        // 1) Import inicial: base (source_amount) = 1000; o app exibe 1000.
        import::import_rows(&pool, "2026", &[mk(1000)], "p-test")
            .await
            .unwrap();

        // 2) O dono ajusta para 1500 no app; o write-back grava 1500 na planilha. Auditoria realinha
        //    a base para 1500.
        sqlx::query("UPDATE \"transaction\" SET amount = 1500 WHERE date = '2026-03-01'")
            .execute(&pool)
            .await
            .unwrap();
        let cell = CellWrite {
            a1: "C5".into(),
            row: 4,
            col: 2,
            date: "2026-03-01".into(),
            kind: "entrada".into(),
            current: "1000,00".into(),
            proposed: "1500,00".into(),
            value_cents: 1500,
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&pool, "2026", &[&cell])
            .await
            .unwrap();
        assert_eq!(realigned, 1, "uma linha de income realinhada");

        // 3) Edição local POSTERIOR para 1800 (planilha continua 1500, posta pelo app).
        sqlx::query("UPDATE \"transaction\" SET amount = 1800 WHERE date = '2026-03-01'")
            .execute(&pool)
            .await
            .unwrap();

        // 4) Re-import com a planilha em 1500: base(1500)==sheet(1500), só o local mudou → KeepLocal,
        //    SEM conflito espúrio.
        import::import_rows(&pool, "2026", &[mk(1500)], "p-test")
            .await
            .unwrap();
        let (conflicts,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(conflicts, 0, "a base realinhada evita o conflito espúrio");

        // E uma trilha de auditoria foi gravada no sync_log com event_type write_back.
        let (audit,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sync_log WHERE event_type = 'write_back'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(audit, 1, "uma linha de auditoria por célula escrita");
    }

    // Step 7 (controle): SEM a auditoria que realinha a base, o MESMO cenário produz o conflito
    // espúrio — prova de que o realinho é o que o evita.
    #[tokio::test]
    async fn reimport_without_audit_would_conflict() {
        use google_sheets::import::{self, ImportedRow, RowKind};

        let pool = fixture_pool().await;
        let mk = |amount: i64| ImportedRow {
            date: "2026-03-01".into(),
            amount,
            description: "Salário".into(),
            is_projection: false,
            kind: RowKind::Entrada,
            raw_note: String::new(),
        };
        // base=1000; o app vai a 1500 (escrita) MAS sem realinhar a base; depois local→1800.
        import::import_rows(&pool, "2026", &[mk(1000)], "p-test")
            .await
            .unwrap();
        sqlx::query("UPDATE \"transaction\" SET amount = 1800 WHERE date = '2026-03-01'")
            .execute(&pool)
            .await
            .unwrap();
        // Re-import com a planilha em 1500 (posta pela escrita): base(1000) != sheet(1500) e
        // base(1000) != local(1800), e sheet != local → CONFLITO (que a auditoria evitaria).
        import::import_rows(&pool, "2026", &[mk(1500)], "p-test")
            .await
            .unwrap();
        let (conflicts,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(conflicts, 1, "sem realinhar a base, o re-import conflita");
    }

    // --- Plan 032 regression tests --------------------------------------------------------

    // Bug A: linhas Saída/Diário com payment_method NULL (o caso manual normal) DEVEM ser
    // realinhadas pela auditoria de write-back. Antes: `NOT (payment_method = 'credit')` virava
    // NULL em SQLite (NULL = 'credit' → NULL → NOT NULL → NULL), então a linha era EXCLUÍDA e a
    // base nunca era atualizada → conflito espúrio a cada write-back. Agora: NULL passa.
    #[tokio::test]
    async fn write_back_audit_realigns_null_payment_method() {
        let pool = fixture_pool().await;
        // Saída fixa REAL com payment_method NÃO informado (NULL) — o lançamento manual padrão.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1,'expense',50000,'2026-03-10',1,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "B10".into(),
            row: 9,
            col: 1,
            date: "2026-03-10".into(),
            kind: "saida".into(),
            current: "500,00".into(),
            proposed: "550,00".into(),
            value_cents: 55000,
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&pool, "2026", &[&cell])
            .await
            .unwrap();
        assert_eq!(realigned, 1, "a linha com payment_method NULL é realinhada");

        let (source_amount,): (Option<i64>,) = sqlx::query_as(
            "SELECT source_amount FROM \"transaction\" WHERE date='2026-03-10' AND type='expense'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            source_amount,
            Some(55000),
            "a base foi realinhada para o valor escrito"
        );
    }

    // Bug C: as Entradas compensatórias DERIVADAS (#reembolso/#dividir, id `derived:%`) NÃO podem
    // entrar na carga do write-back — senão a agregação infla a célula Entrada da planilha. A query
    // de load passa a excluir `id LIKE 'derived:%'`.
    #[tokio::test]
    async fn load_write_back_txns_excludes_derived() {
        let pool = fixture_pool().await;
        // Entrada real.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('t-real','income',100000,'2026-03-05',0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Entrada compensatória derivada (não deve ser somada no write-back).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('derived:reembolso:t-real','income',5000,'2026-03-05',0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let out = load_write_back_txns(&pool, 2026).await.unwrap();
        assert_eq!(
            out.len(),
            1,
            "apenas a transação real entra (a derivada sai)"
        );
        assert_eq!(out[0].kind, import::RowKind::Entrada);
        assert_eq!(out[0].amount_cents, 100000);
    }

    // Bug D: em 1º de JANEIRO o guardrail de poupança não pode ficar mudo. A janela
    // `[ano-01-01, mês-corrente-01)` é vazia nesse dia; o fix desloca para DEZEMBRO do ano anterior,
    // mantendo o guardrail ATIVO com base no último mês completo.
    #[tokio::test]
    async fn realized_annual_savings_active_on_jan_1() {
        let pool = fixture_pool().await;
        // Dezembro do ano anterior (mês completo): renda 120.000, despesa 80.000 → poupança 40.000.
        insert_realized(&pool, "income", 120000, "2025-12-10").await;
        insert_realized(&pool, "expense", 80000, "2025-12-15").await;

        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let (income, savings) = realized_annual_savings(&pool, today).await.unwrap();
        assert_eq!(
            income, 120000,
            "renda de dezembro alimenta o guardrail em 1º/jan"
        );
        assert_eq!(
            savings, 40000,
            "poupança = 120.000 − 80.000, guardrail ATIVO"
        );
    }

    // Bug D (borda): 1º de janeiro SEM dado do dezembro anterior → fallback seguro (0, 0), sem
    // pânico e sem janela quebrada.
    #[tokio::test]
    async fn realized_annual_savings_jan_1_no_prior_data() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let (income, savings) = realized_annual_savings(&pool, today).await.unwrap();
        assert_eq!(
            income, 0,
            "sem dezembro anterior, renda = 0 (fallback seguro)"
        );
        assert_eq!(
            savings, 0,
            "sem dezembro anterior, poupança = 0 (fallback seguro)"
        );
    }

    // P2a: a auditoria de write-back trata o kind `economia` — realinha a base da linha mensal
    // `economia:YYYY-MM` E grava a trilha no sync_log. Antes a Economia caía em `_ => continue`
    // (nenhum realinho, nenhuma trilha). Garante que `apply_economia_write_back` audita de fato.
    #[tokio::test]
    async fn write_back_audit_handles_economia_kind() {
        let pool = fixture_pool().await;
        // Um profile (com person) é pré-requisito para a FK do sync_log.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-econ', 'Tester')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES ('p-econ', 'pe-econ')")
            .execute(&pool)
            .await
            .unwrap();
        // Linha mensal de Economia (transfer) com a base antiga.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection, source_amount) \
             VALUES ('economia:2026-01','transfer',30000,'2026-01-31',0,30000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let cell = CellWrite {
            a1: "E5".into(),
            row: 4,
            col: 4,
            date: "2026-01".into(), // célula de Economia carrega "YYYY-MM"
            kind: "economia".into(),
            current: "300,00".into(),
            proposed: "350,00".into(),
            value_cents: 35000,
            changed: true,
            formula: None,
            note_text: None,
        };
        let realigned = record_write_back_audit(&pool, "Economia", &[&cell])
            .await
            .unwrap();
        assert_eq!(realigned, 1, "a linha mensal de Economia é realinhada");

        let (source_amount,): (Option<i64>,) =
            sqlx::query_as("SELECT source_amount FROM \"transaction\" WHERE id='economia:2026-01'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(source_amount, Some(35000), "base da Economia realinhada");

        let (audit,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sync_log WHERE event_type='write_back' AND source_sheet='Economia'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit, 1, "trilha de auditoria gravada para a Economia");
    }

    // --- Plan 034: tag "Ignorar nos cálculos" ---

    /// Insere um lançamento com id conhecido (para anexar tag depois).
    async fn insert_realized_id(
        pool: &sqlx::SqlitePool,
        id: &str,
        ttype: &str,
        amount: i64,
        date: &str,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES (?1,?2,?3,?4,0)",
        )
        .bind(id)
        .bind(ttype)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    // Uma saída marcada com tag excluída sai do Custo de vida e da Performance do mês; uma saída
    // sem tag permanece. Exercita o filtro de `load_realized_month_events`/`load_year_events`.
    #[tokio::test]
    async fn excluded_tag_drops_expense_from_performance() {
        let pool = fixture_pool().await;
        // Março realizado (hoje = junho): renda 500, duas saídas de 100.
        insert_realized(&pool, "income", 500_000, "2026-03-05").await;
        insert_realized_id(&pool, "exp-ignored", "expense", 100_000, "2026-03-10").await;
        insert_realized_id(&pool, "exp-counted", "expense", 100_000, "2026-03-12").await;

        // Tag "Reembolso" marcada para ignorar; anexada só à saída ignorada.
        let tag = crate::tags::create_tag(&pool, "Reembolso", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_exclude(&pool, &tag, true)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&pool, "exp-ignored", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let a = annual_metrics(&pool, 2026, today).await.unwrap();
        let mar = a.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.income_cents, 500_000, "renda intacta");
        assert_eq!(
            mar.cost_of_living_cents, 100_000,
            "só a saída sem tag conta no Custo de vida"
        );
        assert_eq!(
            mar.performance_cents, 400_000,
            "Performance = 500 − 100 (a saída ignorada não entra)"
        );
    }

    // Um transfer→reserva marcado com tag excluída NÃO infla o Economizado; um sem tag conta.
    // Exercita o filtro de `realized_annual_economia`.
    #[tokio::test]
    async fn excluded_tag_drops_transfer_from_economizado() {
        let pool = fixture_pool().await;
        let pid: (String,) = {
            insert_liquid_account(&pool, 0).await;
            sqlx::query_as("SELECT id FROM person LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap()
        };
        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Reserva','savings',?2,0,'reserve')")
            .bind(&reserve_id).bind(&pid.0).execute(&pool).await.unwrap();

        // Dois transfers→reserva (mês completo, hoje = junho): 50k ignorado, 30k contado.
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES ('tr-ignored','transfer',50_000,'2026-03-20',?1,0)")
            .bind(&reserve_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES ('tr-counted','transfer',30_000,'2026-03-21',?1,0)")
            .bind(&reserve_id).execute(&pool).await.unwrap();

        let tag = crate::tags::create_tag(&pool, "Ignorar", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_exclude(&pool, &tag, true)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&pool, "tr-ignored", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let economia = realized_annual_economia(&pool, today).await.unwrap();
        assert_eq!(
            economia, 30_000,
            "só o transfer→reserva sem tag conta no Economizado (não 80_000)"
        );
    }

    // Regressão: a exclusão NÃO afeta a cadeia do Saldo — o movimento de caixa real permanece.
    // A semente da projeção (`projection_seed`) soma a saída ignorada ao gap do Saldo da planilha,
    // exatamente como uma saída qualquer; só as métricas derivadas a omitem.
    #[tokio::test]
    async fn excluded_tag_does_not_affect_saldo_chain() {
        let pool = fixture_pool().await;
        // Saldo da planilha de ontem = R$ 1.000,00; hoje = 2026-06-13.
        insert_sheet_balance(&pool, "2026", "2026-06-12", 100_000).await;
        // Saída de R$ 200,00 ENTRE o Saldo e hoje, marcada para ignorar nas métricas.
        insert_realized_id(&pool, "exp-ignored", "expense", 20_000, "2026-06-13").await;

        let tag = crate::tags::create_tag(&pool, "Ignorar", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_exclude(&pool, &tag, true)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&pool, "exp-ignored", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let seed = projection_seed(&pool, today).await.unwrap();
        assert_eq!(
            seed, 80_000,
            "o Saldo cai pela saída ignorada (100k − 20k): caixa real intacto"
        );

        // E confirma que a métrica do mesmo mês A ignora.
        let a = annual_metrics(&pool, 2026, today).await.unwrap();
        let jun = a.months.iter().find(|m| m.month == 6).unwrap();
        assert_eq!(
            jun.cost_of_living_cents, 0,
            "a saída ignorada não conta no Custo de vida (só no Saldo)"
        );
    }
}
