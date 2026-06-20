//! Write-back (spec 018): o caminho inverso do import — uma transação vira a célula da planilha
//! que a originaria. Produz um DIFF estruturado para aprovação humana; o envio real ao Google
//! Sheets fica atrás de `WRITE_BACK_ENABLED` (LIGADO, plano 028 Step 9). Núcleo puro + shell gated.
//!
//! Invariável de segurança (AGENTS.md): toda escrita material no Sheets passa por diff + validação +
//! aprovação humana. Aqui a aprovação é a `ApprovalDiffCard` + uma 2ª confirmação na UI; ligar a flag
//! habilita o caminho aprovar-para-escrever, mas cada envio ainda exige aprovação humana explícita.

use super::import::{RowKind, month_blocks_for, parse_number};
use super::layout_detect::{SheetLayout, month_number_from_name};
use serde::Serialize;

/// Mestre da trava: o envio real ao Sheets só acontece com isto `true`. LIGADO (plano 028 Step 9)
/// após as salvaguardas do PR-A: escopo de escrita + re-consentimento, blocklist de fórmulas, gate de
/// conflito, re-verificação de frescura, inspeção do batchUpdate e auditoria. Ligar habilita o
/// caminho aprovar-para-escrever — NÃO escreve sozinho: cada envio ainda exige aprovação + confirmação
/// humana na UI (diff + 2ª confirmação). O diff/preview já funcionava desligado (read-only).
pub const WRITE_BACK_ENABLED: bool = true;

/// Colunas que são FÓRMULAS/estruturais na planilha e o write-back NUNCA pode tocar: `balance`
/// (Saldo é calculado pela própria planilha) e `date` (o Dia é a âncora da linha, não um valor a
/// escrever). Defesa-em-profundidade SOBRE a flag `is_active` do banco: mesmo que um mapeamento
/// fosse gravado `is_active=1` por engano, estas colunas continuam barradas no planejador puro.
pub const FORMULA_ONLY_FIELDS: &[&str] = &["balance", "date"];

/// Falha cedo quando o write-back está desligado. Toda rota que ESCREVE chama isto primeiro.
pub fn ensure_write_back_enabled() -> Result<(), String> {
    if !WRITE_BACK_ENABLED {
        return Err(
            "Write-back desligado: o envio ao Sheets está atrás de uma flag desabilitada.".into(),
        );
    }
    Ok(())
}

/// Uma transação candidata a voltar para a planilha (magnitude positiva; o sinal/coluna vem do tipo).
#[derive(Debug, Clone)]
pub struct WriteBackTxn {
    pub date: String,
    pub kind: RowKind,
    /// Magnitude em centavos (sempre ≥ 0).
    pub amount_cents: i64,
}

/// Uma célula que o write-back tocaria: onde (A1), o que está lá hoje, e o que entraria.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CellWrite {
    pub a1: String,
    pub row: usize,
    pub col: usize,
    pub date: String,
    pub kind: String,
    /// Valor atual da célula (string crua da planilha; vazio = célula em branco).
    pub current: String,
    /// Valor proposto (reais, pt-BR), formatado a partir da magnitude — só para EXIBIÇÃO no diff.
    pub proposed: String,
    /// Magnitude em centavos que será escrita (a escrita real usa o número, não a string pt-BR).
    pub value_cents: i64,
    /// `true` quando o valor proposto difere do atual (por número, não por formatação).
    pub changed: bool,
}

/// Índice de coluna 0-based → letras A1 (0→A, 25→Z, 26→AA…). Base-26 bijetiva.
pub fn col_to_a1(mut col: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (col % 26) as u8);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).expect("ascii")
}

/// Centavos (magnitude) → string pt-BR "1234,56" (sem separador de milhar, decimal vírgula).
/// Round-trip seguro com `parse_number`, que aceita vírgula e ponto.
fn cents_to_ptbr(cents: i64) -> String {
    let c = cents.abs();
    format!("{},{:02}", c / 100, c % 100)
}

fn kind_offset(kind: RowKind, mappings: &[(String, i32)]) -> Option<usize> {
    let field = match kind {
        RowKind::Entrada => "amount_in",
        RowKind::Saida => "amount_out",
        RowKind::Diario => "amount_daily",
    };
    mappings
        .iter()
        .find(|(f, _)| f == field)
        .map(|(_, off)| *off as usize)
}

/// Lê a célula do dia (1..=31). Planilhas reais entregam o dia como float ("1.0000"), não inteiro,
/// então parseamos como `f64` antes de truncar — `parse::<u32>()` falharia e nenhuma linha casaria.
fn parse_day_cell(cell: &str) -> Option<u32> {
    cell.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|d| (1.0..=31.0).contains(d))
        .map(|d| d as u32)
}

/// Linha da grade (0-based) cujo `day_column` é o dia `day`, a partir de `data_start`.
fn find_day_row(
    rows: &[Vec<String>],
    data_start: usize,
    day_col: usize,
    day: u32,
) -> Option<usize> {
    rows.iter()
        .enumerate()
        .skip(data_start)
        .find(|(_, row)| {
            row.get(day_col)
                .and_then(|c| parse_day_cell(c))
                .is_some_and(|d| d == day)
        })
        .map(|(r, _)| r)
}

/// Planeja (e faz o diff) das transações de volta para as células da planilha. PURO e read-only:
/// não escreve nada — só descreve o que escreveria. Transações de outro ano, mês sem bloco, tipo
/// sem coluna mapeada, ou dia sem linha são silenciosamente puladas (não há onde escrever).
pub fn plan_write_back(
    rows: &[Vec<String>],
    layout: &SheetLayout,
    mappings: &[(String, i32)],
    txns: &[WriteBackTxn],
) -> Vec<CellWrite> {
    // Ano não detectado → não há onde escrever com segurança: melhor um plano VAZIO do que assumir
    // um ano e tocar a planilha errada em silêncio.
    let Some(year) = layout.year else {
        return Vec::new();
    };

    // BLOQUEIO de coluna-fórmula (defesa-em-profundidade): descarta QUALQUER mapeamento cujo
    // `target_field` seja de uma coluna calculada/estrutural ANTES de `kind_offset` resolvê-lo.
    // Assim, mesmo que `balance`/`date` chegassem aqui marcados ativos, nenhum offset deles é
    // visível ao planejador → zero `CellWrite` para Saldo/Data. É a base de segurança (STOP).
    let mappings: Vec<(String, i32)> = mappings
        .iter()
        .filter(|(field, _)| !FORMULA_ONLY_FIELDS.contains(&field.as_str()))
        .cloned()
        .collect();
    let mappings = mappings.as_slice();
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    let Some(header) = rows.get(month_row) else {
        return Vec::new();
    };
    let month_blocks = month_blocks_for(header, block_size);

    // Agrega por célula-alvo (data, kind): a planilha guarda UM valor por célula, então duas
    // transações do mesmo dia/tipo SOMAM — senão emitiríamos dois CellWrites para a mesma célula,
    // um sobrescrevendo o outro silenciosamente. `amount_cents` vira magnitude (abs).
    let mut aggregated: Vec<WriteBackTxn> = Vec::new();
    for txn in txns {
        if let Some(e) = aggregated
            .iter_mut()
            .find(|e| e.date == txn.date && e.kind == txn.kind)
        {
            e.amount_cents += txn.amount_cents.abs();
        } else {
            aggregated.push(WriteBackTxn {
                date: txn.date.clone(),
                kind: txn.kind,
                amount_cents: txn.amount_cents.abs(),
            });
        }
    }

    let mut out = Vec::new();
    for txn in &aggregated {
        let parts: Vec<&str> = txn.date.split('-').collect();
        let (Some(y), Some(m), Some(d)) = (
            parts.first().and_then(|s| s.parse::<i32>().ok()),
            parts.get(1).and_then(|s| s.parse::<u32>().ok()),
            parts.get(2).and_then(|s| s.parse::<u32>().ok()),
        ) else {
            continue;
        };
        if y != year {
            continue; // pertence a outra planilha (outro ano)
        }
        let Some(&(block_start, _)) = month_blocks.iter().find(|(_, mm)| *mm == m) else {
            continue;
        };
        let Some(offset) = kind_offset(txn.kind, mappings) else {
            continue;
        };
        let Some(row) = find_day_row(rows, data_start, day_col, d) else {
            continue;
        };
        let col = block_start + offset;
        let current = rows
            .get(row)
            .and_then(|r| r.get(col))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let proposed = cents_to_ptbr(txn.amount_cents);
        let changed = parse_number(&current) != txn.amount_cents.abs();
        out.push(CellWrite {
            a1: format!("{}{}", col_to_a1(col), row + 1),
            row,
            col,
            date: txn.date.clone(),
            kind: txn.kind.as_str().to_string(),
            current,
            proposed,
            value_cents: txn.amount_cents,
            changed,
        });
    }
    out
}

/// Planeja o write-back da Economia para a aba `Economia`. PURO e read-only.
/// `economia_by_month[m-1]` = centavos de Economia REGISTRADA do mês m (1..=12) do `year`.
///
/// A aba real coloca os blocos de ano LADO A LADO nas mesmas linhas (auditado: 2025 em B–E, 2026 em
/// G–J), e também tolera empilhamento vertical: cada bloco tem uma linha-CABEÇALHO com o ANO (número)
/// e os rótulos `Entradas | Economia | %`, seguida de 12 linhas `jan`..`dez` e um `TOTAL`. O mês fica
/// na MESMA coluna do ano; `Economia` é a coluna do rótulo homônimo à DIREITA do ano. Por isso
/// escopamos ao BLOCO do ano-alvo (senão escreveríamos na coluna do ano errado) e só tocamos a coluna
/// `Economia` — `Entradas` e `%` são FÓRMULAS, nunca escritas. Meses sem Economia (0) e células já
/// iguais não geram escrita.
pub fn plan_economia_write_back(
    rows: &[Vec<String>],
    year: i32,
    economia_by_month: &[i64; 12],
) -> Vec<CellWrite> {
    // Cabeçalho do bloco do ANO: a linha que tem o ANO (número INTEIRO) E os rótulos "Economia" E
    // "Entradas". Exigir os dois rótulos evita falso-match com uma célula de dado que por acaso seja
    // igual ao ano. `month_col` = coluna onde o ano aparece (os meses ficam logo abaixo).
    let is_year = |c: &str| {
        c.trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.fract() == 0.0)
            .map(|n| n as i32)
            == Some(year)
    };
    let header = rows.iter().enumerate().find_map(|(r, row)| {
        let has_entradas = row
            .iter()
            .any(|c| c.trim().eq_ignore_ascii_case("entradas"));
        let month_col = row.iter().position(|c| is_year(c))?;
        // `Economia` do bloco = primeiro rótulo à DIREITA da coluna do ano. Na aba real os anos ficam
        // lado a lado, então o `Economia` de 2026 vem depois da coluna de 2026 — não o de 2025.
        let econ_col = row[month_col + 1..]
            .iter()
            .position(|c| c.trim().eq_ignore_ascii_case("economia"))
            .map(|p| month_col + 1 + p)?;
        has_entradas.then_some((r, month_col, econ_col))
    });
    let Some((header_row, month_col, econ_col)) = header else {
        return Vec::new(); // bloco do ano não encontrado nesta aba
    };

    // BLOQUEIO de coluna-fórmula (Economia): `Entradas` e `%` são FÓRMULAS — nunca escritas. O
    // `econ_col` é resolvido pelo rótulo "economia", então NÃO pode ser uma dessas; mas afirmamos
    // por rótulo no índice resolvido como rede de segurança. Se por dado malformado o `econ_col`
    // cair sobre um rótulo `Entradas`/`%`, abortamos com plano VAZIO em vez de escrever a coluna
    // errada (paridade com o STOP do `plan_write_back`: nunca tocar uma coluna calculada).
    let econ_label = rows[header_row]
        .get(econ_col)
        .map(|c| c.trim())
        .unwrap_or_default();
    let targets_formula_col = econ_label.eq_ignore_ascii_case("entradas") || econ_label == "%";
    debug_assert!(
        !targets_formula_col,
        "econ_col deve apontar a coluna Economia, nunca Entradas/% (fórmulas)"
    );
    if targets_formula_col {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate().skip(header_row + 1) {
        let Some(month) = row.get(month_col).and_then(|l| month_number_from_name(l)) else {
            break; // fim do bloco anual (linha vazia, TOTAL ou próximo cabeçalho)
        };
        let cents = economia_by_month[(month - 1) as usize];
        let current = row
            .get(econ_col)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let current_cents = parse_number(&current);
        // Emite escrita quando há economia (> 0) OU quando a célula tem um valor ANTIGO a limpar
        // (economia local foi zerada). Sem o segundo caso, apagar a Economia na origem nunca
        // refletia na planilha — a célula ficava com o valor obsoleto para sempre.
        if cents > 0 || current_cents != 0 {
            out.push(CellWrite {
                a1: format!("{}{}", col_to_a1(econ_col), r + 1),
                row: r,
                col: econ_col,
                date: format!("{year}-{month:02}"),
                kind: "economia".to_string(),
                current,
                proposed: cents_to_ptbr(cents),
                value_cents: cents,
                changed: current_cents != cents,
            });
        }
        if month == 12 {
            break; // chegou em dezembro → fim do bloco do ano
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> SheetLayout {
        SheetLayout {
            id: "l".into(),
            sheet_name: "2026".into(),
            year: Some(2026),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "both".into(),
        }
    }

    // Geometria fiel: col 0 = dia (absoluta). Bloco de JANEIRO ancora no nome do mês (col 1 =
    // Saldo). Offsets relativos ao bloco: Entrada +1 (col 2/C), Saída +2 (col 3/D), Diário +3
    // (col 4/E). Linha de dados do dia 1 = índice 2 → A1 row 3.
    fn grid() -> Vec<Vec<String>> {
        vec![
            vec!["".into(), "JANEIRO".into(), "".into(), "".into(), "".into()],
            vec![
                "Dia".into(),
                "Saldo".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
            ],
            vec![
                "1".into(),
                "".into(),
                "1000,00".into(),
                "".into(),
                "50,00".into(),
            ],
            vec!["2".into(), "".into(), "".into(), "200,00".into(), "".into()],
        ]
    }

    fn mappings() -> Vec<(String, i32)> {
        vec![
            ("amount_in".into(), 1),
            ("amount_out".into(), 2),
            ("amount_daily".into(), 3),
        ]
    }

    #[test]
    fn col_letters_are_bijective_base26() {
        assert_eq!(col_to_a1(0), "A");
        assert_eq!(col_to_a1(25), "Z");
        assert_eq!(col_to_a1(26), "AA");
        assert_eq!(col_to_a1(27), "AB");
        assert_eq!(col_to_a1(701), "ZZ");
        assert_eq!(col_to_a1(702), "AAA");
    }

    #[test]
    fn plans_target_cell_and_detects_change() {
        // Diário do dia 1 mudou de 50,00 para 75,00 → célula D3, changed=true.
        let txns = vec![WriteBackTxn {
            date: "2026-01-01".into(),
            kind: RowKind::Diario,
            amount_cents: 7500,
        }];
        let plan = plan_write_back(&grid(), &layout(), &mappings(), &txns);
        assert_eq!(plan.len(), 1);
        let w = &plan[0];
        assert_eq!(w.a1, "E3"); // Diário = col 4 (E), linha de dados do dia 1 = índice 2 → row 3
        assert_eq!(w.current, "50,00");
        assert_eq!(w.proposed, "75,00");
        assert!(w.changed);
    }

    #[test]
    fn unchanged_when_value_matches_despite_format() {
        // Entrada do dia 1 é 1000,00 na planilha; propor 100000 centavos = mesmo número.
        let txns = vec![WriteBackTxn {
            date: "2026-01-01".into(),
            kind: RowKind::Entrada,
            amount_cents: 100_000,
        }];
        let plan = plan_write_back(&grid(), &layout(), &mappings(), &txns);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].a1, "C3"); // Entrada = col 2 (C)
        assert!(!plan[0].changed, "mesmo valor, formatação não conta");
    }

    #[test]
    fn writes_to_blank_cell_are_changes() {
        // Saída do dia 1 (célula D3) está em branco; propor 30,00 → change.
        let txns = vec![WriteBackTxn {
            date: "2026-01-01".into(),
            kind: RowKind::Saida,
            amount_cents: 3000,
        }];
        let plan = plan_write_back(&grid(), &layout(), &mappings(), &txns);
        assert_eq!(plan[0].a1, "D3"); // Saída = col 3 (D)
        assert_eq!(plan[0].current, "");
        assert!(plan[0].changed);
    }

    // Plano 028 Step 2 (BLOQUEIO, STOP-condition): mesmo com `balance` (offset 4) e `date`
    // (offset 0) presentes e marcados ATIVOS nos mappings, o planejador NUNCA emite um CellWrite
    // para essas colunas-fórmula. Defesa-em-profundidade sobre o `is_active` do banco.
    #[test]
    fn plan_write_back_never_targets_formula_columns_even_if_active() {
        // Mappings adversariais: inclui `date`@0 e `balance`@4 como se estivessem ativos, além das
        // colunas legítimas de movimento. Transações nos três tipos no dia 1.
        let mappings = vec![
            ("date".to_string(), 0),
            ("amount_in".to_string(), 1),
            ("amount_out".to_string(), 2),
            ("amount_daily".to_string(), 3),
            ("balance".to_string(), 4),
        ];
        let txns = vec![
            WriteBackTxn {
                date: "2026-01-01".into(),
                kind: RowKind::Entrada,
                amount_cents: 1000,
            },
            WriteBackTxn {
                date: "2026-01-01".into(),
                kind: RowKind::Saida,
                amount_cents: 2000,
            },
            WriteBackTxn {
                date: "2026-01-01".into(),
                kind: RowKind::Diario,
                amount_cents: 3000,
            },
        ];
        let plan = plan_write_back(&grid(), &layout(), &mappings, &txns);
        // As colunas-fórmula são os offsets 0 (date) e 4 (balance), ancorados no bloco de JANEIRO
        // (col 1 = Saldo): col absoluta do Saldo = block_start(1) + 4? Não — aqui as colunas de
        // movimento ancoram em block_start=1. O invariante testado é direto: NENHUM CellWrite pode
        // cair sobre uma coluna cujo offset de mapeamento era de `date`/`balance`.
        let block_start = 1usize; // JANEIRO ancora na col 1 (ver grid())
        let date_col = block_start; // offset 0 (date) → coluna do Saldo no grid de teste
        let balance_col = block_start + 4; // offset 4 (balance)
        assert!(
            plan.iter()
                .all(|c| c.col != date_col && c.col != balance_col),
            "nenhuma escrita pode tocar as colunas de date/balance (fórmula/estrutural)"
        );
        // E as colunas de movimento legítimas SEGUEM sendo planejadas (o bloqueio não derruba tudo).
        assert_eq!(
            plan.len(),
            3,
            "as três colunas de movimento ainda são planejadas"
        );
    }

    // Plano 028 Step 2 (b): a Economia nunca escreve na coluna Entradas nem na coluna % (fórmulas).
    #[test]
    fn plan_economia_write_back_never_targets_entradas_or_percent_column() {
        let grid = vec![
            vec![
                "".into(),
                "2026".into(),
                "Entradas".into(),
                "Economia".into(),
                "%".into(),
            ],
            vec![
                "".into(),
                "jan".into(),
                "9000.00".into(),
                "0.0000".into(),
                "0".into(),
            ],
        ];
        let mut by = [0i64; 12];
        by[0] = 100_000; // jan tem economia → gera ao menos uma escrita

        let plan = plan_economia_write_back(&grid, 2026, &by);
        assert!(!plan.is_empty(), "deve planejar jan de 2026");
        // Entradas = idx 2, % = idx 4. NENHUMA escrita pode cair nessas colunas.
        assert!(
            plan.iter().all(|c| c.col != 2 && c.col != 4),
            "Economia nunca escreve nas colunas Entradas (2) nem % (4)"
        );
        // Confirma positivamente que escreve na coluna Economia (idx 3).
        assert!(plan.iter().all(|c| c.col == 3));
    }

    #[test]
    fn plans_economia_block_for_target_year_multi_year() {
        // Estrutura REAL da aba Economia (auditada na planilha viva): blocos empilhados por ano;
        // ano e mês na col B (idx 1), Economia na col D (idx 3); col A vazia. Dois anos aqui.
        let h = |y: &str| {
            vec![
                "".into(),
                y.to_string(),
                "Entradas".into(),
                "Economia".into(),
                "%".into(),
            ]
        };
        let m = |name: &str, ent: &str, eco: &str| {
            vec![
                "".into(),
                name.to_string(),
                ent.to_string(),
                eco.to_string(),
                "0".into(),
            ]
        };
        let grid = vec![
            h("2025"),
            m("jan", "5000.00", "1000.00"),
            m("fev", "5000.00", "0.0000"),
            vec!["".into(), "TOTAL".into(), "".into(), "".into(), "".into()],
            h("2026"),
            m("jan", "9000.00", "0.0000"),
        ];
        let mut by = [0i64; 12];
        by[0] = 100_000; // jan
        by[1] = 50_000; // fev

        // 2025: jan já tem 1000,00 (não muda); fev vazio → escreve 500,00. Para no TOTAL.
        let plan = plan_economia_write_back(&grid, 2025, &by);
        assert_eq!(plan.len(), 2);
        let jan = plan.iter().find(|c| c.date == "2025-01").unwrap();
        assert_eq!(jan.a1, "D2");
        assert!(!jan.changed, "jan já tem 1000,00");
        let fev = plan.iter().find(|c| c.date == "2025-02").unwrap();
        assert_eq!(fev.a1, "D3");
        assert_eq!(fev.proposed, "500,00");
        assert!(fev.changed);

        // 2026: escreve no BLOCO de 2026 (D6), não no de 2025 — escopo por ano.
        let plan26 = plan_economia_write_back(&grid, 2026, &by);
        assert_eq!(plan26.len(), 1);
        assert_eq!(plan26[0].a1, "D6");
        assert_eq!(plan26[0].date, "2026-01");
    }

    // Regressão (P1): na aba real os anos ficam LADO A LADO. Escrever 2026 deve cair na coluna de
    // Economia de 2026 ("I"), nunca na de 2025 ("D").
    #[test]
    fn plans_economia_write_back_side_by_side_targets_correct_block() {
        // 2025 em B–E (Economia idx 3 = "D"), 2026 em G–J (Economia idx 8 = "I"); col F (idx 5) gap.
        let header = vec![
            "".into(),
            "2025".into(),
            "Entradas".into(),
            "Economia".into(),
            "%".into(),
            "".into(),
            "2026".into(),
            "Entradas".into(),
            "Economia".into(),
            "%".into(),
        ];
        let data_row = |name: &str, eco25: &str, eco26: &str| {
            vec![
                "".into(),
                name.to_string(),
                "5000.00".into(),
                eco25.to_string(),
                "0".into(),
                "".into(),
                name.to_string(),
                "8000.00".into(),
                eco26.to_string(),
                "0".into(),
            ]
        };
        let grid = vec![header, data_row("jan", "1000.00", "500.00")];

        let mut by = [0i64; 12];
        by[0] = 200_000; // jan = 2000,00

        let plan26 = plan_economia_write_back(&grid, 2026, &by);
        assert!(!plan26.is_empty(), "deve planejar jan de 2026");
        assert!(
            plan26.iter().all(|c| c.col == 8 && c.a1.starts_with('I')),
            "escritas de 2026 vão para a col 8 (I), não a col 3 (D)"
        );

        let plan25 = plan_economia_write_back(&grid, 2025, &by);
        assert!(!plan25.is_empty(), "deve planejar jan de 2025");
        assert!(
            plan25.iter().all(|c| c.col == 3 && c.a1.starts_with('D')),
            "escritas de 2025 vão para a col 3 (D)"
        );
    }

    #[test]
    fn economia_zeroed_locally_clears_stale_sheet_cell() {
        // Regressão (review): a planilha tem 1000,00 em jan, mas a Economia local foi zerada (0).
        // Antes, meses com 0 eram pulados → a célula obsoleta nunca era limpa. Agora gera 1 escrita.
        let row = |name: &str, eco: &str| {
            vec![
                "".into(),
                name.to_string(),
                "5000.00".into(),
                eco.to_string(),
                "0".into(),
            ]
        };
        let grid = vec![
            vec![
                "".into(),
                "2026".into(),
                "Entradas".into(),
                "Economia".into(),
                "%".into(),
            ],
            row("jan", "1000.00"),
        ];
        let by = [0i64; 12]; // nenhuma economia registrada → jan deve ser limpo

        let plan = plan_economia_write_back(&grid, 2026, &by);
        assert_eq!(plan.len(), 1, "a célula obsoleta de jan precisa ser limpa");
        assert_eq!(plan[0].date, "2026-01");
        assert_eq!(plan[0].value_cents, 0);
        assert!(plan[0].changed, "1000,00 → 0 é mudança");
    }

    #[test]
    fn empty_plan_when_year_undetected() {
        // Ano não detectado (None) → plano vazio (não assume um ano e escreve na planilha errada).
        let mut l = layout();
        l.year = None;
        let txns = vec![WriteBackTxn {
            date: "2026-01-01".into(),
            kind: RowKind::Diario,
            amount_cents: 3000,
        }];
        assert!(plan_write_back(&grid(), &l, &mappings(), &txns).is_empty());
    }

    #[test]
    fn aggregates_transactions_on_the_same_cell() {
        // Dois Diários no MESMO dia 1 → UMA célula (E3) com a SOMA, não dois CellWrites.
        let txns = vec![
            WriteBackTxn {
                date: "2026-01-01".into(),
                kind: RowKind::Diario,
                amount_cents: 3000,
            },
            WriteBackTxn {
                date: "2026-01-01".into(),
                kind: RowKind::Diario,
                amount_cents: 4500,
            },
        ];
        let plan = plan_write_back(&grid(), &layout(), &mappings(), &txns);
        assert_eq!(plan.len(), 1, "uma célula-alvo, não duas");
        assert_eq!(plan[0].a1, "E3");
        assert_eq!(plan[0].proposed, "75,00"); // 30,00 + 45,00
    }

    #[test]
    fn finds_day_row_when_cells_are_floats() {
        // Regressão da review adversarial: planilhas reais dão o dia como float ("1.0000").
        // O parse `u32` antigo falhava e NENHUMA célula era encontrada (write-back vazio).
        let mut g = grid();
        g[2][0] = "1.0000".into();
        g[3][0] = "2,0000".into(); // vírgula decimal também ocorre
        let txns = vec![WriteBackTxn {
            date: "2026-01-02".into(),
            kind: RowKind::Saida,
            amount_cents: 20_000,
        }];
        let plan = plan_write_back(&g, &layout(), &mappings(), &txns);
        assert_eq!(plan.len(), 1, "o dia em float precisa casar a linha");
        assert_eq!(plan[0].a1, "D4"); // Saída do dia 2 → linha índice 3 → row 4
    }

    #[test]
    fn parse_day_cell_handles_floats_and_bounds() {
        assert_eq!(parse_day_cell("1.0000"), Some(1));
        assert_eq!(parse_day_cell("31,0000"), Some(31));
        assert_eq!(parse_day_cell(" 15 "), Some(15));
        assert_eq!(parse_day_cell("0"), None); // fora de 1..=31
        assert_eq!(parse_day_cell("32"), None);
        assert_eq!(parse_day_cell("Saldo"), None);
    }

    #[test]
    fn skips_other_year_and_unmapped() {
        let txns = vec![
            WriteBackTxn {
                date: "2025-01-01".into(), // outro ano
                kind: RowKind::Entrada,
                amount_cents: 1000,
            },
            WriteBackTxn {
                date: "2026-12-01".into(), // mês sem bloco na grade de teste
                kind: RowKind::Entrada,
                amount_cents: 1000,
            },
        ];
        assert!(plan_write_back(&grid(), &layout(), &mappings(), &txns).is_empty());
    }

    #[test]
    fn flag_gate_matches_master_switch() {
        // O gate (`ensure_write_back_enabled`) reflete a trava-mestra: com a flag LIGADA (plano 028
        // Step 9) passa; se algum dia for desligada de novo, volta a falhar cedo. O envio real segue
        // exigindo aprovação + 2ª confirmação humana na UI — ligar a flag não escreve sozinho.
        if WRITE_BACK_ENABLED {
            assert!(ensure_write_back_enabled().is_ok());
        } else {
            assert!(ensure_write_back_enabled().is_err());
        }
    }
}
