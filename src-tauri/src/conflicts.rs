//! Gate de conflito de import (spec 013): lista os conflitos pendentes e aplica a resolução
//! humana (planilha vs local). A detecção mora em `google_sheets::import`; aqui é o gate.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct ImportConflict {
    pub id: String,
    pub transaction_id: String,
    /// Campo em conflito: "amount" ou "description".
    pub field: String,
    pub base_value: Option<String>,
    pub local_value: String,
    pub sheet_value: String,
}

/// Conflitos de import ainda não resolvidos, para o gate (ApprovalDiffCard) na UI.
pub async fn list_conflicts(pool: &SqlitePool) -> Result<Vec<ImportConflict>, String> {
    sqlx::query_as::<_, ImportConflict>(
        "SELECT id, transaction_id, field, base_value, local_value, sheet_value \
         FROM import_conflict WHERE resolved_at IS NULL ORDER BY created_at, field",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list conflicts: {e}"))
}

/// Aplica a escolha humana. `choice` = "sheet" (planilha vence) | "local" (mantém a edição).
/// Em ambos, o base (`source_*`) realinha ao valor atual da planilha → o conflito não volta no
/// próximo import com a mesma planilha (só reaparece se a célula mudar de novo).
pub async fn resolve(pool: &SqlitePool, id: &str, choice: &str) -> Result<(), String> {
    if choice != "sheet" && choice != "local" {
        return Err(format!("escolha inválida: {choice}"));
    }
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT transaction_id, field, local_value, sheet_value FROM import_conflict WHERE id = ?1 AND resolved_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("load conflict: {e}"))?;
    let Some((txn_id, field, local_value, sheet_value)) = row else {
        return Ok(()); // já resolvido ou inexistente
    };

    let chosen = if choice == "sheet" {
        &sheet_value
    } else {
        &local_value
    };
    let now = chrono::Utc::now().to_rfc3339();

    match field.as_str() {
        "amount" => {
            let value: i64 = chosen.parse().map_err(|_| "amount inválido")?;
            let source: i64 = sheet_value.parse().map_err(|_| "sheet amount inválido")?;
            sqlx::query(
                "UPDATE \"transaction\" SET amount=?1, source_amount=?2, updated_at=?3 WHERE id=?4",
            )
            .bind(value)
            .bind(source)
            .bind(&now)
            .bind(&txn_id)
            .execute(pool)
            .await
            .map_err(|e| format!("apply amount: {e}"))?;
        }
        "description" => {
            sqlx::query(
                "UPDATE \"transaction\" SET description=?1, source_description=?2, updated_at=?3 WHERE id=?4",
            )
            .bind(chosen)
            .bind(&sheet_value)
            .bind(&now)
            .bind(&txn_id)
            .execute(pool)
            .await
            .map_err(|e| format!("apply description: {e}"))?;
        }
        other => return Err(format!("campo desconhecido: {other}")),
    }

    sqlx::query("UPDATE import_conflict SET resolved_at=?1, resolution=?2 WHERE id=?3")
        .bind(&now)
        .bind(choice)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("mark resolved: {e}"))?;
    Ok(())
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn get_import_conflicts(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<ImportConflict>, String> {
    list_conflicts(pool.inner()).await
}

#[tauri::command]
pub async fn resolve_import_conflict(
    pool: State<'_, SqlitePool>,
    id: String,
    choice: String,
) -> Result<(), String> {
    resolve(pool.inner(), &id, &choice).await
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

    async fn seed_txn(pool: &SqlitePool, id: &str, amount: i64, desc: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, source_amount, source_description) \
             VALUES (?1, 'expense', ?2, ?3, '2026-06-05', 0, ?2, ?3)",
        )
        .bind(id).bind(amount).bind(desc)
        .execute(pool).await.unwrap();
    }

    async fn seed_conflict(
        pool: &SqlitePool,
        id: &str,
        txn: &str,
        field: &str,
        base: &str,
        local: &str,
        sheet: &str,
    ) {
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-06-14')",
        )
        .bind(id).bind(txn).bind(field).bind(base).bind(local).bind(sheet)
        .execute(pool).await.unwrap();
    }

    async fn amount_of(pool: &SqlitePool, id: &str) -> (i64, Option<i64>) {
        sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT amount, source_amount FROM \"transaction\" WHERE id=?1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn resolve_sheet_writes_sheet_value_and_aligns_base() {
        let p = pool().await;
        seed_txn(&p, "t1", 15000, "Mercado e farmácia").await; // valor local editado
        seed_conflict(&p, "c1", "t1", "amount", "10000", "15000", "20000").await;

        resolve(&p, "c1", "sheet").await.unwrap();
        assert_eq!(amount_of(&p, "t1").await, (20000, Some(20000)));
        // Some o conflito da lista.
        assert!(list_conflicts(&p).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_local_keeps_local_value_but_aligns_base_to_sheet() {
        let p = pool().await;
        seed_txn(&p, "t1", 15000, "x").await;
        seed_conflict(&p, "c1", "t1", "amount", "10000", "15000", "20000").await;

        resolve(&p, "c1", "local").await.unwrap();
        // Mantém o local (15000), mas base vai para a planilha (20000) → não reconflita.
        assert_eq!(amount_of(&p, "t1").await, (15000, Some(20000)));
        assert!(list_conflicts(&p).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_description_conflict() {
        let p = pool().await;
        seed_txn(&p, "t1", 100, "Mercado e farmácia").await;
        seed_conflict(
            &p,
            "c1",
            "t1",
            "description",
            "Mercado",
            "Mercado e farmácia",
            "Supermercado",
        )
        .await;

        resolve(&p, "c1", "local").await.unwrap();
        let (desc, src): (String, Option<String>) = sqlx::query_as(
            "SELECT description, source_description FROM \"transaction\" WHERE id='t1'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(desc, "Mercado e farmácia");
        assert_eq!(src, Some("Supermercado".to_string()));
    }

    #[tokio::test]
    async fn invalid_choice_errors() {
        let p = pool().await;
        seed_txn(&p, "t1", 100, "x").await;
        seed_conflict(&p, "c1", "t1", "amount", "1", "2", "3").await;
        assert!(resolve(&p, "c1", "whatever").await.is_err());
    }
}
