use super::super::layout_detect::{SheetLayout, month_number_from_name};
use sqlx::SqlitePool;

use super::classify::{RowKind, classify_row};
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
pub(crate) fn cell_description(
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
pub(crate) fn cell_raw_note(notes: &[Vec<String>], row: usize, col: usize) -> String {
    notes
        .get(row)
        .and_then(|nr| nr.get(col))
        .map(String::as_str)
        .unwrap_or("")
        .to_string()
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
}
