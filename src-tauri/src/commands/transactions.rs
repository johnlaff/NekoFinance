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
#[derive(Debug, serde::Serialize, Clone)]
pub struct LineItemOnRow {
    pub id: String,
    pub transaction_id: String,
    pub amount_cents: i64,
    pub description: String,
    pub position: i64,
    /// Kind derivado da seção da nota, sem fallback por descrição/banco.
    /// Pai `income` → "entrada" (os kinds de seção só fatiam saídas).
    pub kind: String,
    /// Cabeçalho de seção cru da nota (ex.: "CONTAS:"), sem normalização. `None` = sem seção.
    /// Plano 069: a UI precisa disto para propor `match_section` ao marcar o item como
    /// obrigação recorrente — sem ele não há como restringir o casamento à seção do item.
    pub section: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct LineItemDbRow {
    id: String,
    transaction_id: String,
    amount_cents: i64,
    description: String,
    position: i64,
    section: Option<String>,
    /// `type` do lançamento pai — decide se o kind vem do classificador de seção
    /// (pai despesa) ou é "entrada" (partes de uma entrada são entradas).
    parent_type: String,
}

fn line_item_kind_slug(kind: import::ItemKind) -> &'static str {
    match kind {
        import::ItemKind::Saida => "saida",
        import::ItemKind::Cartao => "cartao",
        import::ItemKind::Diario => "diario",
        import::ItemKind::Economia => "economia",
        import::ItemKind::Patrimonio => "patrimonio",
        import::ItemKind::Ajuste => "ajuste",
    }
}

fn line_item_on_row(row: LineItemDbRow) -> LineItemOnRow {
    // Os kinds de seção fatiam SAÍDAS em baldes (saída/cartão/diário/economia/
    // patrimônio); partes de uma entrada são entradas, qualquer que seja a seção.
    let kind = if row.parent_type == "income" {
        "entrada"
    } else {
        line_item_kind_slug(import::classify_line_item(
            row.section.as_deref(),
            row.description.as_str(),
        ))
    };
    LineItemOnRow {
        id: row.id,
        transaction_id: row.transaction_id,
        amount_cents: row.amount_cents,
        description: row.description,
        position: row.position,
        kind: kind.to_string(),
        section: row.section,
    }
}

/// Retorna as partes itemizadas de um lançamento (vazio = lançamento não itemizado).
pub(crate) async fn line_items_for_transaction(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Vec<LineItemOnRow>, String> {
    let rows: Vec<LineItemDbRow> = sqlx::query_as(
        "SELECT li.id, li.transaction_id, li.amount_cents, li.description, li.position, \
                li.section, t.type AS parent_type \
         FROM line_item li JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE li.transaction_id = ?1 ORDER BY li.position",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("line_items_for_transaction: {e}"))?;
    Ok(rows.into_iter().map(line_item_on_row).collect())
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
    /// Plano 045: data de vencimento opcional ("YYYY-MM-DD"); None = sem lembrete de conta.
    /// Metadado consultivo (calendário) — NÃO afeta o Saldo/forecast (que usa `date`).
    pub due_date: Option<String>,
    /// Plano 045: posição 1-based na série de parcelas (1 = primeira). None fora de série recorrente.
    pub installment_index: Option<i64>,
    /// Plano 045: total de parcelas da série. None fora de série recorrente. Derivado de
    /// `recurrence.repetitions` + o índice embutido no id `{rec_id}:{i}` (não-armazenado).
    pub installment_total: Option<i64>,
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
    /// Plano 045: vencimento opcional ("YYYY-MM-DD"); NULL = sem lembrete de conta.
    due_date: Option<String>,
    /// Plano 045: série a que pertence (NULL = lançamento avulso). Usado para derivar "N/M parcelas".
    recurrence_id: Option<String>,
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
                (t.source_amount IS NOT NULL) AS has_source, \
                t.due_date, t.recurrence_id \
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
            "SELECT li.id, li.transaction_id, li.amount_cents, li.description, li.position, \
                    li.section, t.type AS parent_type \
             FROM line_item li JOIN \"transaction\" t ON t.id = li.transaction_id \
             WHERE li.transaction_id IN ({placeholders}) \
             ORDER BY li.transaction_id, li.position"
        );
        let mut q = sqlx::query_as::<_, LineItemDbRow>(sqlx::AssertSqlSafe(sql));
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .map_err(|e| format!("line_item query: {e}"))?
            .into_iter()
            .map(line_item_on_row)
            .collect()
    };
    let mut items_by_txn: std::collections::HashMap<String, Vec<LineItemOnRow>> =
        std::collections::HashMap::new();
    for li in li_rows {
        items_by_txn
            .entry(li.transaction_id.clone())
            .or_default()
            .push(li);
    }

    // Plano 045: total de parcelas (`repetitions`) por série, em UM batch pelos `recurrence_id`
    // DISTINTOS das linhas retornadas — sem N+1 (mesmo padrão das tags/itens acima). O caso comum
    // (linhas sem série) não paga nada quando o conjunto de ids de recorrência é vazio.
    let rec_ids: Vec<String> = {
        let mut set: std::collections::HashSet<String> = std::collections::HashSet::new();
        for r in &rows {
            if let Some(rid) = &r.recurrence_id {
                set.insert(rid.clone());
            }
        }
        set.into_iter().collect()
    };
    let reps_by_rec: std::collections::HashMap<String, i64> = if rec_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let placeholders = vec!["?"; rec_ids.len()].join(",");
        let sql = format!(
            "SELECT id, repetitions FROM recurrence \
             WHERE id IN ({placeholders}) AND repetitions IS NOT NULL"
        );
        let mut q = sqlx::query_as::<_, (String, i64)>(sqlx::AssertSqlSafe(sql));
        for id in &rec_ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .map_err(|e| format!("recurrence reps query: {e}"))?
            .into_iter()
            .collect()
    };

    Ok(rows
        .into_iter()
        .map(|r| {
            let tags = tags_by_txn.get(&r.id).cloned().unwrap_or_default();
            let line_items = items_by_txn.get(&r.id).cloned().unwrap_or_default();
            // Plano 045: "N/M parcelas" só quando a linha pertence a uma série COM repetições.
            // Índice 1-based vem do sufixo `:{i}` do id (0-based → +1); total vem de `repetitions`.
            let (installment_index, installment_total) = match &r.recurrence_id {
                Some(rid) => match reps_by_rec.get(rid) {
                    Some(total) => (
                        crate::recurrence::occurrence_index(&r.id).map(|i| i + 1),
                        Some(*total),
                    ),
                    None => (None, None),
                },
                None => (None, None),
            };
            let owners = if r.owners.is_empty() {
                Vec::new()
            } else {
                // Ordena no Rust (não depende da ordem do GROUP_CONCAT, que não é contratual).
                let mut o: Vec<String> = r.owners.split('|').map(str::to_owned).collect();
                o.sort_by_key(|s| s.to_lowercase());
                o
            };
            let provenance = if r.is_projection != 0 {
                "projetado".to_string()
            } else if r.has_source != 0 {
                "importado".to_string()
            } else {
                "manual".to_string()
            };
            TransactionRow {
                tags,
                line_items,
                id: r.id,
                r#type: r.r#type,
                amount: r.amount,
                description: r.description,
                date: r.date,
                payment_method: r.payment_method,
                is_projection: r.is_projection != 0,
                is_fixed: r.is_fixed != 0,
                owners,
                provenance,
                due_date: r.due_date,
                installment_index,
                installment_total,
            }
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
    due_date: Option<String>,
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
        due_date.as_deref(),
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
    due_date: Option<&str>,
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
        "INSERT INTO \"transaction\" (id, type, amount, description, date, payment_method, is_fixed, to_account_id, is_projection, due_date, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
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
    .bind(due_date)
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
/// Vale também para lançamentos IMPORTADOS: o dono precisa poder detalhar/editar a quebra de uma
/// linha vinda da planilha (esse é o ponto da feature; o plano 043 alinhou os comandos escalares à
/// mesma política). O `source_amount` (base do merge de 3 vias) não é tocado — o breakdown local é
/// uma representação mais rica da MESMA célula; um eventual conflito de total fica para o re-import
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

    // `scenario_id IS NULL`: uma linha hipotética nunca é editada pelos comandos do livro real —
    // se o id apontar para um cenário, `affected == 0` e cai no mesmo erro de "não encontrado".
    let affected = sqlx::query(
        r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1 AND scenario_id IS NULL"#,
    )
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

/// Apaga um lançamento pelo id (plano 043/047): inclui linhas importadas. A planilha é a fonte da
/// verdade — apagar aqui NÃO apaga da planilha. O painel de ações no Livro-razão avisa o usuário
/// disso (notice de "Linha importada").
///
/// Plano 047: o delete agora limpa, na MESMA transação, a metadata de sync que de outro modo deixaria
/// órfãos e desfaria o delete: (1) as linhas DERIVADAS (Entradas compensatórias `derived:%:<id>:%`,
/// sem FK para o pai); (2) o `sync_log` da linha (id determinístico `log:<id>`, sem FK) — sem isto o
/// próximo import RECRIARIA a linha apagada via diff/upsert; (3) os `import_conflict` da linha (sem FK
/// CASCADE) — conflitos órfãos bloqueariam o write-back para sempre. As `line_item` filhas somem via
/// `ON DELETE CASCADE`. Após o fix o delete é "sticky": o próximo import NÃO recria a linha.
#[tauri::command]
pub async fn delete_transaction_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|e| format!("delete (begin): {e}"))?;

    // `scenario_id IS NULL`: uma linha hipotética nunca é apagada pelos comandos do livro real —
    // se o id apontar para um cenário, `affected == 0` e cai no mesmo erro de "não encontrado".
    let affected =
        sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1 AND scenario_id IS NULL"#)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete: {e}"))?
            .rows_affected();
    if affected == 0 {
        // `tx` é descartada sem commit → rollback automático.
        return Err("lançamento não encontrado".into());
    }

    // Linhas derivadas (Entradas compensatórias `derived:<kind>:<id>:<i>`): sem FK para o pai, são
    // limpas só no import; aqui replicamos o diff-delete de `import.rs`.
    sqlx::query(r#"DELETE FROM "transaction" WHERE id LIKE 'derived:%:' || ?1 || ':%'"#)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete derived rows: {e}"))?;

    // `sync_log` da linha (sem FK ao `transaction`): sem remover, o próximo import recria a linha.
    // Delete manual remove o registro de import em TODAS as abas (sem filtro de `source_sheet`).
    sqlx::query("DELETE FROM sync_log WHERE entity_id = ?1 AND entity_type = 'transaction'")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete sync_log: {e}"))?;

    // Conflitos de import órfãos (sem FK CASCADE) bloqueariam o write-back; somem com a linha.
    sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete import_conflict: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("delete (commit): {e}"))?;
    Ok(())
}

/// Edita um lançamento (valor, descrição, método, fixo, data) pelo id (plano 043): inclui linhas
/// importadas. A edição fica no app; um re-import pode sobrescrever o valor se a planilha mudou
/// (o merge de 3 vias reconcilia). O painel de ações avisa o usuário disso.
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
    update_transaction_inner(
        pool.inner(),
        &id,
        &txn_type,
        amount_cents,
        description,
        payment_method,
        is_fixed,
        &date,
    )
    .await
}

/// Caminho real do `update_transaction_cmd`, testável diretamente (recebe `&SqlitePool`, não
/// `State`). Espelha o par `create_transaction` / `create_transaction_inner`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_transaction_inner(
    pool: &SqlitePool,
    id: &str,
    txn_type: &str,
    amount_cents: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
    date: &str,
) -> Result<(), String> {
    // `type` precisa ser atualizável: trocar entrada↔saída no form muda renda↔despesa, e sem isto
    // o sinal do lançamento no forecast ficaria errado. Mesmo conjunto válido do create.
    if !matches!(txn_type, "income" | "expense") {
        return Err(format!("tipo inválido: {txn_type}"));
    }
    // `amount` é magnitude positiva (o sinal vem do `type`); espelha `create_transaction_inner`.
    // Sem este guard, o update poderia gravar 0/negativo, violando o invariante e quebrando as
    // agregações que somam magnitudes. Rejeita antes de qualquer acesso ao banco.
    if amount_cents <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let now = chrono::Utc::now().to_rfc3339();
    // Re-deriva `is_projection` a partir da NOVA data (espelha `create_transaction_inner`):
    // editar uma projeção futura para hoje/passado precisa limpar o flag, senão fica um
    // "Previsto" fantasma na previsão. Hoje é realizado → só datas estritamente futuras projetam.
    let new_date =
        NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| format!("data inválida: {e}"))?;
    let is_projection = new_date > chrono::Local::now().date_naive();

    // Plano 049: a limpeza dos `line_item` e o UPDATE do total do pai precisam ser ATÔMICOS. Antes
    // rodavam como dois statements auto-commit separados; um crash entre eles deixava os itens
    // apagados mas o total antigo no pai (ou o novo total sem itens). Espelha `delete_transaction_cmd`:
    // tudo na mesma `sqlx::Transaction`, com commit só no fim. O early-return em `affected == 0`
    // descarta `tx` sem commit → rollback automático (mesmo padrão do delete).
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("update (begin): {e}"))?;

    // Plano 047: se a linha é ITEMIZADA (tem `line_item`) e o NOVO valor difere do total atual, a
    // quebra não reflete mais o total → limpa os itens (a Σ ficaria divergente no write-back). O
    // usuário re-insere a quebra pelo editor de itens. Idempotente: sem itens ou valor inalterado, é
    // no-op. Consulta o valor + a contagem de itens numa só query (LEFT JOIN agregado).
    // Plano 053: a troca de TIPO (entrada↔saída) também invalida a quebra mesmo com o mesmo valor —
    // itens de renda numa linha de despesa ficam semanticamente errados e confundem o write-back.
    // Por isso a query também carrega o `type` antigo e a limpeza dispara em mudança de tipo.
    let current: Option<(i64, i64, String)> = sqlx::query_as(
        r#"SELECT t.amount, COUNT(li.id), t.type
           FROM "transaction" t
           LEFT JOIN line_item li ON li.transaction_id = t.id
           WHERE t.id = ?1 AND t.scenario_id IS NULL
           GROUP BY t.amount, t.type"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("update (load items): {e}"))?;
    if let Some((old_amount, item_count, old_type)) = current
        && item_count > 0
        && (old_amount != amount_cents || old_type.as_str() != txn_type)
    {
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("update (clear stale items): {e}"))?;
    }

    // `scenario_id IS NULL`: uma linha hipotética nunca é editada pelos comandos do livro real —
    // se o id apontar para um cenário, `affected == 0` e cai no mesmo erro de "não encontrado".
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
           WHERE id = ?1 AND scenario_id IS NULL"#,
    )
    .bind(id)
    .bind(txn_type)
    .bind(amount_cents)
    .bind(&description)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(date)
    .bind(is_projection as i64)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("update: {e}"))?
    .rows_affected();
    if affected == 0 {
        // `tx` é descartada sem commit → rollback automático.
        return Err("lançamento não encontrado".into());
    }

    tx.commit()
        .await
        .map_err(|e| format!("update (commit): {e}"))?;
    Ok(())
}

/// Plano 045: uma conta a vencer — um lançamento com `due_date` na janela [hoje, horizonte].
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct UpcomingBill {
    pub id: String,
    pub description: String,
    pub amount: i64,
    pub due_date: String,
    pub is_projection: bool,
}

/// Núcleo puro: contas com `due_date` nos próximos `days` dias (inclui hoje), ordenadas por
/// vencimento. `today` é injetado (determinístico/testável). A janela superior é uma data CALCULADA
/// em Rust (não interpolação de SQL) e ligada por placeholder — evita o aviso de `AssertSqlSafe`.
/// Limite 100 para não devolver um conjunto ilimitado. NÃO toca Saldo/forecast (só leitura de `due_date`).
pub(crate) async fn upcoming_bills_inner(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    days: i64,
) -> Result<Vec<UpcomingBill>, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    // `days` negativo viraria uma janela invertida (vazia); satura em 0 = só hoje.
    let upper = today_naive
        .checked_add_signed(chrono::Duration::days(days.max(0)))
        .unwrap_or(today_naive)
        .format("%Y-%m-%d")
        .to_string();
    sqlx::query_as::<_, UpcomingBill>(
        "SELECT id, COALESCE(description,'') AS description, ABS(amount) AS amount, \
                due_date, is_projection \
         FROM \"transaction\" \
         WHERE due_date IS NOT NULL AND due_date >= ?1 AND due_date <= ?2 \
         ORDER BY due_date ASC LIMIT 100",
    )
    .bind(&today)
    .bind(&upper)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("upcoming bills: {e}"))
}

/// Contas a vencer nos próximos `days` dias. Adapter fino sobre o núcleo puro (clock injetado aqui).
#[tauri::command]
pub async fn get_upcoming_bills_cmd(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> Result<Vec<UpcomingBill>, String> {
    upcoming_bills_inner(pool.inner(), chrono::Local::now().date_naive(), days).await
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
    async fn line_items_carry_section_derived_kind_without_bank_fallback() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-kind", 3000).await;

        for (id, amount, description, position, section) in [
            (
                "li-kind-card",
                1000,
                "compra no credito",
                0,
                Some("CARTÕES:"),
            ),
            (
                "li-kind-bank-name",
                2000,
                "Banco Exemplo - compra no cartao",
                1,
                None,
            ),
        ] {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
                 VALUES (?1, 'tx-kind', ?2, ?3, ?4, ?5)",
            )
            .bind(id)
            .bind(amount)
            .bind(description)
            .bind(position)
            .bind(section)
            .execute(&pool)
            .await
            .unwrap();
        }

        let items = line_items_for_transaction(&pool, "tx-kind").await.unwrap();

        assert_eq!(items[0].kind, "cartao");
        assert_eq!(
            items[1].kind, "saida",
            "sem seção, nome de banco/cartão na descrição não muda o kind"
        );
    }

    #[tokio::test]
    async fn line_items_of_income_parent_are_entrada() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES ('tx-income', 'income', 300264, '2026-07-12', 0, 0, '2026-07-12T00:00:00Z', '2026-07-12T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (id, amount, description, position, section) in [
            ("li-inc-plain", 257764, "salário", 0, None::<&str>),
            ("li-inc-sec", 42500, "reembolso", 1, Some("CARTÕES:")),
        ] {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
                 VALUES (?1, 'tx-income', ?2, ?3, ?4, ?5)",
            )
            .bind(id)
            .bind(amount)
            .bind(description)
            .bind(position)
            .bind(section)
            .execute(&pool)
            .await
            .unwrap();
        }

        let items = line_items_for_transaction(&pool, "tx-income")
            .await
            .unwrap();

        assert_eq!(items[0].kind, "entrada");
        assert_eq!(
            items[1].kind, "entrada",
            "os kinds de seção fatiam saídas; uma seção na nota não rebaixa parte de entrada a balde de saída"
        );
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

    // Plano 049: o `update_transaction_cmd` limpava os `line_item` e atualizava o total do pai em
    // dois statements auto-commit separados (não atômico). Este teste replica o caminho de troca de
    // valor numa única `sqlx::Transaction` (igual ao comando corrigido) e verifica que, após o
    // commit, OS DOIS lados ficam consistentes: itens removidos E total do pai = novo valor.
    #[tokio::test]
    async fn update_transaction_cmd_clears_items_and_updates_amount_atomically() {
        let pool = test_pool().await;
        // Pai com valor 1000.
        insert_txn(&pool, "tx-upd", 1000).await;
        // Duas linhas de quebra.
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
             VALUES ('li-a', 'tx-upd', 600, 'Part A', 0, 1), \
                    ('li-b', 'tx-upd', 400, 'Part B', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Mesma sequência do comando corrigido: abre transação, limpa itens stale, atualiza o pai.
        let new_amount: i64 = 2000;
        let mut tx = pool.begin().await.unwrap();
        let current: Option<(i64, i64)> = sqlx::query_as(
            r#"SELECT t.amount, COUNT(li.id)
               FROM "transaction" t
               LEFT JOIN line_item li ON li.transaction_id = t.id
               WHERE t.id = ?1
               GROUP BY t.amount"#,
        )
        .bind("tx-upd")
        .fetch_optional(&mut *tx)
        .await
        .unwrap();
        if let Some((old_amount, item_count)) = current
            && item_count > 0
            && old_amount != new_amount
        {
            sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
                .bind("tx-upd")
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        sqlx::query(r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1"#)
            .bind("tx-upd")
            .bind(new_amount)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Os dois lados devem ficar consistentes após o commit.
        let (amount,): (i64,) =
            sqlx::query_as(r#"SELECT amount FROM "transaction" WHERE id = 'tx-upd'"#)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 2000, "parent amount updated to new value");

        let item_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-upd'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            item_count.0, 0,
            "stale line_items cleared when amount changed"
        );
    }

    // Bug 2 (plano 053): `update_transaction_inner` precisa rejeitar `amount <= 0`, espelhando
    // `create_transaction_inner`. `amount` é magnitude positiva; zero/negativo violaria o invariante
    // e quebraria as agregações que somam magnitudes. A linha no banco deve ficar INTOCADA.
    #[tokio::test]
    async fn update_transaction_cmd_rejects_zero_amount() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-guard", 1000).await;

        let err = update_transaction_inner(
            &pool,
            "tx-guard",
            "expense",
            0,
            None,
            None,
            false,
            "2026-03-10",
        )
        .await
        .unwrap_err();
        assert!(err.contains("positivo"), "err: {err}");

        // A linha não foi tocada.
        let (amount,): (i64,) =
            sqlx::query_as(r#"SELECT amount FROM "transaction" WHERE id = 'tx-guard'"#)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 1000, "amount inalterado após rejeição");
    }

    #[tokio::test]
    async fn update_transaction_cmd_rejects_negative_amount() {
        let pool = test_pool().await;
        insert_txn(&pool, "tx-guard-neg", 1000).await;

        let err = update_transaction_inner(
            &pool,
            "tx-guard-neg",
            "expense",
            -500,
            None,
            None,
            false,
            "2026-03-10",
        )
        .await
        .unwrap_err();
        assert!(err.contains("positivo"), "err: {err}");

        let (amount,): (i64,) =
            sqlx::query_as(r#"SELECT amount FROM "transaction" WHERE id = 'tx-guard-neg'"#)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 1000, "amount inalterado após rejeição");
    }

    // Bug 4 (plano 053): trocar o TIPO (entrada↔saída) de uma linha ITEMIZADA com o MESMO valor
    // precisa limpar os `line_item` — itens de renda numa linha de despesa ficam semanticamente
    // errados e confundem o write-back. O bug antigo só limpava quando o VALOR mudava.
    #[tokio::test]
    async fn update_transaction_cmd_clears_items_on_type_change() {
        let pool = test_pool().await;
        // Pai 'income' com valor 1000.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES ('tx-type', 'income', 1000, '2026-03-10', 0, 0, '2026-03-10T00:00:00Z', '2026-03-10T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Duas linhas de quebra.
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
             VALUES ('li-t-a', 'tx-type', 600, 'Part A', 0, 1), \
                    ('li-t-b', 'tx-type', 400, 'Part B', 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Troca income → expense com o MESMO valor (1000).
        update_transaction_inner(
            &pool,
            "tx-type",
            "expense",
            1000,
            None,
            None,
            false,
            "2026-03-10",
        )
        .await
        .unwrap();

        // Itens limpos apesar do valor inalterado.
        let item_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-type'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(item_count.0, 0, "itens limpos na troca de tipo");

        // O total do pai segue 1000 (só o valor não mudou; o tipo sim).
        let (amount, ttype): (i64, String) =
            sqlx::query_as(r#"SELECT amount, type FROM "transaction" WHERE id = 'tx-type'"#)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 1000, "amount inalterado");
        assert_eq!(ttype, "expense", "tipo atualizado");
    }

    // Espelha a lógica de re-deriva de `is_projection` de `update_transaction_cmd`
    // (que recebe `State<'_, SqlitePool>` e não é diretamente testável), igual ao
    // padrão de `run_update_items`.
    async fn run_update_txn_date(
        pool: &SqlitePool,
        id: &str,
        new_date: &str,
    ) -> Result<(), String> {
        use chrono::NaiveDate;
        let new_date_parsed = NaiveDate::parse_from_str(new_date, "%Y-%m-%d")
            .map_err(|e| format!("data inválida: {e}"))?;
        let is_projection = new_date_parsed > chrono::Local::now().date_naive();
        let now = chrono::Utc::now().to_rfc3339();
        let affected = sqlx::query(
            r#"UPDATE "transaction"
               SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
                   is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
               WHERE id = ?1"#,
        )
        .bind(id)
        .bind("expense")
        .bind(1000_i64)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(0_i64)
        .bind(new_date)
        .bind(is_projection as i64)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("update: {e}"))?
        .rows_affected();
        if affected == 0 {
            return Err("not found".into());
        }
        Ok(())
    }

    #[tokio::test]
    async fn update_transaction_date_to_today_clears_is_projection() {
        let pool = test_pool().await;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let future = "2099-12-31";

        // Insert a future transaction (is_projection = 1).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES ('tx-upd', 'expense', 1000, ?1, 0, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(future)
        .execute(&pool)
        .await
        .unwrap();

        // Edit the date to today.
        run_update_txn_date(&pool, "tx-upd", &today).await.unwrap();

        let (is_projection,): (i64,) =
            sqlx::query_as("SELECT is_projection FROM \"transaction\" WHERE id = 'tx-upd'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            is_projection, 0,
            "editing date to today must clear is_projection (Bug 3)"
        );
    }

    // Insere uma linha "importada" (com `source_amount` preenchido, como o import grava) para
    // cobrir a remoção do guarda `source_amount IS NULL` do plano 043.
    async fn insert_imported_txn(pool: &SqlitePool, id: &str, amount: i64, source_amount: i64) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, source_amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES (?1, 'expense', ?2, ?3, '2026-03-10', 0, 0, '2026-03-10T00:00:00Z', '2026-03-10T00:00:00Z')",
        )
        .bind(id)
        .bind(amount)
        .bind(source_amount)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn delete_imported_row_succeeds() {
        // Regressão plano 043: apagar não é mais bloqueado por `source_amount IS NOT NULL`.
        let pool = test_pool().await;
        insert_imported_txn(&pool, "imp-del", 1000, 1000).await;

        let affected = sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1"#)
            .bind("imp-del")
            .execute(&pool)
            .await
            .unwrap()
            .rows_affected();
        assert_eq!(affected, 1, "linha importada deve ser apagável (plano 043)");

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = 'imp-del'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 0, "a linha importada foi removida");
    }

    #[tokio::test]
    async fn update_imported_row_succeeds() {
        // Regressão plano 043: editar não é mais bloqueado por `source_amount IS NOT NULL`.
        let pool = test_pool().await;
        insert_imported_txn(&pool, "imp-upd", 5000, 5000).await;

        let now = chrono::Utc::now().to_rfc3339();
        let affected = sqlx::query(
            r#"UPDATE "transaction"
               SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
                   is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
               WHERE id = ?1"#,
        )
        .bind("imp-upd")
        .bind("expense")
        .bind(9900_i64)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(0_i64)
        .bind("2026-03-11")
        .bind(0_i64)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
        assert_eq!(affected, 1, "linha importada deve ser editável (plano 043)");

        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = 'imp-upd'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 9900, "o novo valor foi gravado");
    }

    // --- Plano 047: delete limpa órfãos + update limpa itens stale ---

    // Núcleo do `delete_transaction_cmd` sem o wrapper `State` (mesmo padrão de `run_update_items`).
    // Replica 1:1 o caminho transacional: DELETE da linha + derivadas + sync_log + import_conflict.
    async fn run_delete_txn(pool: &SqlitePool, id: &str) -> Result<(), String> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("delete (begin): {e}"))?;
        let affected = sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1"#)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete: {e}"))?
            .rows_affected();
        if affected == 0 {
            return Err("lançamento não encontrado".into());
        }
        sqlx::query(r#"DELETE FROM "transaction" WHERE id LIKE 'derived:%:' || ?1 || ':%'"#)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete derived rows: {e}"))?;
        sqlx::query("DELETE FROM sync_log WHERE entity_id = ?1 AND entity_type = 'transaction'")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete sync_log: {e}"))?;
        sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete import_conflict: {e}"))?;
        tx.commit()
            .await
            .map_err(|e| format!("delete (commit): {e}"))?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_imported_row_cleans_sync_log_conflict_and_derived() {
        // Plano 047 (P1): apagar uma linha importada precisa remover sua metadata de sync — senão o
        // próximo import recria a linha (sync_log) e um conflito órfão bloqueia o write-back.
        let pool = test_pool().await;

        // Perfil (FK de sync_log.profile_id → profile → person).
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES ('pr-1', 'pe-1')")
            .execute(&pool)
            .await
            .unwrap();

        // Linha importada + seu sync_log (id determinístico) + um import_conflict aberto + uma
        // Entrada derivada (id prefixado `derived:`).
        insert_imported_txn(&pool, "tx-1", 1000, 1000).await;
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, source_sheet) \
             VALUES ('log:tx-1', 'import', 'transaction', 'tx-1', 'pr-1', '2026')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value) \
             VALUES ('cf-1', 'tx-1', 'amount', '100', '200', '300')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_txn(&pool, "derived:reembolso:tx-1:0", 500).await;

        run_delete_txn(&pool, "tx-1").await.unwrap();

        // A linha-pai sumiu.
        let parent: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = 'tx-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(parent.0, 0, "a linha importada foi removida");

        // A Entrada derivada sumiu (sem FK ao pai — limpa explicitamente).
        let derived: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id = 'derived:reembolso:tx-1:0'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(derived.0, 0, "a linha derivada foi removida");

        // O sync_log sumiu → o próximo import NÃO recria a linha (sem fantasma).
        let log: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sync_log WHERE entity_id = 'tx-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(log.0, 0, "o sync_log foi removido (sem recriação fantasma)");

        // O conflito órfão sumiu → o write-back não fica bloqueado.
        let conflict: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE transaction_id = 'tx-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            conflict.0, 0,
            "o conflito órfão foi removido (write-back livre)"
        );
    }

    #[tokio::test]
    async fn update_amount_clears_stale_line_items() {
        // Plano 047 (P2): editar o VALOR de uma linha itemizada limpa a quebra (Σ não bate mais).
        let pool = test_pool().await;
        insert_txn(&pool, "tx-2", 5000).await;
        // Dois itens somando o total atual (5000).
        for (amt, desc, pos) in [(2000, "A", 0), (3000, "B", 1)] {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
                 VALUES (?1, 'tx-2', ?2, ?3, ?4)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(amt as i64)
            .bind(desc)
            .bind(pos as i64)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Replica a lógica de limpeza de `update_transaction_cmd` com NOVO valor diferente.
        run_update_amount_clears_items(&pool, "tx-2", 8000)
            .await
            .unwrap();

        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id = 'tx-2'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 8000, "o novo total foi gravado");
        let items: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-2'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(items.0, 0, "a quebra stale foi limpa ao mudar o valor");
    }

    #[tokio::test]
    async fn update_amount_unchanged_or_no_items_keeps_items() {
        // Guarda do outro lado: sem itens NÃO falha; e valor INALTERADO preserva a quebra.
        let pool = test_pool().await;

        // (a) Sem itens: não falha.
        insert_txn(&pool, "tx-3", 5000).await;
        run_update_amount_clears_items(&pool, "tx-3", 9000)
            .await
            .unwrap();

        // (b) Itemizada com o MESMO valor: a quebra sobrevive.
        insert_txn(&pool, "tx-4", 5000).await;
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
             VALUES ('li-4', 'tx-4', 5000, 'só', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        run_update_amount_clears_items(&pool, "tx-4", 5000)
            .await
            .unwrap();
        let items: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-4'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(items.0, 1, "valor inalterado preserva a quebra");
    }

    // Espelha a limpeza condicional de itens de `update_transaction_cmd` (que recebe `State` e não é
    // testável direto): SELECT do total + contagem de itens; se itemizada E o valor mudou, DELETE dos
    // itens; depois grava o novo total.
    async fn run_update_amount_clears_items(
        pool: &SqlitePool,
        id: &str,
        amount_cents: i64,
    ) -> Result<(), String> {
        let current: Option<(i64, i64)> = sqlx::query_as(
            r#"SELECT t.amount, COUNT(li.id)
               FROM "transaction" t
               LEFT JOIN line_item li ON li.transaction_id = t.id
               WHERE t.id = ?1
               GROUP BY t.amount"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("update (load items): {e}"))?;
        if let Some((old_amount, item_count)) = current
            && item_count > 0
            && old_amount != amount_cents
        {
            sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| format!("update (clear stale items): {e}"))?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1"#)
            .bind(id)
            .bind(amount_cents)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("update: {e}"))?;
        Ok(())
    }

    // --- Plano 045: due_date + contas a vencer + parcelas ---

    #[tokio::test]
    async fn recent_transactions_carry_due_date() {
        let pool = test_pool().await;
        // Uma linha COM vencimento e uma SEM.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, due_date, created_at, updated_at) \
             VALUES ('due-1', 'expense', 1000, '2026-06-01', 1, 0, '2026-07-10', '2026-06-01T00:00:00Z', '2026-06-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_txn(&pool, "nodue-1", 2000).await;

        let rows = recent_transactions(&pool, 50).await.unwrap();
        let with_due = rows.iter().find(|r| r.id == "due-1").unwrap();
        assert_eq!(with_due.due_date.as_deref(), Some("2026-07-10"));
        let without_due = rows.iter().find(|r| r.id == "nodue-1").unwrap();
        assert_eq!(without_due.due_date, None, "sem vencimento → None");
    }

    #[tokio::test]
    async fn create_transaction_inner_stores_due_date() {
        let pool = test_pool().await;
        let id = create_transaction_inner(
            &pool,
            "expense",
            5000,
            Some("Conta demo".into()),
            "2026-06-01",
            Some("pix".into()),
            true,
            &[],
            None,
            None,
            Some("2026-08-10"),
        )
        .await
        .unwrap();
        let (due,): (Option<String>,) =
            sqlx::query_as("SELECT due_date FROM \"transaction\" WHERE id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(due.as_deref(), Some("2026-08-10"));
    }

    #[tokio::test]
    async fn get_upcoming_bills_returns_bills_in_window() {
        let pool = test_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        // Uma vence em 5 dias (na janela de 10), outra em 90 dias (fora).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, due_date, created_at, updated_at) \
             VALUES ('near', 'expense', 1000, '2026-06-01', 1, 0, '2026-06-06', '2026-06-01T00:00:00Z', '2026-06-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, due_date, created_at, updated_at) \
             VALUES ('far', 'expense', 2000, '2026-06-01', 1, 0, '2026-08-30', '2026-06-01T00:00:00Z', '2026-06-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Sem due_date: nunca aparece.
        insert_txn(&pool, "nodue", 3000).await;

        let bills = upcoming_bills_inner(&pool, today, 10).await.unwrap();
        assert_eq!(bills.len(), 1, "só a conta dentro da janela");
        assert_eq!(bills[0].id, "near");
        assert_eq!(bills[0].amount, 1000, "magnitude (ABS)");
    }

    #[tokio::test]
    async fn installment_index_and_total_populated_for_series() {
        let pool = test_pool().await;
        // Série de 6 parcelas mensais.
        let tmpl = crate::recurrence::RecurringTemplate {
            txn_type: "expense".into(),
            amount: 10000,
            description: Some("Parcela demo".into()),
            start: NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            payment_method: Some("credit".into()),
            is_fixed: false,
        };
        let rec_id = crate::recurrence::create_recurring_series(
            &pool,
            &tmpl,
            crate::recurrence::Frequency::Mensal,
            6,
        )
        .await
        .unwrap();

        let rows = recent_transactions(&pool, 50).await.unwrap();
        let first = rows.iter().find(|r| r.id == format!("{rec_id}:0")).unwrap();
        assert_eq!(first.installment_index, Some(1), "0-based → 1-based");
        assert_eq!(first.installment_total, Some(6));
        let third = rows.iter().find(|r| r.id == format!("{rec_id}:2")).unwrap();
        assert_eq!(third.installment_index, Some(3));
        assert_eq!(
            third.installment_total,
            Some(6),
            "total igual em toda a série"
        );
    }

    #[tokio::test]
    async fn installment_fields_null_for_single_transaction() {
        let pool = test_pool().await;
        insert_txn(&pool, "single", 1000).await;
        let rows = recent_transactions(&pool, 50).await.unwrap();
        let row = rows.iter().find(|r| r.id == "single").unwrap();
        assert_eq!(row.installment_index, None);
        assert_eq!(row.installment_total, None);
    }
}
