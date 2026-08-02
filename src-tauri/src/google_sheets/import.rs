use super::layout_detect::{SheetLayout, month_number_from_name};
use super::reconcile;
use crate::{cards, commands::card_cmds};
use chrono::Datelike;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

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

/// Contexto de cartões lido antes da transação de importação. Manter essas leituras fora da
/// transação evita disputar a única conexão do pool enquanto a planilha é persistida.
#[derive(Debug, Default)]
pub(crate) struct CardScanCtx {
    pub aliases: HashMap<String, String>,
    pub cycles: HashMap<String, (u32, u32)>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CardScanOutcome {
    pub invoices_created: usize,
    pub invoices_updated: usize,
    pub conflicts: usize,
    pub proposals: usize,
    pub ignored_items: usize,
}

/// Carrega identidades e ciclos antes do `begin`. Alias explícito vence o nome implícito caso
/// uma base legada contenha a colisão que as fronteiras atuais já impedem.
pub(crate) async fn load_card_scan_ctx(pool: &SqlitePool) -> Result<CardScanCtx, String> {
    let accounts: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM account WHERE type = 'credit_card'")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("load card accounts: {e}"))?;

    let mut ctx = CardScanCtx::default();
    for (id, name) in &accounts {
        let alias = cards::normalize_alias(name);
        if !alias.is_empty() {
            ctx.aliases.insert(alias, id.clone());
        }
        if let Ok(cycle) = card_cmds::effective_cycle(pool, id).await {
            ctx.cycles.insert(id.clone(), cycle);
        }
    }
    let aliases: Vec<(String, String)> = sqlx::query_as("SELECT alias, account_id FROM card_alias")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load card aliases: {e}"))?;
    for (alias, account_id) in aliases {
        ctx.aliases.insert(alias, account_id);
    }
    Ok(ctx)
}

/// Identidades de cartão que ESTA planilha declara: todo alias escrito sob a seção de cartões,
/// reduzido à sua raiz, mais os aliases das contas já cadastradas. É contra esse vocabulário — e
/// só contra ele — que uma linha sem cabeçalho de seção pode ser reconhecida como fatura.
///
/// A raiz é a identidade porque a planilha distingue ciclos do mesmo cartão no próprio nome
/// (`Nubank (26/09)`, `Nubank (26/12)`): sem reduzir, cada anotação viraria um cartão diferente.
fn sheet_card_lexicon(
    values: &[Vec<String>],
    notes: &[Vec<String>],
    layout: &SheetLayout,
    amount_out_offset: usize,
    blocks: &[(usize, u32)],
    ctx: &CardScanCtx,
) -> cards::CardLexicon<String> {
    // A conta cadastrada é autoridade sobre si mesma: o alias dela nunca é reduzido à raiz.
    let mut entries: Vec<(String, String)> = ctx
        .aliases
        .keys()
        .map(|alias| (alias.clone(), alias.clone()))
        .collect();
    for (row_idx, _row) in values
        .iter()
        .enumerate()
        .skip(layout.data_start_row as usize)
    {
        for &(offset, _month) in blocks {
            let note = notes
                .get(row_idx)
                .and_then(|note_row| note_row.get(offset + amount_out_offset))
                .map(String::as_str)
                .unwrap_or("");
            for item in parse_itemized_note_opts(note, true) {
                if item.kind != ItemKind::Cartao {
                    continue;
                }
                let alias = cards::declared_alias(item.description.trim());
                if !alias.is_empty() {
                    entries.push((alias.clone(), cards::root_alias(&alias)));
                }
            }
        }
    }
    cards::CardLexicon::from_entries(entries)
}

/// Varre diretamente as notas da coluna Saída. Faturas são estrutura da grade, portanto a
/// varredura não depende de a célula ter materializado uma `ImportedRow` nem do checksum.
pub(crate) async fn scan_card_invoices(
    tx: &mut sqlx::SqliteConnection,
    values: &[Vec<String>],
    notes: &[Vec<String>],
    layout: &SheetLayout,
    amount_out_offset: usize,
    ctx: &CardScanCtx,
) -> Result<CardScanOutcome, String> {
    let Some(year) = layout.year else {
        return Ok(CardScanOutcome::default());
    };
    let month_row = layout.month_names_row as usize;
    if month_row >= values.len() {
        return Ok(CardScanOutcome::default());
    }
    let blocks = month_blocks_for(&values[month_row], layout.block_size as usize);
    let lexicon = sheet_card_lexicon(values, notes, layout, amount_out_offset, &blocks, ctx);
    let mut outcome = CardScanOutcome::default();
    let mut present: HashMap<String, HashSet<String>> = HashMap::new();

    for (row_idx, row) in values
        .iter()
        .enumerate()
        .skip(layout.data_start_row as usize)
    {
        let day: u32 = row
            .get(layout.day_column as usize)
            .map_or("", |v| v.trim())
            .parse::<f64>()
            .ok()
            .filter(|day| (1.0..=31.0).contains(day))
            .map(|day| day as u32)
            .unwrap_or(0);
        if day == 0 {
            continue;
        }
        for &(offset, month) in &blocks {
            let Some(due_date_value) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
                continue;
            };
            let due_date = due_date_value.to_string();
            // A célula foi visitada dentro da geometria da Saída, mesmo sem nota: ela participa
            // da reconciliação global das faturas que aquele vencimento pode declarar.
            present.entry(due_date.clone()).or_default();
            let note = notes
                .get(row_idx)
                .and_then(|note_row| note_row.get(offset + amount_out_offset))
                .map(String::as_str)
                .unwrap_or("");
            let cycle_month = cards::cycle_month_of(due_date_value);
            for item in parse_itemized_note_opts(note, true) {
                let raw_name = item.description.trim();
                // Sob o cabeçalho de seção a própria linha DECLARA a identidade do cartão; fora
                // dele, só um nome que o léxico da planilha já conhece conta como fatura. É essa
                // assimetria que recupera a nota sem cabeçalho sem transformar "Fatura Vivo" em
                // cartão.
                let from_section = item.kind == ItemKind::Cartao;
                let alias = if from_section {
                    cards::root_alias(&cards::declared_alias(raw_name))
                } else {
                    match lexicon.resolve(raw_name) {
                        Some(alias) => alias,
                        None => continue,
                    }
                };
                let Some(account_id) = ctx.aliases.get(&alias) else {
                    if alias.is_empty() {
                        outcome.ignored_items += 1;
                        continue;
                    }
                    // Só a linha declarada sob a seção inaugura identidade. Uma linha de fora
                    // apenas ALCANÇA o que já foi declarado — inventar um cartão a partir dela
                    // gravaria "Fatura Visa" como nome.
                    if !from_section {
                        outcome.ignored_items += 1;
                        continue;
                    }
                    let proposal_id = uuid::Uuid::new_v4().to_string();
                    let inserted = sqlx::query(
                        "INSERT INTO card_proposal (id, alias, display_name, source_month, status) \
                         VALUES (?1, ?2, ?3, ?4, 'pending') ON CONFLICT(alias) DO NOTHING",
                    )
                    .bind(&proposal_id)
                    .bind(&alias)
                    .bind(cards::root_display(raw_name))
                    .bind(&cycle_month)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("insert card proposal: {e}"))?
                    .rows_affected();
                    outcome.proposals += inserted as usize;

                    // A grafia desta linha entra como apelido da proposta: é assim que o cadastro
                    // nasce reconhecendo "Nubank (26/09)" e "Nubank" como o mesmo cartão.
                    let declared = cards::declared_alias(raw_name);
                    sqlx::query(
                        "INSERT INTO card_proposal_alias (id, proposal_id, alias) \
                         SELECT ?1, id, ?2 FROM card_proposal WHERE alias = ?3 \
                         ON CONFLICT(alias) DO NOTHING",
                    )
                    .bind(uuid::Uuid::new_v4().to_string())
                    .bind(&declared)
                    .bind(&alias)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("insert card proposal alias: {e}"))?;

                    if inserted == 0 {
                        // O mês de origem é o mais ANTIGO em que o cartão aparece. A varredura
                        // percorre a grade por dia antes de percorrer por mês, então o primeiro
                        // encontro é uma ordem de visita, não uma data.
                        sqlx::query(
                            "UPDATE card_proposal SET source_month = ?1 \
                             WHERE alias = ?2 AND source_month > ?1",
                        )
                        .bind(&cycle_month)
                        .bind(&alias)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("card proposal source month: {e}"))?;
                    }
                    continue;
                };
                let Some(&(closing_day, _due_day)) = ctx.cycles.get(account_id) else {
                    outcome.ignored_items += 1;
                    continue;
                };
                present
                    .entry(due_date.clone())
                    .or_default()
                    .insert(account_id.clone());
                let (closing_year, closing_month) = if closing_day < due_date_value.day() {
                    (due_date_value.year(), due_date_value.month())
                } else if due_date_value.month() == 1 {
                    (due_date_value.year() - 1, 12)
                } else {
                    (due_date_value.year(), due_date_value.month() - 1)
                };
                // O fechamento encurta no mês em que ele cai, como o vencimento — um cartão que
                // fecha dia 29+ existe, e fevereiro é problema da data, não do cadastro.
                let closing_date = chrono::NaiveDate::from_ymd_opt(
                    closing_year,
                    closing_month,
                    cards::closing_day_in(closing_year, closing_month, closing_day),
                )
                .ok_or("closing date inválida")?;
                let existing: Option<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
                    "SELECT id, stated_total_cents, source_stated_total_cents FROM invoice \
                     WHERE account_id = ?1 AND cycle_month = ?2",
                )
                .bind(account_id)
                .bind(&cycle_month)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("load invoice: {e}"))?;
                let (invoice_id, local, base, created) = if let Some((id, local, base)) = existing {
                    sqlx::query("UPDATE invoice SET due_date = ?1 WHERE id = ?2")
                        .bind(&due_date)
                        .bind(&id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("update invoice due date: {e}"))?;
                    (id, local, base, false)
                } else {
                    let id = uuid::Uuid::new_v4().to_string();
                    sqlx::query(
                        "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents, source_stated_total_cents) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    )
                    .bind(&id)
                    .bind(account_id)
                    .bind(&cycle_month)
                    .bind(closing_date.to_string())
                    .bind(&due_date)
                    .bind(item.amount_cents)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("create invoice: {e}"))?;
                    (id, Some(item.amount_cents), Some(item.amount_cents), true)
                };
                if created {
                    outcome.invoices_created += 1;
                    continue;
                }
                outcome.invoices_updated += 1;
                let sheet = item.amount_cents;
                let conflict_id = format!("invoice:{invoice_id}");
                if base == Some(sheet) {
                    sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1 AND field = 'stated_total'")
                        .bind(&conflict_id).execute(&mut *tx).await
                        .map_err(|e| format!("clear invoice conflict: {e}"))?;
                } else if local == base || (local.is_none() && base.is_none()) {
                    sqlx::query("UPDATE invoice SET stated_total_cents = ?1, source_stated_total_cents = ?1 WHERE id = ?2")
                        .bind(sheet).bind(&invoice_id).execute(&mut *tx).await
                        .map_err(|e| format!("apply invoice total: {e}"))?;
                    sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1 AND field = 'stated_total'")
                        .bind(&conflict_id).execute(&mut *tx).await
                        .map_err(|e| format!("clear invoice conflict: {e}"))?;
                } else if local == Some(sheet) {
                    sqlx::query("UPDATE invoice SET source_stated_total_cents = ?1 WHERE id = ?2")
                        .bind(sheet)
                        .bind(&invoice_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("align invoice total base: {e}"))?;
                    sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1 AND field = 'stated_total'")
                        .bind(&conflict_id).execute(&mut *tx).await
                        .map_err(|e| format!("clear invoice conflict: {e}"))?;
                } else {
                    let now = chrono::Utc::now().to_rfc3339();
                    sqlx::query(
                        "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
                         VALUES (?1, ?2, 'stated_total', ?3, ?4, ?5, ?6) \
                         ON CONFLICT(transaction_id, field) DO UPDATE SET base_value=excluded.base_value, \
                         local_value=excluded.local_value, sheet_value=excluded.sheet_value, created_at=excluded.created_at, \
                         resolved_at=NULL, resolution=NULL",
                    )
                    .bind(format!("conf:{conflict_id}:stated_total"))
                    .bind(&conflict_id)
                    .bind(base.map(|v| v.to_string()))
                    .bind(local.unwrap_or_default().to_string())
                    .bind(sheet.to_string())
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("record invoice conflict: {e}"))?;
                    outcome.conflicts += 1;
                }
            }
        }
    }

    // Reconcilia depois de observar TODAS as células Saída. Assim, esvaziar a seção de cartões
    // ainda remove a fatura importada, enquanto uma compra realizada — ou um reembolso vinculado
    // manualmente pelo dono (`refund_invoice_id`, nunca derivado do import) — preserva seu
    // histórico.
    let stale: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, account_id, due_date FROM invoice \
         WHERE source_stated_total_cents IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM \"transaction\" t \
                           WHERE t.invoice_id = invoice.id) \
           AND NOT EXISTS (SELECT 1 FROM \"transaction\" r \
                           WHERE r.refund_invoice_id = invoice.id) \
           AND (stated_total_cents IS NULL \
                OR stated_total_cents = source_stated_total_cents)",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("load removed card invoices: {e}"))?;
    for (invoice_id, account_id, due_date) in stale {
        if present
            .get(&due_date)
            .is_some_and(|accounts| !accounts.contains(&account_id))
        {
            sqlx::query("DELETE FROM invoice WHERE id = ?1")
                .bind(&invoice_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("reconcile removed card invoices: {e}"))?;
            // Sem isto, um `import_conflict` de rodadas anteriores (`invoice:<id>/stated_total`)
            // sobrevive à fatura que ele descreve: órfão, sem alvo resolvível na UI, e trava
            // `unresolved_conflict_count` acima de zero para sempre.
            sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
                .bind(format!("invoice:{invoice_id}"))
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("reconcile removed card invoice conflict: {e}"))?;
        }
    }
    Ok(outcome)
}

/// Variante para os retornos antecipados por checksum: a nota pode ter mudado sem alterar o lote
/// materializado, mas a estrutura de faturas ainda precisa ser observada.
pub(crate) async fn scan_card_invoices_standalone(
    pool: &SqlitePool,
    values: &[Vec<String>],
    notes: &[Vec<String>],
    layout: &SheetLayout,
    amount_out_offset: usize,
    ctx: &CardScanCtx,
) -> Result<CardScanOutcome, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("begin card scan: {e}"))?;
    let outcome =
        scan_card_invoices(&mut tx, values, notes, layout, amount_out_offset, ctx).await?;
    tx.commit()
        .await
        .map_err(|e| format!("commit card scan: {e}"))?;
    Ok(outcome)
}

/// Variante do caminho por checksum: os dados de entrada já foram lidos antes da transação.
pub(crate) async fn link_card_refunds_standalone(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    ctx: &CardScanCtx,
) -> Result<usize, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("begin refund link: {e}"))?;
    let linked = link_card_refunds(&mut tx, sheet_name, rows, ctx).await?;
    tx.commit()
        .await
        .map_err(|e| format!("commit refund link: {e}"))?;
    Ok(linked)
}

/// Liga a Entrada compensatória já derivada pela gramática de notas à sub-fatura correspondente.
/// A identificação do pai reutiliza `imported_row_ids`, a mesma origem dos IDs do import.
pub(crate) async fn link_card_refunds(
    tx: &mut sqlx::SqliteConnection,
    sheet_name: &str,
    rows: &[ImportedRow],
    ctx: &CardScanCtx,
) -> Result<usize, String> {
    let mut linked = 0;
    for (row, parent_id) in rows.iter().zip(imported_row_ids(sheet_name, rows)) {
        if row.raw_note.trim().is_empty() {
            continue;
        }
        let cycle_month = row.date.get(..7).ok_or("data de import inválida")?;
        let items = parse_itemized_note_opts(&row.raw_note, true);
        for tagged in parse_note_markers(&row.raw_note).tagged_lines {
            if !matches!(tagged.kind, NoteMarkerKind::Reembolso) {
                continue;
            }
            let Some(item) = items
                .iter()
                .find(|item| item.position == tagged.line_index && item.kind == ItemKind::Cartao)
            else {
                continue;
            };
            let alias =
                cards::normalize_alias(item.description.split('#').next().unwrap_or("").trim());
            let Some(account_id) = ctx.aliases.get(&alias) else {
                continue;
            };
            let invoice_id: Option<(String,)> =
                sqlx::query_as("SELECT id FROM invoice WHERE account_id = ?1 AND cycle_month = ?2")
                    .bind(account_id)
                    .bind(cycle_month)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| format!("load refund invoice: {e}"))?;
            let Some((invoice_id,)) = invoice_id else {
                continue;
            };
            let derived_id = format!("derived:reembolso:{parent_id}:{}", tagged.line_index);
            linked += sqlx::query("UPDATE \"transaction\" SET refund_invoice_id = ?1 WHERE id = ?2")
                .bind(&invoice_id)
                .bind(&derived_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("link refund: {e}"))?
                .rows_affected() as usize;
        }
    }

    let lexicon = cards::CardLexicon::from_entries(
        ctx.aliases
            .iter()
            .map(|(alias, account_id)| (alias.clone(), account_id.clone())),
    );
    for (row, transaction_id) in rows.iter().zip(imported_row_ids(sheet_name, rows)) {
        if row.kind != RowKind::Entrada {
            continue;
        }
        let markers = parse_note_markers(&row.raw_note);
        // O marcador já declarou a devolução da célula; a inferência não opina onde o dono já falou.
        if markers
            .tagged_lines
            .iter()
            .any(|tagged| matches!(tagged.kind, NoteMarkerKind::Reembolso))
        {
            continue;
        }
        // A célula itemizada é autoritativa sobre si mesma; sem itens, a descrição responde pela
        // célula inteira.
        let items = parse_itemized_note_opts(&row.raw_note, true);
        let named: Vec<(&str, i64)> = if items.is_empty() {
            vec![(row.description.as_str(), row.amount.abs())]
        } else {
            items
                .iter()
                .map(|item| (item.description.as_str(), item.amount_cents.abs()))
                .collect()
        };
        let mut hits = named
            .iter()
            .filter_map(|(description, cents)| lexicon.resolve(description).map(|id| (id, *cents)));
        let Some((account_id, refund_cents)) = hits.next() else {
            continue;
        };
        // Duas identidades de cartão na mesma Entrada não se desempatam sozinhas, e uma devolução
        // que responde por só parte da célula faria o vínculo mentir o valor: ele carrega o
        // lançamento inteiro, então creditaria à fatura dinheiro que não voltou. Os dois casos
        // ficam para o marcador explícito, que declara quanto e de quem.
        if hits.next().is_some() || refund_cents != row.amount.abs() {
            continue;
        }
        let invoice_id: Option<(String,)> =
            sqlx::query_as("SELECT id FROM invoice WHERE account_id = ?1 AND due_date = ?2")
                .bind(&account_id)
                .bind(&row.date)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("load inferred refund invoice: {e}"))?;
        let Some((invoice_id,)) = invoice_id else {
            continue;
        };
        // O alvo é único, e a escolha do dono é autoridade.
        linked += sqlx::query(
            "UPDATE \"transaction\" SET refund_invoice_id = ?1 \
             WHERE id = ?2 AND refund_invoice_id IS NULL AND refund_txn_id IS NULL \
               AND refund_series_id IS NULL AND refund_link_declined = 0",
        )
        .bind(&invoice_id)
        .bind(&transaction_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("link inferred refund: {e}"))?
        .rows_affected() as usize;
    }
    Ok(linked)
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

// API pública mantida como wrapper de pool para testes. O shell usa as variantes `*_in_tx` com
// transação externa única; como `google_sheets` é privado no crate, o wrapper exige
// `allow(dead_code)`.
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
        // MAS só quando as notas vieram de verdade neste ciclo: num ciclo degradado
        // (falha da API de notas / .xlsx sem notas) toda `raw_note` chega vazia — hashear o
        // vazio derrubava o guard de idempotência e disparava um re-import destrutivo.
        if descriptions_trusted {
            hasher.update(row.raw_note.as_bytes());
        }
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

/// IDs dos pais na mesma ordem e com os mesmos slots de `import_rows_core`. As linhas derivadas
/// precisam repetir essa identidade sem duplicar a fórmula de hash.
fn imported_row_ids(sheet_name: &str, rows: &[ImportedRow]) -> Vec<String> {
    let mut slots: HashMap<(String, &'static str), usize> = HashMap::new();
    rows.iter()
        .map(|row| {
            let slot = slots
                .entry((row.date.clone(), row.kind.as_str()))
                .and_modify(|slot| *slot += 1)
                .or_insert(0);
            row_id(sheet_name, &row.date, row.kind, *slot)
        })
        .collect()
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

/// Calcula o checksum de idempotência do batch da MESMA forma que `import_rows_with_options`,
/// para o shell (commands.rs) poder rodar `check_duplicate_import` ANTES de abrir a transação
/// externa (a checagem é uma leitura no pool e não pode acontecer dentro da tx — read-your-writes
/// daria falso-negativo).
pub(crate) fn compute_import_checksum(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    compute_checksum_with_options(rows, descriptions_trusted)
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
/// (sem entradas em `tagged_lines`), de modo que o parser não altera o lote importado.
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

/// Uma parte itemizada extraída de uma linha da nota de célula.
#[derive(Debug, PartialEq)]
pub(crate) struct NoteLineItem {
    /// Magnitude em centavos (positiva). Mesma convenção de `transaction.amount`.
    pub amount_cents: i64,
    pub description: String,
    /// Classificação derivada do cabeçalho de seção, sem fallback por descrição.
    pub kind: ItemKind,
    /// Posição 0-based na nota (ordem de aparição).
    pub position: usize,
    /// Cabeçalho de seção imediatamente anterior a este item na nota original
    /// (ex.: "CONTAS:", "CARTÕES:"). `None` quando o item não está sob um cabeçalho.
    pub section: Option<String>,
}

/// Bucket derivado de um item de nota. `Ajuste` é operacional
/// (reconciliação/diferença), não um bucket financeiro principal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    Saida,
    Cartao,
    Diario,
    Economia,
    Patrimonio,
    Ajuste,
}

/// Também usada pelo resolver de `obligation` (identidade de série confirmada pelo
/// usuário) para casar `line_item.section` contra `obligation.match_section` sem duplicar a
/// lógica de accent-fold/casefold.
pub(crate) fn normalize_item_section(section: &str) -> String {
    let section = section.trim().trim_end_matches(':').trim();
    let mut normalized = String::with_capacity(section.len());
    for ch in section.chars().flat_map(char::to_lowercase) {
        match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => normalized.push('a'),
            'é' | 'è' | 'ê' | 'ë' => normalized.push('e'),
            'í' | 'ì' | 'î' | 'ï' => normalized.push('i'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => normalized.push('o'),
            'ú' | 'ù' | 'û' | 'ü' => normalized.push('u'),
            'ç' => normalized.push('c'),
            other => normalized.push(other),
        }
    }
    normalized
}

/// Classifica um item pela seção imediatamente anterior. A descrição é
/// deliberadamente ignorada: não existe fallback por banco/emissor/palavra-chave.
pub(crate) fn classify_line_item(section: Option<&str>, _description: &str) -> ItemKind {
    let Some(section) = section else {
        return ItemKind::Saida;
    };
    match normalize_item_section(section).as_str() {
        "contas" | "outros" => ItemKind::Saida,
        "diario" => ItemKind::Diario,
        "cartao" | "cartoes" | "fatura" | "faturas" => ItemKind::Cartao,
        "investimento" => ItemKind::Patrimonio,
        "economia" => ItemKind::Economia,
        "ajuste" | "ajustes" => ItemKind::Ajuste,
        _ => ItemKind::Saida,
    }
}

/// Parseia as linhas itemizadas de uma nota de célula.
///
/// O estilo de anotação do usuário é a célula itemizada: um TOTAL que é a SOMA de
/// partes, cada parte descrita em uma linha da nota como `R$ <valor> - <descrição>`.
///
/// GRAMÁTICA: cada linha começando com `R$` (com ou sem espaço entre `R$` e o número)
/// é tratada como um item; o que vem antes do primeiro traço é o valor, o resto é a
/// descrição. Linhas que NÃO começam com `R$` (cabeçalhos, trailers `Total = …`,
/// linhas de orçamento separadas por tab) NÃO viram itens, mas a última linha não-`R$`
/// não-vazia vista é guardada como o `section` (cabeçalho) dos itens seguintes — ela é
/// reproduzida no write-back. Linhas em branco preservam o `section` atual.
///
/// Tolerâncias:
/// - `R$<número>` e `R$ <número>` (espaço opcional após `R$`)
/// - ` - ` e `-` (espaço opcional ao redor do traço)
/// - Valor em pt-BR (`1.234,56`) ou float do xlsx (`1234.5600`) — via `parse_number`
/// - Linha com marcador `#reembolso:`/`#dividir:` no fim: parseia o item normalmente
///   (o marcador fica na descrição). Os dois parsers são leituras INDEPENDENTES da
///   mesma nota; este não substitui nem altera `parse_note_markers`.
///
/// SEGURO POR PADRÃO: nota vazia ou sem linhas `R$` → lista vazia. Esta função só parseia;
/// a persistência é decidida pelo caller (`import_rows_core`): com breakdown reconhecido os
/// itens são persistidos MESMO com soma divergente da célula — a célula continua dona do
/// total, e o resíduo (célula − Σ partes) é reconciliado com sinal no loader de métricas,
/// enquanto o write-back cai para escrita RAW.
///
/// PURA — sem I/O, sem DB, sem panics.
pub(crate) fn parse_itemized_note(note: &str) -> Vec<NoteLineItem> {
    parse_itemized_note_opts(note, false)
}

/// Como [`parse_itemized_note`], mas com a semântica de PLACEHOLDER: em linhas projetadas
/// (meses futuros pré-lançados), um item `R$ 0,00 - <descrição>` é estrutura documentada do
/// futuro ("a preencher"), não ruído — persiste com `amount_cents = 0` para a UI mostrar o
/// esqueleto sem inventar valor. Só um zero GENUÍNO (o valor tem dígitos) conta; linha cujo
/// valor não parseia continua descartada. Em linhas realizadas o zero segue descartado
/// (ajuste/ruído de digitação).
pub(crate) fn parse_itemized_note_opts(
    note: &str,
    keep_zero_placeholders: bool,
) -> Vec<NoteLineItem> {
    let mut items = Vec::new();
    // Cabeçalho de seção mais recente (última linha não-`R$` não-vazia).
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
        // Zero genuíno = o valor tem dígitos e parseia 0 (ex.: "R$ 0,00"); distinto de lixo não
        // parseável, que também retorna 0 mas nunca vira placeholder.
        let genuine_zero = amount_cents == 0 && value_part.chars().any(|c| c.is_ascii_digit());
        let keep_as_placeholder = keep_zero_placeholders && genuine_zero;
        if amount_cents < 0 || (amount_cents == 0 && !keep_as_placeholder) {
            continue; // valor inválido, negativo, ou zero fora do caso placeholder → pula
        }
        let section = current_section.clone();
        items.push(NoteLineItem {
            amount_cents,
            description: desc_part.to_string(),
            kind: classify_line_item(section.as_deref(), desc_part),
            position: pos,
            section,
        });
    }
    items
}

// --- Diagnósticos de precisão do import (nota não itemizada / item↔célula divergente) ---
//
// Dois casos exigem diagnóstico: (1) uma nota que não casa com a gramática de
// `parse_itemized_note` não gera item; (2) a soma dos itens reconhecidos diverge do total da
// célula, que permanece dona do total. O diagnóstico torna visível onde a itemização está
// incompleta sem alterar a decisão de dados.

/// Diagnóstico de precisão de um import — reporta, não decide. `sheet`/`cell`/`detail` são só
/// apresentação; a persistência (célula dona do total, resíduo com sinal no loader de métricas)
/// é inteiramente a de antes.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ImportDiagnostic {
    pub sheet: String,
    /// Sem endereço real de célula na coleta (roda sobre o lote já parseado, não sobre a grade
    /// bruta linha/coluna) — rótulo sintético `"{date} ({kind})"`; colisões são aceitáveis, é só
    /// um rótulo de exibição, não uma chave.
    pub cell: String,
    pub kind: DiagKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum DiagKind {
    /// Nota não vazia que `parse_itemized_note` não reconheceu como itemização (0 itens). Um
    /// memo de 1 linha sem cabeçalho de seção é INTENCIONALMENTE não-breakdown e não gera este
    /// diagnóstico — ver o gate `has_breakdown` espelhado em `collect_import_diagnostics`.
    NoteNotItemized,
    /// Breakdown reconhecido (≥2 itens, ou 1 item sob seção) cuja soma diverge do total da
    /// célula. A célula continua dona do total; isto só reporta o resíduo que o loader de
    /// métricas (`forecast_cmds`) já reconcilia com sinal na leitura.
    ItemsDoNotSumToCell,
    /// Uma linha da coluna Saída se apresenta como fatura de cartão (`Fatura <nome>`) e nenhum
    /// cartão conhecido responde por esse nome. A linha continua sendo Saída fixa — classificar
    /// por palavra-chave transformaria "Fatura Vivo" em cartão —, mas o dinheiro deixa de sumir
    /// calado: ou é um cartão a cadastrar, ou é mesmo uma conta a pagar.
    UnrecognizedInvoiceLine,
    /// A nota é o formato recorrente "plano de gastos mensal" (`Mensal<TAB>R$…<TAB>categoria`
    /// repetido + `Total = R$…` + média diária `R$… / N Dias = R$…`) — não é itemização de
    /// transação nem um erro de digitação isolado, então não leva os rótulos genéricos acima.
    MonthlyBudgetPlanNote,
}

impl std::fmt::Display for DiagKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DiagKind::NoteNotItemized => "nota não itemizada",
            DiagKind::ItemsDoNotSumToCell => "itens não somam à célula",
            DiagKind::UnrecognizedInvoiceLine => "fatura de cartão não reconhecido",
            DiagKind::MonthlyBudgetPlanNote => "plano de gastos mensal",
        })
    }
}

/// Formata centavos como BRL pt-BR (`R$ 1.234,56`) só para o TEXTO do diagnóstico — apresentação,
/// não cálculo financeiro (a UI usa `<Money>` quando há um valor estruturado; aqui o dado já sai
/// como frase pronta do backend).
fn format_cents_brl(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let (reais, centavos) = (abs / 100, abs % 100);
    let digits = reais.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    let reais_str: String = grouped.chars().rev().collect();
    format!("{sign}R$ {reais_str},{centavos:02}")
}

/// Reconhece o formato recorrente "plano de gastos mensal": múltiplas linhas
/// `Mensal<TAB>R$ <valor><TAB><categoria>`, um total (`Total = R$ <valor>`) e uma média diária
/// (`R$ <valor> / <N> Dias = R$ <valor>`). Rotulá-la como `NoteNotItemized`/`ItemsDoNotSumToCell`
/// genérico faria uma nota recorrente e intencional parecer um erro de digitação isolado.
fn is_monthly_budget_plan_note(note: &str) -> bool {
    let lines: Vec<String> = note
        .lines()
        .map(|l| l.trim().to_ascii_lowercase())
        .collect();
    let has_mensal = lines.iter().any(|l| l.starts_with("mensal"));
    let has_total = lines
        .iter()
        .any(|l| l.starts_with("total") && l.contains("r$"));
    let has_dias = lines
        .iter()
        .any(|l| l.contains("dias") && l.contains('/') && l.contains('='));
    has_mensal && has_total && has_dias
}

/// Coleta os diagnósticos de precisão de um LOTE já parseado. PURA: só lê
/// `row.raw_note`/`row.amount`, nunca toca o banco. Por isto sobrevive ao skip de checksum
/// (dedup): o caller roda esta função sobre os MESMOS `rows` tanto quando o import escreve
/// quanto quando o detecta como duplicata idêntica — o diagnóstico é função do LOTE parseado,
/// não da escrita que aconteceu (ou não) nesta rodada.
///
/// Espelha exatamente o gate de itemização de `import_rows_core` (mesma gramática via
/// `parse_itemized_note`, mesmo `has_breakdown`, mesmo resíduo `célula − Σ|partes|`) para nunca
/// divergir do que de fato foi (ou seria) persistido.
/// Léxico do diagnóstico: as identidades que o banco já conhece mais as que ESTE lote declara sob
/// a seção de cartões. Incluir o próprio lote evita acusar a primeira importação de uma planilha
/// cujo cartão está declarado numa célula e escrito sem cabeçalho em outra.
fn diagnostics_card_lexicon(
    rows: &[ImportedRow],
    known_card_aliases: &[String],
) -> cards::CardLexicon<String> {
    let mut entries: Vec<(String, String)> = known_card_aliases
        .iter()
        .map(|alias| {
            let normalized = cards::normalize_alias(alias);
            (normalized.clone(), normalized)
        })
        .collect();
    for row in rows {
        for item in parse_itemized_note(&row.raw_note) {
            if item.kind != ItemKind::Cartao {
                continue;
            }
            let alias = cards::declared_alias(&item.description);
            if !alias.is_empty() {
                entries.push((alias.clone(), cards::root_alias(&alias)));
            }
        }
    }
    cards::CardLexicon::from_entries(entries)
}

pub(crate) fn collect_import_diagnostics(
    sheet_name: &str,
    rows: &[ImportedRow],
    descriptions_trusted: bool,
    known_card_aliases: &[String],
) -> Vec<ImportDiagnostic> {
    // Ciclo degradado (falha da API de notas / .xlsx sem notas legíveis): toda `raw_note` chega
    // vazia — nada de novo para reportar (mesmo gate de confiança do import_rows_core).
    if !descriptions_trusted {
        return Vec::new();
    }
    let lexicon = diagnostics_card_lexicon(rows, known_card_aliases);
    let mut diagnostics = Vec::new();
    for row in rows {
        let raw_note = row.raw_note.trim();
        if raw_note.is_empty() {
            continue;
        }
        let items = parse_itemized_note(&row.raw_note);
        let budget_plan = is_monthly_budget_plan_note(&row.raw_note);

        // Só a coluna Saída carrega fatura; na Entrada, "Fatura Gio" é o reembolso dela.
        if row.kind == RowKind::Saida {
            for item in &items {
                if item.kind == ItemKind::Cartao
                    || !cards::looks_like_invoice_line(&item.description)
                    || lexicon.resolve(&item.description).is_some()
                {
                    continue;
                }
                diagnostics.push(ImportDiagnostic {
                    sheet: sheet_name.to_string(),
                    cell: format!("{} ({})", row.date, DiagKind::UnrecognizedInvoiceLine),
                    kind: DiagKind::UnrecognizedInvoiceLine,
                    detail: format!(
                        "\"{}\" ({}) parece fatura, e nenhum cartão cadastrado ou proposto \
                         responde por esse nome — está contando como conta a pagar",
                        item.description.trim(),
                        format_cents_brl(item.amount_cents),
                    ),
                });
            }
        }

        if items.is_empty() {
            let kind = if budget_plan {
                DiagKind::MonthlyBudgetPlanNote
            } else {
                DiagKind::NoteNotItemized
            };
            diagnostics.push(ImportDiagnostic {
                sheet: sheet_name.to_string(),
                cell: format!("{} ({kind})", row.date),
                kind,
                detail: format!("Nota não reconhecida como itemização: \"{raw_note}\""),
            });
            continue;
        }

        // Mesmo gate de `import_rows_core`: memo de 1 linha sem seção não é breakdown — não é
        // silêncio indevido, é a regra de dados (persistir migraria Diário/Cartão p/ Saída).
        let has_breakdown = items.len() >= 2 || (items.len() == 1 && items[0].section.is_some());
        if !has_breakdown {
            continue;
        }

        let parts_sum: i64 = items.iter().map(|i| i.amount_cents.abs()).sum();
        let cell_total = row.amount.abs();
        let residual = cell_total - parts_sum;
        if residual != 0 {
            let kind = if budget_plan {
                DiagKind::MonthlyBudgetPlanNote
            } else {
                DiagKind::ItemsDoNotSumToCell
            };
            diagnostics.push(ImportDiagnostic {
                sheet: sheet_name.to_string(),
                cell: format!("{} ({kind})", row.date),
                kind,
                detail: format!(
                    "célula {} vs. itens {} (diferença {})",
                    format_cents_brl(cell_total),
                    format_cents_brl(parts_sum),
                    format_cents_brl(residual),
                ),
            });
        }
    }
    diagnostics
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
// Wrapper de pool mantido para testes; o shell usa `store_balance_series_in_tx`.
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

/// Corta a PRÉ-HISTÓRIA da série de Saldo: dias de saldo 0 anteriores à adoção da planilha.
/// Um template anual avalia a fórmula de Saldo como `0` em meses que nunca foram usados — um
/// leitor ingênuo veria "saldo zero por meses", não "antes da adoção". A fronteira de adoção é
/// o que vier PRIMEIRO entre o primeiro saldo ≠ 0 e a primeira transação importada da aba;
/// zeros de saldo a partir daí são dado real (dia zerado legítimo) e ficam. Aba-template só
/// com zeros e sem transação perde a série inteira (nada ali é dado).
pub(crate) fn trim_pre_history_balances(
    series: Vec<DailyBalance>,
    first_txn_date: Option<&str>,
) -> Vec<DailyBalance> {
    let first_nonzero = series
        .iter()
        .filter(|b| b.balance_cents != 0)
        .map(|b| b.date.as_str())
        .min();
    // Fronteira de adoção: o menor entre primeiro saldo ≠ 0 e primeira transação (ISO compara
    // lexicograficamente). Sem nenhum dos dois, não há adoção — tudo é pré-história.
    let adoption = match (first_nonzero, first_txn_date) {
        (Some(a), Some(b)) => Some(if a <= b { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    let Some(adoption) = adoption.map(str::to_owned) else {
        return Vec::new();
    };
    series
        .into_iter()
        .filter(|b| b.balance_cents != 0 || b.date.as_str() >= adoption.as_str())
        .collect()
}

/// Varre a grade de notas da coluna Diário atrás da cerimônia do teto documentada. A nota vive
/// tipicamente numa célula SEM valor (que nunca vira transação), por isso a varredura é direta
/// na grade, fora do fluxo de `ImportedRow`. Retorna `(YYYY-MM, nota crua)` da ocorrência mais
/// recente da aba; o caller decide entre abas.
pub(crate) fn scan_ceiling_ceremony_note(
    rows: &[Vec<String>],
    notes: &[Vec<String>],
    layout: &SheetLayout,
    daily_offset: usize,
) -> Option<(String, String)> {
    let year = layout.year?;
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;
    if month_row >= rows.len() {
        return None;
    }
    let month_blocks = month_blocks_for(&rows[month_row], block_size);

    let mut best: Option<(String, String)> = None;
    for (row_idx, row) in rows.iter().enumerate().skip(data_start) {
        let day_str = row.get(day_col).map_or("", |c| c.trim());
        let day: f64 = day_str.parse().unwrap_or(0.0);
        if !(1.0..=31.0).contains(&day) {
            continue;
        }
        for &(offset, month) in &month_blocks {
            let Some(note) = notes
                .get(row_idx)
                .and_then(|r| r.get(offset + daily_offset))
            else {
                continue;
            };
            if note.trim().is_empty() || super::ceiling_note::parse_ceiling_ceremony(note).is_none()
            {
                continue;
            }
            let ym = format!("{year:04}-{month:02}");
            if best.as_ref().is_none_or(|(b, _)| ym > *b) {
                best = Some((ym, note.clone()));
            }
        }
    }
    best
}

/// Upsert da proposta em transação PRÓPRIA — para os retornos antecipados do import (dedup de
/// checksum, aba sem linhas materializadas): a cerimônia vive em nota de célula sem valor, fora
/// do checksum de transações, então uma edição só na nota precisa ser vista mesmo quando o
/// dataset de linhas não mudou.
pub(crate) async fn upsert_ceiling_proposal_standalone(
    pool: &SqlitePool,
    source_month: &str,
    raw_note: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("ceiling proposal (begin): {e}"))?;
    upsert_ceiling_proposal_in_tx(&mut tx, source_month, raw_note).await?;
    tx.commit()
        .await
        .map_err(|e| format!("ceiling proposal (commit): {e}"))?;
    Ok(())
}

/// Registra a proposta de teto lida da cerimônia, com identidade pelo hash da nota
/// normalizada: a MESMA nota nunca re-propõe (pendente, aceita ou dispensada), e uma nota nova
/// supersede a pendente anterior (só existe uma proposta pendente por vez). Nunca escreve
/// `daily_budget` — aceitar é gesto explícito do dono, fora do import.
pub(crate) async fn upsert_ceiling_proposal_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    source_month: &str,
    raw_note: &str,
) -> Result<bool, String> {
    let Some(ceremony) = super::ceiling_note::parse_ceiling_ceremony(raw_note) else {
        return Ok(false);
    };
    // Hash da nota NORMALIZADA (linhas aparadas, espaços colapsados): reformatação cosmética da
    // mesma cerimônia não vira proposta nova.
    let normalized: String = raw_note
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut h = Sha256::new();
    h.update(normalized.as_bytes());
    let note_hash = hex::encode(h.finalize());

    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT status, source_month FROM ceiling_proposal WHERE note_hash = ?1")
            .bind(&note_hash)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| format!("ceiling proposal (lookup): {e}"))?;
    if let Some((status, stored_month)) = existing {
        // Mesma nota re-vista num mês mais recente: a procedência PENDENTE acompanha a
        // ocorrência mais nova (aceita/dispensada ficam congeladas como histórico).
        if status == "pending" && source_month > stored_month.as_str() {
            sqlx::query("UPDATE ceiling_proposal SET source_month = ?1 WHERE note_hash = ?2")
                .bind(source_month)
                .bind(&note_hash)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("ceiling proposal (refresh month): {e}"))?;
        }
        return Ok(false);
    }

    // Supersede só avança no tempo: uma aba antiga processada depois (ordem de abas do arquivo)
    // não pode apagar a proposta pendente de um mês mais recente.
    let pending_month: Option<(String,)> = sqlx::query_as(
        "SELECT source_month FROM ceiling_proposal WHERE status = 'pending' LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("ceiling proposal (pending month): {e}"))?;
    if let Some((month,)) = pending_month
        && month.as_str() > source_month
    {
        return Ok(false);
    }

    let items: Vec<serde_json::Value> = ceremony
        .items
        .iter()
        .map(|i| serde_json::json!({ "name": i.name, "amount_cents": i.amount_cents }))
        .collect();
    let items_json = serde_json::Value::Array(items).to_string();

    // Nota nova supersede a pendente anterior — o dono só vê uma proposta por vez.
    sqlx::query("DELETE FROM ceiling_proposal WHERE status = 'pending'")
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("ceiling proposal (supersede): {e}"))?;
    sqlx::query(
        "INSERT INTO ceiling_proposal \
         (id, note_hash, per_day_cents, divisor_days, items_json, source_month, status, raw_note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&note_hash)
    .bind(ceremony.per_day_cents)
    .bind(i64::from(ceremony.divisor_days))
    .bind(&items_json)
    .bind(source_month)
    // A nota vai CRUA (não a normalizada do hash): a citação na tela é reprodução, e a
    // formatação do dono — recuos, quebras, a grafia dos itens — faz parte da prova.
    .bind(raw_note)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("ceiling proposal (insert): {e}"))?;
    Ok(true)
}

/// Converte texto monetário em centavos. Regra fechada de separadores:
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
        // Economia. Para SOMENTE quando nenhuma coluna de bloco nomeia um mês (TOTAL/linha vazia/
        // próximo cabeçalho → `!any`). Não há atalho por dezembro: num layout assimétrico lado a
        // lado (ano anterior completo até dez, ano corrente parcial), um break ao ver o dez do ano
        // anterior truncaria as linhas restantes do ano corrente. `month_number_from_name` rejeita
        // "TOTAL"/"Totais"/números puros, então o `!any` para no fim de cada bloco com segurança.
        let mut rr = r + 1;
        while rr < rows.len() {
            let mut any = false;
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
            }
            if !any {
                break;
            }
            rr += 1;
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

    // Valores representativos nos dois locales e no xlsx.
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
    // com a DATA real (não um rótulo genérico "Entrada 2026").
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

    // --- Geometria real (JANEIRO no offset 0, 12 blocos, célula espúria) ---

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

    // Uma anotação com nome de mês depois do bloco real ("MAIO 2026" solto) não pode criar bloco
    // fantasma nem fazer o import ler colunas erradas.
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

    // A aba Economia aceita blocos anuais lado a lado e precisa importar todos.
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
    fn parse_economia_sheet_asymmetric_blocks_no_premature_break() {
        // Um bloco anual completo pode ficar ao lado de outro parcial. Encontrar dezembro em um
        // bloco não encerra os demais; somente `!any` (linha sem mês válido) encerra a leitura.
        //
        // Layout: ano anterior em col B (idx 1) / Economia col D (idx 3);
        //         ano corrente em col F (idx 5) / Economia col H (idx 7).
        let header = vec![
            "".to_string(),
            "2025".to_string(),
            "Entradas".to_string(),
            "Economia".to_string(),
            "".to_string(),
            "2026".to_string(),
            "Entradas".to_string(),
            "Economia".to_string(),
        ];
        let month_names = [
            "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
        ];
        let mut rows = vec![header];
        for (i, &name) in month_names.iter().enumerate() {
            // Ano corrente: meses 1–8 têm valor; 9–12 em branco (parse_number("") == 0).
            let eco_current = if i < 8 {
                format!("{}.00", (i + 1) * 1000)
            } else {
                String::new()
            };
            rows.push(vec![
                "".to_string(),
                name.to_string(),
                "5000.00".to_string(),
                format!("{}.00", (i + 1) * 500), // ano anterior: todos os 12 meses
                "".to_string(),
                name.to_string(),
                "8000.00".to_string(),
                eco_current,
            ]);
        }

        let got = parse_economia_sheet(&rows);

        // Ano anterior deve ter todos os 12 meses (sem break prematuro).
        let prior: Vec<_> = got
            .iter()
            .filter(|&&(y, _, _)| y == 2025)
            .copied()
            .collect();
        assert_eq!(prior.len(), 12, "ano anterior deve ter todos os 12 meses");

        // Ano corrente deve ter todos os 12 meses (9–12 em branco → 0 centavos, mas presentes).
        let current: Vec<_> = got
            .iter()
            .filter(|&&(y, _, _)| y == 2026)
            .copied()
            .collect();
        assert_eq!(
            current.len(),
            12,
            "ano corrente deve ter os 12 meses mesmo com linhas finais em branco"
        );

        // Spot-check: dezembro do ano anterior presente e correto.
        assert_eq!(
            prior.iter().find(|&&(_, mo, _)| mo == 12).unwrap().2,
            600_000, // 12 * 500 = 6000 (R$) → parse_number("6000.00") = 600_000 centavos
            "dezembro do ano anterior presente e correto"
        );
        // Spot-check: setembro do ano corrente (em branco na planilha) é 0, não ausente.
        assert_eq!(
            current.iter().find(|&&(_, mo, _)| mo == 9).unwrap().2,
            0,
            "setembro do ano corrente (em branco) é 0 centavos, não faltante"
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

    // --- Reimport idempotente e atômico por aba ---

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

    // Linha importada com nota de célula crua e flag de projeção (passado/futuro).
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
    // Gramática das notas (parse puro, sem DB)
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

    // --- Parser puro parse_itemized_note (sem I/O) ---

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

    // `parse_itemized_note` captura o cabeçalho de seção das linhas não-`R$`.
    #[test]
    fn itemized_captures_section_header() {
        let note = "CONTAS:\nR$ 100,00 - Item A\nR$ 50,00 - Item B";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
        assert_eq!(items[1].section.as_deref(), Some("CONTAS:"));
    }

    // Duas seções separadas por linha em branco → cada item recebe seu cabeçalho.
    #[test]
    fn itemized_two_sections_assign_correct_header() {
        let note = "CONTAS:\nR$ 100,00 - Item A\n\nCARTÕES:\nR$ 200,00 - Item B";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
        assert_eq!(items[1].section.as_deref(), Some("CARTÕES:"));
    }

    // Item sem cabeçalho anterior → section = None.
    #[test]
    fn itemized_no_header_yields_none_section() {
        let note = "R$ 150,00 - Item sem cabeçalho";
        let items = parse_itemized_note(note);
        assert_eq!(items.len(), 1);
        assert!(items[0].section.is_none());
    }

    // --- Diagnósticos de precisão (collect_import_diagnostics), sem I/O ---

    #[test]
    fn format_cents_brl_formats_pt_br() {
        assert_eq!(format_cents_brl(123_456), "R$ 1.234,56");
        assert_eq!(format_cents_brl(500), "R$ 5,00");
        assert_eq!(format_cents_brl(-500), "-R$ 5,00");
        assert_eq!(format_cents_brl(0), "R$ 0,00");
    }

    #[test]
    fn is_monthly_budget_plan_note_requires_all_three_markers() {
        assert!(is_monthly_budget_plan_note(
            "Mensal\tR$ 300,00\tContas\nTotal = R$ 300,00\nR$ 300,00 / 30 Dias = R$ 10,00"
        ));
        // Só o cabeçalho "Mensal" (sem Total/Dias) não é o formato completo — é o caso já
        // coberto por `itemized_skips_tab_separated_budget_lines` (item comum na sequência).
        assert!(!is_monthly_budget_plan_note("Mensal\tR$ 300,00\tContas"));
        assert!(!is_monthly_budget_plan_note("R$ 100,00 - Item A"));
    }

    // Nota de prosa sem nenhuma linha `R$` → 0 itens (parse_itemized_note) + 1 diagnóstico
    // NoteNotItemized. A DECISÃO de dados (nenhum item persistido) não muda; isto só reporta.
    #[test]
    fn diagnostics_flag_prose_only_note_as_not_itemized() {
        let rows = vec![imported_note(
            "2026-03-01",
            -5_000,
            "Compra qualquer, sem valor detalhado na nota",
            false,
        )];
        assert!(parse_itemized_note(&rows[0].raw_note).is_empty());

        let diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagKind::NoteNotItemized);
        assert_eq!(diagnostics[0].sheet, "2026");
    }

    // Uma linha que se apresenta como fatura e não casa nenhum cartão conhecido é dinheiro que o
    // app está lendo como conta a pagar. Não pode ser classificada por adivinhação — mas também
    // não pode sumir calada: vira diagnóstico, que reporta sem decidir.
    #[test]
    fn diagnostics_flag_an_invoice_line_no_card_recognizes() {
        let rows = vec![imported_note(
            "2026-02-23",
            -990,
            "R$ 9,90 - Fatura Sicoob",
            false,
        )];
        let diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagKind::UnrecognizedInvoiceLine);
        assert!(
            diagnostics[0].detail.contains("Fatura Sicoob"),
            "o diagnóstico nomeia a linha: {}",
            diagnostics[0].detail
        );
    }

    // A contrapartida: reconhecida a identidade, não há o que reportar — nem quando o léxico vem
    // do banco (cartão cadastrado, proposta pendente) nem quando vem da própria planilha.
    #[test]
    fn diagnostics_stay_quiet_for_an_invoice_of_a_declared_card() {
        let from_db = vec![imported_note(
            "2026-01-12",
            -5_000,
            "R$ 50,00 - Fatura Bradesco",
            false,
        )];
        assert!(
            collect_import_diagnostics("2026", &from_db, true, &["bradesco".to_string()])
                .is_empty()
        );

        let from_sheet = vec![
            imported_note("2026-08-12", -5_000, "CARTÕES:\nR$ 50,00 - Bradesco", false),
            imported_note("2026-01-12", -5_000, "R$ 50,00 - Fatura Bradesco", false),
        ];
        assert!(
            collect_import_diagnostics("2026", &from_sheet, true, &[]).is_empty(),
            "o mesmo lote declara o cartão — a linha sem cabeçalho o alcança"
        );
    }

    // Memo de 1 linha SEM seção é intencionalmente não-breakdown (mesmo gate de
    // `import_rows_core`) — não deve gerar diagnóstico nenhum, mesmo tendo 1 item parseável.
    #[test]
    fn diagnostics_skip_single_memo_without_section_intentionally() {
        let rows = vec![imported_note(
            "2026-03-04",
            -5_000,
            "R$ 50,00 - Mercado",
            false,
        )];
        assert_eq!(parse_itemized_note(&rows[0].raw_note).len(), 1);
        assert!(collect_import_diagnostics("2026", &rows, true, &[]).is_empty());
    }

    // Nota limpa (itens somam o total) → zero diagnósticos.
    #[test]
    fn diagnostics_empty_for_clean_note() {
        let rows = vec![imported_note(
            "2026-03-02",
            -15_000,
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
            false,
        )];
        assert!(collect_import_diagnostics("2026", &rows, true, &[]).is_empty());
    }

    // Formato recorrente "plano de gastos mensal" (não itemiza) → MonthlyBudgetPlanNote, NÃO o
    // NoteNotItemized genérico (não é um erro de digitação isolado).
    #[test]
    fn diagnostics_label_monthly_budget_plan_note_distinctly() {
        let note = "Mensal\tR$ 300,00\tContas\n\
                     Mensal\tR$ 150,00\tLazer\n\
                     Mensal\tR$ 400,00\tMercado\n\
                     Mensal\tR$ 200,00\tTransporte\n\
                     Mensal\tR$ 100,00\tOutros\n\
                     Total = R$ 1.150,00\n\
                     R$ 1.150,00 / 30 Dias = R$ 38,33";
        assert!(
            parse_itemized_note(note).is_empty(),
            "nenhuma linha casa a gramática de item"
        );
        let rows = vec![imported_note("2026-03-03", -115_000, note, false)];
        let diagnostics = collect_import_diagnostics("2026", &rows, true, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagKind::MonthlyBudgetPlanNote);
    }

    // Ciclo degradado (raw_note vazia / notas não confiáveis): nunca reporta — mesmo gate de
    // confiança do `import_rows_core` (uma falha transitória da API de notas não deve gerar
    // diagnóstico algum, já que não há nota real para avaliar).
    #[test]
    fn diagnostics_empty_when_descriptions_not_trusted() {
        let rows = vec![imported_note(
            "2026-03-05",
            -5_000,
            "prosa qualquer sem R$",
            false,
        )];
        assert!(collect_import_diagnostics("2026", &rows, false, &[]).is_empty());
    }

    // Classificação pura de itens por seção, sem I/O.
    #[test]
    fn classify_line_item_maps_known_sections_to_kinds() {
        assert_eq!(
            classify_line_item(Some("CONTAS:"), "Aluguel"),
            ItemKind::Saida
        );
        assert_eq!(classify_line_item(Some("OUTROS"), "Taxa"), ItemKind::Saida);
        assert_eq!(
            classify_line_item(Some("DIÁRIO:"), "Mercado"),
            ItemKind::Diario
        );
        assert_eq!(
            classify_line_item(Some("DIARIO"), "Mercado"),
            ItemKind::Diario
        );
        assert_eq!(
            classify_line_item(Some("CARTÕES:"), "Compra parcelada"),
            ItemKind::Cartao
        );
        assert_eq!(
            classify_line_item(Some("CARTOES"), "Compra parcelada"),
            ItemKind::Cartao
        );
        assert_eq!(
            classify_line_item(Some("FATURAS:"), "Fatura mensal"),
            ItemKind::Cartao
        );
        assert_eq!(
            classify_line_item(Some("Fatura:"), "Fatura mensal"),
            ItemKind::Cartao
        );
        assert_eq!(
            classify_line_item(Some("Investimento:"), "Previdencia"),
            ItemKind::Patrimonio
        );
        assert_eq!(
            classify_line_item(Some("ECONOMIA"), "Reserva"),
            ItemKind::Economia
        );
        assert_eq!(
            classify_line_item(Some("AJUSTES"), "Diferenca"),
            ItemKind::Ajuste
        );
    }

    #[test]
    fn classify_line_item_defaults_unknown_or_missing_section_to_saida() {
        assert_eq!(classify_line_item(None, "Sem secao"), ItemKind::Saida);
        assert_eq!(
            classify_line_item(Some("Juros"), "Taxa avulsa"),
            ItemKind::Saida
        );
    }

    #[test]
    fn classify_line_item_has_no_bank_name_fallback() {
        assert_eq!(
            classify_line_item(None, "Banco Exemplo - compra no cartao"),
            ItemKind::Saida
        );
        assert_eq!(
            classify_line_item(Some("OUTROS"), "Fatura Banco Exemplo"),
            ItemKind::Saida
        );
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

    // --- Pré-história: zeros de template anteriores à adoção da planilha ---

    fn bal(date: &str, cents: i64) -> DailyBalance {
        DailyBalance {
            date: date.into(),
            balance_cents: cents,
            is_projection: false,
        }
    }

    // Meses "mortos" do template (saldo 0 avaliado pela fórmula) antes da adoção caem; o
    // primeiro saldo real e tudo dali em diante fica — inclusive um zero legítimo posterior.
    #[test]
    fn trim_pre_history_drops_leading_template_zeros() {
        let series = vec![
            bal("2025-01-15", 0),
            bal("2025-03-10", 0),
            bal("2025-07-06", 364_064),
            bal("2025-08-01", 0), // dia zerado APÓS a adoção: dado real, fica
        ];
        let out = trim_pre_history_balances(series, Some("2025-07-06"));
        let dates: Vec<&str> = out.iter().map(|b| b.date.as_str()).collect();
        assert_eq!(dates, vec!["2025-07-06", "2025-08-01"]);
    }

    // Aba-template pura (só zeros, nenhuma transação): nada ali é dado.
    #[test]
    fn trim_pre_history_template_only_drops_everything() {
        let series = vec![bal("2027-01-01", 0), bal("2027-06-30", 0)];
        assert!(trim_pre_history_balances(series, None).is_empty());
    }

    // A primeira TRANSAÇÃO também abre a adoção: um zero de saldo no dia de movimento real
    // (entrou e saiu o mesmo valor) fica, mesmo antes do primeiro saldo ≠ 0.
    #[test]
    fn trim_pre_history_transaction_opens_adoption_before_first_nonzero_balance() {
        let series = vec![
            bal("2025-05-01", 0), // template, antes de tudo
            bal("2025-06-10", 0), // dia com movimento real que zera o saldo
            bal("2025-07-01", 100_000),
        ];
        let out = trim_pre_history_balances(series, Some("2025-06-10"));
        let dates: Vec<&str> = out.iter().map(|b| b.date.as_str()).collect();
        assert_eq!(dates, vec!["2025-06-10", "2025-07-01"]);
    }

    // --- Placeholder: item R$ 0,00 em nota de linha projetada ---

    // Zero genuíno vira placeholder SÓ quando pedido (linha projetada); lixo não parseável
    // nunca vira item, em nenhum modo.
    #[test]
    fn parse_itemized_note_zero_placeholder_only_on_request_and_genuine() {
        let note = "CARTÕES:\nR$ 0,00 - Banco A\nR$ 150,00 - Banco B\nR$ abc - lixo";
        let strict = parse_itemized_note(note);
        assert_eq!(strict.len(), 1);
        assert_eq!(strict[0].amount_cents, 15_000);

        let with_placeholders = parse_itemized_note_opts(note, true);
        assert_eq!(with_placeholders.len(), 2);
        assert_eq!(with_placeholders[0].amount_cents, 0);
        assert_eq!(with_placeholders[0].description, "Banco A");
        assert_eq!(with_placeholders[1].amount_cents, 15_000);
    }

    // Ponta a ponta: linha projetada persiste o placeholder; linha realizada com a MESMA nota
    // não persiste o zero.
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

    // --- Varredura da cerimônia do teto na grade de notas ---

    // A nota da cerimônia vive numa célula do Diário SEM valor (que nunca vira ImportedRow);
    // a varredura acha a nota mesmo assim, e entre dois meses anotados o mais recente vence.
    #[test]
    fn scan_ceiling_ceremony_finds_valueless_cell_and_prefers_recent_month() {
        let ceremony_old = "R$ 900,00 / 30 Dias = R$ 30,00";
        let ceremony_new = "R$ 1250,00 / 31 Dias = R$ 40,33";
        // Blocos JANEIRO (offset 0) e FEVEREIRO (offset 6); Diário = offset 3 do bloco.
        let rows: Vec<Vec<String>> = vec![
            vec![
                "JANEIRO".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "FEVEREIRO".into(),
                "".into(),
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
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
            ],
        ];
        let mut notes: Vec<Vec<String>> = vec![vec!["".into(); 12]; 3];
        notes[2][3] = ceremony_old.into(); // JANEIRO, Diário do dia 1 (célula sem valor)
        notes[2][9] = ceremony_new.into(); // FEVEREIRO, Diário do dia 1
        let layout = SheetLayout {
            id: "t".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "both".into(),
        };
        let found = scan_ceiling_ceremony_note(&rows, &notes, &layout, 3).unwrap();
        assert_eq!(found.0, "2026-02");
        assert_eq!(found.1, ceremony_new);

        // Nota que não é cerimônia (itemização comum) não conta.
        let mut only_noise = vec![vec!["".into(); 12]; 3];
        only_noise[2][3] = "CONTAS:\nR$ 100,00 - Energia".into();
        assert!(scan_ceiling_ceremony_note(&rows, &only_noise, &layout, 3).is_none());
    }

    // --- Proposta de teto: identidade por hash, supersede de pendente ---

    #[tokio::test]
    async fn ceiling_proposal_upsert_is_idempotent_and_supersedes_pending() {
        let pool = test_pool().await;
        let note_a = "R$ 900,00 / 30 Dias = R$ 30,00";
        let note_b =
            "Mensal R$ 1.250,00 Mercado\nTotal = R$ 1.250,00\nR$ 1.250,00 / 31 Dias = R$ 40,33";

        let mut tx = pool.begin().await.unwrap();
        assert!(
            upsert_ceiling_proposal_in_tx(&mut tx, "2026-01", note_a)
                .await
                .unwrap()
        );
        // Mesma nota (com espaçamento diferente) → mesma identidade, não re-propõe.
        assert!(
            !upsert_ceiling_proposal_in_tx(&mut tx, "2026-02", "R$  900,00 / 30 Dias =  R$ 30,00")
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();

        let (count, status): (i64, String) =
            sqlx::query_as("SELECT COUNT(*), MAX(status) FROM ceiling_proposal")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((count, status.as_str()), (1, "pending"));

        // Dispensar e reimportar a MESMA nota: não volta a propor.
        sqlx::query("UPDATE ceiling_proposal SET status='dismissed'")
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert!(
            !upsert_ceiling_proposal_in_tx(&mut tx, "2026-03", note_a)
                .await
                .unwrap()
        );
        // Nota NOVA propõe de novo (e itens/divisor persistem).
        assert!(
            upsert_ceiling_proposal_in_tx(&mut tx, "2026-03", note_b)
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();

        let (per_day, divisor, items_json, month): (i64, i64, String, String) = sqlx::query_as(
            "SELECT per_day_cents, divisor_days, items_json, source_month FROM ceiling_proposal WHERE status='pending'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(per_day, 4_033);
        assert_eq!(divisor, 31);
        assert_eq!(month, "2026-03");
        assert!(items_json.contains("Mercado"));

        // A dispensada segue registrada (histórico de identidade), sem virar pendente.
        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ceiling_proposal")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 2);
    }

    // Supersede só avança no tempo: aba antiga processada depois não apaga a proposta pendente
    // de um mês mais recente; a MESMA nota re-vista num mês mais novo só atualiza a procedência.
    #[tokio::test]
    async fn ceiling_proposal_older_month_never_supersedes_newer_pending() {
        let pool = test_pool().await;
        let newer = "R$ 1250,00 / 31 Dias = R$ 40,33";
        let older = "R$ 900,00 / 30 Dias = R$ 30,00";

        let mut tx = pool.begin().await.unwrap();
        assert!(
            upsert_ceiling_proposal_in_tx(&mut tx, "2026-05", newer)
                .await
                .unwrap()
        );
        // Aba de um ano anterior chega depois: não supersede.
        assert!(
            !upsert_ceiling_proposal_in_tx(&mut tx, "2025-09", older)
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();
        let (month, per_day): (String, i64) = sqlx::query_as(
            "SELECT source_month, per_day_cents FROM ceiling_proposal WHERE status='pending'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((month.as_str(), per_day), ("2026-05", 4_033));

        // A mesma nota pendente re-vista num mês mais novo atualiza só a procedência.
        let mut tx = pool.begin().await.unwrap();
        assert!(
            !upsert_ceiling_proposal_in_tx(&mut tx, "2026-07", newer)
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();
        let (month,): (String,) =
            sqlx::query_as("SELECT source_month FROM ceiling_proposal WHERE status='pending'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(month, "2026-07");
    }

    #[tokio::test]
    async fn card_scan_creates_invoice_from_card_note() {
        let pool = test_pool().await;
        let account_id = crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Visa principal",
            None,
            Some(20),
            Some(10),
            None,
            None,
            None,
            &["visa".into()],
        )
        .await
        .unwrap();
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![
            vec!["JANEIRO".into(), "".into(), "".into()],
            vec![],
            vec!["1".into(), "".into(), "850,00".into()],
        ];
        let notes = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 850,00 - Visa".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };

        let mut tx = pool.begin().await.unwrap();
        let outcome = scan_card_invoices(&mut tx, &rows, &notes, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(outcome.invoices_created, 1);
        let invoice: (String, String, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT account_id, due_date, stated_total_cents, source_stated_total_cents FROM invoice",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            invoice,
            (account_id, "2026-01-01".into(), Some(85_000), Some(85_000))
        );
    }

    #[tokio::test]
    async fn card_scan_reconciles_an_imported_card_line_removed_from_the_same_due_cell() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Mastercard",
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = |card: &str| {
            vec![
                vec!["".into(); 3],
                vec![],
                vec![
                    "".into(),
                    "".into(),
                    format!("CARTÕES:\nR$ 100,00 - {card}"),
                ],
            ]
        };

        let mut tx = pool.begin().await.unwrap();
        let first = scan_card_invoices(&mut tx, &values, &notes("Visa"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(first.invoices_created, 1);

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes("Mastercard"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let invoices: Vec<(String, Option<i64>)> = sqlx::query_as(
            "SELECT a.name, i.stated_total_cents FROM invoice i \
             JOIN account a ON a.id = i.account_id ORDER BY a.name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(invoices, vec![("Mastercard".into(), Some(10_000))]);

        let due = format!("{year}-08-10");
        let txns = crate::commands::write_back_cmds::load_write_back_txns(&pool, year)
            .await
            .unwrap();
        let candidates: Vec<_> = txns
            .iter()
            .filter(|txn| txn.date == due && txn.kind == RowKind::Saida)
            .collect();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].amount_cents, 10_000,
            "Visa não ressuscita no write-back"
        );

        let (visa_still_present,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invoice WHERE account_id = ?1")
                .bind(visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(visa_still_present, 0);
    }

    #[tokio::test]
    async fn card_scan_removes_invoice_when_its_last_card_line_is_removed_but_keeps_real_purchase()
    {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 100,00 - Visa".into()],
        ];
        let no_cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CONTAS:\nR$ 100,00 - Aluguel".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let first_invoice: String =
            sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
                .bind(&visa)
                .fetch_one(&pool)
                .await
                .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &no_cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let removed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE id = ?1")
            .bind(&first_invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            removed, 0,
            "sem seção de cartões, a fatura importada é removida"
        );

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let retained_invoice: String =
            sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
                .bind(&visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('visa-real-purchase', 'expense', 2_000, ?1, 'credit', 0, 0, ?2)",
        )
        .bind(format!("{year}-07-20"))
        .bind(&retained_invoice)
        .execute(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &no_cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE id = ?1")
            .bind(&retained_invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(kept, 1, "compra realizada vinculada preserva a fatura");
    }

    /// A reconciliação apaga a fatura import-origin órfã, mas até aqui não limpava o
    /// `import_conflict` associado (`invoice:<id>/stated_total`). O conflito sobrevivendo à fatura
    /// travava `unresolved_conflict_count` acima de zero sem nenhum alvo resolvível na UI.
    #[tokio::test]
    async fn card_scan_reconciliation_clears_the_orphan_import_conflict_of_a_deleted_invoice() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 100,00 - Visa".into()],
        ];
        let no_cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CONTAS:\nR$ 100,00 - Aluguel".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let invoice_id: String = sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
            .bind(&visa)
            .fetch_one(&pool)
            .await
            .unwrap();

        // Conflito órfão: sobrevive de uma rodada de import anterior à que vai apagar a fatura.
        sqlx::query(
            "INSERT INTO import_conflict \
             (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
             VALUES ('conf:orphan', ?1, 'stated_total', '10000', '10000', '20000', datetime('now'))",
        )
        .bind(format!("invoice:{invoice_id}"))
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &no_cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let invoice_gone: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE id = ?1")
            .bind(&invoice_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(invoice_gone, 0, "sem seção de cartões, a fatura é removida");
        assert_eq!(
            crate::commands::write_back_cmds::unresolved_conflict_count(&pool)
                .await
                .unwrap(),
            0,
            "o conflito órfão da fatura apagada não pode travar o gate do write-back para sempre"
        );
    }

    #[tokio::test]
    async fn card_scan_keeps_a_removed_imported_card_invoice_that_has_a_real_purchase() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Mastercard",
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = |card: &str| {
            vec![
                vec!["".into(); 3],
                vec![],
                vec![
                    "".into(),
                    "".into(),
                    format!("CARTÕES:\nR$ 100,00 - {card}"),
                ],
            ]
        };
        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes("Visa"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let visa_invoice: String =
            sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
                .bind(&visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('visa-purchase', 'expense', 2_000, ?1, 'credit', 0, 0, ?2)",
        )
        .bind(format!("{year}-07-20"))
        .bind(visa_invoice)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes("Mastercard"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let (visa_still_present,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invoice WHERE account_id = ?1")
                .bind(visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            visa_still_present, 1,
            "uma compra real impede apagar a fatura"
        );
    }

    /// "Sem transação vinculada" tinha só a metade da checagem — só `invoice_id` (compras). Uma
    /// sub-fatura importada sem compras mas com uma expectativa MANUAL de reembolso
    /// (`refund_invoice_id`) tem que sobreviver: a Entrada não é derivada do import, é um vínculo
    /// do app; apagar a fatura anularia essa FK sem o dono pedir.
    #[tokio::test]
    async fn card_scan_keeps_a_removed_imported_card_invoice_that_has_a_linked_refund() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Mastercard",
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = |card: &str| {
            vec![
                vec!["".into(); 3],
                vec![],
                vec![
                    "".into(),
                    "".into(),
                    format!("CARTÕES:\nR$ 100,00 - {card}"),
                ],
            ]
        };
        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes("Visa"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let visa_invoice: String =
            sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
                .bind(&visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, is_fixed, is_projection, refund_invoice_id) \
             VALUES ('visa-refund', 'income', 5_000, ?1, 0, 1, ?2)",
        )
        .bind(format!("{year}-07-25"))
        .bind(&visa_invoice)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes("Mastercard"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let (visa_still_present,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invoice WHERE account_id = ?1")
                .bind(visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            visa_still_present, 1,
            "um reembolso vinculado impede apagar a fatura"
        );
    }

    #[tokio::test]
    async fn card_scan_keeps_a_removed_imported_card_invoice_with_projected_series_purchase() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 100,00 - Visa".into()],
        ];
        let no_cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CONTAS:\nR$ 100,00 - Aluguel".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let invoice: String = sqlx::query_scalar("SELECT id FROM invoice WHERE account_id = ?1")
            .bind(&visa)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('projected-series-occurrence', 'expense', 2_000, ?1, 'credit', 0, 1, ?2)",
        )
        .bind(format!("{year}-07-20"))
        .bind(&invoice)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &no_cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let retained: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE id = ?1")
            .bind(invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            retained, 1,
            "uma ocorrência projetada também preserva a fatura"
        );
    }

    /// A planilha declara a fatura de duas maneiras — sob o cabeçalho `CARTÕES` e, quando o
    /// cabeçalho falta, como uma linha que nomeia o cartão. A varredura precisa enxergar as duas,
    /// senão um mês inteiro de faturas nunca materializa e o dinheiro fica em Saída fixa.
    #[tokio::test]
    async fn card_scan_reads_an_invoice_line_written_outside_the_cards_section() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        // Sem cabeçalho de seção, como a nota de um mês em que o dono esqueceu de escrevê-lo.
        let sectionless = vec![
            vec!["".into(); 3],
            vec![],
            vec![
                "".into(),
                "".into(),
                "R$ 60,00 - Aluguel\nR$ 100,00 - Fatura Visa".into(),
            ],
        ];

        let mut tx = pool.begin().await.unwrap();
        let outcome = scan_card_invoices(&mut tx, &values, &sectionless, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(outcome.invoices_created, 1);
        let stated: Option<i64> =
            sqlx::query_scalar("SELECT stated_total_cents FROM invoice WHERE account_id = ?1")
                .bind(&visa)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            stated,
            Some(10_000),
            "a fatura é a da linha, não do aluguel"
        );
    }

    /// Sem cartão cadastrado, o léxico nasce da própria planilha: um alias declarado sob a seção
    /// em QUALQUER célula reconhece a linha sem cabeçalho de outra célula — e as duas formas do
    /// mesmo cartão continuam sendo uma identidade só, nunca duas propostas.
    #[tokio::test]
    async fn card_scan_lexicon_comes_from_the_sheet_when_no_card_is_registered() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
            vec!["11".into(), "".into(), "50,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = vec![
            vec!["".into(); 3],
            vec![],
            vec![
                "".into(),
                "".into(),
                "CARTÕES:\nR$ 100,00 - Nubank (26/09)".into(),
            ],
            vec!["".into(), "".into(), "R$ 50,00 - Fatura Nubank".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let proposals: Vec<(String,)> = sqlx::query_as("SELECT alias FROM card_proposal")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            proposals.len(),
            1,
            "as duas linhas nomeiam o mesmo cartão: uma proposta, não duas ({proposals:?})"
        );
    }

    /// A planilha marca o ciclo dentro do nome ("Nubank (26/09)"). Isso é anotação humana, não
    /// identidade: sem reduzir à raiz, cada mês propunha um cartão diferente para cadastrar.
    #[tokio::test]
    async fn card_scan_groups_cycle_variants_of_the_same_card_into_one_proposal() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
            vec!["11".into(), "".into(), "50,00".into()],
            vec!["12".into(), "".into(), "70,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = vec![
            vec!["".into(); 3],
            vec![],
            vec![
                "".into(),
                "".into(),
                "CARTÕES:\nR$ 100,00 - Nubank (26/09)".into(),
            ],
            vec![
                "".into(),
                "".into(),
                "CARTÕES:\nR$ 50,00 - Nubank (26/12)".into(),
            ],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 70,00 - Nubank".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let proposals: Vec<(String, String)> =
            sqlx::query_as("SELECT alias, display_name FROM card_proposal")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            proposals.len(),
            1,
            "três anotações, um cartão: {proposals:?}"
        );
        assert_eq!(proposals[0].0, "nubank");
        assert_eq!(proposals[0].1, "Nubank", "o rótulo perde o sufixo de ciclo");

        let aliases: Vec<(String,)> =
            sqlx::query_as("SELECT alias FROM card_proposal_alias ORDER BY alias")
                .fetch_all(&pool)
                .await
                .unwrap();
        let aliases: Vec<String> = aliases.into_iter().map(|(a,)| a).collect();
        assert_eq!(
            aliases,
            vec!["nubank", "nubank (26/09)", "nubank (26/12)"],
            "todas as grafias acompanham a proposta"
        );
    }

    #[tokio::test]
    async fn card_scan_keeps_a_removed_imported_card_invoice_with_local_total_adjustment() {
        let pool = test_pool().await;
        let year = chrono::Local::now().year() + 1;
        let visa = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let values = vec![
            vec!["AGOSTO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: year.to_string(),
            year: Some(year),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CARTÕES:\nR$ 100,00 - Visa".into()],
        ];
        let no_cards_note = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), "CONTAS:\nR$ 100,00 - Aluguel".into()],
        ];

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("UPDATE invoice SET stated_total_cents = 15_000 WHERE account_id = ?1")
            .bind(&visa)
            .execute(&pool)
            .await
            .unwrap();

        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &no_cards_note, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let preserved: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT stated_total_cents, source_stated_total_cents FROM invoice WHERE account_id = ?1",
        )
        .bind(visa)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(preserved, (Some(15_000), Some(10_000)));
    }

    #[tokio::test]
    async fn card_scan_keeps_zero_structure_and_proposes_unknown_alias_once() {
        let pool = test_pool().await;
        crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Visa Infinite",
            None,
            Some(20),
            Some(10),
            None,
            None,
            None,
            &["visa infinite".into()],
        )
        .await
        .unwrap();
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![
            vec!["JANEIRO".into(), "".into(), "".into()],
            vec![],
            vec!["1".into(), "".into(), "".into()],
        ];
        let notes = vec![
            vec!["".into(); 3],
            vec![],
            vec![
                "".into(),
                "".into(),
                "FATURAS:\nR$ 0,00 - VISA Infinite\nR$ 100,00 - Nubank".into(),
            ],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        for expected_proposals in [1, 0] {
            let mut tx = pool.begin().await.unwrap();
            let outcome = scan_card_invoices(&mut tx, &rows, &notes, &layout, 2, &ctx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            assert_eq!(outcome.proposals, expected_proposals);
        }
        let stated: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT stated_total_cents, source_stated_total_cents FROM invoice")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stated, (Some(0), Some(0)));
        let proposal: (String, String, String) =
            sqlx::query_as("SELECT display_name, source_month, status FROM card_proposal")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            proposal,
            ("Nubank".into(), "2026-01".into(), "pending".into())
        );
    }

    #[tokio::test]
    async fn card_scan_preserves_local_total_and_records_three_way_conflict() {
        let pool = test_pool().await;
        let account_id = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![
            vec!["JANEIRO".into(), "".into(), "".into()],
            vec![],
            vec!["15".into(), "".into(), "100,00".into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let notes = |amount: &str| {
            vec![
                vec!["".into(); 3],
                vec![],
                vec![
                    "".into(),
                    "".into(),
                    format!("CARTÕES:\nR$ {amount} - Visa"),
                ],
            ]
        };
        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &rows, &notes("100,00"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("UPDATE invoice SET stated_total_cents = 15000 WHERE account_id = ?1")
            .bind(&account_id)
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let outcome = scan_card_invoices(&mut tx, &rows, &notes("200,00"), &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(outcome.conflicts, 1);
        let invoice: (String, Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT id, stated_total_cents, source_stated_total_cents FROM invoice")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((invoice.1, invoice.2), (Some(15_000), Some(10_000)));
        assert_eq!(
            crate::commands::write_back_cmds::unresolved_conflict_count(&pool)
                .await
                .unwrap(),
            1
        );
        let conflict: (String, String, String) =
            sqlx::query_as("SELECT transaction_id, field, sheet_value FROM import_conflict")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            conflict,
            (
                format!("invoice:{}", invoice.0),
                "stated_total".into(),
                "20000".into()
            )
        );
    }

    #[tokio::test]
    async fn link_card_refunds_targets_the_invoice_for_the_tagged_card_line() {
        let pool = test_pool().await;
        let holder = crate::commands::card_cmds::create_card_account_inner(
            &pool,
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
        crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Visa Bia",
            None,
            None,
            None,
            None,
            Some("Bia"),
            Some(&holder),
            &["visa bia".into()],
        )
        .await
        .unwrap();
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let raw_note = "CARTÕES:\nR$ 530,00 - Visa Bia #reembolso:Bia";
        let values = vec![
            vec!["JANEIRO".into(), "".into(), "".into()],
            vec![],
            vec!["10".into(), "".into(), "530,00".into()],
        ];
        let notes = vec![
            vec!["".into(); 3],
            vec![],
            vec!["".into(), "".into(), raw_note.into()],
        ];
        let layout = SheetLayout {
            id: "layout".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 3,
            date_direction: "both".into(),
        };
        let mut tx = pool.begin().await.unwrap();
        scan_card_invoices(&mut tx, &values, &notes, &layout, 2, &ctx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let rows = vec![ImportedRow {
            date: "2026-01-10".into(),
            amount: -53_000,
            description: "fatura".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: raw_note.into(),
        }];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            1
        );
        tx.commit().await.unwrap();
        let linked: Option<String> = sqlx::query_scalar(
            "SELECT refund_invoice_id FROM \"transaction\" WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(linked.is_some());
    }

    #[tokio::test]
    async fn link_card_refunds_links_an_income_that_names_its_card_on_its_due_date() {
        let pool = test_pool().await;
        let account_id = crate::commands::card_cmds::create_card_account_inner(
            &pool,
            "Visa (26/09)",
            None,
            Some(20),
            Some(26),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('visa-set', ?1, '2026-09', '2026-09-20', '2026-09-26', 53_000)",
        )
        .bind(&account_id)
        .execute(&pool)
        .await
        .unwrap();
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            1
        );
        tx.commit().await.unwrap();

        let linked: (i64, Option<String>) =
            sqlx::query_as("SELECT amount, refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-26", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, (12_345, Some("visa-set".into())));
    }

    async fn create_refund_invoice(
        pool: &SqlitePool,
        card_name: &str,
        due_date: &str,
        stated_total_cents: i64,
    ) -> (String, String) {
        let account_id = crate::commands::card_cmds::create_card_account_inner(
            pool,
            card_name,
            None,
            Some(20),
            Some(26),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap();
        let invoice_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES (?1, ?2, ?3, '2026-09-20', ?4, ?5)",
        )
        .bind(&invoice_id)
        .bind(&account_id)
        .bind(&due_date[..7])
        .bind(due_date)
        .bind(stated_total_cents)
        .execute(pool)
        .await
        .unwrap();
        (account_id, invoice_id)
    }

    #[tokio::test]
    async fn link_card_refunds_ignores_an_income_that_names_a_card_outside_its_due_date() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-22", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-22", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }

    /// O vínculo carrega o lançamento inteiro, então uma Entrada em que a devolução divide a
    /// célula com outra origem creditaria à fatura dinheiro que não voltou.
    #[tokio::test]
    async fn link_card_refunds_ignores_an_income_that_only_partly_names_its_card() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let mut row = imported_desc("2026-09-26", 500_000, "Entradas do dia");
        row.raw_note = "R$ 4.700,00 - Salário\nR$ 300,00 - Fatura Visa".into();
        let rows = vec![row];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-26", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }

    #[tokio::test]
    async fn link_card_refunds_does_not_confuse_cards_with_different_due_dates() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        create_refund_invoice(&pool, "Mastercard", "2026-09-10", 80_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-10", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-10", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }

    #[tokio::test]
    async fn link_card_refunds_preserves_a_previously_linked_income() {
        let pool = test_pool().await;
        let (_, visa_invoice_id) =
            create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let (_, other_invoice_id) =
            create_refund_invoice(&pool, "Mastercard", "2026-09-10", 80_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();
        let transaction_id = row_id("2026", "2026-09-26", RowKind::Entrada, 0);
        sqlx::query("UPDATE \"transaction\" SET refund_invoice_id = ?1 WHERE id = ?2")
            .bind(&other_invoice_id)
            .bind(&transaction_id)
            .execute(&pool)
            .await
            .unwrap();
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(&transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked.as_deref(), Some(other_invoice_id.as_str()));
        assert_ne!(linked.as_deref(), Some(visa_invoice_id.as_str()));
    }

    #[tokio::test]
    async fn link_card_refunds_preserves_the_owner_target_for_a_transaction_or_series() {
        let pool = test_pool().await;
        let (account_id, _) =
            create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();
        let transaction_id = row_id("2026", "2026-09-26", RowKind::Entrada, 0);
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('purchase-target', 'expense', 12_345, '2026-09-26', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE \"transaction\" SET refund_txn_id = 'purchase-target' WHERE id = ?1")
            .bind(&transaction_id)
            .execute(&pool)
            .await
            .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();
        let links: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT refund_invoice_id, refund_txn_id, refund_series_id \
             FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(links, (None, Some("purchase-target".into()), None));

        sqlx::query(
            "INSERT INTO card_series \
             (id, account_id, description, amount_cents, count, start_cycle_month) \
             VALUES ('series-target', ?1, 'Assinatura', 12_345, NULL, '2026-09')",
        )
        .bind(&account_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE \"transaction\" SET refund_txn_id = NULL, refund_series_id = 'series-target' \
             WHERE id = ?1",
        )
        .bind(&transaction_id)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();
        let links: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT refund_invoice_id, refund_txn_id, refund_series_id \
             FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(links, (None, None, Some("series-target".into())));
    }

    #[tokio::test]
    async fn link_card_refunds_does_not_infer_an_income_already_marked_as_a_refund() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let mut row = imported_desc("2026-09-26", 12_345, "Fatura Visa");
        row.raw_note = "R$ 123,45 - Fatura Visa #reembolso:Gio".into();
        let rows = vec![row];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-26", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }

    #[tokio::test]
    async fn link_card_refunds_does_not_restore_an_owner_declined_link() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Fatura Visa")];
        import_rows_with_options(
            &pool,
            "2026",
            &rows,
            "profile",
            ImportRowsOptions::default(),
        )
        .await
        .unwrap();
        let transaction_id = row_id("2026", "2026-09-26", RowKind::Entrada, 0);
        sqlx::query("UPDATE \"transaction\" SET refund_link_declined = 1 WHERE id = ?1")
            .bind(&transaction_id)
            .execute(&pool)
            .await
            .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            link_card_refunds(&mut tx, "2026", &rows, &ctx)
                .await
                .unwrap(),
            0
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(&transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }
}
