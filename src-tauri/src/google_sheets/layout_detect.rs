use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetLayout {
    pub id: String,
    pub sheet_name: String,
    pub year: Option<i32>,
    pub month_names_row: i32,
    pub header_row: i32,
    pub data_start_row: i32,
    pub day_column: i32,
    pub block_size: i32,
    pub date_direction: String,
}

const MONTH_NAMES_PT: &[(&str, u32)] = &[
    ("JANEIRO", 1),
    ("FEVEREIRO", 2),
    ("MARÇO", 3),
    ("MARCO", 3),
    ("ABRIL", 4),
    ("MAIO", 5),
    ("JUNHO", 6),
    ("JULHO", 7),
    ("AGOSTO", 8),
    ("SETEMBRO", 9),
    ("OUTUBRO", 10),
    ("NOVEMBRO", 11),
    ("DEZEMBRO", 12),
];

const HEADER_KEYWORDS: &[&str] = &[
    "DATA", "ENTRADA", "SAÍDA", "SAIDA", "DIÁRIO", "DIARIO", "SALDO",
];

pub fn detect_layout(rows: &[Vec<String>], sheet_name: &str) -> Result<SheetLayout, String> {
    if rows.len() < 3 {
        return Err("sheet has fewer than 3 rows".to_string());
    }

    let month_names_row = find_month_names_row(rows)?;
    let header_row = find_header_row(rows, month_names_row)?;
    let data_start_row = header_row + 1;
    let day_column = find_day_column(rows, data_start_row as usize)?;
    let block_size = calculate_block_size(rows, month_names_row as usize)?;

    let year = parse_year_from_name(sheet_name);

    Ok(SheetLayout {
        id: uuid::Uuid::new_v4().to_string(),
        sheet_name: sheet_name.to_string(),
        year,
        month_names_row,
        header_row,
        data_start_row,
        day_column,
        block_size,
        date_direction: "both".to_string(),
    })
}

fn find_month_names_row(rows: &[Vec<String>]) -> Result<i32, String> {
    for (i, row) in rows.iter().enumerate().take(5) {
        let matches = row.iter().filter(|cell| is_month_name(cell)).count();
        if matches >= 2 {
            return Ok(i as i32);
        }
    }
    Err("no row with month names found in first 5 rows".to_string())
}

fn find_header_row(rows: &[Vec<String>], month_names_row: i32) -> Result<i32, String> {
    let start = (month_names_row + 1) as usize;
    for (i, row) in rows.iter().enumerate().skip(start).take(3) {
        let matches = row.iter().filter(|cell| is_header_keyword(cell)).count();
        if matches >= 3 {
            return Ok(i as i32);
        }
    }
    Err("no header row found after month names row".to_string())
}

fn find_day_column(rows: &[Vec<String>], data_start_row: usize) -> Result<i32, String> {
    if data_start_row >= rows.len() {
        return Err("no data rows available".to_string());
    }

    let row = &rows[data_start_row];
    for (col, cell) in row.iter().enumerate() {
        let trimmed = cell.trim();
        if let Ok(num) = trimmed.parse::<f64>()
            && (1.0..=31.0).contains(&num)
        {
            return Ok(col as i32);
        }
    }
    Ok(0)
}

fn calculate_block_size(rows: &[Vec<String>], month_names_row: usize) -> Result<i32, String> {
    if month_names_row >= rows.len() {
        return Ok(6);
    }

    let row = &rows[month_names_row];
    let mut month_positions = Vec::new();

    for (i, cell) in row.iter().enumerate() {
        if is_month_name(cell) {
            month_positions.push(i);
        }
    }

    if month_positions.len() < 2 {
        return Ok(6);
    }

    let mut distances = Vec::new();
    for i in 1..month_positions.len() {
        distances.push(month_positions[i] - month_positions[i - 1]);
    }

    if distances.is_empty() {
        return Ok(6);
    }

    let most_common = distances
        .iter()
        .copied()
        .max_by_key(|&d| distances.iter().filter(|&&x| x == d).count())
        .unwrap_or(6);

    Ok(most_common as i32)
}

fn is_month_name(cell: &str) -> bool {
    month_number_from_name(cell).is_some()
}

/// Mapeia o nome de um mês PT-BR para o seu número (1–12). Aceita o nome exato, a
/// abreviação exata de 3 letras ("FEV") ou o nome completo como prefixo ("JANEIRO 2026") —
/// mas nunca prefixo solto de 3 letras, senão "OUTROS" viraria OUTUBRO.
/// É a âncora do parse por bloco: o mês vem do NOME na célula, nunca da posição do bloco,
/// para que JANEIRO na coluna 0 e células espúrias entre blocos não desloquem os meses.
pub fn month_number_from_name(cell: &str) -> Option<u32> {
    let upper = cell.trim().to_uppercase();
    if upper.len() < 3 {
        return None;
    }
    MONTH_NAMES_PT
        .iter()
        .find(|(name, _)| upper == *name || upper == name[..3] || upper.starts_with(*name))
        .map(|(_, n)| *n)
}

/// Abas de métricas do método (`Economia`, `Totais`): layout `mês|Entradas|Economia|%`,
/// não blocos mensais — nunca importar como transações. A detecção estrutural já as rejeita
/// (exige ≥2 nomes de mês na MESMA linha; nelas os meses ficam um por linha), mas o skip
/// por nome é a garantia explícita. A aba `Economia` é processada pelo importador de métricas
/// dedicado.
pub fn is_metric_tab(sheet_name: &str) -> bool {
    matches!(
        sheet_name.trim().to_lowercase().as_str(),
        "economia" | "totais" | "total"
    )
}

fn is_header_keyword(cell: &str) -> bool {
    let upper = cell.trim().to_uppercase();
    HEADER_KEYWORDS
        .iter()
        .any(|&kw| upper == kw || upper.contains(kw))
}

fn parse_year_from_name(name: &str) -> Option<i32> {
    name.parse::<i32>().ok()
}

pub fn generate_mappings(layout: &SheetLayout) -> Vec<SheetMappingEntry> {
    let mut mappings = Vec::new();

    let field_names = ["data", "entrada", "saida", "diario", "saldo"];
    // `amount_daily` (não `daily_budget`, que é a tabela do check-in) é o target que o importador
    // procura para o Diário variável — a estrela do método. Mantê-los em sincronia é fidelidade.
    let target_fields = ["date", "amount_in", "amount_out", "amount_daily", "balance"];

    for (offset, (field, target)) in field_names.iter().zip(target_fields.iter()).enumerate() {
        mappings.push(SheetMappingEntry {
            id: uuid::Uuid::new_v4().to_string(),
            sheet_name: layout.sheet_name.clone(),
            column_letter: format!("+{}", offset + 1),
            column_header: Some(field.to_string()),
            target_table: "transaction".to_string(),
            target_field: target.to_string(),
            date_direction: layout.date_direction.clone(),
            layout_id: Some(layout.id.clone()),
            block_offset: offset as i32,
            is_active: matches!(*target, "amount_in" | "amount_out" | "amount_daily") as i32,
        });
    }

    mappings
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetMappingEntry {
    pub id: String,
    pub sheet_name: String,
    pub column_letter: String,
    pub column_header: Option<String>,
    pub target_table: String,
    pub target_field: String,
    pub date_direction: String,
    pub layout_id: Option<String>,
    pub block_offset: i32,
    pub is_active: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_tabs_are_never_importable_as_transactions() {
        assert!(is_metric_tab("Economia"));
        assert!(is_metric_tab("ECONOMIA"));
        assert!(is_metric_tab(" Totais "));
        assert!(!is_metric_tab("2026"));
        assert!(!is_metric_tab("Economia Doméstica"));
    }

    /// A aba Economia tem meses um por linha (`mês|Entradas|Economia|%`) — a detecção
    /// estrutural deve rejeitá-la mesmo sem o filtro por nome.
    #[test]
    fn economia_layout_fails_structural_detection() {
        let rows: Vec<Vec<String>> = vec![
            vec![
                "mês".into(),
                "Entradas".into(),
                "Economia".into(),
                "%".into(),
            ],
            vec!["JANEIRO".into(), "5000".into(), "1500".into(), "30%".into()],
            vec![
                "FEVEREIRO".into(),
                "5200".into(),
                "1000".into(),
                "19%".into(),
            ],
            vec!["MARÇO".into(), "5100".into(), "1200".into(), "24%".into()],
        ];
        assert!(detect_layout(&rows, "Economia").is_err());
    }

    fn fixture_cashflow_rows() -> Vec<Vec<String>> {
        vec![
            vec![
                "".into(),
                "JANEIRO".into(),
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
                "MARÇO".into(),
            ],
            vec![
                "".into(),
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
                "".into(),
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
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
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
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
            ],
            vec![
                "3".into(),
                "".into(),
                "".into(),
                "120".into(),
                "3335".into(),
                "3335".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
            ],
        ]
    }

    #[test]
    fn test_detect_layout_basic() {
        let rows = fixture_cashflow_rows();
        let layout = detect_layout(&rows, "2025").unwrap();

        assert_eq!(layout.sheet_name, "2025");
        assert_eq!(layout.year, Some(2025));
        assert_eq!(layout.month_names_row, 0);
        assert_eq!(layout.header_row, 1);
        assert_eq!(layout.data_start_row, 2);
        assert_eq!(layout.day_column, 0);
        assert_eq!(layout.block_size, 6);
        assert_eq!(layout.date_direction, "both");
    }

    #[test]
    fn test_detect_layout_too_few_rows() {
        let rows = vec![vec!["a".into()], vec!["b".into()]];
        assert!(detect_layout(&rows, "test").is_err());
    }

    #[test]
    fn test_is_month_name() {
        assert!(is_month_name("JANEIRO"));
        assert!(is_month_name("janeiro"));
        assert!(is_month_name("FEV"));
        assert!(is_month_name("MARÇO"));
        assert!(!is_month_name(""));
        assert!(!is_month_name("foo"));
    }

    #[test]
    fn test_month_number_from_name() {
        assert_eq!(month_number_from_name("JANEIRO"), Some(1));
        assert_eq!(month_number_from_name("janeiro"), Some(1));
        assert_eq!(month_number_from_name("MARÇO"), Some(3));
        assert_eq!(month_number_from_name("MARCO"), Some(3));
        assert_eq!(month_number_from_name("DEZEMBRO"), Some(12));
        assert_eq!(month_number_from_name("DEZ"), Some(12));
        assert_eq!(month_number_from_name("JANEIRO 2026"), Some(1));
        assert_eq!(month_number_from_name("TOTAL"), None);
        assert_eq!(month_number_from_name(""), None);
        assert_eq!(month_number_from_name("Saldo"), None);
        // "OUTROS" é cabeçalho real de bloco de notas — não pode virar OUTUBRO.
        assert_eq!(month_number_from_name("OUTROS"), None);
    }

    #[test]
    fn test_is_header_keyword() {
        assert!(is_header_keyword("Data"));
        assert!(is_header_keyword("ENTRADA"));
        assert!(is_header_keyword("Saída"));
        assert!(is_header_keyword("SAIDA"));
        assert!(is_header_keyword("Diário"));
        assert!(is_header_keyword("SALDO"));
        assert!(!is_header_keyword(""));
        assert!(!is_header_keyword("foo"));
    }

    #[test]
    fn test_parse_year_from_name() {
        assert_eq!(parse_year_from_name("2025"), Some(2025));
        assert_eq!(parse_year_from_name("2026"), Some(2026));
        assert_eq!(parse_year_from_name("Economia"), None);
        assert_eq!(parse_year_from_name("abc"), None);
    }

    #[test]
    fn test_generate_mappings() {
        let layout = SheetLayout {
            id: "test-id".into(),
            sheet_name: "2025".into(),
            year: Some(2025),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "both".into(),
        };

        let mappings = generate_mappings(&layout);
        assert_eq!(mappings.len(), 5);

        let entrada = mappings
            .iter()
            .find(|m| m.target_field == "amount_in")
            .unwrap();
        assert_eq!(entrada.block_offset, 1);
        assert_eq!(entrada.is_active, 1);

        let saida = mappings
            .iter()
            .find(|m| m.target_field == "amount_out")
            .unwrap();
        assert_eq!(saida.block_offset, 2);
        assert_eq!(saida.is_active, 1);

        // Diário precisa sair do detector como `amount_daily` e ativo, exatamente o `target_field`
        // consumido pelo importador; `daily_budget` inativo impediria a importação da coluna.
        let diario = mappings
            .iter()
            .find(|m| m.target_field == "amount_daily")
            .expect("detector deve mapear a coluna Diário como amount_daily");
        assert_eq!(diario.block_offset, 3);
        assert_eq!(diario.is_active, 1);
        assert!(
            !mappings.iter().any(|m| m.target_field == "daily_budget"),
            "daily_budget é a tabela do check-in, nunca um target de mapeamento de coluna"
        );

        let saldo = mappings
            .iter()
            .find(|m| m.target_field == "balance")
            .unwrap();
        assert_eq!(saldo.is_active, 0);
    }

    #[test]
    fn test_calculate_block_size() {
        let rows = fixture_cashflow_rows();
        let size = calculate_block_size(&rows, 0).unwrap();
        assert_eq!(size, 6);
    }
}
