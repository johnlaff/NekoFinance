//! Tags livres: nome + cor, transversais a qualquer lançamento, somam por mês.
//! `emoji` e `is_special` (fixa a tag no topo) são afordâncias próprias do Neko, não do modelo de
//! tags do método. Funções puras-de-IO no shell; determinísticas e testáveis com um pool injetado.

use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
    /// Interruptores de contabilidade por régua: `true` tira os lançamentos desta tag SÓ da
    /// régua homônima (Performance · Custo de vida · Economia · Diário médio), e de nenhuma
    /// outra. O Saldo (cadeia de caixa) nunca tem máscara — dinheiro que entrou e saiu de
    /// verdade sempre conta.
    pub exclude_from_performance: bool,
    pub exclude_from_cost_of_living: bool,
    pub exclude_from_savings: bool,
    pub exclude_from_daily_avg: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct TagTotal {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
    /// Ver `Tag` — os quatro interruptores de régua.
    pub exclude_from_performance: bool,
    pub exclude_from_cost_of_living: bool,
    pub exclude_from_savings: bool,
    pub exclude_from_daily_avg: bool,
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
        "SELECT id, name, color, emoji, is_special, exclude_from_performance, \
                exclude_from_cost_of_living, exclude_from_savings, exclude_from_daily_avg FROM tag \
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
    set_transaction_tags_on_conn(&mut tx, transaction_id, tag_ids).await?;
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(())
}

/// Substitui as tags usando a transação do chamador para que um gesto composto persista tudo ou
/// nada. A validação de chave estrangeira dos IDs mantém a fronteira de tags explícita.
pub(crate) async fn set_transaction_tags_on_conn(
    conn: &mut SqliteConnection,
    transaction_id: &str,
    tag_ids: &[String],
) -> Result<(), String> {
    sqlx::query("DELETE FROM transaction_tag WHERE transaction_id = ?1")
        .bind(transaction_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("clear tags: {e}"))?;
    for tag_id in tag_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)",
        )
        .bind(transaction_id)
        .bind(tag_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("attach tag: {e}"))?;
    }
    Ok(())
}

/// Grava os quatro interruptores de régua de uma tag num único UPDATE (idempotente). Cada bool
/// liga a exclusão da régua homônima; nenhum toca no Saldo. Erro se a tag não existir.
pub async fn update_tag_rulers(
    pool: &SqlitePool,
    tag_id: &str,
    exclude_from_performance: bool,
    exclude_from_cost_of_living: bool,
    exclude_from_savings: bool,
    exclude_from_daily_avg: bool,
) -> Result<(), String> {
    let rows = sqlx::query(
        "UPDATE tag SET exclude_from_performance = ?1, exclude_from_cost_of_living = ?2, \
                        exclude_from_savings = ?3, exclude_from_daily_avg = ?4 WHERE id = ?5",
    )
    .bind(exclude_from_performance as i64)
    .bind(exclude_from_cost_of_living as i64)
    .bind(exclude_from_savings as i64)
    .bind(exclude_from_daily_avg as i64)
    .bind(tag_id)
    .execute(pool)
    .await
    .map_err(|e| format!("update_tag_rulers: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!("tag not found: {tag_id}"));
    }
    Ok(())
}

/// Renomeia/recolore uma tag existente (nome, cor e emoji). `is_special` segue a convenção do
/// nome (`!` no início fixa no topo), então é re-derivado aqui — igual ao caminho de criação.
/// Erro se a tag não existir. Não toca nos interruptores de régua (toggle próprio).
pub async fn update_tag(
    pool: &SqlitePool,
    tag_id: &str,
    name: &str,
    color: &str,
    emoji: Option<&str>,
) -> Result<(), String> {
    let is_special = name.trim_start().starts_with('!');
    let rows = sqlx::query(
        "UPDATE tag SET name = ?1, color = ?2, emoji = ?3, is_special = ?4 WHERE id = ?5",
    )
    .bind(name)
    .bind(color)
    .bind(emoji)
    .bind(is_special as i64)
    .bind(tag_id)
    .execute(pool)
    .await
    .map_err(|e| format!("update_tag: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!("tag not found: {tag_id}"));
    }
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
        "SELECT t.id, t.name, t.color, t.emoji, t.is_special, t.exclude_from_performance, \
                t.exclude_from_cost_of_living, t.exclude_from_savings, t.exclude_from_daily_avg, \
                COALESCE(SUM(ABS(tr.amount)), 0) AS total_cents \
         FROM tag t \
         LEFT JOIN transaction_tag tt ON tt.tag_id = t.id \
         LEFT JOIN \"transaction\" tr ON tr.id = tt.transaction_id \
                AND substr(tr.date, 1, 7) = ?1 \
                AND tr.type IN ('expense', 'transfer') \
                AND tr.scenario_id IS NULL \
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

#[tauri::command]
pub async fn update_tag_cmd(
    pool: State<'_, SqlitePool>,
    tag_id: String,
    name: String,
    color: String,
    emoji: Option<String>,
) -> Result<(), String> {
    update_tag(pool.inner(), &tag_id, &name, &color, emoji.as_deref()).await
}

#[tauri::command]
pub async fn update_tag_rulers_cmd(
    pool: State<'_, SqlitePool>,
    tag_id: String,
    exclude_from_performance: bool,
    exclude_from_cost_of_living: bool,
    exclude_from_savings: bool,
    exclude_from_daily_avg: bool,
) -> Result<(), String> {
    update_tag_rulers(
        pool.inner(),
        &tag_id,
        exclude_from_performance,
        exclude_from_cost_of_living,
        exclude_from_savings,
        exclude_from_daily_avg,
    )
    .await
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

    // Renomear/recolorir tag existente: `is_special` re-deriva da convenção `!` do nome;
    // os interruptores de régua não são tocados (toggle próprio).
    #[tokio::test]
    async fn update_tag_renames_recolors_and_rederives_special() {
        let p = pool().await;
        let id = create_tag(&p, "Viagem", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        update_tag_rulers(&p, &id, true, true, true, true)
            .await
            .unwrap();

        update_tag(&p, &id, "! Pagar", "var(--cat-brass)", Some("⚠️"))
            .await
            .unwrap();

        let tags = list_tags(&p).await.unwrap();
        let t = tags.iter().find(|t| t.id == id).unwrap();
        assert_eq!(t.name, "! Pagar");
        assert_eq!(t.color, "var(--cat-brass)");
        assert_eq!(t.emoji.as_deref(), Some("⚠️"));
        assert!(t.is_special, "nome com '!' fixa no topo (re-derivado)");
        assert!(
            t.exclude_from_performance
                && t.exclude_from_cost_of_living
                && t.exclude_from_savings
                && t.exclude_from_daily_avg,
            "interruptores de régua preservados por update_tag"
        );

        // Tag inexistente → erro explícito (não upsert silencioso).
        assert!(update_tag(&p, "nope", "X", "c", None).await.is_err());
    }

    // Cada interruptor é independente: gravar SÓ savings deixa as outras três réguas ligadas.
    #[tokio::test]
    async fn update_tag_rulers_writes_each_flag_independently() {
        let p = pool().await;
        let id = create_tag(&p, "Terceiros", "var(--cat-sky)", None, false)
            .await
            .unwrap();
        // Só a régua de Economia desligada.
        update_tag_rulers(&p, &id, false, false, true, false)
            .await
            .unwrap();
        let tags = list_tags(&p).await.unwrap();
        let t = tags.iter().find(|t| t.id == id).unwrap();
        assert!(!t.exclude_from_performance);
        assert!(!t.exclude_from_cost_of_living);
        assert!(t.exclude_from_savings, "só a Economia foi excluída");
        assert!(!t.exclude_from_daily_avg);

        // Tag inexistente → erro explícito.
        assert!(
            update_tag_rulers(&p, "nope", true, true, true, true)
                .await
                .is_err()
        );
    }
}
