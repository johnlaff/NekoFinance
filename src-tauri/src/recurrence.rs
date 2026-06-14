//! Recorrências/séries (spec 016): gera N ocorrências (transações) compartilhando um
//! `recurrence_id`; permite apagar "deste ponto" ou "toda a série". Core de datas puro +
//! shell determinístico.

use crate::forecast::last_day_of_month;
use chrono::{Datelike, Duration, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Diaria,
    Semanal,
    Mensal,
}

impl Frequency {
    pub fn as_str(self) -> &'static str {
        match self {
            Frequency::Diaria => "diaria",
            Frequency::Semanal => "semanal",
            Frequency::Mensal => "mensal",
        }
    }
    pub fn parse(s: &str) -> Option<Frequency> {
        match s {
            "diaria" => Some(Frequency::Diaria),
            "semanal" => Some(Frequency::Semanal),
            "mensal" => Some(Frequency::Mensal),
            _ => None,
        }
    }
}

/// Soma `n` meses a `d`, fixando o dia ao último válido do mês de destino (31/jan + 1 mês = 28/fev).
pub fn add_months(d: NaiveDate, n: u32) -> NaiveDate {
    let total = d.month0() + n;
    let year = d.year() + (total / 12) as i32;
    let month = total % 12 + 1;
    let day = d.day().min(last_day_of_month(year, month).day());
    NaiveDate::from_ymd_opt(year, month, day).expect("valid recurring date")
}

/// As datas das `count` ocorrências a partir de `start`, conforme a frequência.
pub fn occurrence_dates(start: NaiveDate, freq: Frequency, count: usize) -> Vec<NaiveDate> {
    (0..count)
        .map(|i| match freq {
            Frequency::Diaria => start + Duration::days(i as i64),
            Frequency::Semanal => start + Duration::weeks(i as i64),
            Frequency::Mensal => add_months(start, i as u32),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct RecurringTemplate {
    pub txn_type: String,
    pub amount: i64,
    pub description: Option<String>,
    pub start: NaiveDate,
    pub payment_method: Option<String>,
    pub is_fixed: bool,
}

/// Cria uma série: 1 linha `recurrence` + N transações projetadas compartilhando o `recurrence_id`.
pub async fn create_recurring_series(
    pool: &SqlitePool,
    t: &RecurringTemplate,
    freq: Frequency,
    repetitions: usize,
) -> Result<String, String> {
    let rec_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;

    sqlx::query(
        "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
         VALUES (?1, ?2, 0, ?3, ?4)",
    )
    .bind(&rec_id)
    .bind(freq.as_str())
    .bind(repetitions as i64)
    .bind(t.start.format("%Y-%m-%d").to_string())
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("insert recurrence: {e}"))?;

    for (i, date) in occurrence_dates(t.start, freq, repetitions)
        .into_iter()
        .enumerate()
    {
        let txn_id = format!("{rec_id}:{i}");
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, payment_method, is_fixed, is_projection, recurrence_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9)",
        )
        .bind(&txn_id)
        .bind(&t.txn_type)
        .bind(t.amount)
        .bind(&t.description)
        .bind(date.format("%Y-%m-%d").to_string())
        .bind(&t.payment_method)
        .bind(t.is_fixed as i64)
        .bind(&rec_id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert occurrence: {e}"))?;
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(rec_id)
}

/// Apaga a ocorrência indicada e TODAS as posteriores da mesma série ("deste ponto em diante").
pub async fn delete_series_from(pool: &SqlitePool, transaction_id: &str) -> Result<u64, String> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT recurrence_id, date FROM \"transaction\" WHERE id = ?1 AND recurrence_id IS NOT NULL",
    )
    .bind(transaction_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("lookup: {e}"))?;
    let (rec_id, date) = match row {
        Some(v) => v,
        None => return Ok(0),
    };
    let res = sqlx::query("DELETE FROM \"transaction\" WHERE recurrence_id = ?1 AND date >= ?2")
        .bind(&rec_id)
        .bind(&date)
        .execute(pool)
        .await
        .map_err(|e| format!("delete from: {e}"))?;
    Ok(res.rows_affected())
}

/// Apaga TODA a série + a linha `recurrence`.
pub async fn delete_series_all(pool: &SqlitePool, recurrence_id: &str) -> Result<u64, String> {
    let res = sqlx::query("DELETE FROM \"transaction\" WHERE recurrence_id = ?1")
        .bind(recurrence_id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete all: {e}"))?;
    sqlx::query("DELETE FROM recurrence WHERE id = ?1")
        .bind(recurrence_id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete recurrence: {e}"))?;
    Ok(res.rows_affected())
}

// --- Tauri command wrappers ---

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_recurring_series_cmd(
    pool: State<'_, SqlitePool>,
    txn_type: String,
    amount: i64,
    description: Option<String>,
    start: String,
    payment_method: Option<String>,
    is_fixed: bool,
    frequency: String,
    repetitions: usize,
) -> Result<String, String> {
    let freq = Frequency::parse(&frequency).ok_or("frequência inválida")?;
    let start = NaiveDate::parse_from_str(&start, "%Y-%m-%d").map_err(|e| format!("data: {e}"))?;
    let t = RecurringTemplate {
        txn_type,
        amount,
        description,
        start,
        payment_method,
        is_fixed,
    };
    create_recurring_series(pool.inner(), &t, freq, repetitions).await
}

#[tauri::command]
pub async fn delete_series_from_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
) -> Result<u64, String> {
    delete_series_from(pool.inner(), &transaction_id).await
}

#[tauri::command]
pub async fn delete_series_all_cmd(
    pool: State<'_, SqlitePool>,
    recurrence_id: String,
) -> Result<u64, String> {
    delete_series_all(pool.inner(), &recurrence_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn occurrence_dates_by_frequency() {
        assert_eq!(
            occurrence_dates(d("2026-06-10"), Frequency::Diaria, 3),
            vec![d("2026-06-10"), d("2026-06-11"), d("2026-06-12")]
        );
        assert_eq!(
            occurrence_dates(d("2026-06-10"), Frequency::Semanal, 3),
            vec![d("2026-06-10"), d("2026-06-17"), d("2026-06-24")]
        );
        // Mensal com clamp do dia 31 → 28/fev.
        assert_eq!(
            occurrence_dates(d("2026-01-31"), Frequency::Mensal, 3),
            vec![d("2026-01-31"), d("2026-02-28"), d("2026-03-31")]
        );
    }

    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    fn tmpl() -> RecurringTemplate {
        RecurringTemplate {
            txn_type: "income".into(),
            amount: 500000,
            description: Some("Salário".into()),
            start: d("2026-06-05"),
            payment_method: None,
            is_fixed: false,
        }
    }

    async fn count_in_series(pool: &SqlitePool, rec_id: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM \"transaction\" WHERE recurrence_id = ?1")
            .bind(rec_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_then_delete_from_keeps_earlier() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Mensal, 4)
            .await
            .unwrap();
        assert_eq!(count_in_series(&p, &rec).await, 4); // jun, jul, ago, set

        // Apaga "deste ponto" a partir da 3ª ocorrência (índice 2 = agosto).
        let removed = delete_series_from(&p, &format!("{rec}:2")).await.unwrap();
        assert_eq!(removed, 2, "agosto + setembro removidos");
        assert_eq!(count_in_series(&p, &rec).await, 2, "jun + jul permanecem");
    }

    #[tokio::test]
    async fn delete_all_removes_series_and_recurrence_row() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Semanal, 3)
            .await
            .unwrap();
        let removed = delete_series_all(&p, &rec).await.unwrap();
        assert_eq!(removed, 3);
        assert_eq!(count_in_series(&p, &rec).await, 0);
        let recs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM recurrence WHERE id = ?1")
            .bind(&rec)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(recs.0, 0, "linha recurrence também removida");
    }
}
