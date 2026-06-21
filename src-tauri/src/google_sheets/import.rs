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
    // Hoje é REALIZADO (não projeção): `<=` inclui a data de hoje em `is_past`,
    // então `is_projection = !is_past = false` no modo "both". O `<` antigo
    // jogava o gasto de hoje no painel de previsão.
    let is_past = date_str <= today.as_str();

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
        // `is_projection` NÃO entra no checksum: é um campo DERIVADO de `Local::now()`
        // no import, não dado-fonte. Incluí-lo fazia a MESMA planilha inalterada gerar
        // um checksum diferente a cada dia → re-import integral espúrio diário.
        hasher.update(row.kind.as_str().as_bytes());
        // A nota crua entra no checksum: editar SÓ a nota de célula (ex.: retag de
        // `#reembolso:`/`#dividir:`) é uma mudança real que o re-import deve aplicar —
        // o bloco de marcadores re-deriva splits/Entradas a partir da nota (autoritativa).
        hasher.update(row.raw_note.as_bytes());
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

        // --- Plan 023: gramática das notas (#reembolso:/#dividir:) ---
        // Opt-in e forward-only: nota sem marcador → no-op (idêntico ao comportamento de hoje).
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
                              created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(tagged.line_amount_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
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
                              created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(share_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert dividir Entrada: {e}"))?;
                    }
                }
            }
        }
        // --- fim Plan 023 ---

        // --- Plan 035/036: nota itemizada → linhas em line_item ---
        // O estilo de anotação do usuário é a célula itemizada: o TOTAL da célula é a
        // SOMA de partes, cada parte descrita em uma linha da nota. Aqui surfeamos essas
        // partes como filhos descritivos (passado E projetado), sem NUNCA mexer no total.
        //
        // PRESERVAÇÃO DE EDIÇÃO LOCAL (plano 036): o app deixa o dono EDITAR as partes
        // (`update_transaction_items_cmd` grava com `is_user_edited = 1`). Essas edições
        // locais são autoritativas até a NOTA da planilha mudar. Por isso só re-derivamos
        // da nota quando ela MUDOU desde o último import — comparando `row.raw_note` com o
        // `source_note` (base) guardado no pai. Espelha o merge de 3 vias do `source_amount`:
        // base = nota vista no último import; local = itens editados no app; entrante = nota
        // atual. Nota inalterada + itens editados → mantém o local; nota mudou → a nota vence
        // (re-deriva), consistente com o bloco 023.
        //
        // SEGURO POR PADRÃO: se a nota não tem ≥2 linhas `R$` OU o somatório das partes
        // diverge do total da célula além de 1 centavo (arredondamento), nenhum item é
        // gravado — só o total da transação fica. O total do pai jamais é alterado.
        {
            // Base (nota do último import) + se há item editado pelo usuário nesta txn.
            let (prev_source_note, has_user_edited): (Option<String>, i64) = {
                let snote: Option<(Option<String>,)> =
                    sqlx::query_as(r#"SELECT source_note FROM "transaction" WHERE id = ?1"#)
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
            // a nota atual da planilha passa a ser a base do próximo import.
            sqlx::query(r#"UPDATE "transaction" SET source_note = ?2 WHERE id = ?1"#)
                .bind(&txn_id)
                .bind(&row.raw_note)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("set source_note for {txn_id}: {e}"))?;

            if !keep_local {
                let items = parse_itemized_note(&row.raw_note);
                let parts_sum: i64 = items.iter().map(|i| i.amount_cents).sum();
                let parent_total = row.amount.abs();
                // Exige ≥2 partes (1 parte não é um breakdown) e somatório casando com o total.
                let sum_matches = items.len() >= 2 && (parts_sum - parent_total).abs() <= 1;

                // Limpa os itens antigos desta txn (idempotente no re-import; a nota é autoritativa).
                sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
                    .bind(&txn_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| format!("clear line_items for {txn_id}: {e}"))?;

                if sum_matches {
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
                // Somatório não bate ou nota sem itens: nenhum item inserido; total do pai intacto.
            }
            // keep_local: itens editados no app sobrevivem (a nota não mudou) — nada a fazer.
        }
        // --- fim Plan 035/036 ---

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
/// (sem entradas em `tagged_lines`), de modo que o import se comporta
/// byte-a-byte como hoje.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct NoteMarkers {
    /// Linhas da nota que carregam um marcador reconhecido, na ordem em que
    /// aparecem. Linhas sem marcador não aparecem aqui.
    pub tagged_lines: Vec<TaggedLine>,
}

/// Uma linha de nota com marcador reconhecido.
#[derive(Debug, PartialEq)]
pub(crate) struct TaggedLine {
    /// Índice 0-based da linha dentro da nota (para id determinístico).
    pub line_index: usize,
    /// Valor da linha em centavos inteiros (magnitude positiva).
    /// Extraído do prefixo `R$ <valor>` da linha.
    pub line_amount_cents: i64,
    /// Nome do terceiro que divide ou reembolsa (sem normalização de caixa).
    pub person_name: String,
    /// Tipo do marcador.
    pub kind: NoteMarkerKind,
}

/// Tipo de marcador de nota.
#[derive(Debug, PartialEq)]
pub(crate) enum NoteMarkerKind {
    /// `#reembolso:<quem>` — o VALOR INTEGRAL da linha será reembolsado por <quem>.
    /// Gera uma Entrada compensatória de `line_amount_cents`.
    Reembolso,
    /// `#dividir:<quem>` ou `#dividir:<quem>:<valor>` — a parte de <quem>.
    /// `share_cents` é 50% de `line_amount_cents` (arredondado para baixo) quando
    /// não explicitado; caso contrário, o valor explícito.
    /// Gera um split para <quem> E uma Entrada compensatória de `share_cents`.
    Dividir {
        /// Parte de <quem> em centavos (já resolvida: padrão 50% ou valor explícito).
        share_cents: i64,
    },
}

/// Plan 035: uma parte itemizada extraída de uma linha da nota de célula.
#[derive(Debug, PartialEq)]
pub(crate) struct NoteLineItem {
    /// Magnitude em centavos (positiva). Mesma convenção de `transaction.amount`.
    pub amount_cents: i64,
    pub description: String,
    /// Posição 0-based na nota (ordem de aparição).
    pub position: usize,
    /// Cabeçalho de seção imediatamente anterior a este item na nota original
    /// (ex.: "CONTAS:", "CARTÕES:"). `None` quando o item não está sob um cabeçalho.
    pub section: Option<String>,
}

/// Parseia as linhas itemizadas de uma nota de célula (Plan 035).
///
/// O estilo de anotação do usuário é a célula itemizada: um TOTAL que é a SOMA de
/// partes, cada parte descrita em uma linha da nota como `R$ <valor> - <descrição>`.
///
/// GRAMÁTICA: cada linha começando com `R$` (com ou sem espaço entre `R$` e o número)
/// é tratada como um item; o que vem antes do primeiro traço é o valor, o resto é a
/// descrição. Linhas que NÃO começam com `R$` (cabeçalhos, trailers `Total = …`,
/// linhas de orçamento separadas por tab) NÃO viram itens, mas a última linha não-`R$`
/// não-vazia vista é guardada como o `section` (cabeçalho) dos itens seguintes — ela é
/// reproduzida no write-back (plano 048). Linhas em branco preservam o `section` atual.
///
/// Tolerâncias:
/// - `R$<número>` e `R$ <número>` (espaço opcional após `R$`)
/// - ` - ` e `-` (espaço opcional ao redor do traço)
/// - Valor em pt-BR (`1.234,56`) ou float do xlsx (`1234.5600`) — via `parse_number`
/// - Linha com marcador `#reembolso:`/`#dividir:` no fim: parseia o item normalmente
///   (o marcador fica na descrição). Os dois parsers são leituras INDEPENDENTES da
///   mesma nota; este não substitui nem altera `parse_note_markers`.
///
/// SEGURO POR PADRÃO: nota vazia ou sem linhas `R$` → lista vazia. A reconciliação
/// (somatório das partes ≈ total do pai) é decidida na camada de persistência, não aqui:
/// se não bater, nenhum item é gravado e o total do pai fica intocado.
///
/// PURA — sem I/O, sem DB, sem panics.
pub(crate) fn parse_itemized_note(note: &str) -> Vec<NoteLineItem> {
    let mut items = Vec::new();
    // Cabeçalho de seção mais recente (última linha não-`R$` não-vazia). Plano 048.
    let mut current_section: Option<String> = None;
    for (pos, line) in note.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Linha em branco: preserva o contexto de seção (espaçamento da gramática), pula.
            continue;
        }
        // Linha não-`R$` → trata como cabeçalho de seção: atualiza o contexto e pula.
        if trimmed.len() < 2 || !trimmed[..2].eq_ignore_ascii_case("r$") {
            current_section = Some(trimmed.to_string());
            continue;
        }
        let rest = trimmed[2..].trim_start();
        // Separador no PRIMEIRO traço: o que vem antes é o valor, o resto é a descrição.
        // Usar o primeiro traço permite descrições com traço (ex.: "Produto A - loja B")
        // sem truncar, porque o valor (positivo) nunca contém traço.
        let (value_part, desc_part) = if let Some(idx) = rest.find('-') {
            (rest[..idx].trim_end(), rest[idx + 1..].trim_start())
        } else {
            // Sem separador → a linha inteira é o valor, sem descrição.
            (rest, "")
        };
        let amount_cents = parse_number(value_part.trim());
        if amount_cents <= 0 {
            continue; // valor inválido, zero ou negativo → pula
        }
        items.push(NoteLineItem {
            amount_cents,
            description: desc_part.to_string(),
            position: pos,
            section: current_section.clone(),
        });
    }
    items
}

/// GRAMÁTICA DAS NOTAS (contrato público — opt-in, explícito, seguro por padrão).
///
/// Cada linha da nota é analisada de forma independente. Uma linha só vira
/// marcador quando casa EXATAMENTE com uma das formas estruturadas abaixo;
/// uma nota sem marcador não produz split nem Entrada compensatória
/// (idêntico ao comportamento anterior — provado por teste).
///
/// A sintaxe foi escolhida para não colidir com a convenção pessoal de prosa livre
/// do usuário (validado contra a planilha de referência: zero linhas começando com
/// `R$` E terminando com `#reembolso:` ou `#dividir:`).
///
/// Formas reconhecidas (cada linha analisada individualmente):
///
///   `R$ <valor> - <descrição> #reembolso:<quem>`
///       O valor INTEGRAL da linha é reembolsado por <quem>.
///       Gera uma Entrada compensatória de <valor> centavos, datada na data
///       da transação pai, `description = "Reembolso: <quem>"`.
///       Cashflow líquido = zero (Saída anulada pela Entrada).
///
///   `R$ <valor> - <descrição> #dividir:<quem>`
///       50% de <valor> (arredondado para baixo) é a parte de <quem>.
///       Gera: (1) split na transação pai com owner=<quem>, amount=share;
///             (2) Entrada compensatória de share centavos.
///
///   `R$ <valor> - <descrição> #dividir:<quem>:<valor_da_parte>`
///       Igual, mas com valor explícito para a parte de <quem>.
///
/// Exemplos:
///   `"R$ 530 - Cartões Pessoa B #reembolso:Pessoa B"` → Entrada R$530, owner Pessoa B
///   `"R$ 200 - Almoço #dividir:Pessoa B"`     → split+Entrada R$100 (50%)
///   `"R$ 200 - Almoço #dividir:Pessoa B:80"`  → split+Entrada R$80 (explícito)
///   `"R$ 1.200 - Parcela carro"`              → NENHUM marcador (prosa livre)
///   `"Mercado da semana"`                     → NENHUM marcador
///
/// Pura — sem I/O, sem DB, sem panics. Testável sem pool.
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut tagged_lines: Vec<TaggedLine> = Vec::new();

    for (line_index, line) in note.lines().enumerate() {
        let trimmed = line.trim();

        // Marcador deve estar no sufixo: localiza o último '#' na linha.
        let Some(hash_pos) = trimmed.rfind('#') else {
            continue;
        };
        let before_hash = &trimmed[..hash_pos];
        let tag_suffix = &trimmed[hash_pos..]; // inclui o '#'

        let tag_lower = tag_suffix.to_ascii_lowercase();

        // Extrai <quem> e opcional <valor_da_parte> do sufixo reconhecido.
        // Retorna (marker_kind_tag, (person_name, Option<valor_da_parte_str>)).
        let (marker_kind_tag, person_raw, explicit_valor_str) =
            if tag_lower.starts_with("#reembolso:") {
                let person = tag_suffix["#reembolso:".len()..].trim();
                ("reembolso", person, None::<&str>)
            } else if tag_lower.starts_with("#dividir:") {
                let payload = tag_suffix["#dividir:".len()..].trim();
                if let Some(colon) = payload.find(':') {
                    let person = payload[..colon].trim();
                    let val = payload[colon + 1..].trim();
                    ("dividir", person, Some(val))
                } else {
                    ("dividir", payload, None::<&str>)
                }
            } else {
                continue; // tag não reconhecida
            };

        let person_name = person_raw.to_string();
        if person_name.is_empty() {
            continue; // <quem> vazio → ignora
        }

        // Extrai R$ <valor> do prefixo `before_hash`.
        // Formato esperado: `R$ <número> - <descrição> ` (com espaço antes do `#`).
        let before = before_hash.trim();
        // Prefixo `R$` case-insensitive; fatia a partir da string original para preservar
        // a grafia dos dígitos (parse_number só precisa de vírgula/ponto/dígitos).
        let line_amount_cents = if before
            .get(..2)
            .is_some_and(|p| p.eq_ignore_ascii_case("r$"))
        {
            let rest = &before[2..];
            // Tudo antes do primeiro ` - ` é o valor.
            let value_part = if let Some(dash) = rest.find(" - ") {
                &rest[..dash]
            } else {
                rest
            };
            // Usa parse_number existente (lida com vírgula/ponto); retorna i64 em centavos.
            parse_number(value_part.trim())
        } else {
            continue; // linha não começa com R$ → ignora
        };

        if line_amount_cents <= 0 {
            continue; // valor inválido ou zero → ignora
        }

        let kind = match marker_kind_tag {
            "reembolso" => NoteMarkerKind::Reembolso,
            "dividir" => {
                let share_cents = if let Some(val_str) = explicit_valor_str {
                    let v = parse_number(val_str);
                    if v > 0 { v } else { line_amount_cents / 2 }
                } else {
                    line_amount_cents / 2 // 50% arredondado para baixo
                };
                NoteMarkerKind::Dividir { share_cents }
            }
            _ => continue,
        };

        tagged_lines.push(TaggedLine {
            line_index,
            line_amount_cents,
            person_name,
            kind,
        });
    }

    NoteMarkers { tagged_lines }
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
    fn classify_row_today_is_realized() {
        // A row dated today must be realized (is_projection = false), not projected.
        // Bug 1: the old `<` comparison made today a projection.
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert!(
            !classify_row(&today, "both").unwrap(),
            "today must be realized (is_projection=false) in 'both' mode"
        );
        // "past_only" and "future_only" are direction overrides; unchanged by this fix.
        assert!(!classify_row(&today, "past_only").unwrap());
        assert!(classify_row(&today, "future_only").unwrap());
    }

    #[test]
    fn checksum_excludes_is_projection_field() {
        // Bug 2: is_projection is date-relative (computed from today), so including it
        // in the checksum caused the same unchanged sheet to produce a different checksum
        // on a different calendar day → daily spurious full re-import.
        // Fix: is_projection must NOT affect the checksum.
        let row_as_future = ImportedRow {
            date: "2099-01-15".into(),
            amount: 50000,
            description: "Gasto fixo".into(),
            is_projection: true, // "future" classification
            kind: RowKind::Saida,
            raw_note: String::new(),
        };
        let row_as_past = ImportedRow {
            date: "2099-01-15".into(),
            amount: 50000,
            description: "Gasto fixo".into(),
            is_projection: false, // same source data, different derived classification
            kind: RowKind::Saida,
            raw_note: String::new(),
        };
        // Same source data → same checksum regardless of is_projection.
        assert_eq!(
            compute_checksum(&[row_as_future]),
            compute_checksum(&[row_as_past]),
            "checksum must not depend on is_projection (derived field)"
        );
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

    // Plan 035: linha importada com nota de célula crua e flag de projeção (passado/futuro).
    fn imported_note(date: &str, amount: i64, raw_note: &str, is_projection: bool) -> ImportedRow {
        ImportedRow {
            raw_note: raw_note.into(),
            is_projection,
            ..imported_desc(date, amount, &format!("Linha {date}"))
        }
    }

    async fn count_line_items(pool: &SqlitePool, txn_id: &str) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1")
            .bind(txn_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
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
    // Plan 023: gramática das notas (parse puro, sem DB)
    // ===================================================================

    #[test]
    fn parse_note_markers_empty_note() {
        let m = parse_note_markers("");
        assert!(m.tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_free_prose_ignored() {
        // Notas de prosa livre NÃO disparam marcador algum.
        // Formato real da planilha: "R$ X - descrição" sem tag.
        let note = "R$ 65,00 - Vivo · faltou só o frango";
        assert!(parse_note_markers(note).tagged_lines.is_empty());

        // Linha sem R$ também é ignorada.
        assert!(
            parse_note_markers("Mercado da semana")
                .tagged_lines
                .is_empty()
        );
    }

    #[test]
    fn parse_note_markers_reembolso_full_value() {
        let note = "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].line_index, 0);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 53000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa B");
        assert_eq!(m.tagged_lines[0].kind, NoteMarkerKind::Reembolso);
    }

    #[test]
    fn parse_note_markers_dividir_default_50_percent() {
        let note = "R$ 200,00 - Almoço #dividir:Pessoa A";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 20000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa A");
        assert_eq!(
            m.tagged_lines[0].kind,
            NoteMarkerKind::Dividir { share_cents: 10000 } // 50% de 200
        );
    }

    #[test]
    fn parse_note_markers_dividir_explicit_value() {
        let note = "R$ 200,00 - Almoço #dividir:Pessoa A:80,00";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(
            m.tagged_lines[0].kind,
            NoteMarkerKind::Dividir { share_cents: 8000 } // valor explícito
        );
    }

    #[test]
    fn parse_note_markers_multiple_tagged_lines() {
        // Nota com duas linhas marcadas e uma linha de prosa livre.
        let note = "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B\n\
                    R$ 1.200,00 - Parcela carro\n\
                    R$ 191,00 - Empréstimo Pessoa C #reembolso:Pessoa C";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 2);
        assert_eq!(m.tagged_lines[0].line_index, 0);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 53000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa B");
        assert_eq!(m.tagged_lines[1].line_index, 2);
        assert_eq!(m.tagged_lines[1].line_amount_cents, 19100);
        assert_eq!(m.tagged_lines[1].person_name, "Pessoa C");
    }

    #[test]
    fn parse_note_markers_case_insensitive_tag() {
        // O marcador é case-insensitive.
        let note = "R$ 100,00 - Teste #REEMBOLSO:Pessoa A";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].kind, NoteMarkerKind::Reembolso);
    }

    #[test]
    fn parse_note_markers_no_rs_prefix_ignored() {
        // Linha sem `R$` não é marcador — mesmo que termine com `#reembolso:`.
        let note = "Transferência bancária #reembolso:Pessoa A";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_empty_person_ignored() {
        // `#reembolso:` sem <quem> → ignora.
        let note = "R$ 100,00 - Teste #reembolso:";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_old_at_syntax_ignored() {
        // `@Pessoa A: 150,00` (sintaxe anterior) já não é um marcador reconhecido.
        let note = "@Pessoa A: 150,00";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_old_credito_ignored() {
        // `#credito` (sintaxe anterior) não é mais reconhecido.
        let note = "#credito";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    // ===================================================================
    // Plan 023: testes de integração (DB)
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

    // --- Plan 035: parser puro parse_itemized_note (sem I/O) ---

    // Happy path: gramática padrão → duas partes com valor, descrição e posição.
    #[test]
    fn itemized_standard_form_parses_parts() {
        let note = "R$ 150,00 - Categoria A\nR$ 200,50 - Categoria B";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].amount_cents, 15_000);
        assert_eq!(items[0].description, "Categoria A");
        assert_eq!(items[0].position, 0);
        assert_eq!(items[1].amount_cents, 20_050);
        assert_eq!(items[1].description, "Categoria B");
        assert_eq!(items[1].position, 1);
    }

    // Tolerância: sem espaço depois do `R$`.
    #[test]
    fn itemized_tolerates_no_space_after_rs() {
        let items = parse_itemized_note("R$300,00 - Item");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].amount_cents, 30_000);
        assert_eq!(items[0].description, "Item");
    }

    // Tolerância: sem espaço ao redor do traço.
    #[test]
    fn itemized_tolerates_no_space_around_dash() {
        let items = parse_itemized_note("R$ 50,00-Descrição do item");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].amount_cents, 5_000);
        assert_eq!(items[0].description, "Descrição do item");
    }

    // Cabeçalho (sem `R$`) e trailer `Total = …` são pulados.
    #[test]
    fn itemized_skips_header_lines() {
        let note = "CONTAS:\nR$ 100,00 - Item A\nTotal = R$ 100,00";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 1, "só a linha R$ do meio é item");
        assert_eq!(items[0].amount_cents, 10_000);
        assert_eq!(items[0].description, "Item A");
    }

    // Linha de orçamento separada por tab (sem `R$` à esquerda) é pulada.
    #[test]
    fn itemized_skips_tab_separated_budget_lines() {
        let note = "Mensal\tR$ 300,00\tCategoria\nR$ 50,00 - Outro item";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].description, "Outro item");
    }

    // Nota vazia / só espaços → nenhum item (seguro por padrão).
    #[test]
    fn itemized_empty_note_yields_no_items() {
        assert!(parse_itemized_note("").is_empty());
        assert!(parse_itemized_note("   ").is_empty());
    }

    // Nota só com prosa (sem linhas `R$`) → nenhum item.
    #[test]
    fn itemized_no_rs_lines_yields_no_items() {
        assert!(parse_itemized_note("Descrição geral sem itens").is_empty());
    }

    // Linha com sufixo de marcador: o item é parseado; o marcador fica na descrição.
    // (parse_note_markers faz o trabalho dele na mesma nota, de forma independente.)
    #[test]
    fn itemized_line_with_marker_parses_as_item() {
        let note = "R$ 200,00 - Item X #reembolso:Pessoa Y";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].amount_cents, 20_000);
        assert!(items[0].description.contains("Item X"));
    }

    // O parse produz os valores individuais corretos independentemente da reconciliação
    // (a decisão de anexar/descartar é da camada de persistência, não do parser).
    #[test]
    fn itemized_mismatched_sum_still_parses_individual_amounts() {
        let note = "R$ 100,00 - Item A\nR$ 100,00 - Item B"; // soma = 200
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].amount_cents + items[1].amount_cents, 20_000);
    }

    // Descrição com traço interno não trunca (usa só o primeiro traço como separador).
    #[test]
    fn itemized_keeps_dash_inside_description() {
        let items = parse_itemized_note("R$ 80,00 - Produto A - loja B");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].amount_cents, 8_000);
        assert_eq!(items[0].description, "Produto A - loja B");
    }

    // Valor em float do xlsx (ponto decimal) é tolerado via parse_number.
    #[test]
    fn itemized_tolerates_xlsx_float_value() {
        let items = parse_itemized_note("R$ 1234.5600 - Item");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].amount_cents, 123_456);
    }

    // Plano 048: parse_itemized_note captura o cabeçalho de seção das linhas não-`R$`.
    #[test]
    fn itemized_captures_section_header() {
        let note = "CONTAS:\nR$ 100,00 - Item A\nR$ 50,00 - Item B";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
        assert_eq!(items[1].section.as_deref(), Some("CONTAS:"));
    }

    // Plano 048: duas seções separadas por linha em branco → cada item recebe seu cabeçalho.
    #[test]
    fn itemized_two_sections_assign_correct_header() {
        let note = "CONTAS:\nR$ 100,00 - Item A\n\nCARTÕES:\nR$ 200,00 - Item B";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
        assert_eq!(items[1].section.as_deref(), Some("CARTÕES:"));
    }

    // Plano 048: item sem cabeçalho anterior → section = None.
    #[test]
    fn itemized_no_header_yields_none_section() {
        let note = "R$ 150,00 - Item sem cabeçalho";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 1);
        assert!(items[0].section.is_none());
    }

    // --- Plan 035: persistência de line_item no import (camada DB) ---

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

    // Mismatch: partes não somam o total → nenhum item; total do pai intocado.
    #[tokio::test]
    async fn line_items_not_stored_when_sum_mismatches() {
        let pool = test_pool().await;
        // Total R$ 100,00; nota soma R$ 120,00 (não bate) → 0 itens.
        let rows = vec![imported_note(
            "2026-02-11",
            -10_000,
            "R$ 60,00 - A\nR$ 60,00 - B",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-11", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 0);
        assert_eq!(amount_by_date(&pool, "2026-02-11").await, 10_000);
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

    // Uma única parte não é um breakdown → nenhum item (evita "item-fantasma").
    #[tokio::test]
    async fn line_items_single_part_not_stored() {
        let pool = test_pool().await;
        let rows = vec![imported_note(
            "2026-02-15",
            -10_000,
            "R$ 100,00 - Único",
            false,
        )];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let txn_id = row_id("2026", "2026-02-15", RowKind::Saida, 0);
        assert_eq!(count_line_items(&pool, &txn_id).await, 0);
    }

    // Plano 036: edição LOCAL das partes sobrevive ao re-import enquanto a nota da planilha não
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

    // Plano 036: quando a NOTA da planilha muda, ela vence — re-deriva e descarta a edição local
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
}
