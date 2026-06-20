use super::layout_detect::{SheetLayout, month_number_from_name};
use super::reconcile;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

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

/// Coluna do método de onde a linha veio. Define o tipo/is_fixed na transação E ancora a
/// identidade determinística (a posição estável: aba + dia + coluna).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Entrada,
    Saida,
    Diario,
}

impl RowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RowKind::Entrada => "entrada",
            RowKind::Saida => "saida",
            RowKind::Diario => "diario",
        }
    }
    /// `transaction.type`.
    pub fn txn_type(self) -> &'static str {
        match self {
            RowKind::Entrada => "income",
            RowKind::Saida | RowKind::Diario => "expense",
        }
    }
    /// Saída = estilo de vida FIXO (→ FixedOut no engine); Diário = variável (→ Daily).
    pub fn is_fixed(self) -> bool {
        matches!(self, RowKind::Saida)
    }
}

#[derive(Debug)]
pub struct ImportedRow {
    pub date: String,
    pub amount: i64,
    pub description: String,
    pub is_projection: bool,
    pub kind: RowKind,
    /// Nota de célula CRUA (multi-linha, preservando `\n`). Usada por
    /// `import_rows_core` para extrair splits de titular e `payment_method` via
    /// `parse_note_markers`. String vazia quando não há nota (path xlsx ou célula
    /// sem comentário) → sem marcadores, comportamento idêntico ao de hoje.
    pub raw_note: String,
}

pub fn classify_row(date_str: &str, date_direction: &str) -> Result<bool, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_past = date_str < today.as_str();

    match date_direction {
        "past_only" => Ok(false),
        "future_only" => Ok(true),
        "both" => Ok(!is_past),
        _ => Err(format!("unknown date_direction: {date_direction}")),
    }
}

// API pública retida (plan 002): o shell migrou para as variantes `*_in_tx` (transação externa
// única), então estes wrappers de pool agora só têm chamadores nos testes. Mantidos de propósito
// como API estável para os testes e chamadores futuros (o módulo `google_sheets` é privado no
// crate, então o `dead_code` dispara sem o allow).
#[allow(dead_code)]
pub fn compute_checksum(rows: &[ImportedRow]) -> String {
    compute_checksum_with_options(rows, true)
}

fn compute_checksum_with_options(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.date.as_bytes());
        hasher.update(row.amount.to_le_bytes());
        if descriptions_trusted {
            hasher.update(row.description.as_bytes());
        }
        hasher.update([row.is_projection as u8]);
        hasher.update(row.kind.as_str().as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Id DETERMINÍSTICO de uma linha importada = `sha256(aba|data|kind|slot)`. Não inclui valor nem
/// descrição → editar o valor/nota na planilha **preserva** o id (o UPSERT atualiza em vigor e o
/// enriquecimento — split, tags, payment_method — sobrevive). `slot` desempata o caso raro de
/// mais de uma linha com a mesma (aba,data,kind).
///
/// ÂNCORAS data+kind (limitação aceita): como `data` e `kind` ENTRAM no id, editar o DIA de um
/// lançamento na planilha (ou mover o valor da coluna Saída para Diário) recomputa o id; o diff-
/// delete remove o id antigo (com seu enriquecimento) e insere o novo "pelado". É o trade-off do
/// modelo de identidade — edições de VALOR/NOTA (o caso comum) são preservadas; edições de
/// dia/coluna não. Re-anexar o enriquecimento ao mudar o dia é um endurecimento futuro.
///
/// LIMITAÇÃO CONHECIDA (slot posicional): `slot` é atribuído pela ordem de aparição. Se houver
/// 2+ linhas com a mesma (aba,data,kind) e a 1ª for removida da planilha, a sobrevivente herda o
/// `slot` (e o id) da removida, migrando o enriquecimento para os dados errados. Inalcançável no
/// grid canônico do método (1 célula por dia×coluna → no máximo 1 linha por (data,kind); ver
/// `parse_rows_with_layout`); só ocorre em planilha malformada com dias duplicados. NÃO ancoramos
/// em (linha,coluna) física de propósito: mudaria o esquema do id e regeneraria TODOS os ids no
/// próximo import, órfãos o enriquecimento de quem já importou. Travado pelo teste
/// `slot_identity_is_positional_known_limitation`.
pub fn row_id(sheet: &str, date: &str, kind: RowKind, slot: usize) -> String {
    let mut h = Sha256::new();
    h.update(b"txn-v1|");
    h.update(sheet.as_bytes());
    h.update(b"|");
    h.update(date.as_bytes());
    h.update(b"|");
    h.update(kind.as_str().as_bytes());
    h.update(b"|");
    h.update(slot.to_le_bytes());
    hex::encode(h.finalize())
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

// API pública retida (plan 002) — ver nota em `compute_checksum`. Wrapper de pool usado pelos
// testes; o shell usa `import_rows_with_options_in_tx` na transação externa.
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

/// Calcula o checksum de idempotência do batch da MESMA forma que `import_rows_with_options`,
/// para o shell (commands.rs) poder rodar `check_duplicate_import` ANTES de abrir a transação
/// externa (a checagem é uma leitura no pool e não pode acontecer dentro da tx — read-your-writes
/// daria falso-negativo).
pub(crate) fn compute_import_checksum(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    compute_checksum_with_options(rows, descriptions_trusted)
}

// API pública retida (plan 002) — ver nota em `compute_checksum`. Wrapper de pool (begin→core→
// commit) usado pelos testes; o shell usa `import_rows_with_options_in_tx`.
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

    // Reconciliação NÃO-destrutiva por aba (spec 012): identidade determinística + UPSERT preserva
    // o id (e o enriquecimento — split/tags/payment_method ancorados nele) quando a célula é
    // editada; diff-delete remove só as linhas que sumiram da planilha. Substitui o DELETE-all +
    // uuid novo (que regenerava ids e matava o enriquecimento a cada re-import — P0-2).
    let profile_id = resolve_profile_id(tx, profile_id).await?;

    let mut slot_counter: std::collections::HashMap<(String, &'static str), usize> =
        std::collections::HashMap::new();
    let mut current_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in rows {
        let slot = {
            let c = slot_counter
                .entry((row.date.clone(), row.kind.as_str()))
                .or_insert(0);
            let s = *c;
            *c += 1;
            s
        };
        let txn_id = row_id(sheet_name, &row.date, row.kind, slot);
        current_ids.insert(txn_id.clone());

        let sheet_amount = row.amount.abs();
        let sheet_desc = row.description.clone();

        // Merge de 3 vias (spec 013): a planilha não vence cego. Carrega o estado atual + o base
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

        // --- Plan 004: splits de titular + payment_method='credit' via gramática da nota ---
        // Marcadores OPT-IN (ver `parse_note_markers`): nota sem marcador → bloco inteiro é
        // no-op, idêntico ao comportamento de hoje (sem split, payment_method intocado).
        let markers = parse_note_markers(&row.raw_note);

        if markers.is_credit {
            // Despesa de cartão: marca para o engine dobrar no lump da fatura (classify()).
            sqlx::query("UPDATE \"transaction\" SET payment_method = 'credit' WHERE id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("set payment_method credit: {e}"))?;
        }

        if !markers.owners.is_empty() {
            // Re-import idempotente: a planilha é a fonte autoritativa da gramática, então
            // substituímos os splits desta transação. ON DELETE CASCADE no schema cobre o
            // caso de diff-delete da própria transação. (Limitação conhecida: sobrescreve
            // splits editados manualmente — endereçado pelo plan 015 com flag de lock.)
            sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear splits for txn: {e}"))?;

            // Valor do split = magnitude positiva da transação (mesma convenção do amount da
            // transação). Alocação parcial por titular fica para o plan 019 (entidade fatura).
            let split_amount = row.amount.abs();
            for owner_name in &markers.owners {
                // Resolve a pessoa pelo nome (case-insensitive); cria sob demanda na MESMA tx,
                // espelhando o bootstrap de person em `resolve_profile_id`. Pessoa-sem-profile é
                // válida para titulares não-primários.
                let person_id: String = {
                    let existing: Option<(String,)> = sqlx::query_as(
                        "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
                    )
                    .bind(owner_name)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| format!("lookup person '{owner_name}': {e}"))?;

                    match existing {
                        Some((id,)) => id,
                        None => {
                            let new_id = uuid::Uuid::new_v4().to_string();
                            sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                                .bind(&new_id)
                                .bind(owner_name)
                                .execute(&mut **tx)
                                .await
                                .map_err(|e| format!("create person '{owner_name}': {e}"))?;
                            new_id
                        }
                    }
                };

                let split_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(&split_id)
                .bind(&txn_id)
                .bind(split_amount)
                .bind(&person_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("insert split for '{owner_name}': {e}"))?;
            }
        }
        // --- fim Plan 004 ---

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

pub async fn get_layout_for_sheet(
    pool: &SqlitePool,
    sheet_name: &str,
) -> Result<Option<SheetLayout>, String> {
    let result = sqlx::query_as::<_, (String, String, Option<i32>, i32, i32, i32, i32, i32, String)>(
        "SELECT id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction FROM sheet_layout WHERE sheet_name = ?1"
    )
    .bind(sheet_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query layout: {e}"))?;

    Ok(
        result.map(|(id, sn, year, mnr, hr, dsr, dc, bs, dd)| SheetLayout {
            id,
            sheet_name: sn,
            year,
            month_names_row: mnr,
            header_row: hr,
            data_start_row: dsr,
            day_column: dc,
            block_size: bs,
            date_direction: dd,
        }),
    )
}

pub async fn get_active_mappings_for_sheet(
    pool: &SqlitePool,
    sheet_name: &str,
) -> Result<Vec<(String, i32)>, String> {
    let rows = sqlx::query_as::<_, (String, i32)>(
        "SELECT target_field, block_offset FROM sheet_mapping WHERE sheet_name = ?1 AND is_active = 1 ORDER BY block_offset"
    )
    .bind(sheet_name)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query mappings: {e}"))?;

    Ok(rows)
}

/// Mapeia os blocos mensais de uma linha-cabeçalho para `(coluna_inicial, mês)`. Ancorado no
/// NOME do mês — JANEIRO vive no offset 0 na planilha real, e células espúrias entre blocos
/// (títulos, totais) não podem deslocar os meses seguintes. Primeira ocorrência de cada mês
/// vence: uma anotação posterior ("Março 2026") não cria bloco-fantasma lendo colunas erradas.
/// Fallback (nenhum nome de mês): fatia a largura em passos de `block_size`.
pub(crate) fn month_blocks_for(header_row_data: &[String], block_size: usize) -> Vec<(usize, u32)> {
    let mut month_blocks: Vec<(usize, u32)> = Vec::new();
    let mut seen_months = [false; 13];
    for (i, cell) in header_row_data.iter().enumerate() {
        if let Some(m) = month_number_from_name(cell)
            && !seen_months[m as usize]
        {
            seen_months[m as usize] = true;
            month_blocks.push((i, m));
        }
    }
    if month_blocks.is_empty() {
        month_blocks = (0..header_row_data.len())
            .step_by(block_size.max(1))
            .enumerate()
            .take(12)
            .map(|(idx, i)| (i, idx as u32 + 1))
            .collect();
    }
    month_blocks
}

/// Descrição de uma célula: a NOTA real da planilha (o método guarda aí quem/o quê/quanto por
/// item), com as quebras de linha viradas em " · "; vazia → fallback `"{kind} {date}"`. `notes`
/// é a grade `[linha][coluna]` alinhada a `rows`; vazia (path xlsx) cai sempre no fallback.
fn cell_description(
    notes: &[Vec<String>],
    row: usize,
    col: usize,
    date: &str,
    kind: &str,
) -> String {
    let note = notes
        .get(row)
        .and_then(|nr| nr.get(col))
        .map(|s| s.trim())
        .unwrap_or("");
    if note.is_empty() {
        format!("{kind} {date}")
    } else {
        note.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Nota CRUA de uma célula, preservando as quebras de linha (≠ `cell_description`,
/// que junta as linhas em " · "). Alimenta `parse_note_markers` na fase de
/// escrita. Célula ausente/sem nota → string vazia (sem marcadores).
fn cell_raw_note(notes: &[Vec<String>], row: usize, col: usize) -> String {
    notes
        .get(row)
        .and_then(|nr| nr.get(col))
        .map(String::as_str)
        .unwrap_or("")
        .to_string()
}

/// Marcadores OPT-IN extraídos de uma nota de célula (`parse_note_markers`).
///
/// SEGURO POR PADRÃO: uma nota sem marcador devolve `NoteMarkers::default()`
/// (sem owners, `is_credit=false`), de modo que o import se comporta byte-a-byte
/// como hoje. A esmagadora maioria das notas reais é prosa livre — ver a
/// GRAMÁTICA DAS NOTAS em `parse_note_markers`.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct NoteMarkers {
    /// Nomes de exibição dos titulares, na ordem em que aparecem na nota.
    /// Resolvidos para `person.name` (case-insensitive) na fase de escrita.
    pub owners: Vec<String>,
    /// `true` se a nota contém o marcador `#credito` em qualquer linha.
    pub is_credit: bool,
}

/// GRAMÁTICA DAS NOTAS (contrato público — opt-in, explícito, seguro por padrão).
///
/// Cada linha da nota é analisada de forma independente. Uma linha SÓ vira
/// marcador quando casa EXATAMENTE com uma das formas estruturadas abaixo; uma
/// nota sem marcador não produz split nem altera `payment_method` (idêntico ao
/// comportamento de hoje — provado por teste). A sintaxe foi escolhida para não
/// colidir com a convenção pessoal de prosa livre do dono (validado contra a
/// planilha de referência: zero linhas começando com `@` ou `#`).
///
/// Formas reconhecidas:
///   `@<nome>: <resto>`  — MARCADOR DE TITULAR. A linha deve COMEÇAR com `@`,
///                         seguido de um nome NÃO-vazio e DOIS-PONTOS. O `<nome>`
///                         (aparado) casa case-insensitive com `person.name` na
///                         escrita; o `<resto>` (tipicamente um valor) é IGNORADO
///                         no import — o valor da transação é canônico. Cada
///                         marcador gera uma linha em `split` com `owner_person_id`
///                         apontando para a pessoa (criada sob demanda se ausente).
///   `#credito`          — MARCADOR DE MÉTODO DE PAGAMENTO. O token `#credito`
///                         (case-insensitive) sozinho ou como PRIMEIRA palavra da
///                         linha (terminado por fim-de-linha ou espaço — `#creditox`
///                         NÃO casa). Define `payment_method='credit'` na transação,
///                         dobrando a despesa no lump da fatura (`classify()`).
///
/// Exemplos:
///   `"@Pessoa A: 150,00"`        → split com owner="Pessoa A"
///   `"@Pessoa A: 150\n@Pessoa B: 50"` → dois splits (mesma transação)
///   `"#credito"`                 → payment_method='credit'
///   `"@Pessoa A: 200\n#credito"` → ambos
///   `"Mercado da semana"`        → NENHUM marcador (prosa livre intocada)
///
/// Pura — sem I/O, sem DB, sem panics. Testável sem pool.
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut owners: Vec<String> = Vec::new();
    let mut is_credit = false;

    for line in note.lines() {
        let trimmed = line.trim();

        // Token de crédito: a linha começa com `#credito` e o próximo char (se houver)
        // é um separador (espaço/tab) — evita falso-positivo de `#creditocard` etc.
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("#credito")
            && rest.chars().next().is_none_or(char::is_whitespace)
        {
            is_credit = true;
        }

        // Marcador de titular: `@<nome>: ...` com nome não-vazio antes dos dois-pontos.
        if let Some(rest) = trimmed.strip_prefix('@')
            && let Some(colon_pos) = rest.find(':')
        {
            let name = rest[..colon_pos].trim().to_string();
            if !name.is_empty() {
                owners.push(name);
            }
        }
    }

    NoteMarkers { owners, is_credit }
}

pub fn parse_rows_with_layout(
    rows: &[Vec<String>],
    layout: &SheetLayout,
    mappings: &[(String, i32)],
    notes: &[Vec<String>],
) -> Result<Vec<ImportedRow>, String> {
    let mut imported = Vec::new();

    // Fail loudly when the year could not be detected from the sheet name. Silently dating every
    // row to a hardcoded fallback year misdates the entire tab with no signal to caller or user;
    // an explicit error is safer than wrong dates.
    let year = layout.year.ok_or_else(|| {
        format!(
            "não foi possível detectar o ano da aba '{}' (o nome da aba deve ser um ano de 4 dígitos)",
            layout.sheet_name
        )
    })?;
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    if month_row >= rows.len() {
        return Ok(imported);
    }

    let month_blocks = month_blocks_for(&rows[month_row], block_size);

    let amount_in_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_in")
        .map(|(_, offset)| *offset as usize);
    let amount_out_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_out")
        .map(|(_, offset)| *offset as usize);
    // Coluna Diário (variável): mapeada como `amount_daily`. Quando ausente (planilhas antigas),
    // o ramo simplesmente não emite nada.
    let amount_daily_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_daily")
        .map(|(_, offset)| *offset as usize);

    for (r, row) in rows.iter().enumerate().skip(data_start) {
        if row.is_empty() || row.get(day_col).is_none_or(|c| c.trim().is_empty()) {
            continue;
        }

        let day_str = row.get(day_col).map_or("", |c| c.trim());
        let day: f64 = day_str.parse().unwrap_or(0.0);
        if !(1.0..=31.0).contains(&day) {
            continue;
        }

        let day_num = day as u32;

        for &(offset, month) in &month_blocks {
            // A geometria tem linhas fixas de dia 1–31 em todos os blocos; fevereiro 29–31
            // carrega fórmulas herdadas. Dia inexistente no mês não vira transação.
            if chrono::NaiveDate::from_ymd_opt(year, month, day_num).is_none() {
                continue;
            }
            let date = format!("{:04}-{:02}-{:02}", year, month, day_num);
            let is_projection = classify_row(&date, &layout.date_direction).unwrap_or(false);

            if let Some(in_off) = amount_in_offset
                && offset + in_off < row.len()
            {
                let amount_in = parse_number(&row[offset + in_off]);
                if amount_in > 0 {
                    imported.push(ImportedRow {
                        date: date.clone(),
                        amount: amount_in,
                        description: cell_description(notes, r, offset + in_off, &date, "Entrada"),
                        is_projection,
                        kind: RowKind::Entrada,
                        raw_note: cell_raw_note(notes, r, offset + in_off),
                    });
                }
            }

            if let Some(out_off) = amount_out_offset
                && offset + out_off < row.len()
            {
                let amount_out = parse_number(&row[offset + out_off]);
                if amount_out > 0 {
                    imported.push(ImportedRow {
                        date: date.clone(),
                        amount: -amount_out,
                        description: cell_description(notes, r, offset + out_off, &date, "Saída"),
                        is_projection,
                        kind: RowKind::Saida,
                        raw_note: cell_raw_note(notes, r, offset + out_off),
                    });
                }
            }

            if let Some(d_off) = amount_daily_offset
                && offset + d_off < row.len()
            {
                let amount_daily = parse_number(&row[offset + d_off]);
                if amount_daily > 0 {
                    imported.push(ImportedRow {
                        date: date.clone(),
                        amount: -amount_daily,
                        description: cell_description(notes, r, offset + d_off, &date, "Diário"),
                        is_projection,
                        kind: RowKind::Diario,
                        raw_note: cell_raw_note(notes, r, offset + d_off),
                    });
                }
            }
        }
    }

    Ok(imported)
}

/// Um ponto da série de Saldo corrente lida da planilha (coluna `Saldo` do método).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyBalance {
    pub date: String,
    pub balance_cents: i64,
    pub is_projection: bool,
}

/// Extrai a série diária da coluna `Saldo` — o saldo corrente encadeado, que no método "bate
/// com o banco" e carrega todo o histórico + o carry-over de anos anteriores. Usa a MESMA
/// geometria de blocos das transações; o Saldo vive em `offset + balance_offset` (offset =
/// início do bloco do mês). Diferente de Entrada/Saída, é UM valor por dia e pode ser
/// negativo (mês "vermelho"). Células vazias são puladas (dia sem saldo lançado).
///
/// Alimenta dois consumidores: a SEMENTE da projeção (saldo de partida ≤ hoje) e, adiante, a
/// visão histórica do livro-razão (a coluna Saldo da grade ano a ano).
pub fn parse_balance_series(
    rows: &[Vec<String>],
    layout: &SheetLayout,
    balance_offset: usize,
) -> Result<Vec<DailyBalance>, String> {
    let mut out = Vec::new();

    // Fail loudly when the year could not be detected (see `parse_rows_with_layout`): a hardcoded
    // fallback would misdate the entire Saldo series, corrupting the projection seed.
    let year = layout.year.ok_or_else(|| {
        format!(
            "não foi possível detectar o ano da aba '{}' (o nome da aba deve ser um ano de 4 dígitos)",
            layout.sheet_name
        )
    })?;
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    if month_row >= rows.len() {
        return Ok(out);
    }
    let month_blocks = month_blocks_for(&rows[month_row], block_size);

    for row in rows.iter().skip(data_start) {
        if row.is_empty() || row.get(day_col).is_none_or(|c| c.trim().is_empty()) {
            continue;
        }
        let day_str = row.get(day_col).map_or("", |c| c.trim());
        let day: f64 = day_str.parse().unwrap_or(0.0);
        if !(1.0..=31.0).contains(&day) {
            continue;
        }
        let day_num = day as u32;

        for &(offset, month) in &month_blocks {
            if chrono::NaiveDate::from_ymd_opt(year, month, day_num).is_none() {
                continue;
            }
            let Some(cell) = row.get(offset + balance_offset) else {
                continue;
            };
            let cell = cell.trim();
            if cell.is_empty() {
                continue;
            }
            let date = format!("{:04}-{:02}-{:02}", year, month, day_num);
            let is_projection = classify_row(&date, &layout.date_direction).unwrap_or(false);
            out.push(DailyBalance {
                date,
                balance_cents: parse_number(cell),
                is_projection,
            });
        }
    }

    Ok(out)
}

/// Bloco de offset da coluna `Saldo` para a aba (do mapeamento `target_field = 'balance'`,
/// que existe mesmo com `is_active = 0`). Default 4 = 5ª coluna do bloco `Data|Entrada|Saída|
/// Diário|Saldo`.
pub async fn get_balance_offset_for_sheet(
    pool: &SqlitePool,
    sheet_name: &str,
) -> Result<usize, String> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT block_offset FROM sheet_mapping WHERE sheet_name = ?1 AND target_field = 'balance' LIMIT 1",
    )
    .bind(sheet_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query balance offset: {e}"))?;
    Ok(row.map(|(o,)| o as usize).unwrap_or(4))
}

/// Grava a série de Saldo diário, replace-all por aba (igual às transações): re-importar a
/// planilha editada substitui atomicamente a série antiga desta aba.
// API pública retida (plan 002) — ver nota em `compute_checksum`. Wrapper de pool usado pelos
// testes; o shell usa `store_balance_series_in_tx`.
#[allow(dead_code)]
pub async fn store_balance_series(
    pool: &SqlitePool,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let n = store_balance_series_core(&mut tx, sheet_name, series).await?;
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(n)
}

/// Grava a série de Saldo numa transação JÁ ABERTA — o chamador é dono do commit/rollback, para
/// participar do mesmo tudo-ou-nada das linhas/layout/mappings. NÃO faz commit.
pub(crate) async fn store_balance_series_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
    store_balance_series_core(tx, sheet_name, series).await
}

async fn store_balance_series_core(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
    sqlx::query("DELETE FROM sheet_daily_balance WHERE sheet_name = ?1")
        .bind(sheet_name)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("clear balances: {e}"))?;

    // Insere em lote (multi-row VALUES) em vez de uma query por linha: ~365 round-trips por aba
    // viravam 1 só por chunk. SQLite limita 32.766 parâmetros por statement; com 4 params/linha o
    // teto é 8.191 — CHUNK=8.000 (× 4 = 32.000) fica folgado dentro do limite. Mesma semântica
    // `INSERT OR REPLACE`, mesmas colunas/valores; só muda o empacotamento.
    const CHUNK: usize = 8_000;
    for chunk in series.chunks(CHUNK) {
        let placeholders: String = (0..chunk.len())
            .map(|i| {
                let b = i * 4;
                format!("(?{}, ?{}, ?{}, ?{})", b + 1, b + 2, b + 3, b + 4)
            })
            .collect::<Vec<_>>()
            .join(", ");
        // Placeholders posicionais (só `?`, sem dado interpolado) + binds — seguro com AssertSqlSafe.
        let sql = format!(
            "INSERT OR REPLACE INTO sheet_daily_balance \
             (sheet_name, date, balance_cents, is_projection) VALUES {placeholders}"
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for b in chunk {
            q = q
                .bind(sheet_name)
                .bind(&b.date)
                .bind(b.balance_cents)
                .bind(b.is_projection as i64);
        }
        q.execute(&mut **tx)
            .await
            .map_err(|e| format!("insert balance: {e}"))?;
    }

    Ok(series.len())
}

/// Converte texto monetário em centavos. Regra fechada de separadores (spec 010, slice 0):
/// com `.` e `,` presentes, o que aparece POR ÚLTIMO é o decimal (cobre pt-BR `1.234,56` e
/// en_US `1,234.56`); um separador sozinho é decimal, exceto padrão claro de agrupamento de
/// milhar (`1.234`, `1.234.567`). Floats do xlsx chegam normalizados com 4 casas fixas
/// (ver `xlsx_cell_to_string`), então nunca caem na ambiguidade de 3 dígitos.
pub fn parse_number(s: &str) -> i64 {
    // Negativo contábil entre parênteses ("(1.234,56)" = −1.234,56): os parênteses são removidos
    // pelo filtro abaixo, então capturamos o sinal antes. Comum em export de planilha/extrato.
    let trimmed = s.trim();
    let negative_paren = trimmed.starts_with('(') && trimmed.ends_with(')');

    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == ',')
        .collect();
    if cleaned.is_empty() {
        return 0;
    }

    let has_dot = cleaned.contains('.');
    let has_comma = cleaned.contains(',');
    let normalized = if has_dot && has_comma {
        if cleaned.rfind('.') > cleaned.rfind(',') {
            cleaned.replace(',', "")
        } else {
            cleaned.replace('.', "").replace(',', ".")
        }
    } else if has_comma && is_thousands_grouping(&cleaned, ',') {
        cleaned.replace(',', "")
    } else if has_comma {
        cleaned.replace(',', ".")
    } else if has_dot && is_thousands_grouping(&cleaned, '.') {
        cleaned.replace('.', "")
    } else {
        cleaned
    };

    let value = if let Ok(f) = normalized.parse::<f64>() {
        (f * 100.0).round() as i64
    } else {
        return 0;
    };
    if negative_paren { -value.abs() } else { value }
}

/// Parseia a aba `Economia` → `(ano, mês 1..=12, centavos)` para cada mês encontrado.
/// A aba coloca os blocos de ano LADO A LADO nas mesmas linhas (auditado na planilha viva: 2025 em
/// B–E, 2026 em G–J — o CABEÇALHO de cada bloco tem o ANO + os rótulos `Entradas`/`Economia`, e os
/// 12 meses `jan`..`dez` ficam logo abaixo, na coluna do ano). Também tolera blocos EMPILHADOS
/// verticalmente. Cada bloco usa a SUA coluna de mês e a SUA coluna `Economia` (o primeiro rótulo
/// `Economia` à DIREITA do ano). PURA — só lê. Zeros/brancos são preservados para o import conseguir
/// remover uma Economia que foi apagada na planilha.
pub fn parse_economia_sheet(rows: &[Vec<String>]) -> Vec<(i32, u32, i64)> {
    let mut out = Vec::new();
    let mut r = 0;
    while r < rows.len() {
        let row = &rows[r];
        let has_entradas = row
            .iter()
            .any(|c| c.trim().eq_ignore_ascii_case("entradas"));
        // Coleta TODOS os blocos `(month_col, ano, econ_col)` deste cabeçalho. `econ_col` de um bloco
        // é o primeiro rótulo `Economia` à direita do ano — assim 2026 (lado a lado) usa a coluna de
        // 2026, não a de 2025.
        let mut blocks: Vec<(usize, i32, usize)> = Vec::new();
        if has_entradas {
            for (i, c) in row.iter().enumerate() {
                let Some(year) = c
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.fract() == 0.0 && (2000.0..2100.0).contains(n))
                    .map(|n| n as i32)
                else {
                    continue;
                };
                if let Some(econ_col) = row[i + 1..]
                    .iter()
                    .position(|e| e.trim().eq_ignore_ascii_case("economia"))
                    .map(|p| i + 1 + p)
                {
                    blocks.push((i, year, econ_col));
                }
            }
        }
        if blocks.is_empty() {
            r += 1;
            continue;
        }
        // Lê as linhas de mês logo abaixo do cabeçalho; cada bloco lê a SUA coluna de mês e de
        // Economia. Para quando nenhuma coluna de bloco nomeia um mês (TOTAL/linha vazia/próximo
        // cabeçalho) ou logo após dezembro.
        let mut rr = r + 1;
        while rr < rows.len() {
            let mut any = false;
            let mut saw_december = false;
            for &(month_col, year, econ_col) in &blocks {
                let Some(month) = rows[rr]
                    .get(month_col)
                    .and_then(|l| month_number_from_name(l))
                else {
                    continue;
                };
                any = true;
                let cents = rows[rr].get(econ_col).map(|c| parse_number(c)).unwrap_or(0);
                out.push((year, month, cents));
                if month == 12 {
                    saw_december = true;
                }
            }
            if !any {
                break;
            }
            rr += 1;
            if saw_december {
                break;
            }
        }
        r = rr;
    }
    out
}

/// Padrão inequívoco de milhar: primeiro grupo com 1–3 dígitos e todos os demais com
/// exatamente 3 (`3.012`, `1.234.567`) — qualquer outra forma é tratada como decimal.
fn is_thousands_grouping(s: &str, sep: char) -> bool {
    let unsigned = s.trim_start_matches('-');
    let mut parts = unsigned.split(sep);
    let Some(first) = parts.next() else {
        return false;
    };
    let rest: Vec<&str> = parts.collect();
    !first.is_empty()
        && first.len() <= 3
        && first.chars().all(|c| c.is_ascii_digit())
        && !rest.is_empty()
        && rest
            .iter()
            .all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_row_past() {
        let past = "2020-01-15";
        assert!(!classify_row(past, "both").unwrap());
        assert!(!classify_row(past, "past_only").unwrap());
        assert!(classify_row(past, "future_only").unwrap());
    }

    #[test]
    fn test_classify_row_future() {
        let future = "2099-12-31";
        assert!(classify_row(future, "both").unwrap());
        assert!(classify_row(future, "future_only").unwrap());
    }

    #[test]
    fn test_classify_row_invalid_direction() {
        assert!(classify_row("2025-01-01", "invalid").is_err());
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("100"), 10000);
        assert_eq!(parse_number("1.234,56"), 123456);
        assert_eq!(parse_number("-50"), -5000);
        assert_eq!(parse_number(""), 0);
    }

    // Regressão spec 010 slice 0: valores representativos nos dois locales + xlsx.
    #[test]
    fn test_parse_number_separator_rules() {
        // xlsx/calamine: ponto decimal puro — antes inflava 100× (12.34 → 123400).
        assert_eq!(parse_number("12.34"), 1234);
        assert_eq!(parse_number("1234.56"), 123456);
        // Valor com 4 casas: arredonda a centavos na fronteira.
        assert_eq!(parse_number("5678.1234"), 567812);
        assert_eq!(parse_number("456.7891"), 45679);
        // Float do xlsx normalizado com 4 casas fixas (xlsx_cell_to_string).
        assert_eq!(parse_number("12.3400"), 1234);
        assert_eq!(parse_number("123.4560"), 12346);
        // Sheets FORMATTED pt-BR e en_US: o último separador é o decimal.
        assert_eq!(parse_number("3.012,73"), 301273);
        assert_eq!(parse_number("3,012.73"), 301273);
        assert_eq!(parse_number("R$ 1.234,56"), 123456);
        // Separador único com agrupamento claro de milhar.
        assert_eq!(parse_number("3.012"), 301200);
        assert_eq!(parse_number("1.234.567"), 123456700);
        assert_eq!(parse_number("3,012"), 301200);
        // Decimal pt-BR sem milhar; negativos.
        assert_eq!(parse_number("1370,5"), 137050);
        assert_eq!(parse_number("-45,00"), -4500);
        assert_eq!(parse_number("-45.00"), -4500);
        // Negativo contábil entre parênteses (export de planilha/extrato).
        assert_eq!(parse_number("(1.234,56)"), -123456);
        assert_eq!(parse_number("(50,00)"), -5000);
        assert_eq!(parse_number("(R$ 1.000,00)"), -100000);
        assert_eq!(parse_number("(0)"), 0);
    }

    #[test]
    fn test_parse_rows_with_layout() {
        let rows = vec![
            vec![
                "".into(),
                "JANEIRO".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "FEVEREIRO".into(),
            ],
            vec![
                "".into(),
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
                "Data".into(),
            ],
            vec![
                "1".into(),
                "".into(),
                "3500".into(),
                "".into(),
                "3500".into(),
                "3500".into(),
                "".into(),
            ],
            vec![
                "2".into(),
                "".into(),
                "".into(),
                "45".into(),
                "3455".into(),
                "3455".into(),
                "".into(),
            ],
        ];

        let layout = SheetLayout {
            id: "test".into(),
            sheet_name: "2025".into(),
            year: Some(2025),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "past_only".into(),
        };

        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].amount, 350000);
        assert_eq!(result[0].date, "2025-01-01");
        assert!(!result[0].is_projection);
        assert_eq!(result[1].amount, -4500);
        assert_eq!(result[1].date, "2025-01-02");
    }

    // A nota da célula vira a descrição (com " · " no lugar das quebras); sem nota, fallback
    // com a DATA real (não mais "Entrada 2026" genérico) — auditoria vs planilha oficial.
    #[test]
    fn description_comes_from_cell_note_with_date_fallback() {
        let rows = real_geometry_rows(false);
        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];
        // Nota na célula da Entrada de JANEIRO (linha 2, col 1); resto sem nota.
        let mut notes = vec![Vec::new(); rows.len()];
        notes[2] = vec![String::new(); rows[0].len()];
        notes[2][1] = "Nota de exemplo\nSegunda linha da nota".into();

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &notes).unwrap();

        let entrada = result.iter().find(|r| r.amount > 0).unwrap();
        assert_eq!(
            entrada.description,
            "Nota de exemplo · Segunda linha da nota"
        );
        // A Saída de DEZEMBRO não tem nota → fallback com a data.
        let saida = result.iter().find(|r| r.amount < 0).unwrap();
        assert_eq!(saida.description, "Saída 2026-12-01");
    }

    #[test]
    fn test_parse_balance_series() {
        // Coluna Saldo (offset 4) com saldo de partida, um saldo negativo (mês vermelho) e
        // um dia sem saldo lançado (pulado). Valores crus como vêm do UNFORMATTED_VALUE.
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
                "".into(),
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
            ],
            vec![
                "1".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "12345.6748".into(),
            ],
            vec![
                "2".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "-78.90".into(),
            ],
            vec![
                "3".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
            ],
        ];
        let layout = SheetLayout {
            id: "t".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "past_only".into(),
        };

        let series = parse_balance_series(&rows, &layout, 4).unwrap();

        assert_eq!(series.len(), 2); // dia 3 (Saldo vazio) é pulado
        assert_eq!(
            series[0],
            DailyBalance {
                date: "2026-01-01".into(),
                balance_cents: 1_234_567, // 12345.6748 → centavos (sub-centavo truncado)
                is_projection: false,
            }
        );
        assert_eq!(series[1].date, "2026-01-02");
        assert_eq!(series[1].balance_cents, -7890); // saldo negativo preservado
    }

    #[test]
    fn test_compute_checksum() {
        let rows = vec![ImportedRow {
            date: "2025-01-01".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
            kind: RowKind::Entrada,
            raw_note: String::new(),
        }];
        let checksum1 = compute_checksum(&rows);
        let checksum2 = compute_checksum(&rows);
        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64);

        let different_rows = vec![ImportedRow {
            date: "2025-01-02".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
            kind: RowKind::Entrada,
            raw_note: String::new(),
        }];
        let checksum3 = compute_checksum(&different_rows);
        assert_ne!(checksum1, checksum3);
    }

    // --- Spec 010 slice 0: geometria real (JANEIRO no offset 0, 12 blocos, célula espúria) ---

    fn real_geometry_layout() -> SheetLayout {
        SheetLayout {
            id: "real".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "past_only".into(),
        }
    }

    /// Espelha a planilha real: nomes de mês na linha 0 a cada 6 colunas COMEÇANDO na
    /// coluna A (offset 0), header Data|Entrada|Saída|Diário|Saldo por bloco, dia na coluna A.
    fn real_geometry_rows(spurious_cell: bool) -> Vec<Vec<String>> {
        const MONTHS: [&str; 12] = [
            "JANEIRO",
            "FEVEREIRO",
            "MARÇO",
            "ABRIL",
            "MAIO",
            "JUNHO",
            "JULHO",
            "AGOSTO",
            "SETEMBRO",
            "OUTUBRO",
            "NOVEMBRO",
            "DEZEMBRO",
        ];
        let width = 12 * 6;
        let mut month_row = vec![String::new(); width];
        for (i, m) in MONTHS.iter().enumerate() {
            month_row[i * 6] = (*m).to_string();
        }
        if spurious_cell {
            month_row[5] = "TOTAL".into();
        }
        let mut header_row = vec![String::new(); width];
        for i in 0..12 {
            header_row[i * 6] = "Data".into();
            header_row[i * 6 + 1] = "Entrada".into();
            header_row[i * 6 + 2] = "Saída".into();
            header_row[i * 6 + 3] = "Diário".into();
            header_row[i * 6 + 4] = "Saldo".into();
        }
        let mut day1 = vec![String::new(); width];
        day1[0] = "1".into();
        day1[1] = "1234.56".into(); // Entrada em JANEIRO (bloco no offset 0)
        day1[66 + 2] = "12.34".into(); // Saída em DEZEMBRO (bloco no offset 66)
        vec![month_row, header_row, day1]
    }

    // Regressão do bug `i > 0`: JANEIRO era dropado e todo mês deslocava 1 para trás.
    #[test]
    fn january_at_offset_zero_and_december_resolve_by_month_name() {
        let rows = real_geometry_rows(false);
        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();

        assert_eq!(result.len(), 2);
        let entrada = result.iter().find(|r| r.amount > 0).unwrap();
        assert_eq!(entrada.date, "2026-01-01");
        assert_eq!(entrada.amount, 123456);
        let saida = result.iter().find(|r| r.amount < 0).unwrap();
        assert_eq!(saida.date, "2026-12-01");
        assert_eq!(saida.amount, -1234);
    }

    // Ano não detectado (nome de aba que não é um ano de 4 dígitos) → erro explícito, NUNCA datar
    // as linhas com um ano hardcoded. Vale para os dois parsers que dependem de `layout.year`.
    #[test]
    fn year_none_returns_error() {
        let rows = real_geometry_rows(false);
        let mut layout = real_geometry_layout();
        layout.year = None;
        layout.sheet_name = "Finanças".into();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let rows_err = parse_rows_with_layout(&rows, &layout, &mappings, &[]);
        assert!(rows_err.is_err());
        assert!(rows_err.unwrap_err().contains("Finanças"));

        let balance_err = parse_balance_series(&rows, &layout, 4);
        assert!(balance_err.is_err());
    }

    // Regressão (review adversarial): anotação com nome de mês depois do bloco real
    // ("MAIO 2026" solto) não pode virar bloco-fantasma lendo colunas erradas.
    #[test]
    fn duplicate_month_annotation_does_not_create_ghost_block() {
        let mut rows = real_geometry_rows(false);
        let width = rows[0].len();
        rows[0][width - 3] = "MAIO 2026".into(); // anotação espúria (MAIO real está no offset 24)
        rows[2][width - 2] = "50.00".into(); // valor sob a anotação, na posição de Entrada

        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();

        // Só as duas linhas reais; o 50.00 sob o bloco-fantasma não é importado.
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.amount != 5000));
    }

    // Regressão: as linhas de dia 29–31 existem em todos os blocos (fevereiro herda
    // fórmulas) — dia inexistente no mês não pode virar transação com data inválida.
    #[test]
    fn nonexistent_day_of_month_is_skipped() {
        let mut rows = real_geometry_rows(false);
        // Linha do dia 30 com valor no bloco de FEVEREIRO (offset 6, Entrada em +1).
        let width = rows[0].len();
        let mut day30 = vec![String::new(); width];
        day30[0] = "30".into();
        day30[6 + 1] = "100.00".into();
        rows.push(day30);

        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();

        // Só as duas linhas válidas do dia 1; "2026-02-30" não existe.
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.date != "2026-02-30"));
    }

    // Regressão: célula não-vazia entre blocos não pode virar bloco nem deslocar meses.
    #[test]
    fn parse_economia_sheet_reads_blocks_per_year() {
        // Estrutura REAL: blocos por ano; ano/mês na col B (idx 1), Economia na col D (idx 3).
        let h = |y: &str| {
            vec![
                "".to_string(),
                y.to_string(),
                "Entradas".to_string(),
                "Economia".to_string(),
                "%".to_string(),
            ]
        };
        let m = |name: &str, eco: &str| {
            vec![
                "".to_string(),
                name.to_string(),
                "5000.00".to_string(),
                eco.to_string(),
                "0".to_string(),
            ]
        };
        let rows = vec![
            h("2025"),
            m("jan", "1000.00"),
            m("fev", "0.0000"), // 0 → ignorado
            vec!["".into(), "TOTAL".into(), "".into(), "".into(), "".into()],
            h("2026"),
            m("jan", "1500.50"),
        ];
        let got = parse_economia_sheet(&rows);
        assert_eq!(
            got,
            vec![(2025, 1, 100_000), (2025, 2, 0), (2026, 1, 150_050)]
        );
    }

    // Regressão (P1): a aba real coloca os anos LADO A LADO nas mesmas linhas. Antes, o parser pegava
    // só o primeiro bloco e descartava silenciosamente o ano corrente (2026).
    #[test]
    fn parse_economia_sheet_side_by_side_blocks() {
        // 2025 em B–E (idx 1–4), 2026 em G–J (idx 6–9); col F (idx 5) é o gap.
        let header = vec![
            "".to_string(),
            "2025".to_string(),
            "Entradas".to_string(),
            "Economia".to_string(),
            "%".to_string(),
            "".to_string(),
            "2026".to_string(),
            "Entradas".to_string(),
            "Economia".to_string(),
            "%".to_string(),
        ];
        let m = |name: &str, eco25: &str, eco26: &str| {
            vec![
                "".to_string(),
                name.to_string(),
                "5000.00".to_string(),
                eco25.to_string(),
                "0".to_string(),
                "".to_string(),
                name.to_string(),
                "8000.00".to_string(),
                eco26.to_string(),
                "0".to_string(),
            ]
        };
        let rows = vec![
            header,
            m("jan", "1000.00", "1500.00"),
            m("fev", "0.0000", "2000.00"),
        ];
        let got = parse_economia_sheet(&rows);

        let y2025: Vec<_> = got
            .iter()
            .filter(|&&(y, _, _)| y == 2025)
            .copied()
            .collect();
        let y2026: Vec<_> = got
            .iter()
            .filter(|&&(y, _, _)| y == 2026)
            .copied()
            .collect();
        assert_eq!(y2025.len(), 2, "2025 deve ter jan e fev");
        assert_eq!(y2026.len(), 2, "2026 deve ter jan e fev");
        assert_eq!(
            y2025.iter().find(|&&(_, mo, _)| mo == 1).unwrap().2,
            100_000
        );
        assert_eq!(y2025.iter().find(|&&(_, mo, _)| mo == 2).unwrap().2, 0); // 0 preservado
        assert_eq!(
            y2026.iter().find(|&&(_, mo, _)| mo == 1).unwrap().2,
            150_000
        );
        assert_eq!(
            y2026.iter().find(|&&(_, mo, _)| mo == 2).unwrap().2,
            200_000
        );
    }

    #[test]
    fn spurious_cell_between_blocks_does_not_shift_months() {
        let rows = real_geometry_rows(true);
        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.iter().find(|r| r.amount > 0).unwrap().date,
            "2026-01-01"
        );
        assert_eq!(
            result.iter().find(|r| r.amount < 0).unwrap().date,
            "2026-12-01"
        );
    }

    // --- Spec 010 slice 1: re-import idempotente (replace-all por aba, atômico) ---

    async fn test_pool() -> SqlitePool {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn imported(date: &str, amount: i64) -> ImportedRow {
        imported_desc(date, amount, &format!("Linha {date}"))
    }

    fn imported_desc(date: &str, amount: i64, description: &str) -> ImportedRow {
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

    async fn count_transactions(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    async fn description_and_source(
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

    async fn count_sync_log(pool: &SqlitePool, sheet: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1")
            .bind(sheet)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    // Regressão: o frontend envia crypto.randomUUID() como profile_id; com a FK de
    // sync_log.profile_id ligada (default do sqlx), o import inteiro falhava. O backend
    // agora resolve/bootstrapa o profile em vez de confiar no id do frontend.
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

    // --- Spec 013: merge de 3 vias (drift por célula + gate de conflito) ---

    async fn amount_by_date(pool: &SqlitePool, date: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT amount FROM \"transaction\" WHERE date = ?1")
            .bind(date)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }
    async fn set_local_amount(pool: &SqlitePool, date: &str, amount: i64) {
        sqlx::query("UPDATE \"transaction\" SET amount = ?1 WHERE date = ?2")
            .bind(amount)
            .bind(date)
            .execute(pool)
            .await
            .unwrap();
    }
    async fn conflict_count(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM import_conflict")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

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

    // Regressão do bloqueador nº 1 do dogfooding: re-importar a planilha com QUALQUER
    // edição re-inseria todas as linhas (checksum era do batch inteiro, INSERT puro).
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

    // P0-2: re-importar uma célula EDITADA preserva o id determinístico → o enriquecimento
    // (aqui payment_method, mas idem split/tags ancorados no id) SOBREVIVE. Antes, o DELETE-all +
    // uuid novo o destruía a cada re-import.
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

    // Plan 002: o import é tudo-ou-nada numa única transação. Se algo falha entre a fase de
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

    // Plan 002: um import bem-sucedido comita linhas E série de Saldo juntas na mesma transação;
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

    // Plan 009: o insert em lote grava o mesmo conjunto de linhas que o loop linha-a-linha e o
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
    // Plan 004: gramática das notas (parse puro, sem DB)
    // ===================================================================

    #[test]
    fn parse_note_markers_empty_note() {
        let m = parse_note_markers("");
        assert!(m.owners.is_empty());
        assert!(!m.is_credit);
    }

    #[test]
    fn parse_note_markers_owner_only() {
        let m = parse_note_markers("@Pessoa A: 150,00");
        assert_eq!(m.owners, vec!["Pessoa A"]);
        assert!(!m.is_credit);
    }

    #[test]
    fn parse_note_markers_credit_only() {
        let m = parse_note_markers("#credito");
        assert!(m.owners.is_empty());
        assert!(m.is_credit);
    }

    #[test]
    fn parse_note_markers_credit_case_insensitive() {
        assert!(parse_note_markers("#Credito").is_credit);
        assert!(parse_note_markers("#CREDITO").is_credit);
    }

    #[test]
    fn parse_note_markers_credit_substring_not_matched() {
        // `#creditocard` NÃO casa: o token precisa terminar em fim-de-linha ou espaço.
        assert!(!parse_note_markers("#creditocard").is_credit);
        // Mas `#credito` seguido de espaço/texto casa (token isolado).
        assert!(parse_note_markers("#credito fatura nubank").is_credit);
    }

    #[test]
    fn parse_note_markers_owner_and_credit() {
        let note = "@Pessoa A: 200,00\n#credito";
        let m = parse_note_markers(note);
        assert_eq!(m.owners, vec!["Pessoa A"]);
        assert!(m.is_credit);
    }

    #[test]
    fn parse_note_markers_multiple_owners() {
        let note = "@Pessoa A: 150,00\n@Pessoa B: 50,00";
        let m = parse_note_markers(note);
        assert_eq!(m.owners, vec!["Pessoa A", "Pessoa B"]);
        assert!(!m.is_credit);
    }

    #[test]
    fn parse_note_markers_free_prose_ignored() {
        // Notas de prosa livre existentes NÃO podem disparar marcadores por acidente.
        // (Formato real da planilha de referência: "R$ X - descrição".)
        let note = "R$ 65,00 - Vivo · faltou só o frango";
        let m = parse_note_markers(note);
        assert!(m.owners.is_empty());
        assert!(!m.is_credit);
    }

    #[test]
    fn parse_note_markers_owner_name_trimmed() {
        let m = parse_note_markers("@ Pessoa A :  valor");
        // Espaço antes/depois do nome é aparado; dois-pontos após espaço ainda casa.
        assert_eq!(m.owners, vec!["Pessoa A"]);
    }

    #[test]
    fn parse_note_markers_at_without_colon_ignored() {
        // `@` sem dois-pontos não é marcador de titular (não vira split por acidente).
        let m = parse_note_markers("email @ provedor sem dois pontos");
        assert!(m.owners.is_empty());
    }

    // ===================================================================
    // Plan 004: testes de integração (DB)
    // ===================================================================

    #[tokio::test]
    async fn import_sets_credit_payment_method_from_note() {
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-10".into(),
            amount: -30000, // R$300 Saída
            description: "Fatura cartão · #credito".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "#credito".into(),
        }];

        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (pm,): (Option<String>,) =
            sqlx::query_as("SELECT payment_method FROM \"transaction\" WHERE date = '2026-01-10'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pm.as_deref(), Some("credit"));
    }

    #[tokio::test]
    async fn import_creates_split_with_owner_from_note() {
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-15".into(),
            amount: -30000,
            description: "@Pessoa A: 30000".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "@Pessoa A: 30000".into(),
        }];

        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // A linha de split precisa existir, com magnitude positiva.
        let splits: Vec<(String, i64)> = sqlx::query_as(
            "SELECT p.name, s.amount FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             JOIN \"transaction\" t ON t.id = s.transaction_id \
             WHERE t.date = '2026-01-15'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].0, "Pessoa A");
        assert_eq!(splits[0].1, 30000); // magnitude positiva

        // A pessoa foi criada sob demanda.
        let (pcount,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name)='pessoa a'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pcount, 1);
    }

    #[tokio::test]
    async fn import_creates_multiple_splits_for_multiple_owners() {
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-02-01".into(),
            amount: -30000,
            description: "@Pessoa A: 200 · @Pessoa B: 100".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "@Pessoa A: 200,00\n@Pessoa B: 100,00".into(),
        }];

        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (scount,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM split s \
             JOIN \"transaction\" t ON t.id = s.transaction_id \
             WHERE t.date = '2026-02-01'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(scount, 2);
    }

    #[tokio::test]
    async fn reimport_replaces_splits_idempotently() {
        let pool = test_pool().await;

        // Primeiro import: um titular.
        let v1 = vec![ImportedRow {
            date: "2026-03-01".into(),
            amount: -30000,
            description: "@Pessoa A: 30000".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "@Pessoa A: 30000".into(),
        }];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Segundo import (a nota da planilha mudou para dois titulares).
        let v2 = vec![ImportedRow {
            date: "2026-03-01".into(),
            amount: -30000,
            description: "@Pessoa A: 200 · @Pessoa B: 100".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "@Pessoa A: 200,00\n@Pessoa B: 100,00".into(),
        }];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        let (scount,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM split s \
             JOIN \"transaction\" t ON t.id = s.transaction_id \
             WHERE t.date = '2026-03-01'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(scount, 2, "re-import substituiu o único split por dois");
    }

    #[tokio::test]
    async fn import_no_note_leaves_payment_method_null_and_no_splits() {
        // PROVA DE SEGURANÇA: nota ausente → comportamento idêntico ao de hoje.
        let pool = test_pool().await;
        let rows = vec![imported("2026-04-01", -10000)]; // raw_note vazio
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (pm, splits): (Option<String>, i64) = sqlx::query_as(
            "SELECT t.payment_method, \
                    (SELECT COUNT(*) FROM split WHERE transaction_id = t.id) \
             FROM \"transaction\" t WHERE t.date = '2026-04-01'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pm.is_none(), "sem nota → payment_method permanece NULL");
        assert_eq!(splits, 0, "sem nota → nenhum split criado");
    }

    #[tokio::test]
    async fn import_unmarked_prose_note_leaves_payment_method_null_and_no_splits() {
        // PROVA DE SEGURANÇA reforçada: nota com PROSA LIVRE real (formato da planilha,
        // contendo nomes próprios soltos) NÃO dispara marcador algum.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-04-02".into(),
            amount: -10000,
            description: "R$ 300 - Pagamento Contas".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 300 - Pagamento Contas\nR$ 60 - Empréstimo".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (pm, splits): (Option<String>, i64) = sqlx::query_as(
            "SELECT t.payment_method, \
                    (SELECT COUNT(*) FROM split WHERE transaction_id = t.id) \
             FROM \"transaction\" t WHERE t.date = '2026-04-02'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pm.is_none(), "prosa livre → payment_method permanece NULL");
        assert_eq!(splits, 0, "prosa livre → nenhum split criado");
    }

    #[tokio::test]
    async fn import_owner_lookup_is_case_insensitive() {
        let pool = test_pool().await;
        // Pré-semeia a pessoa com nome em maiúscula/minúscula misto.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pid-pa', 'Pessoa A')")
            .execute(&pool)
            .await
            .unwrap();

        let rows = vec![ImportedRow {
            date: "2026-05-01".into(),
            amount: -10000,
            description: "@pessoa a: 100".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "@pessoa a: 100".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // Deve reutilizar a pessoa existente (sem criar duplicata).
        let (pcount,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name) = 'pessoa a'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pcount, 1, "nenhuma pessoa duplicada criada");
        let (owner,): (String,) = sqlx::query_as(
            "SELECT p.name FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             JOIN \"transaction\" t ON t.id = s.transaction_id \
             WHERE t.date = '2026-05-01'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            owner, "Pessoa A",
            "split aponta para a pessoa pré-existente"
        );
    }
}
