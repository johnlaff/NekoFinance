//! Tags livres (spec 014): nome + cor, transversais a qualquer lançamento, somam por mês.
//! `emoji` e `is_special` (fixa a tag no topo) são afordâncias próprias do Neko, não do modelo de
//! tags do método. Funções puras-de-IO no shell; determinísticas e testáveis com um pool injetado.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct TagTotal {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
    /// Soma (em centavos, valor absoluto) dos lançamentos do mês com esta tag.
    pub total_cents: i64,
}

pub async fn create_tag(
    pool: &SqlitePool,
    name: &str,
    color: &str,
    emoji: Option<&str>,
    is_special: bool,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO tag (id, name, color, emoji, is_special) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(&id)
        .bind(name)
        .bind(color)
        .bind(emoji)
        .bind(is_special as i64)
        .execute(pool)
        .await
        .map_err(|e| format!("create_tag: {e}"))?;
    Ok(id)
}

pub async fn list_tags(pool: &SqlitePool) -> Result<Vec<Tag>, String> {
    sqlx::query_as::<_, Tag>(
        "SELECT id, name, color, emoji, is_special FROM tag \
         ORDER BY is_special DESC, name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_tags: {e}"))
}

/// Substitui (UPSERT do conjunto) as tags de um lançamento.
pub async fn set_transaction_tags(
    pool: &SqlitePool,
    transaction_id: &str,
    tag_ids: &[String],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    sqlx::query("DELETE FROM transaction_tag WHERE transaction_id = ?1")
        .bind(transaction_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("clear tags: {e}"))?;
    for tag_id in tag_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)",
        )
        .bind(transaction_id)
        .bind(tag_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("attach tag: {e}"))?;
    }
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(())
}

/// Total por tag no mês (`YYYY-MM`). Inclui tags sem lançamento (total 0). `is_special` no topo.
pub async fn tag_totals_for_month(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> Result<Vec<TagTotal>, String> {
    let ym = format!("{year:04}-{month:02}");
    sqlx::query_as::<_, TagTotal>(
        "SELECT t.id, t.name, t.color, t.emoji, t.is_special, \
                COALESCE(SUM(ABS(tr.amount)), 0) AS total_cents \
         FROM tag t \
         LEFT JOIN transaction_tag tt ON tt.tag_id = t.id \
         LEFT JOIN \"transaction\" tr ON tr.id = tt.transaction_id \
                AND substr(tr.date, 1, 7) = ?1 \
         GROUP BY t.id \
         ORDER BY t.is_special DESC, total_cents DESC, t.name COLLATE NOCASE",
    )
    .bind(&ym)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("tag_totals_for_month: {e}"))
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn create_tag_cmd(
    pool: State<'_, SqlitePool>,
    name: String,
    color: String,
    emoji: Option<String>,
    is_special: bool,
) -> Result<String, String> {
    create_tag(pool.inner(), &name, &color, emoji.as_deref(), is_special).await
}

#[tauri::command]
pub async fn list_tags_cmd(pool: State<'_, SqlitePool>) -> Result<Vec<Tag>, String> {
    list_tags(pool.inner()).await
}

#[tauri::command]
pub async fn set_transaction_tags_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
    tag_ids: Vec<String>,
) -> Result<(), String> {
    set_transaction_tags(pool.inner(), &transaction_id, &tag_ids).await
}

#[tauri::command]
pub async fn tag_totals_for_month_cmd(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<Vec<TagTotal>, String> {
    tag_totals_for_month(pool.inner(), year, month).await
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

    async fn insert_txn(pool: &SqlitePool, id: &str, ttype: &str, amount: i64, date: &str) {
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

    #[tokio::test]
    async fn create_list_orders_special_first() {
        let p = pool().await;
        create_tag(&p, "Viagem", "var(--cat-sky)", Some("✈️"), false)
            .await
            .unwrap();
        create_tag(&p, "! Pagar", "var(--brass-400)", None, true)
            .await
            .unwrap();
        let tags = list_tags(&p).await.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "! Pagar", "tag especial no topo");
        assert!(tags[0].is_special);
        assert_eq!(tags[1].emoji.as_deref(), Some("✈️"));
    }

    #[tokio::test]
    async fn set_tags_replaces_and_totals_sum_by_month() {
        let p = pool().await;
        let viagem = create_tag(&p, "Viagem", "c", None, false).await.unwrap();
        let delivery = create_tag(&p, "Delivery", "c", None, false).await.unwrap();

        insert_txn(&p, "t1", "expense", -10000, "2026-06-05").await;
        insert_txn(&p, "t2", "expense", -35000, "2026-06-10").await;
        insert_txn(&p, "t3", "expense", -99900, "2026-07-01").await; // outro mês

        set_transaction_tags(&p, "t1", std::slice::from_ref(&viagem))
            .await
            .unwrap();
        set_transaction_tags(&p, "t2", std::slice::from_ref(&delivery))
            .await
            .unwrap();
        set_transaction_tags(&p, "t3", std::slice::from_ref(&viagem))
            .await
            .unwrap();

        // Substituição: re-set de t1 troca a tag, não acumula.
        set_transaction_tags(&p, "t1", std::slice::from_ref(&delivery))
            .await
            .unwrap();

        let totals = tag_totals_for_month(&p, 2026, 6).await.unwrap();
        let get = |id: &str| totals.iter().find(|x| x.id == id).unwrap().total_cents;
        assert_eq!(get(&delivery), 10000 + 35000, "t1(troca)+t2 em junho");
        assert_eq!(get(&viagem), 0, "t1 deixou de ser Viagem; t3 é julho");
    }
}
