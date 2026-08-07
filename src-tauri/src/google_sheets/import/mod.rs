use super::reconcile;
use sqlx::SqlitePool;

#[cfg(test)]
use super::layout_detect::SheetLayout;

mod classify;
mod grid;
mod lexicon;
mod proposals;

pub use classify::*;
pub use grid::*;
pub(crate) use lexicon::*;
pub(crate) use proposals::*;

type ExistingTransactionRow = (i64, Option<String>, Option<i64>, Option<String>);

/// Registra (ou limpa) o conflito de um campo. Conflito presente → UPSERT idempotente por
/// (transação, campo); ausente → apaga o conflito (re-import resolveu ou convergiu).
#[allow(clippy::too_many_arguments)]
async fn record_conflict(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    txn_id: &str,
    field: &str,
    conflict: bool,
    base_value: Option<String>,
    local_value: &str,
    sheet_value: &str,
    now: &str,
) -> Result<(), String> {
    if conflict {
        let id = format!("conf:{txn_id}:{field}");
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(transaction_id, field) DO UPDATE SET base_value=excluded.base_value, \
               local_value=excluded.local_value, sheet_value=excluded.sheet_value, \
               created_at=excluded.created_at, resolved_at=NULL, resolution=NULL",
        )
        .bind(&id)
        .bind(txn_id)
        .bind(field)
        .bind(&base_value)
        .bind(local_value)
        .bind(sheet_value)
        .bind(now)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("record conflict: {e}"))?;
    } else {
        sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1 AND field = ?2")
            .bind(txn_id)
            .bind(field)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("clear conflict: {e}"))?;
    }
    Ok(())
}

pub async fn check_duplicate_import(
    pool: &SqlitePool,
    sheet_name: &str,
    checksum: &str,
) -> Result<bool, String> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1 AND checksum = ?2")
            .bind(sheet_name)
            .bind(checksum)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("check duplicate: {e}"))?;

    Ok(count > 0)
}

// Wrapper de pool mantido para testes; o shell usa `import_rows_with_options_in_tx` na transação
// externa.

#[allow(dead_code)]
pub async fn import_rows(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
) -> Result<usize, String> {
    import_rows_with_options(
        pool,
        sheet_name,
        rows,
        profile_id,
        ImportRowsOptions::default(),
    )
    .await
}

#[derive(Debug, Clone, Copy)]
pub struct ImportRowsOptions {
    /// `true` when descriptions came from Sheets cell notes or another authoritative source.
    /// `false` means parser fallbacks like "Saída YYYY-MM-DD" must not overwrite existing notes.
    pub descriptions_trusted: bool,
}

impl Default for ImportRowsOptions {
    fn default() -> Self {
        Self {
            descriptions_trusted: true,
        }
    }
}

// Wrapper de pool (begin→core→commit) mantido para testes; o shell usa
// `import_rows_with_options_in_tx`.
#[allow(dead_code)]
pub async fn import_rows_with_options(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    let checksum = if options.descriptions_trusted {
        compute_checksum(rows)
    } else {
        compute_checksum_with_options(rows, false)
    };
    if check_duplicate_import(pool, sheet_name, &checksum).await? {
        // Dataset idêntico ao último import desta aba — nada mudou, não toca o banco.
        return Ok(0);
    }

    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let n = import_rows_core(&mut tx, sheet_name, rows, profile_id, options, &checksum).await?;
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(n)
}

/// Importa as linhas numa transação JÁ ABERTA — o chamador (o shell) é dono do commit/rollback,
/// de modo que layout + mappings + linhas + série de Saldo gravem tudo-ou-nada numa única tx.
/// A checagem de duplicata (`check_duplicate_import`) é responsabilidade do chamador e deve rodar
/// no pool ANTES de abrir a transação. `checksum` é o do batch (ver `compute_import_checksum`).
pub(crate) async fn import_rows_with_options_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
    checksum: &str,
) -> Result<usize, String> {
    import_rows_core(tx, sheet_name, rows, profile_id, options, checksum).await
}

/// Corpo do import de linhas dentro de uma transação recebida; NÃO faz commit. Toda a IO usa
/// `&mut **tx` (a transação dereferencia para `&mut SqliteConnection`).
async fn import_rows_core(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
    checksum: &str,
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();

    // Reconciliação não destrutiva por aba: identidade determinística + UPSERT preservam o id e o
    // enriquecimento ancorado nele quando a célula é editada; diff-delete remove apenas as linhas
    // ausentes da planilha.
    let profile_id = resolve_profile_id(tx, profile_id).await?;

    let mut current_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (row, txn_id) in rows.iter().zip(imported_row_ids(sheet_name, rows)) {
        current_ids.insert(txn_id.clone());

        let sheet_amount = row.amount.abs();
        let sheet_desc = row.description.clone();

        // Merge de 3 vias: a planilha não vence cego. Carrega o estado atual + o base
        // (source_*) e decide por campo — preservando edição local e abrindo conflito quando ambos
        // divergem, em vez de sobrescrever em silêncio. `is_fixed`/`is_projection`/`type` são
        // estruturais (seguem a planilha).
        let existing: Option<ExistingTransactionRow> = sqlx::query_as(
            "SELECT amount, description, source_amount, source_description \
             FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&txn_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| format!("load existing txn: {e}"))?;

        match existing {
            None => {
                // Linha nova: a planilha semeia valor e base.
                let trusted_desc = options.descriptions_trusted.then_some(sheet_desc.as_str());
                sqlx::query(
                    "INSERT INTO \"transaction\" (id, type, amount, description, date, is_fixed, is_projection, source_amount, source_description, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?3, ?4, ?8, ?8)",
                )
                .bind(&txn_id)
                .bind(row.kind.txn_type())
                .bind(sheet_amount)
                .bind(trusted_desc)
                .bind(&row.date)
                .bind(row.kind.is_fixed() as i64)
                .bind(row.is_projection as i64)
                .bind(&now)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("insert row {row:?}: {e}"))?;
            }
            Some((local_amount, local_desc, src_amount, src_desc)) => {
                let amt = reconcile::apply(src_amount.as_ref(), &local_amount, &sheet_amount);
                let local_desc_for_merge = local_desc.clone().unwrap_or_default();
                let desc = options.descriptions_trusted.then(|| {
                    reconcile::apply(src_desc.as_ref(), &local_desc_for_merge, &sheet_desc)
                });
                let next_description = desc
                    .as_ref()
                    .map(|d| Some(d.value.clone()))
                    .unwrap_or_else(|| local_desc.clone());
                let next_source_description = desc
                    .as_ref()
                    .map(|d| Some(d.source.clone()))
                    .unwrap_or_else(|| src_desc.clone());

                sqlx::query(
                    "UPDATE \"transaction\" SET type=?2, amount=?3, description=?4, date=?5, \
                       is_fixed=?6, is_projection=?7, source_amount=?8, source_description=?9, updated_at=?10 \
                     WHERE id=?1",
                )
                .bind(&txn_id)
                .bind(row.kind.txn_type())
                .bind(amt.value)
                .bind(next_description.as_deref())
                .bind(&row.date)
                .bind(row.kind.is_fixed() as i64)
                .bind(row.is_projection as i64)
                .bind(amt.source)
                .bind(next_source_description.as_deref())
                .bind(&now)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("update row {row:?}: {e}"))?;

                record_conflict(
                    tx,
                    &txn_id,
                    "amount",
                    amt.conflict,
                    src_amount.map(|v| v.to_string()),
                    &local_amount.to_string(),
                    &sheet_amount.to_string(),
                    &now,
                )
                .await?;
                if let Some(desc) = desc {
                    record_conflict(
                        tx,
                        &txn_id,
                        "description",
                        desc.conflict,
                        src_desc,
                        &local_desc_for_merge,
                        &sheet_desc,
                        &now,
                    )
                    .await?;
                }
            }
        }

        // --- Gramática das notas (#reembolso:/#dividir:) ---
        // Opt-in e forward-only: nota sem marcador → no-op.
        let markers = parse_note_markers(&row.raw_note);

        if !markers.tagged_lines.is_empty() {
            // Idempotência no re-import: descarta as linhas derivadas e splits anteriores
            // desta transação, depois re-insere a partir da nota atual. A nota é autoritativa.
            sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
                .bind(format!("derived:%:{txn_id}:%"))
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear derived rows for {txn_id}: {e}"))?;

            sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear splits for {txn_id}: {e}"))?;

            for tagged in &markers.tagged_lines {
                // Resolve a pessoa pelo nome (case-insensitive); cria sob demanda na MESMA tx.
                let person_id: String = {
                    let existing: Option<(String,)> = sqlx::query_as(
                        "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
                    )
                    .bind(&tagged.person_name)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| format!("lookup person '{}': {e}", tagged.person_name))?;

                    match existing {
                        Some((id,)) => id,
                        None => {
                            let new_id = uuid::Uuid::new_v4().to_string();
                            sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                                .bind(&new_id)
                                .bind(&tagged.person_name)
                                .execute(&mut **tx)
                                .await
                                .map_err(|e| {
                                    format!("create person '{}': {e}", tagged.person_name)
                                })?;
                            new_id
                        }
                    }
                };

                match &tagged.kind {
                    NoteMarkerKind::Reembolso => {
                        // Entrada compensatória: valor integral da linha. Id determinístico
                        // ancorado ao pai → re-import substitui (não duplica); sem linha em
                        // sync_log → nunca diff-deletada por engano (só via cleanup do pai).
                        let derived_id =
                            format!("derived:reembolso:{txn_id}:{}", tagged.line_index);
                        let desc = format!("Reembolso: {}", tagged.person_name);
                        sqlx::query(
                            "INSERT OR REPLACE INTO \"transaction\" \
                             (id, type, amount, description, date, is_fixed, is_projection, \
                              counterparty_person_id, created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?6, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(tagged.line_amount_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
                        .bind(&person_id)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert reembolso Entrada: {e}"))?;
                    }
                    NoteMarkerKind::Dividir { share_cents } => {
                        // Split na transação pai para <quem>.
                        let split_id = uuid::Uuid::new_v4().to_string();
                        sqlx::query(
                            "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
                             VALUES (?1, ?2, ?3, ?4)",
                        )
                        .bind(&split_id)
                        .bind(&txn_id)
                        .bind(share_cents)
                        .bind(&person_id)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert split for '{}': {e}", tagged.person_name))?;

                        // Entrada compensatória pela parte de <quem>.
                        let derived_id = format!("derived:dividir:{txn_id}:{}", tagged.line_index);
                        let desc = format!("Dividir: {}", tagged.person_name);
                        sqlx::query(
                            "INSERT OR REPLACE INTO \"transaction\" \
                             (id, type, amount, description, date, is_fixed, is_projection, \
                              counterparty_person_id, created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?6, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(share_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
                        .bind(&person_id)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert dividir Entrada: {e}"))?;
                    }
                }
            }
        }
        // --- Nota itemizada → linhas em line_item ---
        // O estilo de anotação do usuário é a célula itemizada: o TOTAL da célula é a
        // SOMA de partes, cada parte descrita em uma linha da nota. Aqui surfeamos essas
        // partes como filhos descritivos (passado E projetado), sem NUNCA mexer no total.
        //
        // PRESERVAÇÃO DE EDIÇÃO LOCAL: o app deixa o dono EDITAR as partes
        // (`update_transaction_items_cmd` grava com `is_user_edited = 1`). Essas edições
        // locais são autoritativas até a NOTA da planilha mudar. Por isso só re-derivamos
        // da nota quando ela MUDOU desde o último import — comparando `row.raw_note` com o
        // `source_note` (base) guardado no pai. Espelha o merge de 3 vias do `source_amount`:
        // base = nota vista no último import; local = itens editados no app; entrante = nota
        // atual. Nota inalterada + itens editados → mantém o local; nota alterada → a nota vence e
        // as partes são derivadas novamente.
        //
        // SEGURO POR PADRÃO: o total do pai jamais é alterado. Quando o somatório das
        // partes diverge da célula, o breakdown sobrevive (a classificação é preservada)
        // e o resíduo célula − Σpartes é reconciliado COM SINAL no
        // loader de métricas — a convenção AJUSTES "Diferença" da planilha, sem persistir
        // item sintético. O write-back escreve RAW quando a soma não bate.
        //
        // GATE DE CONFIANÇA: num ciclo degradado — falha da API
        // de notas ou import .xlsx (calamine não expõe notas) — toda `raw_note` chega
        // VAZIA. Re-derivar aqui destruiria os itens classificados (Cartão/Economia/
        // Patrimônio) e as edições locais do último import bom. Itens e `source_note`
        // só mudam quando as notas vieram de verdade neste ciclo.
        if options.descriptions_trusted {
            // Base (nota do último import) + se há item editado pelo usuário nesta txn.
            let (prev_source_note, has_user_edited): (Option<String>, i64) = {
                let snote: Option<(Option<String>,)> = sqlx::query_as(
                    r#"SELECT source_note FROM "transaction" WHERE id = ?1 AND scenario_id IS NULL"#,
                )
                .bind(&txn_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| format!("load source_note for {txn_id}: {e}"))?;
                let (edited,): (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1 AND is_user_edited = 1",
                )
                .bind(&txn_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| format!("count user-edited items for {txn_id}: {e}"))?;
                (snote.and_then(|(n,)| n), edited)
            };
            let note_changed = prev_source_note.as_deref() != Some(row.raw_note.as_str());
            let keep_local = has_user_edited > 0 && !note_changed;

            // Sempre realinha a base da nota (igual ao realinho de `source_amount` do write-back):
            // A nota atual da planilha torna-se a base do próximo import.
            sqlx::query(r#"UPDATE "transaction" SET source_note = ?2 WHERE id = ?1"#)
                .bind(&txn_id)
                .bind(&row.raw_note)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("set source_note for {txn_id}: {e}"))?;

            if !keep_local {
                // Linhas projetadas preservam itens R$ 0,00 como placeholders ("a preencher").
                let items = parse_itemized_note_opts(&row.raw_note, row.is_projection);
                // Persistir itens é seguro quando eles CARREGAM classificação: ≥2 partes
                // (breakdown de verdade) OU 1 parte SOB cabeçalho de seção — o gate antigo de
                // ≥2 jogava Economia/Cartão de item único no custo de vida. Memo de linha única
                // SEM seção não é breakdown (persistir migraria um Diário/Cartão para Saída).
                //
                // Soma divergente NÃO descarta mais o breakdown (a classificação sobrevive):
                // o resíduo célula − Σpartes entra COM SINAL como Saída fixa no loader de
                // métricas (a convenção AJUSTES "Diferença" da planilha, sem persistir item
                // sintético — id/posição colidiriam e o sinal quebraria o write-back), e o
                // write-back cai para escrita RAW do total quando a soma não bate. O total da
                // célula do dono jamais é alterado.
                let has_breakdown =
                    items.len() >= 2 || (items.len() == 1 && items[0].section.is_some());

                // Limpa os itens antigos desta txn (idempotente no re-import; a nota é autoritativa).
                sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
                    .bind(&txn_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| format!("clear line_items for {txn_id}: {e}"))?;

                if has_breakdown {
                    for item in &items {
                        // Id determinístico `li:<txn_id>:<pos>` → re-import estável (UPSERT).
                        // `is_user_edited = 0`: derivado da nota (não-editado), reset explícito.
                        let item_id = format!("li:{}:{}", txn_id, item.position);
                        sqlx::query(
                            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
                             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6) \
                             ON CONFLICT(id) DO UPDATE SET \
                               amount_cents=excluded.amount_cents, \
                               description=excluded.description, \
                               position=excluded.position, \
                               is_user_edited=0, \
                               section=excluded.section",
                        )
                        .bind(&item_id)
                        .bind(&txn_id)
                        .bind(item.amount_cents)
                        .bind(&item.description)
                        .bind(item.position as i64)
                        .bind(item.section.as_deref())
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert line_item {item_id}: {e}"))?;
                    }
                }
                // Nota sem breakdown (vazia ou memo de 1 linha sem seção): nenhum item inserido.
            }
            // keep_local: itens editados no app sobrevivem (a nota não mudou) — nada a fazer.
        }
        // sync_log com id determinístico (1:1 com o txn) → UPSERT idempotente.
        let log_id = format!("log:{txn_id}");
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, metadata, source_sheet, checksum) \
             VALUES (?1, 'import', 'transaction', ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET timestamp=excluded.timestamp, metadata=excluded.metadata, checksum=excluded.checksum"
        )
        .bind(&log_id)
        .bind(&txn_id)
        .bind(&profile_id)
        .bind(&now)
        .bind(format!(r#"{{"source":"{sheet_name}","date":"{}","amount":{},"kind":"{}"}}"#, row.date, row.amount, row.kind.as_str()))
        .bind(sheet_name)
        .bind(checksum)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("sync_log error: {e}"))?;
    }

    // Diff-delete: linhas removidas da planilha (no sync_log desta aba, mas fora do import atual).
    let existing: Vec<(String,)> = sqlx::query_as(
        "SELECT entity_id FROM sync_log WHERE source_sheet = ?1 AND entity_type = 'transaction'",
    )
    .bind(sheet_name)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("load existing ids: {e}"))?;
    for (eid,) in existing {
        if !current_ids.contains(&eid) {
            sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
                .bind(&eid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete removed txn: {e}"))?;
            // Linhas derivadas (Entradas compensatórias) têm ids determinísticos prefixados
            // com "derived:<kind>:<parent_id>:<i>" e NÃO têm linha em sync_log. Limpamos aqui
            // quando o pai é removido pelo diff-delete. (`eid` é um SHA-256 hex de row_id, sem
            // o prefixo `derived:`, então o LIKE não casa transações não-derivadas.)
            sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
                .bind(format!("derived:%:{eid}:%"))
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete derived rows for {eid}: {e}"))?;
            sqlx::query(
                "DELETE FROM sync_log WHERE entity_id = ?1 AND source_sheet = ?2 AND entity_type = 'transaction'",
            )
            .bind(&eid)
            .bind(sheet_name)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("delete removed sync_log: {e}"))?;
            // Conflitos órfãos somem com a transação removida.
            sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
                .bind(&eid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete removed conflicts: {e}"))?;
        }
    }

    Ok(rows.len())
}

/// `sync_log.profile_id` tem FK para `profile` (sqlx liga `foreign_keys` por default) e o
/// frontend historicamente envia um UUID aleatório que não existe — o que fazia o import
/// inteiro falhar com FK violation. Usa o profile pedido se existir; senão o primeiro
/// profile do banco; senão cria o default (person "Eu" + profile) na mesma transação.
async fn resolve_profile_id(
    tx: &mut sqlx::SqliteConnection,
    requested: &str,
) -> Result<String, String> {
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM profile WHERE id = ?1")
        .bind(requested)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("query profile: {e}"))?;
    if exists.is_some() {
        return Ok(requested.to_string());
    }

    let first: Option<(String,)> =
        sqlx::query_as("SELECT id FROM profile ORDER BY created_at LIMIT 1")
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("query profile: {e}"))?;
    if let Some((id,)) = first {
        return Ok(id);
    }

    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("create person: {e}"))?;
    let (person_id,): (String,) =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("query person: {e}"))?;

    let profile_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
        .bind(&profile_id)
        .bind(&person_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("create profile: {e}"))?;

    Ok(profile_id)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) async fn test_pool() -> SqlitePool {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    pub(crate) fn imported(date: &str, amount: i64) -> ImportedRow {
        imported_desc(date, amount, &format!("Linha {date}"))
    }

    pub(crate) fn imported_desc(date: &str, amount: i64, description: &str) -> ImportedRow {
        ImportedRow {
            date: date.into(),
            amount,
            description: description.into(),
            is_projection: false,
            kind: if amount >= 0 {
                RowKind::Entrada
            } else {
                RowKind::Saida
            },
            raw_note: String::new(),
        }
    }

    pub(crate) fn imported_note(
        date: &str,
        amount: i64,
        raw_note: &str,
        is_projection: bool,
    ) -> ImportedRow {
        ImportedRow {
            raw_note: raw_note.into(),
            is_projection,
            ..imported_desc(date, amount, &format!("Linha {date}"))
        }
    }

    pub(crate) async fn count_line_items(pool: &SqlitePool, txn_id: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1")
            .bind(txn_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    pub(crate) async fn count_transactions(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    pub(crate) async fn description_and_source(
        pool: &SqlitePool,
        date: &str,
    ) -> (Option<String>, Option<String>) {
        sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT description, source_description FROM \"transaction\" WHERE date = ?1",
        )
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    pub(crate) async fn count_sync_log(pool: &SqlitePool, sheet: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1")
            .bind(sheet)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    pub(crate) async fn amount_by_date(pool: &SqlitePool, date: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT amount FROM \"transaction\" WHERE date = ?1")
            .bind(date)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    pub(crate) async fn set_local_amount(pool: &SqlitePool, date: &str, amount: i64) {
        sqlx::query("UPDATE \"transaction\" SET amount = ?1 WHERE date = ?2")
            .bind(amount)
            .bind(date)
            .execute(pool)
            .await
            .unwrap();
    }

    pub(crate) async fn conflict_count(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM import_conflict")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::*;

    // --- Reimport idempotente e atômico por aba ---

    // Linha importada com nota de célula crua e flag de projeção (passado/futuro).

    // O backend resolve ou cria o profile antes de gravar `sync_log`, pois o identificador recebido
    // do frontend pode não satisfazer a chave estrangeira.
    #[tokio::test]
    async fn import_bootstraps_default_profile_when_id_is_unknown() {
        let pool = test_pool().await;
        let rows = vec![imported("2026-01-05", 10_000)];

        let count = import_rows(&pool, "2026", &rows, "uuid-aleatorio-do-frontend")
            .await
            .unwrap();
        assert_eq!(count, 1);

        // O sync_log aponta para um profile que existe de verdade.
        let (orphans,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sync_log WHERE profile_id NOT IN (SELECT id FROM profile)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(orphans, 0);
    }

    #[tokio::test]
    async fn reimport_identical_dataset_is_noop() {
        let pool = test_pool().await;
        let rows = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
        ];

        assert_eq!(import_rows(&pool, "2026", &rows, "p1").await.unwrap(), 2);
        assert_eq!(import_rows(&pool, "2026", &rows, "p1").await.unwrap(), 0);

        assert_eq!(count_transactions(&pool).await, 2);
        assert_eq!(count_sync_log(&pool, "2026").await, 2);
    }

    // --- Merge de 3 vias (drift por célula + gate de conflito) ---

    #[tokio::test]
    async fn local_edit_preserved_when_only_local_changed() {
        let pool = test_pool().await;
        // rowB existe só para mudar o checksum do batch no re-import (rowA fica igual na planilha).
        let v1 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -1_000),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Usuário corrige o valor de A localmente (base=10000 → local=15000).
        set_local_amount(&pool, "2026-01-05", 15_000).await;

        // Re-import: A com a MESMA célula (10000), B mudou → batch difere, merge roda em A.
        let v2 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -2_000),
        ];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        assert_eq!(
            amount_by_date(&pool, "2026-01-05").await,
            15_000,
            "edição local preservada"
        );
        assert_eq!(
            conflict_count(&pool).await,
            0,
            "sem conflito: só o local mudou"
        );
    }

    #[tokio::test]
    async fn sheet_update_applied_when_no_local_edit() {
        let pool = test_pool().await;
        import_rows(&pool, "2026", &[imported("2026-01-05", 10_000)], "p1")
            .await
            .unwrap();
        // Planilha mudou a célula; local intacto → aplica a planilha.
        import_rows(&pool, "2026", &[imported("2026-01-05", 12_000)], "p1")
            .await
            .unwrap();
        assert_eq!(amount_by_date(&pool, "2026-01-05").await, 12_000);
        assert_eq!(conflict_count(&pool).await, 0);
    }

    #[tokio::test]
    async fn untrusted_descriptions_do_not_overwrite_existing_description_or_source() {
        let pool = test_pool().await;
        import_rows(
            &pool,
            "2026",
            &[imported_desc("2026-01-05", -10_000, "Nota sintética")],
            "p1",
        )
        .await
        .unwrap();

        import_rows_with_options(
            &pool,
            "2026",
            &[imported_desc("2026-01-05", -12_000, "Saída 2026-01-05")],
            "p1",
            ImportRowsOptions {
                descriptions_trusted: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(amount_by_date(&pool, "2026-01-05").await, 12_000);
        assert_eq!(
            description_and_source(&pool, "2026-01-05").await,
            (Some("Nota sintética".into()), Some("Nota sintética".into()))
        );
        assert_eq!(conflict_count(&pool).await, 0);
    }

    #[tokio::test]
    async fn untrusted_descriptions_insert_new_rows_without_generic_source() {
        let pool = test_pool().await;
        import_rows_with_options(
            &pool,
            "2026",
            &[imported_desc("2026-01-05", -12_000, "Saída 2026-01-05")],
            "p1",
            ImportRowsOptions {
                descriptions_trusted: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            description_and_source(&pool, "2026-01-05").await,
            (None, None)
        );
    }

    #[tokio::test]
    async fn conflict_recorded_when_both_changed() {
        let pool = test_pool().await;
        import_rows(&pool, "2026", &[imported("2026-01-05", 10_000)], "p1")
            .await
            .unwrap();
        set_local_amount(&pool, "2026-01-05", 15_000).await; // edição local

        // Planilha foi para outro valor (20000) → ambos divergem do base (10000) → conflito.
        import_rows(&pool, "2026", &[imported("2026-01-05", 20_000)], "p1")
            .await
            .unwrap();

        assert_eq!(
            amount_by_date(&pool, "2026-01-05").await,
            15_000,
            "não sobrescreve o local"
        );
        let conflicts = crate::conflicts::list_conflicts(&pool).await.unwrap();
        let amt = conflicts.iter().find(|c| c.field == "amount").unwrap();
        assert_eq!(amt.base_value.as_deref(), Some("10000"));
        assert_eq!(amt.local_value, "15000");
        assert_eq!(amt.sheet_value, "20000");

        // Resolvendo pela planilha, o valor passa a 20000 e o conflito some.
        crate::conflicts::resolve(&pool, &amt.id, "sheet")
            .await
            .unwrap();
        assert_eq!(amount_by_date(&pool, "2026-01-05").await, 20_000);
        assert!(
            crate::conflicts::list_conflicts(&pool)
                .await
                .unwrap()
                .is_empty()
        );
    }

    // Reimportar uma planilha editada substitui as linhas determinísticas afetadas sem duplicar as
    // demais, mesmo que o checksum cubra o lote inteiro.
    #[tokio::test]
    async fn reimport_after_edit_replaces_instead_of_duplicating() {
        let pool = test_pool().await;
        let v1 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // O dono lança um gasto novo na planilha e re-importa.
        let v2 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
            imported("2026-01-07", -2_500),
        ];
        assert_eq!(import_rows(&pool, "2026", &v2, "p1").await.unwrap(), 3);

        assert_eq!(count_transactions(&pool).await, 3);
        assert_eq!(count_sync_log(&pool, "2026").await, 3);
    }

    // Reimportar uma célula editada preserva o id determinístico e o enriquecimento ancorado nele
    // (`payment_method`, splits e tags), enquanto atualiza os campos vindos da planilha.
    #[tokio::test]
    async fn reimport_preserves_transaction_identity_and_enrichment() {
        let pool = test_pool().await;
        import_rows(&pool, "2026", &[imported("2026-01-05", 10_000)], "p1")
            .await
            .unwrap();

        let (id_before,): (String,) =
            sqlx::query_as("SELECT id FROM \"transaction\" WHERE date='2026-01-05'")
                .fetch_one(&pool)
                .await
                .unwrap();
        // Enriquecimento numa coluna que o import NÃO escreve.
        sqlx::query("UPDATE \"transaction\" SET payment_method='credit' WHERE id=?1")
            .bind(&id_before)
            .execute(&pool)
            .await
            .unwrap();

        // O dono edita o VALOR na planilha e re-importa.
        import_rows(&pool, "2026", &[imported("2026-01-05", 12_345)], "p1")
            .await
            .unwrap();

        let (id_after, amount, pm): (String, i64, Option<String>) = sqlx::query_as(
            "SELECT id, amount, payment_method FROM \"transaction\" WHERE date='2026-01-05'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(id_after, id_before, "id estável (determinístico)");
        assert_eq!(amount, 12_345, "valor atualizado em vigor (UPSERT)");
        assert_eq!(
            pm.as_deref(),
            Some("credit"),
            "enriquecimento sobrevive ao re-import"
        );
        assert_eq!(count_transactions(&pool).await, 1, "sem duplicar");
    }

    // WRONG #2 (parte do import): Saída entra com is_fixed=1 (→ FixedOut no engine), não Diário.
    #[tokio::test]
    async fn import_sets_is_fixed_for_saida() {
        let pool = test_pool().await;
        import_rows(&pool, "2026", &[imported("2026-01-05", -5_000)], "p1")
            .await
            .unwrap();
        let (is_fixed, ttype): (i64, String) =
            sqlx::query_as("SELECT is_fixed, type FROM \"transaction\" WHERE date='2026-01-05'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(ttype, "expense");
        assert_eq!(is_fixed, 1, "Saída = estilo de vida fixo");
    }

    // A coluna Diário é importada quando mapeada (amount_daily) → RowKind::Diario (variável).
    #[test]
    fn parse_imports_diario_column() {
        // Coluna 0 = dia (absoluto); bloco JANEIRO no offset 1 (Entrada=2, Saída=3, Diário=4).
        let rows = vec![
            vec![
                "".into(),
                "JANEIRO".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
            ],
            vec![
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
                "".into(),
            ],
            vec![
                "1".into(),
                "".into(),
                "".into(),
                "".into(),
                "30".into(),
                "".into(),
            ],
        ];
        let layout = SheetLayout {
            id: "t".into(),
            sheet_name: "2025".into(),
            year: Some(2025),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "past_only".into(),
        };
        let mappings = vec![
            ("amount_in".to_string(), 1),
            ("amount_out".to_string(), 2),
            ("amount_daily".to_string(), 3),
        ];
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, RowKind::Diario);
        assert_eq!(result[0].amount, -3000); // −R$30,00 (variável)
        assert_eq!(result[0].date, "2025-01-01");
    }

    #[tokio::test]
    async fn reimport_drops_rows_removed_from_sheet() {
        let pool = test_pool().await;
        let v1 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
            imported("2026-01-07", -2_500),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        let v2 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
        ];
        assert_eq!(import_rows(&pool, "2026", &v2, "p1").await.unwrap(), 2);

        assert_eq!(count_transactions(&pool).await, 2);
    }

    // Trava a LIMITAÇÃO CONHECIDA do slot posicional (ver doc de `row_id`). Inalcançável no grid
    // canônico do método (1 linha por data×kind); aqui forçamos 2 linhas mesma (aba,data,kind) para
    // documentar que, removida a 1ª, o enriquecimento segue o SLOT (não os dados) — a sobrevivente
    // assume o slot 0/id da removida e herda a tag. Se um dia ancorarmos id em (linha,coluna), este
    // teste muda de propósito.
    #[tokio::test]
    async fn slot_identity_is_positional_known_limitation() {
        let pool = test_pool().await;
        // Duas Saídas no MESMO dia (planilha malformada): slots 0 e 1.
        let v1 = vec![
            imported("2026-01-06", -5_000),
            imported("2026-01-06", -7_000),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();
        assert_eq!(count_transactions(&pool).await, 2);

        // Enriquece o SLOT 0 com uma tag.
        let id0 = row_id("2026", "2026-01-06", RowKind::Saida, 0);
        sqlx::query("INSERT INTO tag (id, name, color) VALUES ('tg','T','c')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1,'tg')")
            .bind(&id0)
            .execute(&pool)
            .await
            .unwrap();

        // Re-import com só UMA Saída no dia (a 1ª sumiu) → a sobrevivente toma o slot 0 = id0.
        let v2 = vec![imported("2026-01-06", -7_000)];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();
        assert_eq!(count_transactions(&pool).await, 1);

        // Comportamento documentado: id0 sobrevive com os DADOS da sobrevivente (7.000) e ainda
        // carrega a tag — o enriquecimento é posicional (segue o slot, não a linha original).
        let (amount, tags): (i64, i64) = sqlx::query_as(
            "SELECT t.amount, (SELECT COUNT(*) FROM transaction_tag WHERE transaction_id = t.id) \
             FROM \"transaction\" t WHERE t.id = ?1",
        )
        .bind(&id0)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(amount, 7_000, "id0 carrega os dados da sobrevivente");
        assert_eq!(
            tags, 1,
            "enriquecimento seguiu o slot (limitação conhecida)"
        );
    }

    #[tokio::test]
    async fn replace_is_scoped_to_its_own_sheet() {
        let pool = test_pool().await;
        let aba_2025 = vec![
            imported("2025-03-01", 7_000),
            imported("2025-03-02", -1_000),
        ];
        let aba_2026 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
        ];
        import_rows(&pool, "2025", &aba_2025, "p1").await.unwrap();
        import_rows(&pool, "2026", &aba_2026, "p1").await.unwrap();

        // Re-import só da aba 2026, editada — 2025 fica intacta.
        let aba_2026_v2 = vec![
            imported("2026-01-05", 10_000),
            imported("2026-01-06", -5_000),
            imported("2026-01-07", -2_500),
        ];
        import_rows(&pool, "2026", &aba_2026_v2, "p1")
            .await
            .unwrap();

        assert_eq!(count_transactions(&pool).await, 5);
        assert_eq!(count_sync_log(&pool, "2025").await, 2);
        assert_eq!(count_sync_log(&pool, "2026").await, 3);

        let (count_2025,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date LIKE '2025-%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count_2025, 2);
    }

    // O import é tudo-ou-nada numa única transação. Se algo falha entre a fase de
    // linhas e a de Saldo, o rollback desfaz TUDO — zero linhas, zero saldo — e o gate de
    // duplicata NÃO é envenenado (o sync_log da tentativa revertida some junto), para o retry
    // poder reimportar.
    #[tokio::test]
    async fn atomic_import_rolls_back_on_balance_error() {
        let pool = test_pool().await;
        let rows = vec![imported("2026-03-01", 50_000)];

        // Primeiro: um import normal teria sucesso; aqui escrevemos as linhas e então simulamos
        // uma falha na fase de Saldo fazendo rollback explícito (= crash no meio do import).
        let checksum = compute_import_checksum(&rows, true);
        assert!(
            !check_duplicate_import(&pool, "2026", &checksum)
                .await
                .unwrap()
        );
        {
            let mut tx = pool.begin().await.unwrap();
            let n = import_rows_with_options_in_tx(
                &mut tx,
                "2026",
                &rows,
                "p1",
                ImportRowsOptions::default(),
                &checksum,
            )
            .await
            .unwrap();
            assert_eq!(n, 1);
            tx.rollback().await.unwrap();
        }

        // Após o rollback: zero transações, série de Saldo vazia.
        assert_eq!(count_transactions(&pool).await, 0);
        let (bal_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            bal_count, 0,
            "série de Saldo também ausente após o rollback"
        );

        // O gate de duplicata continua false: o sync_log da tentativa revertida não pode bloquear
        // o retry.
        assert!(
            !check_duplicate_import(&pool, "2026", &checksum)
                .await
                .unwrap(),
            "import revertido não pode envenenar o gate de duplicata"
        );
    }

    // Um import bem-sucedido comita linhas E série de Saldo juntas na mesma transação;
    // ambas ficam legíveis após o commit.
    #[tokio::test]
    async fn atomic_import_commits_rows_and_balance_together() {
        let pool = test_pool().await;
        let rows = vec![imported("2026-04-01", 30_000)];
        let series = vec![DailyBalance {
            date: "2026-04-01".into(),
            balance_cents: 30_000,
            is_projection: false,
        }];
        let checksum = compute_import_checksum(&rows, true);

        let mut tx = pool.begin().await.unwrap();
        import_rows_with_options_in_tx(
            &mut tx,
            "2026",
            &rows,
            "p1",
            ImportRowsOptions::default(),
            &checksum,
        )
        .await
        .unwrap();
        store_balance_series_in_tx(&mut tx, "2026", &series)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(count_transactions(&pool).await, 1);
        let (bal_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            bal_count, 1,
            "série de Saldo comitada junto com as transações"
        );
    }

    // O insert em lote grava o mesmo conjunto de linhas que o loop linha-a-linha e o
    // re-import (DELETE + lote) substitui atomicamente — inclusive com slice vazio (sem placeholders
    // = sem query, sem panic), preservando as linhas já gravadas pelo DELETE da chamada anterior.
    #[tokio::test]
    async fn store_balance_series_batches_400_rows() {
        let pool = test_pool().await;
        let series: Vec<DailyBalance> = (0..400)
            .map(|i| DailyBalance {
                // datas únicas e ISO-válidas; o valor exato não importa para a contagem
                date: format!("2026-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1),
                balance_cents: 1_000 + i as i64,
                is_projection: i % 2 == 0,
            })
            .collect();

        let n = store_balance_series(&pool, "Jan", &series).await.unwrap();
        assert_eq!(n, 400, "store_balance_series deve reportar 400 linhas");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance WHERE sheet_name = 'Jan'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 400, "400 linhas gravadas pelo lote");

        // Re-import com slice vazio: DELETE roda, nenhum INSERT (chunk vazio não gera query),
        // commit OK. A aba 'Jan' fica zerada — replace-all atômico, sem corromper a tabela.
        let n_empty = store_balance_series(&pool, "Jan", &[]).await.unwrap();
        assert_eq!(n_empty, 0);
        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance WHERE sheet_name = 'Jan'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 0, "slice vazio limpa a aba sem erro");
    }

    // ===================================================================
    // Testes de integração da gramática das notas (DB)
    // ===================================================================

    #[tokio::test]
    async fn import_reembolso_creates_compensating_entrada() {
        // #reembolso: gera uma Entrada compensatória; cashflow líquido = zero.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-10".into(),
            amount: -53000, // R$530 Saída
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // A Entrada compensatória deve existir.
        let (tipo, amount, desc): (String, i64, String) = sqlx::query_as(
            "SELECT type, amount, description FROM \"transaction\" \
             WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tipo, "income");
        assert_eq!(amount, 53000);
        assert!(desc.contains("Pessoa B"), "descrição menciona a pessoa");

        // Cashflow líquido: Saída 530 + Entrada 530 = 0.
        let (net,): (i64,) = sqlx::query_as(
            "SELECT SUM(CASE type WHEN 'income' THEN amount ELSE -amount END) \
             FROM \"transaction\" WHERE date = '2026-01-10'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(net, 0, "cashflow líquido deve ser zero");
    }

    #[tokio::test]
    async fn import_dividir_creates_split_and_compensating_entrada() {
        // #dividir: gera split + Entrada compensatória pela parte de <quem>.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-15".into(),
            amount: -20000, // R$200 Saída
            description: "Almoço".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 200,00 - Almoço #dividir:Pessoa A".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // Split criado com 50% do valor.
        let (split_amount,): (i64,) = sqlx::query_as(
            "SELECT s.amount FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             WHERE LOWER(p.name) = 'pessoa a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(split_amount, 10000, "50% de R$200");

        // Entrada compensatória para 50%.
        let (tipo, amount): (String, i64) = sqlx::query_as(
            "SELECT type, amount FROM \"transaction\" \
             WHERE id LIKE 'derived:dividir:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tipo, "income");
        assert_eq!(amount, 10000);
    }

    #[tokio::test]
    async fn import_dividir_explicit_value() {
        // #dividir:<quem>:<valor> usa o valor explícito.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-02-01".into(),
            amount: -20000,
            description: "Almoço".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 200,00 - Almoço #dividir:Pessoa A:80,00".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (split_amount,): (i64,) = sqlx::query_as(
            "SELECT s.amount FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             WHERE LOWER(p.name) = 'pessoa a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(split_amount, 8000, "valor explícito R$80");
    }

    #[tokio::test]
    async fn import_multiple_tagged_lines_same_note() {
        // Nota com duas linhas marcadas: dois reembolsos independentes.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-03-01".into(),
            amount: -72100, // R$721 total
            description: "Múltiplas despesas".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B\n\
                       R$ 1.200,00 - Parcela carro\n\
                       R$ 191,00 - Empréstimo Pessoa C #reembolso:Pessoa C"
                .into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // Duas Entradas compensatórias.
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn reimport_replaces_derived_rows_idempotently() {
        // Re-import substitui as Entradas derivadas e splits (idempotente).
        let pool = test_pool().await;

        let v1 = vec![ImportedRow {
            date: "2026-04-01".into(),
            amount: -53000,
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
        }];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Segundo import com nota diferente.
        let v2 = vec![ImportedRow {
            date: "2026-04-01".into(),
            amount: -53000,
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa C".into(),
        }];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        // Deve haver exatamente uma Entrada derivada (a do segundo import).
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "re-import substituiu a Entrada derivada");

        let (desc,): (String,) =
            sqlx::query_as("SELECT description FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            desc.contains("Pessoa C"),
            "nova Entrada aponta para Pessoa C"
        );
    }

    #[tokio::test]
    async fn diff_delete_removes_derived_rows() {
        // Quando a transação pai é removida pelo diff-delete, as Entradas derivadas também somem.
        // O diff-delete só roda num re-import NÃO-vazio (slice vazio é no-op em `import_rows`),
        // então re-importamos uma lista que OMITE a linha marcada — espelhando uma linha que
        // sumiu da planilha (igual a `reimport_drops_rows_removed_from_sheet`).
        let pool = test_pool().await;

        let v1 = vec![
            ImportedRow {
                date: "2026-05-01".into(),
                amount: -53000,
                description: "Cartões Pessoa B".into(),
                is_projection: false,
                kind: RowKind::Saida,
                raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
            },
            // Linha-âncora sem nota: mantém a aba não-vazia no re-import seguinte.
            imported("2026-05-02", -1_000),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Confirma que a Entrada derivada existe.
        let (before,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, 1);

        // Re-import que OMITE a linha marcada → diff-delete remove a transação pai.
        let v2 = vec![imported("2026-05-02", -1_000)];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 0, "Entrada derivada removida junto com o pai");
    }

    #[tokio::test]
    async fn diff_delete_removes_orphan_import_conflict() {
        // Uma linha removida da planilha não pode deixar um conflito de import aberto órfão, pois
        // esse conflito bloquearia o write-back.
        let pool = test_pool().await;

        // Importa duas linhas: a primeira recebe o conflito; a segunda é a âncora que mantém o
        // re-import seguinte não-vazio (o diff-delete só roda em re-import não-vazio).
        let v1 = vec![
            imported("2026-03-01", -10_000),
            imported("2026-03-02", -5_000),
        ];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Id da transação criada pelo import (SHA-256 hex, sem prefixo `derived:`).
        let (txn_id,): (String,) =
            sqlx::query_as("SELECT id FROM \"transaction\" WHERE date = '2026-03-01'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Insere um conflito EM ABERTO (resolved_at NULL) para essa transação.
        let conf_id = format!("conf:{txn_id}:amount");
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
             VALUES (?1, ?2, 'amount', '10000', '12000', '11000', '2026-03-01T00:00:00Z')",
        )
        .bind(&conf_id)
        .bind(&txn_id)
        .execute(&pool)
        .await
        .unwrap();

        let (before,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE transaction_id = ?1")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, 1, "conflito existe antes do re-import");

        // Re-import que OMITE a linha do conflito (simula a linha removida da planilha).
        let v2 = vec![imported("2026-03-02", -5_000)];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        // Conflito órfão deve sumir junto com a transação removida pelo diff-delete.
        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE transaction_id = ?1")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 0, "conflito órfão removido pelo diff-delete");
    }

    #[tokio::test]
    async fn import_no_note_leaves_no_derived_rows_and_no_splits() {
        // PROVA DE SEGURANÇA: nota ausente → comportamento idêntico ao de hoje.
        let pool = test_pool().await;
        let rows = vec![imported("2026-06-01", -10000)];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (derived,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(derived, 0, "sem nota → sem Entradas derivadas");

        let (splits,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM split")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(splits, 0, "sem nota → sem splits");
    }

    #[tokio::test]
    async fn import_unmarked_prose_note_leaves_no_derived_rows() {
        // PROVA DE SEGURANÇA reforçada: nota de prosa livre real NÃO dispara marcadores.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-06-02".into(),
            amount: -72100,
            description: "Contas mensais".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa D\nR$ 1.200,00 - Parcela carro\n\
                       R$ 191,00 - Empréstimo Viagem Pessoa E"
                .into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (derived,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(derived, 0, "prosa livre → sem Entradas derivadas");

        let (splits,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM split")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(splits, 0, "prosa livre → sem splits");
    }

    #[tokio::test]
    async fn import_person_created_on_demand_for_reembolso() {
        // Pessoa não-existente é criada sob demanda na mesma transação DB.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-07-01".into(),
            amount: -10000,
            description: "Teste".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 100,00 - Teste #reembolso:Nova Pessoa".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name) = 'nova pessoa'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "pessoa criada sob demanda");
    }

    #[tokio::test]
    async fn import_person_reuse_case_insensitive() {
        // Pessoa pré-existente é reutilizada (sem duplicata) mesmo com caixa diferente.
        let pool = test_pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pid-pa', 'Pessoa A')")
            .execute(&pool)
            .await
            .unwrap();

        let rows = vec![ImportedRow {
            date: "2026-08-01".into(),
            amount: -10000,
            description: "Teste".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 100,00 - Teste #reembolso:PESSOA A".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name) = 'pessoa a'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "nenhuma pessoa duplicada criada");
    }

    // --- Persistência de line_item no import (camada DB) ---

    // Happy path: nota com 2 linhas R$ cujo somatório bate com o total → itens gravados.
    #[tokio::test]
    async fn line_items_stored_when_note_sums_match_total() {
        let pool = test_pool().await;
        // Saída de R$ 150,00; nota: R$ 100,00 + R$ 50,00 = R$ 150,00 (bate).
        let rows = vec![imported_note(
            "2026-02-10",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-10", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 2);

        // O total do pai NÃO mudou (continua a magnitude da célula).
        assert_eq!(amount_by_date(&pool, "2026-02-10").await, 15_000);
    }

    // Mismatch: partes não somam o total → o breakdown SOBREVIVE (a classificação é o que
    // importa; antes era descartado inteiro) sem nenhum item sintético persistido — o resíduo
    // é reconciliado com sinal no loader de métricas e o write-back cai para RAW.
    #[tokio::test]
    async fn line_items_sum_mismatch_keeps_breakdown_without_synthetic() {
        let pool = test_pool().await;
        // Total R$ 100,00; nota soma R$ 120,00 → os 2 itens persistem como estão.
        let rows = vec![imported_note(
            "2026-02-11",
            -10_000,
            "R$ 60,00 - A\nR$ 60,00 - B",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-11", RowKind::Saida, 0);
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            2,
            "sem item sintético"
        );
        let (diferenca_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1 AND description = 'Diferença'",
        )
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(diferenca_count, 0);
        // O total do pai segue intocado (a célula é a verdade).
        assert_eq!(amount_by_date(&pool, "2026-02-11").await, 10_000);
    }

    // Uma divergência entre itens e célula gera exatamente um diagnóstico
    // `ItemsDoNotSumToCell` sem impedir a persistência dos itens; a célula permanece dona do total.
    #[tokio::test]
    async fn diagnostics_report_sum_mismatch_while_items_still_persist() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-11",
            -10_000,
            "R$ 60,00 - A\nR$ 60,00 - B",
            false,
        )];

        let diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagKind::ItemsDoNotSumToCell);
        assert!(
            diagnostics[0].detail.contains("R$ 100,00"),
            "total da célula"
        );
        assert!(
            diagnostics[0].detail.contains("R$ 120,00"),
            "soma dos itens"
        );

        import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        let txn_id = row_id("2026", "2026-02-11", RowKind::Saida, 0);
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            2,
            "itens persistem apesar da divergência — a célula continua dona do total"
        );
    }

    // O diagnóstico é função do lote parseado, portanto permanece disponível mesmo quando um
    // reimport idêntico é deduplicado por checksum e não grava nada.
    #[tokio::test]
    async fn diagnostics_survive_checksum_deduped_reimport() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-20",
            -10_000,
            "R$ 60,00 - A\nR$ 60,00 - B",
            false,
        )];

        let first_write = import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        assert_eq!(first_write, 1, "1ª rodada escreve de fato");
        let first_diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(first_diagnostics.len(), 1);

        // 2ª rodada: mesmo checksum → check_duplicate_import bate e import_rows_with_options
        // retorna Ok(0) sem tocar o banco (ver import_rows_with_options acima).
        let second_write = import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        assert_eq!(
            second_write, 0,
            "dedup: dataset idêntico, nada escrito de novo"
        );
        let second_diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(
            second_diagnostics, first_diagnostics,
            "o diagnóstico sobrevive ao skip de checksum — é função do lote, não da escrita"
        );
    }

    // Memo de UMA linha sem cabeçalho de seção NÃO é breakdown: persistir migraria um
    // Diário/Cartão para Saída fixa via classify_line_item(None).
    #[tokio::test]
    async fn line_items_single_memo_without_section_not_stored() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-18",
            -5_000,
            "R$ 50,00 - Mercado",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-18", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 0);
    }

    // Re-import idêntico: mesmos itens, sem duplicar (clear-then-reinsert).
    #[tokio::test]
    async fn line_items_are_idempotent_on_reimport() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-12",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-12", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 2);
    }

    // Mudança de nota no re-import: itens atualizados (2 → 3).
    #[tokio::test]
    async fn line_items_update_on_note_change() {
        let pool = test_pool().await;
        let first = vec![imported_note(
            "2026-02-13",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        import_rows(&pool, "2026", &first, "p1").await.unwrap();

        // Nova nota com 3 partes somando o mesmo total (R$ 150,00).
        let second = vec![imported_note(
            "2026-02-13",
            -15_000,
            "R$ 50,00 - Parte A\nR$ 50,00 - Parte B\nR$ 50,00 - Parte C",
            false,
        )];
        import_rows(&pool, "2026", &second, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-13", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 3);
    }

    // Seguro por padrão: nota vazia → 0 itens, sem erro.
    #[tokio::test]
    async fn line_items_empty_note_inserts_none() {
        let pool = test_pool().await;
        let rows = vec![imported_note("2026-02-14", -10_000, "", false)];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-14", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 0);
    }

    // Lançamento PROJETADO (futuro) também recebe os itens da nota.
    #[tokio::test]
    async fn line_items_stored_for_projected_rows() {
        let pool = test_pool().await;
        // Entrada projetada de R$ 300,00; nota soma R$ 300,00.
        let rows = vec![imported_note(
            "2099-12-01",
            30_000,
            "R$ 100,00 - Parte A\nR$ 200,00 - Parte B",
            true,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2099-12-01", RowKind::Entrada, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 2);

        // Confirma que a linha é mesmo projeção.
        let (is_proj,): (i64,) =
            sqlx::query_as("SELECT is_projection FROM \"transaction\" WHERE id = ?1")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(is_proj, 1);
    }

    // Uma única parte sob cabeçalho de seção é um breakdown classificável; uma única parte sem
    // seção não é persistida, evitando um item fantasma classificado como Saída.
    #[tokio::test]
    async fn line_items_single_part_stored_and_classified() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-15",
            -10_000,
            "ECONOMIA\nR$ 100,00 - Poupança do mês",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-15", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 1);
        let (section,): (Option<String>,) =
            sqlx::query_as("SELECT section FROM line_item WHERE transaction_id = ?1")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            classify_line_item(section.as_deref(), ""),
            ItemKind::Economia
        );
    }

    // Ciclo com notas indisponíveis (falha da API de notas / .xlsx) NÃO
    // pode destruir os itens classificados nem a base `source_note` do último import bom.
    #[tokio::test]
    async fn untrusted_notes_preserve_line_items_and_source_note() {
        let pool = test_pool().await;
        let note = "CARTÕES\nR$ 100,00 - Nubank\nR$ 50,00 - Inter";
        let rows = vec![imported_note("2026-02-16", -15_000, note, false)];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-16", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 2);

        // Ciclo degradado: mesma linha, raw_note vazia, notas NÃO confiáveis.
        import_rows_with_options(
            &pool,
            "2026",
            &[imported_desc("2026-02-16", -15_000, "Linha 2026-02-16")],
            "p1",
            ImportRowsOptions {
                descriptions_trusted: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            2,
            "itens classificados sobrevivem ao ciclo sem notas"
        );
        let (source_note,): (Option<String>,) =
            sqlx::query_as(r#"SELECT source_note FROM "transaction" WHERE id = ?1"#)
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            source_note.as_deref(),
            Some(note),
            "a base da nota não é clobberada pelo ciclo degradado"
        );
    }

    // Num ciclo degradado a raw_note (vazia) não entra no checksum —
    // senão o guard de idempotência quebrava e o re-import destrutivo rodava sempre.
    #[test]
    fn checksum_ignores_raw_note_when_untrusted() {
        let with_note = vec![imported_note("2026-02-17", -10_000, "R$ 100,00 - X", false)];
        let without_note = vec![imported_desc("2026-02-17", -10_000, "Linha 2026-02-17")];
        assert_eq!(
            compute_import_checksum(&with_note, false),
            compute_import_checksum(&without_note, false)
        );
        assert_ne!(
            compute_import_checksum(&with_note, true),
            compute_import_checksum(&without_note, true)
        );
    }

    // Edição LOCAL das partes sobrevive ao re-import enquanto a nota da planilha não
    // muda. O importer só re-deriva da nota quando `source_note` (base) difere da nota atual.
    #[tokio::test]
    async fn user_edited_items_survive_reimport_when_note_unchanged() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-04-10",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        let txn_id = row_id("2026", "2026-04-10", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 2);

        // O dono EDITA as partes no app: 3 partes, marcadas is_user_edited = 1.
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind(&txn_id)
            .execute(&pool)
            .await
            .unwrap();
        for (i, amt) in [(0, 5000), (1, 5000), (2, 5000)] {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
                 VALUES (?1, ?2, ?3, 'editado', ?4, 1)",
            )
            .bind(format!("user:{txn_id}:{i}"))
            .bind(&txn_id)
            .bind(amt)
            .bind(i)
            .execute(&pool)
            .await
            .unwrap();
        }
        assert_eq!(count_line_items(&pool, &txn_id).await, 3);

        // Re-import com a MESMA nota → as 3 partes editadas pelo dono PERMANECEM (nota inalterada).
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            3,
            "itens editados sobrevivem ao re-import com nota inalterada"
        );
        let (edited,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1 AND is_user_edited = 1",
        )
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(edited, 3, "a marca de edição local é preservada");
    }

    // Quando a NOTA da planilha muda, ela vence — re-deriva e descarta a edição local
    // (a nota é autoritativa; o dono deve editar a planilha primeiro, depois refinar no app).
    #[tokio::test]
    async fn user_edited_items_overwritten_when_note_changes() {
        let pool = test_pool().await;
        let first = vec![imported_note(
            "2026-04-11",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        import_rows(&pool, "2026", &first, "p1").await.unwrap();
        let txn_id = row_id("2026", "2026-04-11", RowKind::Saida, 0);

        // Edição local (1 parte marcada como editada).
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind(&txn_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
             VALUES (?1, ?2, 15000, 'só local', 0, 1)",
        )
        .bind(format!("user:{txn_id}:0"))
        .bind(&txn_id)
        .execute(&pool)
        .await
        .unwrap();

        // Re-import com a nota MUDADA (mesmo total, 3 partes) → a nota vence: 3 itens derivados.
        let second = vec![imported_note(
            "2026-04-11",
            -15_000,
            "R$ 50,00 - Parte A\nR$ 50,00 - Parte B\nR$ 50,00 - Parte C",
            false,
        )];
        import_rows(&pool, "2026", &second, "p1").await.unwrap();
        assert_eq!(count_line_items(&pool, &txn_id).await, 3);
        let (edited,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1 AND is_user_edited = 1",
        )
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            edited, 0,
            "nota nova é autoritativa: itens re-derivados (não-editados)"
        );
    }

    #[tokio::test]
    async fn zero_note_items_persist_as_placeholders_only_on_projected_rows() {
        let pool = test_pool().await;
        let note = "CARTÕES:\nR$ 0,00 - Banco A\nR$ 150,00 - Banco B";
        let rows = vec![
            imported_note("2099-12-01", -15_000, note, true),
            imported_note("2026-02-10", -15_000, note, false),
        ];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let projected_id = row_id("2026", "2099-12-01", RowKind::Saida, 0);
        let realized_id = row_id("2026", "2026-02-10", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &projected_id).await, 2);
        assert_eq!(count_line_items(&pool, &realized_id).await, 1);

        let (zero_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1 AND amount_cents = 0",
        )
        .bind(&projected_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(zero_count, 1);
    }
}
