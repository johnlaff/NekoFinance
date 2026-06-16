//! Multi-titular (read-side do split, spec 017): quem é responsável/pagou cada parte de um
//! lançamento. Surge na UI como OwnerChip — suporte a múltiplos titulares (vários pagadores
//! por lançamento).

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct SplitRow {
    pub id: String,
    pub transaction_id: String,
    pub amount: i64,
    pub owner_person_id: String,
    pub owner_name: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct OwnerTotal {
    pub owner_person_id: String,
    pub owner_name: String,
    /// Soma (centavos, valor absoluto) das parcelas do titular nos lançamentos do mês.
    pub total_cents: i64,
}

/// Parcelas (com titular) de um lançamento.
pub async fn splits_for_transaction(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Vec<SplitRow>, String> {
    sqlx::query_as::<_, SplitRow>(
        "SELECT s.id, s.transaction_id, s.amount, s.owner_person_id, p.name AS owner_name, s.note \
         FROM split s JOIN person p ON p.id = s.owner_person_id \
         WHERE s.transaction_id = ?1 \
         ORDER BY p.name COLLATE NOCASE",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("splits_for_transaction: {e}"))
}

/// Quanto cada titular respondeu no mês (`YYYY-MM`) — a divisão de despesas do método.
pub async fn owner_totals_for_month(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> Result<Vec<OwnerTotal>, String> {
    let ym = format!("{year:04}-{month:02}");
    sqlx::query_as::<_, OwnerTotal>(
        "SELECT p.id AS owner_person_id, p.name AS owner_name, \
                COALESCE(SUM(ABS(s.amount)), 0) AS total_cents \
         FROM split s \
         JOIN person p ON p.id = s.owner_person_id \
         JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE substr(t.date, 1, 7) = ?1 \
         GROUP BY p.id \
         ORDER BY total_cents DESC, p.name COLLATE NOCASE",
    )
    .bind(&ym)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("owner_totals_for_month: {e}"))
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn splits_for_transaction_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
) -> Result<Vec<SplitRow>, String> {
    splits_for_transaction(pool.inner(), &transaction_id).await
}

#[tauri::command]
pub async fn owner_totals_for_month_cmd(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<Vec<OwnerTotal>, String> {
    owner_totals_for_month(pool.inner(), year, month).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn person(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn txn(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES (?1,'expense',?2,?3,0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn split(pool: &SqlitePool, id: &str, txn_id: &str, amount: i64, owner: &str) {
        sqlx::query(
            "INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1,?2,?3,?4)",
        )
        .bind(id)
        .bind(txn_id)
        .bind(amount)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn splits_carry_owner_name() {
        let p = pool().await;
        person(&p, "bruno", "Bruno").await;
        person(&p, "ana", "Ana").await;
        txn(&p, "t1", -30000, "2026-06-05").await;
        split(&p, "s1", "t1", -20000, "bruno").await;
        split(&p, "s2", "t1", -10000, "ana").await;

        let rows = splits_for_transaction(&p, "t1").await.unwrap();
        assert_eq!(rows.len(), 2);
        // Ordenado por nome: Ana antes de Bruno.
        assert_eq!(rows[0].owner_name, "Ana");
        assert_eq!(rows[1].owner_name, "Bruno");
    }

    #[tokio::test]
    async fn owner_totals_sum_by_person_and_month() {
        let p = pool().await;
        person(&p, "bruno", "Bruno").await;
        person(&p, "ana", "Ana").await;
        txn(&p, "t1", -30000, "2026-06-05").await;
        split(&p, "s1", "t1", -20000, "bruno").await;
        split(&p, "s2", "t1", -10000, "ana").await;
        txn(&p, "t2", -50000, "2026-06-20").await;
        split(&p, "s3", "t2", -50000, "bruno").await;
        txn(&p, "t3", -99900, "2026-07-01").await; // outro mês
        split(&p, "s4", "t3", -99900, "ana").await;

        let totals = owner_totals_for_month(&p, 2026, 6).await.unwrap();
        let get = |id: &str| {
            totals
                .iter()
                .find(|x| x.owner_person_id == id)
                .unwrap()
                .total_cents
        };
        assert_eq!(get("bruno"), 70000, "20 + 50 em junho");
        assert_eq!(get("ana"), 10000, "só 10 em junho (99,90 é julho)");
        // Bruno primeiro (maior total).
        assert_eq!(totals[0].owner_person_id, "bruno");
    }
}
