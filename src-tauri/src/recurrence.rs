//! Recorrências/séries: gera N ocorrências (transações) compartilhando um
//! `recurrence_id`; permite apagar "deste ponto" ou "toda a série". Core de datas puro +
//! shell determinístico.

use crate::calendar::last_day_of_month;
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
    if t.amount <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    // Limite superior além do `< 1`: um `repetitions` enorme inseriria N linhas e poderia estourar
    // a aritmética de datas (add_months) no caminho de escrita financeira. 600 = 50 anos mensais.
    if !(1..=600).contains(&repetitions) {
        return Err("repetições deve estar entre 1 e 600".into());
    }
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

/// Índice da ocorrência embutido no id `{rec_id}:{i}`. Cortar "deste ponto em diante" pelo ÍNDICE
/// (não pela data) é order-independent — robusto se uma janela rolante futura gerar datas não
/// estritamente crescentes, onde `date >= pivot` apagaria/editaria ocorrências erradas.
pub(crate) fn occurrence_index(transaction_id: &str) -> Option<i64> {
    transaction_id
        .rsplit_once(':')
        .and_then(|(_, i)| i.parse().ok())
}

/// Apaga a ocorrência indicada e TODAS as posteriores da mesma série ("deste ponto em diante").
///
/// Toda mutação de série filtra `scenario_id IS NULL`: uma edição do livro-razão REAL nunca pode
/// alcançar linhas hipotéticas de cenário, mesmo que elas reutilizem `recurrence_id`.
pub async fn delete_series_from(pool: &SqlitePool, transaction_id: &str) -> Result<u64, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT recurrence_id FROM \"transaction\" \
         WHERE id = ?1 AND recurrence_id IS NOT NULL AND scenario_id IS NULL",
    )
    .bind(transaction_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("lookup: {e}"))?;
    let (Some((rec_id,)), Some(pivot)) = (row, occurrence_index(transaction_id)) else {
        return Ok(0);
    };
    // `substr(id, length(recurrence_id) + 2)` = parte após o ':' → o índice como inteiro.
    let res = sqlx::query(
        "DELETE FROM \"transaction\" WHERE recurrence_id = ?1 AND scenario_id IS NULL \
         AND CAST(substr(id, length(recurrence_id) + 2) AS INTEGER) >= ?2",
    )
    .bind(&rec_id)
    .bind(pivot)
    .execute(pool)
    .await
    .map_err(|e| format!("delete from: {e}"))?;
    Ok(res.rows_affected())
}

/// Campos editáveis de uma série (o tipo e o calendário permanecem; muda só o que varia ao
/// reajustar uma recorrência — ex.: aluguel subiu).
#[derive(Debug, Clone)]
pub struct SeriesEdit {
    pub amount: i64,
    pub description: Option<String>,
    pub payment_method: Option<String>,
    pub is_fixed: bool,
}

/// Edita a ocorrência indicada e TODAS as posteriores ("deste ponto em diante") — o passado
/// realizado fica intacto.
pub async fn update_series_from(
    pool: &SqlitePool,
    transaction_id: &str,
    edit: &SeriesEdit,
) -> Result<u64, String> {
    if edit.amount <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT recurrence_id FROM \"transaction\" \
         WHERE id = ?1 AND recurrence_id IS NOT NULL AND scenario_id IS NULL",
    )
    .bind(transaction_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("lookup: {e}"))?;
    let (Some((rec_id,)), Some(pivot)) = (row, occurrence_index(transaction_id)) else {
        return Ok(0);
    };
    let now = chrono::Utc::now().to_rfc3339();
    // Corte por índice (ver `occurrence_index`), não por data.
    let res = sqlx::query(
        "UPDATE \"transaction\" SET amount = ?1, description = ?2, payment_method = ?3, \
         is_fixed = ?4, updated_at = ?5 WHERE recurrence_id = ?6 AND scenario_id IS NULL \
         AND CAST(substr(id, length(recurrence_id) + 2) AS INTEGER) >= ?7",
    )
    .bind(edit.amount)
    .bind(&edit.description)
    .bind(&edit.payment_method)
    .bind(edit.is_fixed as i64)
    .bind(&now)
    .bind(&rec_id)
    .bind(pivot)
    .execute(pool)
    .await
    .map_err(|e| format!("update from: {e}"))?;
    Ok(res.rows_affected())
}

/// Edita TODA a série de uma vez.
pub async fn update_series_all(
    pool: &SqlitePool,
    recurrence_id: &str,
    edit: &SeriesEdit,
) -> Result<u64, String> {
    if edit.amount <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE \"transaction\" SET amount = ?1, description = ?2, payment_method = ?3, \
         is_fixed = ?4, updated_at = ?5 WHERE recurrence_id = ?6 AND scenario_id IS NULL",
    )
    .bind(edit.amount)
    .bind(&edit.description)
    .bind(&edit.payment_method)
    .bind(edit.is_fixed as i64)
    .bind(&now)
    .bind(recurrence_id)
    .execute(pool)
    .await
    .map_err(|e| format!("update all: {e}"))?;
    Ok(res.rows_affected())
}

/// Apaga TODA a série + a linha `recurrence`. Os dois DELETEs correm numa única transação: senão
/// uma falha entre eles deixaria a linha `recurrence` órfã (sem ocorrências) ou vice-versa.
pub async fn delete_series_all(pool: &SqlitePool, recurrence_id: &str) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let res =
        sqlx::query("DELETE FROM \"transaction\" WHERE recurrence_id = ?1 AND scenario_id IS NULL")
            .bind(recurrence_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete all: {e}"))?;
    sqlx::query("DELETE FROM recurrence WHERE id = ?1")
        .bind(recurrence_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete recurrence: {e}"))?;
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(res.rows_affected())
}

// --- Tauri command wrappers ---

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

#[tauri::command]
pub async fn update_series_from_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
    amount: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
) -> Result<u64, String> {
    let edit = SeriesEdit {
        amount,
        description,
        payment_method,
        is_fixed,
    };
    update_series_from(pool.inner(), &transaction_id, &edit).await
}

#[tauri::command]
pub async fn update_series_all_cmd(
    pool: State<'_, SqlitePool>,
    recurrence_id: String,
    amount: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
) -> Result<u64, String> {
    let edit = SeriesEdit {
        amount,
        description,
        payment_method,
        is_fixed,
    };
    update_series_all(pool.inner(), &recurrence_id, &edit).await
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

    async fn amount_at(pool: &SqlitePool, txn_id: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT amount FROM \"transaction\" WHERE id = ?1")
            .bind(txn_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn update_from_changes_this_and_later_only() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Mensal, 4)
            .await
            .unwrap();
        let edit = SeriesEdit {
            amount: 550000,
            description: Some("Salário reajustado".into()),
            payment_method: None,
            is_fixed: false,
        };
        // Reajuste a partir da 3ª ocorrência (índice 2).
        let n = update_series_from(&p, &format!("{rec}:2"), &edit)
            .await
            .unwrap();
        assert_eq!(n, 2, "agosto + setembro reajustados");
        assert_eq!(
            amount_at(&p, &format!("{rec}:0")).await,
            500000,
            "junho intacto"
        );
        assert_eq!(
            amount_at(&p, &format!("{rec}:1")).await,
            500000,
            "julho intacto"
        );
        assert_eq!(
            amount_at(&p, &format!("{rec}:2")).await,
            550000,
            "agosto reajustado"
        );
        assert_eq!(
            amount_at(&p, &format!("{rec}:3")).await,
            550000,
            "setembro reajustado"
        );
    }

    // O corte "deste ponto" é por ÍNDICE, não por data. Prova com
    // (a) uma data fora de ordem (índice 1 movido para o futuro) e (b) índice de 2 dígitos — onde
    // o corte por data ou por string quebraria.
    #[tokio::test]
    async fn delete_from_cuts_by_index_not_date_even_out_of_order() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Mensal, 12)
            .await
            .unwrap();
        // Move o índice 1 para DEPOIS do pivô (índice 10): data-based apagaria o índice 1 junto.
        sqlx::query("UPDATE \"transaction\" SET date = '2030-01-01' WHERE id = ?1")
            .bind(format!("{rec}:1"))
            .execute(&p)
            .await
            .unwrap();
        // Corta a partir do índice 10 (2 dígitos): só 10 e 11 saem.
        let removed = delete_series_from(&p, &format!("{rec}:10")).await.unwrap();
        assert_eq!(removed, 2, "só índices 10 e 11 (por índice, não por data)");
        assert_eq!(count_in_series(&p, &rec).await, 10);
        // O índice 1, apesar da data futura, permanece — o corte é por índice.
        assert_eq!(
            amount_at(&p, &format!("{rec}:1")).await,
            500000,
            "índice 1 intacto"
        );
    }

    #[tokio::test]
    async fn update_all_changes_whole_series() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Semanal, 3)
            .await
            .unwrap();
        let edit = SeriesEdit {
            amount: 480000,
            description: Some("Salário".into()),
            payment_method: None,
            is_fixed: false,
        };
        let n = update_series_all(&p, &rec, &edit).await.unwrap();
        assert_eq!(n, 3);
        for i in 0..3 {
            assert_eq!(amount_at(&p, &format!("{rec}:{i}")).await, 480000);
        }
    }

    #[tokio::test]
    async fn rejects_repetitions_out_of_bounds() {
        let p = pool().await;
        assert!(
            create_recurring_series(&p, &tmpl(), Frequency::Mensal, 0)
                .await
                .is_err()
        );
        assert!(
            create_recurring_series(&p, &tmpl(), Frequency::Mensal, 601)
                .await
                .is_err()
        );
        assert!(
            create_recurring_series(&p, &tmpl(), Frequency::Mensal, 600)
                .await
                .is_ok()
        );
    }

    async fn count_all_transactions(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    async fn count_all_recurrences(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM recurrence")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_recurring_series_rejects_non_positive_amount_and_inserts_nothing() {
        let p = pool().await;
        let mut t = tmpl();
        t.amount = 0;
        assert!(
            create_recurring_series(&p, &t, Frequency::Mensal, 3)
                .await
                .is_err()
        );
        assert_eq!(count_all_recurrences(&p).await, 0, "nenhuma série criada");
        assert_eq!(
            count_all_transactions(&p).await,
            0,
            "nenhuma ocorrência inserida"
        );
    }

    #[tokio::test]
    async fn update_series_from_rejects_non_positive_amount_and_leaves_series_intact() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Mensal, 3)
            .await
            .unwrap();
        let bad_edit = SeriesEdit {
            amount: -100,
            description: Some("Inválido".into()),
            payment_method: None,
            is_fixed: false,
        };
        assert!(
            update_series_from(&p, &format!("{rec}:0"), &bad_edit)
                .await
                .is_err()
        );
        for i in 0..3 {
            assert_eq!(amount_at(&p, &format!("{rec}:{i}")).await, 500000);
        }
    }

    #[tokio::test]
    async fn update_series_all_rejects_non_positive_amount() {
        let p = pool().await;
        let rec = create_recurring_series(&p, &tmpl(), Frequency::Semanal, 3)
            .await
            .unwrap();
        let bad_edit = SeriesEdit {
            amount: 0,
            description: Some("Inválido".into()),
            payment_method: None,
            is_fixed: false,
        };
        assert!(update_series_all(&p, &rec, &bad_edit).await.is_err());
        for i in 0..3 {
            assert_eq!(amount_at(&p, &format!("{rec}:{i}")).await, 500000);
        }
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
