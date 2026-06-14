use super::layout_detect::{SheetLayout, month_number_from_name};
use super::reconcile;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

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

pub fn compute_checksum(rows: &[ImportedRow]) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.date.as_bytes());
        hasher.update(row.amount.to_le_bytes());
        hasher.update(row.description.as_bytes());
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

pub async fn import_rows(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    let checksum = compute_checksum(rows);
    if check_duplicate_import(pool, sheet_name, &checksum).await? {
        // Dataset idêntico ao último import desta aba — nada mudou, não toca o banco.
        return Ok(0);
    }

    let now = chrono::Utc::now().to_rfc3339();

    // Reconciliação NÃO-destrutiva por aba (spec 012): identidade determinística + UPSERT preserva
    // o id (e o enriquecimento — split/tags/payment_method ancorados nele) quando a célula é
    // editada; diff-delete remove só as linhas que sumiram da planilha. Substitui o DELETE-all +
    // uuid novo (que regenerava ids e matava o enriquecimento a cada re-import — P0-2).
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;

    let profile_id = resolve_profile_id(&mut tx, profile_id).await?;

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
        let existing: Option<(i64, String, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT amount, COALESCE(description,''), source_amount, source_description \
             FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&txn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("load existing txn: {e}"))?;

        match existing {
            None => {
                // Linha nova: a planilha semeia valor e base.
                sqlx::query(
                    "INSERT INTO \"transaction\" (id, type, amount, description, date, is_fixed, is_projection, source_amount, source_description, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?3, ?4, ?8, ?8)",
                )
                .bind(&txn_id)
                .bind(row.kind.txn_type())
                .bind(sheet_amount)
                .bind(&sheet_desc)
                .bind(&row.date)
                .bind(row.kind.is_fixed() as i64)
                .bind(row.is_projection as i64)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("insert row {row:?}: {e}"))?;
            }
            Some((local_amount, local_desc, src_amount, src_desc)) => {
                let amt = reconcile::apply(src_amount.as_ref(), &local_amount, &sheet_amount);
                let desc = reconcile::apply(src_desc.as_ref(), &local_desc, &sheet_desc);

                sqlx::query(
                    "UPDATE \"transaction\" SET type=?2, amount=?3, description=?4, date=?5, \
                       is_fixed=?6, is_projection=?7, source_amount=?8, source_description=?9, updated_at=?10 \
                     WHERE id=?1",
                )
                .bind(&txn_id)
                .bind(row.kind.txn_type())
                .bind(amt.value)
                .bind(&desc.value)
                .bind(&row.date)
                .bind(row.kind.is_fixed() as i64)
                .bind(row.is_projection as i64)
                .bind(amt.source)
                .bind(&desc.source)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("update row {row:?}: {e}"))?;

                record_conflict(
                    &mut tx,
                    &txn_id,
                    "amount",
                    amt.conflict,
                    src_amount.map(|v| v.to_string()),
                    &local_amount.to_string(),
                    &sheet_amount.to_string(),
                    &now,
                )
                .await?;
                record_conflict(
                    &mut tx,
                    &txn_id,
                    "description",
                    desc.conflict,
                    src_desc,
                    &local_desc,
                    &sheet_desc,
                    &now,
                )
                .await?;
            }
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
        .bind(&checksum)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("sync_log error: {e}"))?;
    }

    // Diff-delete: linhas removidas da planilha (no sync_log desta aba, mas fora do import atual).
    let existing: Vec<(String,)> = sqlx::query_as(
        "SELECT entity_id FROM sync_log WHERE source_sheet = ?1 AND entity_type = 'transaction'",
    )
    .bind(sheet_name)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("load existing ids: {e}"))?;
    for (eid,) in existing {
        if !current_ids.contains(&eid) {
            sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
                .bind(&eid)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("delete removed txn: {e}"))?;
            sqlx::query(
                "DELETE FROM sync_log WHERE entity_id = ?1 AND source_sheet = ?2 AND entity_type = 'transaction'",
            )
            .bind(&eid)
            .bind(sheet_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete removed sync_log: {e}"))?;
            // Conflitos órfãos somem com a transação removida.
            sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
                .bind(&eid)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("delete removed conflicts: {e}"))?;
        }
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;

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

pub fn parse_rows_with_layout(
    rows: &[Vec<String>],
    layout: &SheetLayout,
    mappings: &[(String, i32)],
    notes: &[Vec<String>],
) -> Vec<ImportedRow> {
    let mut imported = Vec::new();

    let year = layout.year.unwrap_or(2025);
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    if month_row >= rows.len() {
        return imported;
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
                    });
                }
            }
        }
    }

    imported
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
) -> Vec<DailyBalance> {
    let mut out = Vec::new();

    let year = layout.year.unwrap_or(2025);
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    if month_row >= rows.len() {
        return out;
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

    out
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
pub async fn store_balance_series(
    pool: &SqlitePool,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;

    sqlx::query("DELETE FROM sheet_daily_balance WHERE sheet_name = ?1")
        .bind(sheet_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("clear balances: {e}"))?;

    for b in series {
        sqlx::query(
            "INSERT OR REPLACE INTO sheet_daily_balance (sheet_name, date, balance_cents, is_projection) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(sheet_name)
        .bind(&b.date)
        .bind(b.balance_cents)
        .bind(b.is_projection as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert balance: {e}"))?;
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(series.len())
}

/// Converte texto monetário em centavos. Regra fechada de separadores (spec 010, slice 0):
/// com `.` e `,` presentes, o que aparece POR ÚLTIMO é o decimal (cobre pt-BR `6.012,73` e
/// en_US `6,012.73`); um separador sozinho é decimal, exceto padrão claro de agrupamento de
/// milhar (`6.012`, `1.234.567`). Floats do xlsx chegam normalizados com 4 casas fixas
/// (ver `xlsx_cell_to_string`), então nunca caem na ambiguidade de 3 dígitos.
pub fn parse_number(s: &str) -> i64 {
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

    if let Ok(f) = normalized.parse::<f64>() {
        (f * 100.0).round() as i64
    } else {
        0
    }
}

/// Padrão inequívoco de milhar: primeiro grupo com 1–3 dígitos e todos os demais com
/// exatamente 3 (`6.012`, `1.234.567`) — qualquer outra forma é tratada como decimal.
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

    // Regressão spec 010 slice 0: valores reais da planilha nos dois locales + xlsx.
    #[test]
    fn test_parse_number_separator_rules() {
        // xlsx/calamine: ponto decimal puro — antes inflava 100× (65.28 → 652800).
        assert_eq!(parse_number("65.28"), 6528);
        assert_eq!(parse_number("6012.73"), 601273);
        // Saldo real com 4 casas: arredonda a centavos na fronteira.
        assert_eq!(parse_number("10805.5048"), 1080550);
        assert_eq!(parse_number("289.9252"), 28993);
        // Float do xlsx normalizado com 4 casas fixas (xlsx_cell_to_string).
        assert_eq!(parse_number("65.2800"), 6528);
        assert_eq!(parse_number("123.4560"), 12346);
        // Sheets FORMATTED pt-BR e en_US: o último separador é o decimal.
        assert_eq!(parse_number("6.012,73"), 601273);
        assert_eq!(parse_number("6,012.73"), 601273);
        assert_eq!(parse_number("R$ 1.234,56"), 123456);
        // Separador único com agrupamento claro de milhar.
        assert_eq!(parse_number("6.012"), 601200);
        assert_eq!(parse_number("1.234.567"), 123456700);
        assert_eq!(parse_number("6,012"), 601200);
        // Decimal pt-BR sem milhar; negativos.
        assert_eq!(parse_number("1370,5"), 137050);
        assert_eq!(parse_number("-45,00"), -4500);
        assert_eq!(parse_number("-45.00"), -4500);
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

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);

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
        notes[2][1] = "R$ 18,33 - Pagamento\nR$ 2,32 - Rendimentos".into();

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &notes);

        let entrada = result.iter().find(|r| r.amount > 0).unwrap();
        assert_eq!(
            entrada.description,
            "R$ 18,33 - Pagamento · R$ 2,32 - Rendimentos"
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
                "10571.0048".into(),
            ],
            vec![
                "2".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "-46.33".into(),
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

        let series = parse_balance_series(&rows, &layout, 4);

        assert_eq!(series.len(), 2); // dia 3 (Saldo vazio) é pulado
        assert_eq!(
            series[0],
            DailyBalance {
                date: "2026-01-01".into(),
                balance_cents: 1_057_100, // 10571.0048 → centavos
                is_projection: false,
            }
        );
        assert_eq!(series[1].date, "2026-01-02");
        assert_eq!(series[1].balance_cents, -4633); // saldo negativo preservado
    }

    #[test]
    fn test_compute_checksum() {
        let rows = vec![ImportedRow {
            date: "2025-01-01".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
            kind: RowKind::Entrada,
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
        day1[1] = "6012.73".into(); // Entrada em JANEIRO (bloco no offset 0)
        day1[66 + 2] = "65.28".into(); // Saída em DEZEMBRO (bloco no offset 66)
        vec![month_row, header_row, day1]
    }

    // Regressão do bug `i > 0`: JANEIRO era dropado e todo mês deslocava 1 para trás.
    #[test]
    fn january_at_offset_zero_and_december_resolve_by_month_name() {
        let rows = real_geometry_rows(false);
        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);

        assert_eq!(result.len(), 2);
        let entrada = result.iter().find(|r| r.amount > 0).unwrap();
        assert_eq!(entrada.date, "2026-01-01");
        assert_eq!(entrada.amount, 601273);
        let saida = result.iter().find(|r| r.amount < 0).unwrap();
        assert_eq!(saida.date, "2026-12-01");
        assert_eq!(saida.amount, -6528);
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
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);

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
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);

        // Só as duas linhas válidas do dia 1; "2026-02-30" não existe.
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.date != "2026-02-30"));
    }

    // Regressão: célula não-vazia entre blocos não pode virar bloco nem deslocar meses.
    #[test]
    fn spurious_cell_between_blocks_does_not_shift_months() {
        let rows = real_geometry_rows(true);
        let layout = real_geometry_layout();
        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);

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
        ImportedRow {
            date: date.into(),
            amount,
            description: format!("Linha {date}"),
            is_projection: false,
            kind: if amount >= 0 {
                RowKind::Entrada
            } else {
                RowKind::Saida
            },
        }
    }

    async fn count_transactions(pool: &SqlitePool) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(pool)
            .await
            .unwrap()
            .0
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
        let result = parse_rows_with_layout(&rows, &layout, &mappings, &[]);
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
}
