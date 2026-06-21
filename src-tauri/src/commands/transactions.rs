use super::*;

/// Tag anexada a um lançamento (para os chips do Livro-razão).
#[derive(serde::Serialize, sqlx::FromRow, Clone)]
pub struct TagOnRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
}

/// Plan 035: uma parte itemizada de um lançamento (breakdown da nota de célula).
/// O total do lançamento pai é a SOMA destas partes; aqui só leitura (edição = plano 036).
#[derive(Debug, serde::Serialize, sqlx::FromRow, Clone)]
pub struct LineItemOnRow {
    pub id: String,
    pub transaction_id: String,
    pub amount_cents: i64,
    pub description: String,
    pub position: i64,
}

/// Retorna as partes itemizadas de um lançamento (vazio = lançamento não itemizado).
pub(crate) async fn line_items_for_transaction(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Vec<LineItemOnRow>, String> {
    sqlx::query_as::<_, LineItemOnRow>(
        "SELECT id, transaction_id, amount_cents, description, position \
         FROM line_item WHERE transaction_id = ?1 ORDER BY position",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("line_items_for_transaction: {e}"))
}

#[tauri::command]
pub async fn get_line_items_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
) -> Result<Vec<LineItemOnRow>, String> {
    line_items_for_transaction(pool.inner(), &transaction_id).await
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
    /// Despesa fixa (veio da coluna Saída da planilha) vs variável (Diário). Distingue Saída × Diário.
    pub is_fixed: bool,
    /// Titulares distintos das parcelas (multi-titular). Vazio = sem split por pessoa.
    pub owners: Vec<String>,
    /// Tags anexadas (diagnóstico). Mostradas como chips no Livro-razão.
    pub tags: Vec<TagOnRow>,
    /// Proveniência: "projetado" (previsto), "importado" (da planilha) ou "manual" (do app).
    pub provenance: String,
    /// Partes itemizadas da nota (vazio = lançamento não itemizado). Plan 035 — só leitura.
    pub line_items: Vec<LineItemOnRow>,
}

#[tauri::command]
pub async fn get_recent_transactions(
    pool: State<'_, SqlitePool>,
    limit: i64,
) -> Result<Vec<TransactionRow>, String> {
    recent_transactions(pool.inner(), limit).await
}

#[derive(sqlx::FromRow)]
pub(crate) struct RecentRow {
    id: String,
    r#type: String,
    amount: i64,
    description: String,
    date: String,
    payment_method: String,
    is_projection: i64,
    is_fixed: i64,
    /// Titulares distintos, juntados por '|' no SQL (vazio = sem split por pessoa).
    owners: String,
    /// `source_amount` é NULL quando nunca veio da planilha (lançamento manual no app).
    has_source: i64,
}

pub(crate) async fn recent_transactions(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<TransactionRow>, String> {
    // Titulares vêm de um subquery agregado (GROUP_CONCAT com separador '|') — sem N+1.
    let rows: Vec<RecentRow> = sqlx::query_as(
        "SELECT t.id, t.type, t.amount, COALESCE(t.description,'') AS description, t.date, \
                COALESCE(t.payment_method,'') AS payment_method, t.is_projection, t.is_fixed, \
                COALESCE((SELECT GROUP_CONCAT(name, '|') FROM \
                    (SELECT DISTINCT p.name FROM split s \
                     JOIN person p ON p.id = s.owner_person_id \
                     WHERE s.transaction_id = t.id ORDER BY p.name COLLATE NOCASE)), '') AS owners, \
                (t.source_amount IS NOT NULL) AS has_source \
         FROM \"transaction\" t ORDER BY t.date DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    // Tags só das transações EFETIVAMENTE retornadas acima (busca pelos IDs reais). Uma janela
    // `ORDER BY date DESC LIMIT ?1` separada não garante o MESMO conjunto quando há empate de data
    // na borda do LIMIT (desempate arbitrário do SQLite), e linhas visíveis perderiam suas tags.
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let tag_rows: Vec<(String, String, String, String, Option<String>)> = if ids.is_empty() {
        Vec::new()
    } else {
        // Placeholders posicionais (só `?`, sem dado interpolado) + binds — seguro com AssertSqlSafe.
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT tt.transaction_id, t.id, t.name, t.color, t.emoji \
             FROM transaction_tag tt JOIN tag t ON t.id = tt.tag_id \
             WHERE tt.transaction_id IN ({placeholders}) \
             ORDER BY t.is_special DESC, t.name COLLATE NOCASE"
        );
        let mut q = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            sqlx::AssertSqlSafe(sql),
        );
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .map_err(|e| format!("tag query: {e}"))?
    };
    let mut tags_by_txn: std::collections::HashMap<String, Vec<TagOnRow>> =
        std::collections::HashMap::new();
    for (txn_id, id, name, color, emoji) in tag_rows {
        tags_by_txn.entry(txn_id).or_default().push(TagOnRow {
            id,
            name,
            color,
            emoji,
        });
    }

    // Plan 035: partes itemizadas das linhas EFETIVAMENTE retornadas (mesma janela de ids
    // que as tags). Batch único por ids — sem N+1. O caso comum (lançamentos sem itens)
    // não paga nada quando line_item está vazio.
    let li_rows: Vec<LineItemOnRow> = if ids.is_empty() {
        Vec::new()
    } else {
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT id, transaction_id, amount_cents, description, position \
             FROM line_item WHERE transaction_id IN ({placeholders}) \
             ORDER BY transaction_id, position"
        );
        let mut q = sqlx::query_as::<_, LineItemOnRow>(sqlx::AssertSqlSafe(sql));
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .map_err(|e| format!("line_item query: {e}"))?
    };
    let mut items_by_txn: std::collections::HashMap<String, Vec<LineItemOnRow>> =
        std::collections::HashMap::new();
    for li in li_rows {
        items_by_txn
            .entry(li.transaction_id.clone())
            .or_default()
            .push(li);
    }

    Ok(rows
        .into_iter()
        .map(|r| TransactionRow {
            tags: tags_by_txn.get(&r.id).cloned().unwrap_or_default(),
            line_items: items_by_txn.get(&r.id).cloned().unwrap_or_default(),
            id: r.id,
            r#type: r.r#type,
            amount: r.amount,
            description: r.description,
            date: r.date,
            payment_method: r.payment_method,
            is_projection: r.is_projection != 0,
            is_fixed: r.is_fixed != 0,
            owners: if r.owners.is_empty() {
                Vec::new()
            } else {
                // Ordena no Rust (não depende da ordem do GROUP_CONCAT, que não é contratual).
                let mut o: Vec<String> = r.owners.split('|').map(str::to_owned).collect();
                o.sort_by_key(|s| s.to_lowercase());
                o
            },
            provenance: if r.is_projection != 0 {
                "projetado".to_string()
            } else if r.has_source != 0 {
                "importado".to_string()
            } else {
                "manual".to_string()
            },
        })
        .collect())
}

/// Repetição opcional de um lançamento ("Repetir": frequência + nº de ocorrências).
#[derive(serde::Deserialize)]
pub struct RecurrenceInput {
    pub frequency: String,
    pub repetitions: usize,
}

/// Cria um lançamento manual (caminho de escrita do app). `amount_cents` é magnitude positiva;
/// a direção vem de `txn_type` ('income'/'expense'/'transfer'). Com `recurrence`, gera a série
/// projetada inteira em vez de um único realizado. As `tag_ids` são anexadas a toda linha criada.
/// Para `transfer` (Economia), `to_account_id` é obrigatório e precisa apontar para uma conta
/// reserve/illiquid — a MESMA forma que o import grava, para que `classify()` a conte como Economia.
/// Retorna o id do lançamento (ou da série, quando recorrente).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_transaction(
    pool: State<'_, SqlitePool>,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    date: String,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: Vec<String>,
    recurrence: Option<RecurrenceInput>,
    to_account_id: Option<String>,
) -> Result<String, String> {
    create_transaction_inner(
        pool.inner(),
        &txn_type,
        amount_cents,
        description,
        &date,
        payment_method,
        is_fixed,
        &tag_ids,
        recurrence,
        to_account_id.as_deref(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_transaction_inner(
    pool: &SqlitePool,
    txn_type: &str,
    amount_cents: i64,
    description: Option<String>,
    date: &str,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: &[String],
    recurrence: Option<RecurrenceInput>,
    to_account_id: Option<&str>,
) -> Result<String, String> {
    // Tipos aceitos no caminho manual: income/expense (gasto/renda) e transfer (Economia). Para
    // transfer, a conta-destino precisa ser reserve/illiquid — a mesma forma que o import grava,
    // que `classify()` (forecast) reconhece como Economia. transfer→líquido seria net-zero (não é
    // poupar) e transfer→restricted é gasto restrito: ambos rejeitados.
    match txn_type {
        "income" | "expense" => {
            if to_account_id.is_some_and(|s| !s.is_empty()) {
                return Err("conta-destino só se aplica a transfer (Economia)".into());
            }
        }
        "transfer" => {
            let dest_id = to_account_id
                .filter(|s| !s.is_empty())
                .ok_or("transfer requer conta-destino (to_account_id)")?;
            let row: Option<(String,)> =
                sqlx::query_as("SELECT COALESCE(liquidity,'') FROM account WHERE id = ?1")
                    .bind(dest_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| format!("query account: {e}"))?;
            match row {
                None => return Err("conta-destino não encontrada".into()),
                Some((liq,)) if liq == "reserve" || liq == "illiquid" => {}
                Some((liq,)) => {
                    return Err(format!(
                        "conta-destino deve ter liquidez 'reserve' ou 'illiquid', encontrado '{liq}'"
                    ));
                }
            }
        }
        other => return Err(format!("tipo inválido: {other}")),
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let start = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| format!("data: {e}"))?;

    // Caminho recorrente: delega à série projetada e anexa as tags a cada ocorrência. Economia
    // (transfer) é um lançamento único — a recorrência valida só income/expense (ver plano).
    if let Some(rec) = recurrence {
        if txn_type == "transfer" {
            return Err("Economia não suporta recorrência".into());
        }
        let freq =
            crate::recurrence::Frequency::parse(&rec.frequency).ok_or("frequência inválida")?;
        let template = crate::recurrence::RecurringTemplate {
            txn_type: txn_type.to_string(),
            amount: amount_cents,
            description,
            start,
            payment_method,
            is_fixed,
        };
        let rec_id =
            crate::recurrence::create_recurring_series(pool, &template, freq, rec.repetitions)
                .await?;
        if !tag_ids.is_empty() {
            for i in 0..rec.repetitions {
                crate::tags::set_transaction_tags(pool, &format!("{rec_id}:{i}"), tag_ids).await?;
            }
        }
        return Ok(rec_id);
    }

    // Lançamento único. Data FUTURA → projeção (igual ao import: `classify_row`); hoje/passado →
    // realizado. Marcar um futuro como realizado distorceria o "já aconteceu" do dashboard.
    let is_projection = start > chrono::Local::now().date_naive();
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    // `to_account_id` só viaja para transfer (Economia); income/expense ficam com from/to NULL —
    // a mesma forma do import (`store_economia_entries`), que `classify()` conta como Economia.
    let dest = if txn_type == "transfer" {
        to_account_id
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, description, date, payment_method, is_fixed, to_account_id, is_projection, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
    )
    .bind(&id)
    .bind(txn_type)
    .bind(amount_cents)
    .bind(&description)
    .bind(date)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(dest)
    .bind(is_projection as i64)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("insert transaction: {e}"))?;

    if !tag_ids.is_empty() {
        crate::tags::set_transaction_tags(pool, &id, tag_ids).await?;
    }
    Ok(id)
}

/// Plan 036: uma parte itemizada vinda do app (caminho de EDIÇÃO). `position` = ordem 0-based.
/// `amount_cents` é magnitude positiva (a direção/coluna vem do tipo do lançamento pai).
#[derive(serde::Deserialize)]
pub struct LineItemInput {
    pub amount_cents: i64,
    pub description: String,
    pub position: i64,
}

/// Plan 036: substitui TODAS as partes itemizadas de um lançamento e fixa o total do pai = Σ partes.
///
/// Diferente de `update_transaction_cmd`, este comando NÃO tem o guarda `source_amount IS NULL`: o
/// dono precisa poder detalhar/editar a quebra de um lançamento IMPORTADO (esse é o ponto da
/// feature). O `source_amount` (base do merge de 3 vias) não é tocado — o breakdown local é uma
/// representação mais rica da MESMA célula; um eventual conflito de total fica para o re-import
/// resolver. As partes inseridas são marcadas `is_user_edited = 1` para SOBREVIVEREM ao próximo
/// re-import enquanto a nota da planilha não mudar (ver o bloco do plano 035/036 no importer).
///
/// As três operações (DELETE itens antigos + INSERT novos + UPDATE total do pai) correm numa ÚNICA
/// transação SQLite — uma falha no meio não deixa o total e as partes divergentes.
#[tauri::command]
pub async fn update_transaction_items_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
    items: Vec<LineItemInput>,
) -> Result<(), String> {
    if transaction_id.trim().is_empty() {
        return Err("transaction_id vazio".into());
    }
    if items.is_empty() {
        // Lista vazia = "lançamento sem itens": o caller deve usar o valor simples (form normal),
        // não este comando. Rejeitamos para não zerar o total do pai sem querer.
        return Err("informe ao menos um item (ou edite o valor simples)".into());
    }
    let mut total_cents: i64 = 0;
    for item in &items {
        if item.amount_cents <= 0 {
            return Err("cada item deve ter valor positivo (magnitude)".into());
        }
        total_cents += item.amount_cents;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|e| format!("begin items: {e}"))?;

    sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
        .bind(&transaction_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("clear items: {e}"))?;

    for item in &items {
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&transaction_id)
        .bind(item.amount_cents)
        .bind(&item.description)
        .bind(item.position)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert item: {e}"))?;
    }

    let affected =
        sqlx::query(r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1"#)
            .bind(&transaction_id)
            .bind(total_cents)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("update parent total: {e}"))?
            .rows_affected();
    if affected == 0 {
        return Err("lançamento não encontrado".into());
    }

    tx.commit()
        .await
        .map_err(|e| format!("commit items: {e}"))?;
    Ok(())
}

/// Apaga um lançamento manual (não importado) pelo id. O guarda `source_amount IS NULL` impede
/// remover histórico vindo da planilha pelo app — esses precisam de um fluxo próprio.
#[tauri::command]
pub async fn delete_transaction_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let affected =
        sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL"#)
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| format!("delete: {e}"))?
            .rows_affected();
    if affected == 0 {
        return Err(
            "lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)"
                .into(),
        );
    }
    Ok(())
}

/// Edita um lançamento manual (valor, descrição, método, fixo, data) pelo id. Mesmo guarda de
/// `delete_transaction_cmd`: importados da planilha não são editáveis pelo app.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_transaction_cmd(
    pool: State<'_, SqlitePool>,
    id: String,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
    date: String,
) -> Result<(), String> {
    // `type` precisa ser atualizável: trocar entrada↔saída no form muda renda↔despesa, e sem isto
    // o sinal do lançamento no forecast ficaria errado. Mesmo conjunto válido do create.
    if !matches!(txn_type.as_str(), "income" | "expense") {
        return Err(format!("tipo inválido: {txn_type}"));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, updated_at = ?8
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
    .bind(&id)
    .bind(&txn_type)
    .bind(amount_cents)
    .bind(&description)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(&date)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("update: {e}"))?
    .rows_affected();
    if affected == 0 {
        return Err(
            "lançamento não encontrado ou importado da planilha (não pode ser editado pelo app)"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_txn(pool: &SqlitePool, id: &str, amount: i64) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES (?1, 'expense', ?2, '2026-03-10', 0, 0, '2026-03-10T00:00:00Z', '2026-03-10T00:00:00Z')",
        )
        .bind(id)
        .bind(amount)
        .execute(pool)
        .await
        .unwrap();
    }

    fn item(amount_cents: i64, description: &str, position: i64) -> LineItemInput {
        LineItemInput {
            amount_cents,
            description: description.into(),
            position,
        }
    }

    // Núcleo do comando exposto, sem o wrapper `State` (intestável diretamente). Replica a lógica
    // 1:1 para cobrir o caminho transacional (DELETE + INSERT + UPDATE total) num pool em memória.
    async fn run_update_items(
        pool: &SqlitePool,
        transaction_id: &str,
        items: Vec<LineItemInput>,
    ) -> Result<(), String> {
        if transaction_id.trim().is_empty() {
            return Err("transaction_id vazio".into());
        }
        if items.is_empty() {
            return Err("informe ao menos um item (ou edite o valor simples)".into());
        }
        let mut total_cents: i64 = 0;
        for it in &items {
            if it.amount_cents <= 0 {
                return Err("cada item deve ter valor positivo (magnitude)".into());
            }
            total_cents += it.amount_cents;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind(transaction_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("clear: {e}"))?;
        for it in &items {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(transaction_id)
            .bind(it.amount_cents)
            .bind(&it.description)
            .bind(it.position)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("insert: {e}"))?;
        }
        let affected =
            sqlx::query(r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1"#)
                .bind(transaction_id)
                .bind(total_cents)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("update: {e}"))?
                .rows_affected();
        if affected == 0 {
            return Err("lançamento não encontrado".into());
        }
        tx.commit().await.map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    #[tokio::test]
    async fn update_transaction_items_cmd_sets_total() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-1", 1000).await;

        run_update_items(&pool, "tx-1", vec![item(500, "A", 0), item(750, "B", 1)])
            .await
            .unwrap();

        // Total do pai = Σ partes (500 + 750 = 1250).
        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = 'tx-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 1250);

        // Duas partes persistidas, marcadas como editadas pelo usuário.
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT amount_cents, is_user_edited FROM line_item WHERE transaction_id = 'tx-1' ORDER BY position",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows, vec![(500, 1), (750, 1)]);
    }

    #[tokio::test]
    async fn update_transaction_items_cmd_rejects_empty_list() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-2", 1000).await;
        let err = run_update_items(&pool, "tx-2", vec![]).await.unwrap_err();
        assert!(err.contains("ao menos um item"), "err: {err}");
        // O total do pai NÃO foi tocado.
        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = 'tx-2'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 1000);
    }

    #[tokio::test]
    async fn update_transaction_items_cmd_replaces_previous_items() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-3", 1000).await;
        run_update_items(&pool, "tx-3", vec![item(500, "A", 0), item(500, "B", 1)])
            .await
            .unwrap();
        // Segunda edição substitui (clear + reinsert), não acumula.
        run_update_items(&pool, "tx-3", vec![item(900, "C", 0)])
            .await
            .unwrap();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-3'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 1);
        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = 'tx-3'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 900);
    }

    #[tokio::test]
    async fn update_transaction_items_cmd_rejects_nonpositive_amount() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-4", 1000).await;
        let err = run_update_items(&pool, "tx-4", vec![item(0, "zero", 0)])
            .await
            .unwrap_err();
        assert!(err.contains("positivo"), "err: {err}");
    }
}
