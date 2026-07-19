//! Gate de conflito de import: lista os conflitos pendentes e aplica a resolução
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
    // Em "local", preservamos o valor ATUAL da linha, que pode ter sido editado depois da detecção,
    // em vez do snapshot `local_value`; restaurar o snapshot descartaria edições mais recentes.
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT transaction_id, field, sheet_value FROM import_conflict WHERE id = ?1 AND resolved_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("load conflict: {e}"))?;
    let Some((txn_id, field, sheet_value)) = row else {
        return Ok(()); // já resolvido ou inexistente
    };

    let now = chrono::Utc::now().to_rfc3339();
    let sheet_wins = choice == "sheet";

    // Atomicidade: a escrita na `transaction` e a marcação de `resolved_at` precisam ser tudo-ou-
    // nada. Sem a transação, um crash entre as duas deixaria a linha atualizada mas o conflito
    // ainda pendente (ou vice-versa).
    let mut tx = pool.begin().await.map_err(|e| format!("begin tx: {e}"))?;

    match field.as_str() {
        "amount" => {
            let source: i64 = sheet_value.parse().map_err(|_| "sheet amount inválido")?;
            if sheet_wins {
                // Planilha vence: grava o valor da planilha e realinha o base num só lugar.
                sqlx::query(
                    "UPDATE \"transaction\" SET amount=?1, source_amount=?1, updated_at=?2 WHERE id=?3",
                )
                .bind(source)
                .bind(&now)
                .bind(&txn_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("apply amount: {e}"))?;
            } else {
                // Mantém a edição local (o que estiver AGORA na linha) e só realinha o base.
                sqlx::query(
                    "UPDATE \"transaction\" SET source_amount=?1, updated_at=?2 WHERE id=?3",
                )
                .bind(source)
                .bind(&now)
                .bind(&txn_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("align amount base: {e}"))?;
            }
        }
        "description" => {
            if sheet_wins {
                sqlx::query(
                    "UPDATE \"transaction\" SET description=?1, source_description=?1, updated_at=?2 WHERE id=?3",
                )
                .bind(&sheet_value)
                .bind(&now)
                .bind(&txn_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("apply description: {e}"))?;
            } else {
                sqlx::query(
                    "UPDATE \"transaction\" SET source_description=?1, updated_at=?2 WHERE id=?3",
                )
                .bind(&sheet_value)
                .bind(&now)
                .bind(&txn_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("align description base: {e}"))?;
            }
        }
        "stated_total" if txn_id.starts_with("invoice:") => {
            let invoice_id = txn_id.trim_start_matches("invoice:");
            let sheet: i64 = sheet_value
                .parse()
                .map_err(|_| "sheet stated_total inválido")?;
            if sheet_wins {
                sqlx::query(
                    "UPDATE invoice SET stated_total_cents=?1, source_stated_total_cents=?1 WHERE id=?2",
                )
                .bind(sheet)
                .bind(invoice_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("apply invoice stated_total: {e}"))?;
            } else {
                sqlx::query("UPDATE invoice SET source_stated_total_cents=?1 WHERE id=?2")
                    .bind(sheet)
                    .bind(invoice_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("align invoice stated_total base: {e}"))?;
            }
        }
        other => return Err(format!("campo desconhecido: {other}")),
    }

    sqlx::query("UPDATE import_conflict SET resolved_at=?1, resolution=?2 WHERE id=?3")
        .bind(&now)
        .bind(choice)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("mark resolved: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("commit resolve: {e}"))?;
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
    async fn resolve_local_preserves_newer_edit_not_the_snapshot() {
        // O usuário editou a linha para 18000 depois da detecção do conflito, cujo snapshot local é
        // 15000. "Manter local" deve preservar 18000, enquanto a base realinha para a planilha em
        // 20000.
        let p = pool().await;
        seed_txn(&p, "t1", 18000, "x").await;
        seed_conflict(&p, "c1", "t1", "amount", "10000", "15000", "20000").await;

        resolve(&p, "c1", "local").await.unwrap();
        assert_eq!(amount_of(&p, "t1").await, (18000, Some(20000)));
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

    // Idempotência + caminho transacional: as duas escritas (transaction + import_conflict) são
    // tudo-ou-nada num único `tx.commit()`. A 2ª chamada vê o conflito já resolvido (resolved_at
    // IS NOT NULL → não casa o SELECT) e é um no-op, sem corromper o estado já gravado.
    #[tokio::test]
    async fn resolve_idempotent_second_call_is_noop() {
        let p = pool().await;
        seed_txn(&p, "t1", 10000, "x").await;
        seed_conflict(&p, "c1", "t1", "amount", "8000", "10000", "12000").await;

        resolve(&p, "c1", "sheet").await.unwrap();
        // 2ª chamada: conflito já resolvido → linha não encontrada no SELECT.
        // Não pode errar nem corromper o estado já resolvido.
        resolve(&p, "c1", "sheet").await.unwrap();
        assert_eq!(amount_of(&p, "t1").await, (12000, Some(12000)));
        assert!(list_conflicts(&p).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_invoice_stated_total_applies_or_aligns_the_sheet_base() {
        let p = pool().await;
        let card_id = crate::commands::card_cmds::create_card_account_inner(
            &p,
            "Visa",
            None,
            Some(20),
            Some(10),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents, source_stated_total_cents) \
             VALUES ('inv', ?1, '2026-06', '2026-05-20', '2026-06-10', 15000, 10000)",
        )
        .bind(card_id)
        .execute(&p)
        .await
        .unwrap();
        seed_conflict(
            &p,
            "invoice-conflict",
            "invoice:inv",
            "stated_total",
            "10000",
            "15000",
            "20000",
        )
        .await;
        resolve(&p, "invoice-conflict", "local").await.unwrap();
        let total: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT stated_total_cents, source_stated_total_cents FROM invoice WHERE id = 'inv'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(total, (Some(15_000), Some(20_000)));

        sqlx::query(
            "UPDATE import_conflict SET base_value='20000', local_value='15000', sheet_value='22000', resolved_at=NULL, resolution=NULL WHERE id='invoice-conflict'",
        )
        .execute(&p)
        .await
        .unwrap();
        resolve(&p, "invoice-conflict", "sheet").await.unwrap();
        let total: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT stated_total_cents, source_stated_total_cents FROM invoice WHERE id = 'inv'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(total, (Some(22_000), Some(22_000)));
    }
}
