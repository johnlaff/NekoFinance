use crate::cards;
use chrono::Datelike;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

use super::super::layout_detect::SheetLayout;
#[cfg(test)]
use super::classify::row_id;
use super::classify::{
    ItemKind, NoteMarkerKind, RowKind, imported_row_ids, parse_itemized_note_opts,
    parse_note_markers,
};
use super::grid::{ImportedRow, month_blocks_for};
use super::lexicon::CardScanCtx;
#[cfg(test)]
use super::lexicon::load_card_scan_ctx;
use super::lexicon::sheet_card_lexicon;
#[cfg(test)]
use super::{ImportRowsOptions, import_rows_with_options};
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CardScanOutcome {
    pub invoices_created: usize,
    pub invoices_updated: usize,
    pub conflicts: usize,
    pub proposals: usize,
    pub ignored_items: usize,
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
            // A nota rege a Entrada derivada a cada import; refund_link_declined só protege a inferência, não a declaração explícita da planilha.
            linked += sqlx::query(
                "UPDATE \"transaction\" SET refund_invoice_id = ?1 \
                 WHERE id = ?2 AND refund_txn_id IS NULL AND refund_series_id IS NULL",
            )
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
        let mut hits = HashMap::new();
        for (description, cents) in named {
            if let Some(account_id) = lexicon.resolve(description).or_else(|| {
                lexicon.resolve(&cards::root_alias(&cards::declared_alias(description)))
            }) {
                *hits.entry(account_id).or_insert(0) += cents;
            }
        }
        if hits.len() != 1 {
            continue;
        }
        let Some((account_id, refund_cents)) = hits.into_iter().next() else {
            continue;
        };
        // Duas identidades de cartão na mesma Entrada não se desempatam sozinhas, e uma devolução
        // que responde por só parte da célula faria o vínculo mentir o valor: ele carrega o
        // lançamento inteiro, então creditaria à fatura dinheiro que não voltou. Os dois casos
        // ficam para o marcador explícito, que declara quanto e de quem.
        if refund_cents != row.amount.abs() {
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
            if note.trim().is_empty()
                || super::super::ceiling_note::parse_ceiling_ceremony(note).is_none()
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
    let Some(ceremony) = super::super::ceiling_note::parse_ceiling_ceremony(raw_note) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_sheets::import::test_support::*;

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

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) VALUES ('purchase-target', 'expense', 53_000, '2026-01-10', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE \"transaction\" SET refund_invoice_id = NULL, refund_txn_id = 'purchase-target' WHERE id LIKE 'derived:reembolso:%'",
        )
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
        let links: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT refund_invoice_id, refund_txn_id FROM \"transaction\" WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(links, (None, Some("purchase-target".into())));
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

    #[tokio::test]
    async fn link_card_refunds_aggregates_two_note_mentions_of_the_same_card() {
        let pool = test_pool().await;
        let (_, invoice_id) =
            create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let mut row = imported_desc("2026-09-26", 10_000, "Entradas do dia");
        row.raw_note = "R$ 40,00 - Fatura Visa\nR$ 60,00 - Visa".into();
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
            1
        );
        tx.commit().await.unwrap();

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-26", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, Some(invoice_id));
    }

    #[tokio::test]
    async fn link_card_refunds_keeps_two_card_identities_in_one_income_ambiguous() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Visa (26/09)", "2026-09-26", 53_000).await;
        create_refund_invoice(&pool, "Mastercard", "2026-09-26", 80_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let mut row = imported_desc("2026-09-26", 10_000, "Entradas do dia");
        row.raw_note = "R$ 40,00 - Fatura Visa\nR$ 60,00 - Mastercard".into();
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
    async fn link_card_refunds_resolves_an_income_with_its_cycle_annotation() {
        let pool = test_pool().await;
        let (_, invoice_id) = create_refund_invoice(&pool, "Nubank", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Nubank (26/09)")];
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

        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(row_id("2026", "2026-09-26", RowKind::Entrada, 0))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, Some(invoice_id));
    }

    #[tokio::test]
    async fn link_card_refunds_keeps_an_income_outside_the_lexicon_unlinked() {
        let pool = test_pool().await;
        create_refund_invoice(&pool, "Nubank", "2026-09-26", 53_000).await;
        let ctx = load_card_scan_ctx(&pool).await.unwrap();
        let rows = vec![imported_desc("2026-09-26", 12_345, "Cartão fora (26/09)")];
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
