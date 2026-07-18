//! Leitor da cerimônia do teto do Diário documentada em nota de célula.
//!
//! A cerimônia da "previsão do diário" vive numa nota da coluna Diário — frequentemente numa
//! célula sem valor, que nunca vira transação; por isso este parser opera sobre o texto cru da
//! nota, fora do fluxo de linhas materializadas. Gramática reconhecida (separadores `\t` ou
//! espaços):
//!
//! ```text
//! Mensal  R$ 300,00  Transporte
//! Mensal  R$ 200,00  Farmácia
//! Total = R$ 500,00
//! R$ 500,00 / 31 Dias = R$ 16,12
//! ```
//!
//! A linha do divisor é obrigatória (carrega total mensal, divisor e teto/dia declarados); itens
//! e linha `Total =` são opcionais, mas quando presentes precisam ser consistentes — uma nota
//! que não fecha é rejeitada por inteiro, nunca vira proposta silenciosamente errada. O divisor
//! é parte da cerimônia (a nota real mantém "/ 31 Dias" mesmo em meses de 30 dias): o teto/dia
//! DECLARADO pelo dono é o valor proposto; a recomputação `total ÷ divisor` só valida
//! (tolerância de arredondamento de 1 centavo).

use crate::google_sheets::import::parse_number;

/// Um item mensal da cerimônia (`Mensal R$ 300,00 Transporte`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyItem {
    pub name: String,
    pub amount_cents: i64,
}

/// Cerimônia do teto parseada de uma nota de célula do Diário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeilingCeremony {
    /// Teto por dia declarado na nota (o número que o dono usa).
    pub per_day_cents: i64,
    /// Divisor declarado ("/ N Dias").
    pub divisor_days: u32,
    /// Total mensal da cerimônia.
    pub monthly_total_cents: i64,
    /// Itens mensais, na ordem da nota (pode ser vazio: nota só com a linha do divisor).
    pub items: Vec<CeremonyItem>,
}

/// Normaliza uma linha da nota: tabs viram espaço, espaços colapsam, pontas aparadas.
fn normalize_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extrai o primeiro montante `R$ <valor>` de `s` a partir de `from`, devolvendo
/// `(cents, fim_do_match)`. O valor termina no primeiro caractere que não é dígito/`.`/`,`.
fn take_brl(s: &str, from: usize) -> Option<(i64, usize)> {
    let rest = &s[from..];
    let rs = rest.find("R$")?;
    let after = from + rs + 2;
    let tail = s[after..].trim_start();
    let skipped = s[after..].len() - tail.len();
    let start = after + skipped;
    let end_rel = tail
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .unwrap_or(tail.len());
    if end_rel == 0 {
        return None;
    }
    let cents = parse_number(&tail[..end_rel]);
    if cents <= 0 {
        return None;
    }
    Some((cents, start + end_rel))
}

/// Linha do divisor: `R$ <total> / <N> Dias = R$ <per_day>`.
fn parse_divisor_line(line: &str) -> Option<(i64, u32, i64)> {
    let (total, after_total) = take_brl(line, 0)?;
    let rest = line[after_total..].trim_start();
    let rest = rest.strip_prefix('/')?.trim_start();
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let divisor: u32 = rest[..digits_end].parse().ok()?;
    if divisor == 0 {
        return None;
    }
    let rest = rest[digits_end..].trim_start();
    let lower = rest.to_lowercase();
    if !lower.starts_with("dia") {
        return None;
    }
    let eq = rest.find('=')?;
    let (per_day, _) = take_brl(rest, eq + 1)?;
    Some((total, divisor, per_day))
}

/// Linha de total: `Total = R$ <valor>`.
fn parse_total_line(line: &str) -> Option<i64> {
    let lower = line.to_lowercase();
    let rest = lower.strip_prefix("total")?;
    let rest = rest.trim_start().strip_prefix('=')?;
    // Reaproveita o offset no texto original (mesmos índices: to_lowercase é 1:1 em ASCII e o
    // prefixo "Total =" da gramática é ASCII).
    let offset = line.len() - rest.len();
    take_brl(line, offset).map(|(cents, _)| cents)
}

/// Linha de item: `[Mensal] R$ <valor> <categoria>`.
fn parse_item_line(line: &str) -> Option<CeremonyItem> {
    let lower = line.to_lowercase();
    let body_start = if lower.starts_with("mensal") {
        "mensal".len()
    } else {
        0
    };
    let body = line[body_start..].trim_start();
    if !body.starts_with("R$") {
        return None;
    }
    let offset = line.len() - body.len();
    let (amount_cents, end) = take_brl(line, offset)?;
    let name = line[end..].trim().trim_start_matches(['-', '–']).trim();
    if name.is_empty() {
        return None;
    }
    Some(CeremonyItem {
        name: name.to_string(),
        amount_cents,
    })
}

/// `per_day` declarado é aceitável para `total ÷ divisor`? Piso, teto e arredondamento comercial
/// do quociente exato são todos aceitos (a nota real arredonda para cima o resto).
fn declared_per_day_ok(total: i64, divisor: u32, declared: i64) -> bool {
    let div = i64::from(divisor);
    let floor = total / div;
    let ceil = (total + div - 1) / div;
    declared == floor || declared == ceil
}

/// Parseia a nota inteira. `None` = a nota não é (ou não fecha como) uma cerimônia de teto.
pub fn parse_ceiling_ceremony(note: &str) -> Option<CeilingCeremony> {
    let mut items: Vec<CeremonyItem> = Vec::new();
    let mut total_line: Option<i64> = None;
    let mut divisor_line: Option<(i64, u32, i64)> = None;

    for raw in note.lines() {
        let line = normalize_line(raw);
        if line.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_divisor_line(&line) {
            // Duas linhas de divisor = nota ambígua; rejeita.
            if divisor_line.replace(parsed).is_some() {
                return None;
            }
        } else if let Some(total) = parse_total_line(&line) {
            if total_line.replace(total).is_some() {
                return None;
            }
        } else if let Some(item) = parse_item_line(&line) {
            items.push(item);
        }
        // Linhas que não casam com a gramática são ignoradas (notas reais carregam ruído).
    }

    let (monthly_total_cents, divisor_days, per_day_cents) = divisor_line?;
    if let Some(total) = total_line
        && total != monthly_total_cents
    {
        return None;
    }
    if !items.is_empty() {
        let sum: i64 = items.iter().map(|i| i.amount_cents).sum();
        if sum != monthly_total_cents {
            return None;
        }
    }
    if !declared_per_day_ok(monthly_total_cents, divisor_days, per_day_cents) {
        return None;
    }
    Some(CeilingCeremony {
        per_day_cents,
        divisor_days,
        monthly_total_cents,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A forma real da nota: itens tab-separados com prefixo "Mensal", linha de total, linha do
    // divisor com "/ 31 Dias" e teto/dia arredondado para cima.
    const REAL_SHAPE: &str = "Mensal\tR$ 300,00\tTransporte\nMensal\tR$ 200,00\tFarm\u{e1}cia\nMensal\tR$ 300,00\tAlimenta\u{e7}\u{e3}o\nMensal\tR$ 200,00\tLazer\nMensal\tR$ 250,00\tCompras\n\nTotal = R$ 1250,00\n\nR$ 1250,00 / 31 Dias = R$ 40,33";

    #[test]
    fn parses_real_shaped_note_with_tabs() {
        let c = parse_ceiling_ceremony(REAL_SHAPE).unwrap();
        assert_eq!(c.per_day_cents, 4_033); // declarado pelo dono (arredonda o resto p/ cima)
        assert_eq!(c.divisor_days, 31);
        assert_eq!(c.monthly_total_cents, 125_000);
        assert_eq!(c.items.len(), 5);
        assert_eq!(c.items[0].name, "Transporte");
        assert_eq!(c.items[0].amount_cents, 30_000);
        assert_eq!(c.items[4].name, "Compras");
    }

    #[test]
    fn parses_with_spaces_and_thousands_dot() {
        let note = "Mensal R$ 1.000,00 Mercado\nMensal R$ 250,00 Lazer\nTotal = R$ 1.250,00\nR$ 1.250,00 / 31 Dias = R$ 40,32";
        let c = parse_ceiling_ceremony(note).unwrap();
        assert_eq!(c.per_day_cents, 4_032); // piso também aceito
        assert_eq!(c.monthly_total_cents, 125_000);
        assert_eq!(c.items.len(), 2);
    }

    #[test]
    fn divisor_line_alone_is_a_valid_ceremony() {
        let c = parse_ceiling_ceremony("R$ 900,00 / 30 dias = R$ 30,00").unwrap();
        assert_eq!(c.per_day_cents, 3_000);
        assert_eq!(c.divisor_days, 30);
        assert!(c.items.is_empty());
    }

    #[test]
    fn rejects_when_items_do_not_sum_to_total() {
        let note = "Mensal R$ 300,00 A\nMensal R$ 100,00 B\nTotal = R$ 500,00\nR$ 500,00 / 31 Dias = R$ 16,12";
        assert!(parse_ceiling_ceremony(note).is_none());
    }

    #[test]
    fn rejects_when_declared_per_day_is_off() {
        // 125000 / 31 = 4032,25… — 40,32 (piso) e 40,33 (teto) fecham; 41,00 não.
        let note = "R$ 1250,00 / 31 Dias = R$ 41,00";
        assert!(parse_ceiling_ceremony(note).is_none());
    }

    #[test]
    fn rejects_when_total_line_disagrees_with_divisor_total() {
        let note = "Total = R$ 900,00\nR$ 1250,00 / 31 Dias = R$ 40,33";
        assert!(parse_ceiling_ceremony(note).is_none());
    }

    #[test]
    fn note_without_divisor_line_is_not_a_ceremony() {
        assert!(parse_ceiling_ceremony("Mensal R$ 300,00 Transporte\nTotal = R$ 300,00").is_none());
        assert!(parse_ceiling_ceremony("CONTAS:\nR$ 120,00 - Energia").is_none());
        assert!(parse_ceiling_ceremony("").is_none());
    }

    #[test]
    fn ignores_noise_lines_around_the_ceremony() {
        let note = "previs\u{e3}o do di\u{e1}rio\nR$ 600,00 / 30 Dias = R$ 20,00\nrevisar em julho";
        let c = parse_ceiling_ceremony(note).unwrap();
        assert_eq!(c.per_day_cents, 2_000);
    }

    #[test]
    fn divisor_keyword_is_case_insensitive() {
        assert!(parse_ceiling_ceremony("R$ 600,00 / 30 DIAS = R$ 20,00").is_some());
        assert!(parse_ceiling_ceremony("R$ 600,00 / 30 dia = R$ 20,00").is_some());
    }

    #[test]
    fn zero_or_missing_values_never_parse() {
        assert!(parse_ceiling_ceremony("R$ 0,00 / 31 Dias = R$ 0,00").is_none());
        assert!(parse_ceiling_ceremony("R$ 600,00 / 0 Dias = R$ 20,00").is_none());
    }
}
