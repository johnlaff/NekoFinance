//! Write-back (spec 018): o caminho inverso do import — uma transação vira a célula da planilha
//! que a originaria. Produz um DIFF estruturado para aprovação humana; o envio real ao Google
//! Sheets fica atrás de `WRITE_BACK_ENABLED` (DESLIGADO). Núcleo puro + shell gated.
//!
//! Invariável de segurança (AGENTS.md): toda escrita material no Sheets passa por diff + validação
//! + aprovação humana. Aqui a aprovação é a `ApprovalDiffCard`; o envio só ocorre com a flag ligada.

use super::import::{RowKind, month_blocks_for, parse_number};
use super::layout_detect::SheetLayout;
use serde::Serialize;

/// Mestre da trava: o envio real ao Sheets só acontece com isto `true`. Mantido `false` até a
/// fase de write-back ser explicitamente liberada — o diff/preview funciona desligado (read-only).
pub const WRITE_BACK_ENABLED: bool = false;

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
    /// Valor proposto (reais, pt-BR), formatado a partir da magnitude.
    pub proposed: String,
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
    let year = layout.year.unwrap_or(2025);
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    let Some(header) = rows.get(month_row) else {
        return Vec::new();
    };
    let month_blocks = month_blocks_for(header, block_size);

    let mut out = Vec::new();
    for txn in txns {
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
            changed,
        });
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
        // Saída do dia 1 (célula C3) está em branco; propor 30,00 → change.
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
    fn apply_is_blocked_while_flag_off() {
        // Enquanto a flag estiver desligada (estado de nascimento), o gate falha cedo.
        if !WRITE_BACK_ENABLED {
            assert!(ensure_write_back_enabled().is_err());
        } else {
            assert!(ensure_write_back_enabled().is_ok());
        }
    }
}
