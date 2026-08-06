use crate::{cards, commands::card_cmds};
use sqlx::SqlitePool;
use std::collections::HashMap;

use super::super::layout_detect::SheetLayout;
use super::classify::{ItemKind, parse_itemized_note_opts};
/// Contexto de cartões lido antes da transação de importação. Manter essas leituras fora da
/// transação evita disputar a única conexão do pool enquanto a planilha é persistida.
#[derive(Debug, Default)]
pub(crate) struct CardScanCtx {
    pub aliases: HashMap<String, String>,
    pub cycles: HashMap<String, (u32, u32)>,
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
pub(crate) fn sheet_card_lexicon(
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
