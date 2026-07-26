//! A hipótese que não é gravada.
//!
//! O cenário salvo tem tela, nome e linhas no banco. A conversa precisa da pergunta mais barata
//! que existe — "e se?" — sem cobrar da pessoa o gesto de criar, nomear e depois apagar um
//! cenário. Aqui as linhas nascem em memória, passam pelo MESMO motor que projeta o cenário
//! salvo e morrem com a resposta.

use super::envelope::{ErrorCode, Listing, Period, ToolError, ToolOutput, ToolResult};
use super::ledger_tools::MOVEMENTS;
use super::{Args, insert};
use crate::scenarios::{HypotheticalLine, simulate_hypothesis};
use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// Uma linha da hipótese como a resposta a devolve — o eco do que foi simulado, para que a
/// resposta possa citar cada linha em vez de resumir o conjunto.
#[derive(Serialize)]
struct LineDto {
    movement: &'static str,
    amount_cents: i64,
    date: String,
    description: Option<String>,
}

/// O retrato de um dos dois mundos. Os mesmos campos dos dois lados: a diferença entre eles é
/// leitura direta, sem que ninguém precise casar nomes diferentes.
#[derive(Serialize)]
struct BranchDto {
    safe_to_spend_today_cents: i64,
    binding: String,
    performance_cents: i64,
    cost_of_living_cents: i64,
    lowest_balance: Option<PointDto>,
}

#[derive(Serialize)]
struct PointDto {
    date: String,
    balance_cents: i64,
}

#[derive(Serialize)]
struct MonthEndDto {
    month: String,
    real_balance_cents: i64,
    hypothesis_balance_cents: i64,
    delta_cents: i64,
}

pub(crate) async fn simulate_scenario(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let changes = args.objects("changes")?;
    if changes.is_empty() {
        return Err(ToolError::new(
            ErrorCode::InvalidArgument,
            "Uma simulação precisa de pelo menos uma mudança.",
            format!(
                "Chame de novo com changes: [{{\"movement\": \"saida\", \"amount_cents\": 50000, \
                 \"date\": \"{}\"}}] — movement em: {}.",
                today.format("%Y-%m-%d"),
                MOVEMENTS.join(", ")
            ),
        ));
    }

    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or_else(|| ToolError::read_failed("mês corrente inválido"))?;
    let horizon_end = today
        .checked_add_months(Months::new(MAX_HYPOTHESIS_HORIZON_YEARS * 12))
        .ok_or_else(|| ToolError::read_failed("horizonte máximo da hipótese inválido"))?;
    let mut planned: Vec<PlannedLine> = Vec::new();
    for (index, change) in changes.iter().enumerate() {
        let change = Change::parse(change, index, month_start, horizon_end)?;
        for date in change.dates()? {
            planned.push(PlannedLine {
                movement: change.movement,
                line: movement_line(change.movement, change.amount_cents, date),
                description: change.description.map(str::to_string),
            });
        }
        // O teto de linhas vale também na ENTRADA: uma série de quarenta anos custa quarenta anos
        // de projeção, e o modelo que a pediu não sabe o preço.
        if planned.len() > super::envelope::MAX_ROWS {
            return Err(ToolError::new(
                ErrorCode::InvalidArgument,
                format!(
                    "A hipótese passa de {} linhas depois de repetir os meses.",
                    super::envelope::MAX_ROWS
                ),
                "Chame de novo com menos mudanças ou um repeat_months menor.".to_string(),
            ));
        }
    }

    let hypothesis: Vec<HypotheticalLine> = planned.iter().map(|p| p.line.clone()).collect();
    let comparison = simulate_hypothesis(pool, &hypothesis, today)
        .await
        .map_err(ToolError::read_failed)?;

    let last_date = planned
        .iter()
        .map(|p| p.line.date)
        .max()
        .unwrap_or(comparison.real_horizon_end);
    let echo: Vec<LineDto> = planned
        .iter()
        .map(|p| LineDto {
            movement: p.movement,
            amount_cents: p.line.amount_cents,
            date: p.line.date.format("%Y-%m-%d").to_string(),
            description: p.description.clone(),
        })
        .collect();

    let mut data = json!({
        // A hipótese não deixa rastro: nenhuma linha é gravada, e por isso não há o que apagar
        // depois. Simular não muda o histórico de ninguém.
        "ephemeral": true,
        "horizon_end": comparison.real_horizon_end.format("%Y-%m-%d").to_string(),
        "lines": Listing::capped(echo),
        "baseline": BranchDto {
            safe_to_spend_today_cents: comparison.real_safe_to_spend_today_cents,
            binding: comparison.real_binding_guardrail.clone(),
            performance_cents: comparison.real_performance_cents,
            cost_of_living_cents: comparison.real_cost_of_living_cents,
            lowest_balance: comparison.real_deepest_deficit.as_ref().map(point),
        },
        "hypothesis": BranchDto {
            safe_to_spend_today_cents: comparison.scenario_safe_to_spend_today_cents,
            binding: comparison.scenario_binding_guardrail.clone(),
            performance_cents: comparison.scenario_performance_cents,
            cost_of_living_cents: comparison.scenario_cost_of_living_cents,
            lowest_balance: comparison.scenario_deepest_deficit.as_ref().map(point),
        },
        // As diferenças já subtraídas: o que a hipótese custa é a pergunta, e a conta não pode
        // ficar para quem lê.
        "delta": {
            "safe_to_spend_cents": comparison.safe_to_spend_delta_cents,
            "performance_cents": comparison.performance_delta_cents,
            "cost_of_living_cents": comparison.cost_of_living_delta_cents,
            "lowest_balance_cents": comparison.deepest_deficit_delta_cents,
        },
    });

    if args.wants("month_end") {
        let month_end: Vec<MonthEndDto> = comparison
            .month_end
            .iter()
            .map(|m| MonthEndDto {
                month: format!("{:04}-{:02}", m.year, m.month),
                real_balance_cents: m.real_balance_cents,
                hypothesis_balance_cents: m.scenario_balance_cents,
                delta_cents: m.delta_cents,
            })
            .collect();
        insert(&mut data, "month_end", Listing::capped(month_end));
    }

    Ok(ToolOutput {
        // O recorte cobre o que foi simulado, mesmo quando a série passa do horizonte que o mundo
        // real alcança — a projeção estica até a última linha da hipótese.
        period: Period::between(today, comparison.real_horizon_end.max(last_date)),
        data,
    })
}

/// Uma linha já materializada: o tipo do método que a originou, a linha que o motor projeta e o
/// rótulo que a resposta cita.
struct PlannedLine {
    movement: &'static str,
    line: HypotheticalLine,
    description: Option<String>,
}

/// Uma mudança como o argumento a declara, já validada.
struct Change<'a> {
    movement: &'static str,
    amount_cents: i64,
    date: NaiveDate,
    repeat_months: u32,
    description: Option<&'a str>,
}

impl<'a> Change<'a> {
    fn parse(
        object: &'a serde_json::Map<String, Value>,
        index: usize,
        month_start: NaiveDate,
        horizon_end: NaiveDate,
    ) -> Result<Self, ToolError> {
        let position = index + 1;
        let refuse = |what: &str, fix: String| {
            ToolError::new(
                ErrorCode::InvalidArgument,
                format!("A mudança #{position} {what}"),
                fix,
            )
        };
        for key in object.keys() {
            if !CHANGE_FIELDS.contains(&key.as_str()) {
                return Err(refuse(
                    &format!("traz o campo \"{key}\", que não existe."),
                    format!("Use só: {}.", CHANGE_FIELDS.join(", ")),
                ));
            }
        }

        let movement = object
            .get("movement")
            .and_then(Value::as_str)
            .and_then(|raw| MOVEMENTS.iter().find(|m| **m == raw).copied())
            .ok_or_else(|| {
                refuse(
                    "não diz em que régua entra.",
                    format!("Use movement em: {}.", MOVEMENTS.join(", ")),
                )
            })?;

        // Magnitude, como no livro-razão: o sinal vem do tipo, nunca do número. Um valor negativo
        // aqui viraria uma saída que soma — o oposto do que quem perguntou quis dizer.
        let amount_cents = object
            .get("amount_cents")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                refuse(
                    "não traz um valor positivo em centavos.",
                    "Use amount_cents: 50000 (R$ 500,00) — o sinal vem do movement.".to_string(),
                )
            })?;
        if amount_cents <= 0 {
            return Err(refuse(
                "não traz um valor positivo em centavos.",
                "Use amount_cents: 50000 (R$ 500,00) — o sinal vem do movement.".to_string(),
            ));
        }
        if amount_cents > MAX_HYPOTHESIS_AMOUNT_CENTS {
            return Err(refuse(
                "passa do teto de R$ 1.000.000.000,00 por linha da hipótese.",
                "Use amount_cents até R$ 1.000.000.000,00; esse teto mantém a soma dos eventos na faixa do motor."
                    .to_string(),
            ));
        }

        let date = object
            .get("date")
            .and_then(Value::as_str)
            .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                refuse(
                    "não traz uma data em YYYY-MM-DD.",
                    format!("Use date: \"{}\".", month_start.format("%Y-%m-%d")),
                )
            })?;
        // A projeção começa no mês corrente: uma data anterior seria inerte, e devolver "nada
        // mudou" para uma hipótese que a pessoa levou a sério é pior que recusar.
        if date < month_start {
            return Err(refuse(
                "cai antes do mês corrente, onde a projeção não alcança.",
                format!(
                    "Use uma data a partir de {} — o passado se lê em get_month_analysis.",
                    month_start.format("%Y-%m-%d")
                ),
            ));
        }
        if date > horizon_end {
            return Err(refuse(
                format!(
                    "passa do horizonte máximo da hipótese, que termina em {}.",
                    horizon_end.format("%Y-%m-%d")
                )
                .as_str(),
                format!(
                    "Use uma data até {} — uma hipótese cobre no máximo dez anos.",
                    horizon_end.format("%Y-%m-%d")
                ),
            ));
        }

        let repeat_months = match object.get("repeat_months") {
            None | Some(Value::Null) => 1,
            Some(raw) => raw
                .as_u64()
                .filter(|n| (1..=MAX_REPEAT).contains(n))
                .ok_or_else(|| {
                    refuse(
                        "repete um número de meses que não existe.",
                        format!("Use repeat_months entre 1 e {MAX_REPEAT}."),
                    )
                })? as u32,
        };

        let last_date = date
            .checked_add_months(Months::new(repeat_months - 1))
            .ok_or_else(|| {
                refuse(
                    "repete até uma data que o calendário não alcança.",
                    "Chame de novo com um repeat_months menor.".to_string(),
                )
            })?;
        if last_date > horizon_end {
            return Err(refuse(
                format!(
                    "se estende até {}, além do horizonte máximo que termina em {}.",
                    last_date.format("%Y-%m-%d"),
                    horizon_end.format("%Y-%m-%d")
                )
                .as_str(),
                format!(
                    "Use uma série cuja última data seja até {}.",
                    horizon_end.format("%Y-%m-%d")
                ),
            ));
        }

        let description = match object.get("description") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_str().ok_or_else(|| {
                refuse(
                    "traz description com um tipo que não é texto.",
                    "Use description: \"rótulo da mudança\" ou omita description.".to_string(),
                )
            })?),
        };

        Ok(Self {
            movement,
            amount_cents,
            date,
            repeat_months,
            description,
        })
    }

    /// As datas da série. A repetição é mensal e a conta é da ferramenta: pedir ao modelo que
    /// some doze meses a uma data é pedir aritmética, e é justamente isso que a fachada evita.
    fn dates(&self) -> Result<Vec<NaiveDate>, ToolError> {
        (0..self.repeat_months)
            .map(|step| {
                self.date
                    .checked_add_months(Months::new(step))
                    .ok_or_else(|| {
                        ToolError::new(
                            ErrorCode::InvalidArgument,
                            "A repetição passa da última data que o calendário alcança.",
                            "Chame de novo com um repeat_months menor.".to_string(),
                        )
                    })
            })
            .collect()
    }
}

const CHANGE_FIELDS: &[&str] = &[
    "movement",
    "amount_cents",
    "date",
    "repeat_months",
    "description",
];

/// Teto da repetição: quarenta anos de série. O teto real é o de linhas por chamada, que corta
/// bem antes; este existe para que um número absurdo seja recusado com nome, não com estouro.
const MAX_REPEAT: u64 = 480;

/// O custo da projeção cresce com o horizonte; uma hipótese testa uma decisão, não um plano de
/// século.
const MAX_HYPOTHESIS_HORIZON_YEARS: u32 = 10;

/// O teto por linha mantém a soma dos eventos dentro da faixa do inteiro que o motor de projeção
/// usa.
const MAX_HYPOTHESIS_AMOUNT_CENTS: i64 = 100_000_000_000;

/// O tipo do método vira a linha que o motor classifica de volta nele — a inversa exata de
/// `forecast::classify`. Escrever a inversa aqui é o que mantém o vocabulário único: quem leu
/// "cartao" numa busca escreve "cartao" numa hipótese.
fn movement_line(movement: &str, amount_cents: i64, date: NaiveDate) -> HypotheticalLine {
    let (kind, is_fixed, payment_method, to_liquidity) = match movement {
        "entrada" => ("income", false, None, None),
        "saida" => ("expense", true, Some("debit"), None),
        "diario" => ("expense", false, Some("debit"), None),
        "cartao" => ("expense", false, Some("credit"), None),
        "economia" => ("transfer", false, None, Some("reserve")),
        // O vocabulário é fechado na entrada; sobra o Patrimônio.
        _ => ("transfer", false, None, Some("illiquid")),
    };
    HypotheticalLine {
        kind,
        amount_cents,
        date,
        payment_method: payment_method.map(str::to_string),
        is_fixed,
        to_liquidity: to_liquidity.map(str::to_string),
    }
}

fn point(p: &crate::commands::DayPointDto) -> PointDto {
    PointDto {
        date: p.date.clone(),
        balance_cents: p.balance_cents,
    }
}

/// A última data da hipótese — o recorte da resposta cobre o que foi simulado, mesmo quando a
/// série passa do horizonte que o mundo real alcança.
fn last_date(lines: &[(&'static str, HypotheticalLine, Option<String>)]) -> NaiveDate {
    lines
        .iter()
        .map(|(_, line, _)| line.date)
        .max()
        .unwrap_or(NaiveDate::MIN)
}
