use crate::cards;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use super::grid::{ImportedRow, parse_number};
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

pub(crate) fn compute_checksum_with_options(
    rows: &[ImportedRow],
    descriptions_trusted: bool,
) -> String {
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
pub(crate) fn imported_row_ids(sheet_name: &str, rows: &[ImportedRow]) -> Vec<String> {
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

/// Calcula o checksum de idempotência do batch da MESMA forma que `import_rows_with_options`,
/// para o shell (commands.rs) poder rodar `check_duplicate_import` ANTES de abrir a transação
/// externa (a checagem é uma leitura no pool e não pode acontecer dentro da tx — read-your-writes
/// daria falso-negativo).
pub(crate) fn compute_import_checksum(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    compute_checksum_with_options(rows, descriptions_trusted)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_sheets::import::test_support::*;

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
}
