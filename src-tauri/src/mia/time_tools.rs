//! As quatro perguntas de tempo: um mês em detalhe, um ano na régua anual, a projeção à frente
//! e o calendário de caixa.
//!
//! Nenhuma conta nasce aqui. As figuras vêm dos mesmos helpers que as telas Este mês, O ano,
//! Horizonte e Calendário chamam; o que a fachada acrescenta é a diferença já subtraída, a
//! variação já dividida e o recorte impresso ao lado do número — para que quem consome nunca
//! precise de aritmética para ler a resposta.

use super::envelope::{
    Cursor, DataState, Delta, Listing, Page, Period, ToolError, ToolOutput, ToolResult,
};
use super::{Args, insert};
use crate::commands::{
    MonthCoverageDto, MonthGridDayDto, SAVINGS_CEILING_BPS, SAVINGS_FLOOR_BPS, SAVINGS_TARGET_BPS,
    annual_month_end, annual_month_metrics, forecast_dto, month_grid_at, reserve_reading,
};
use crate::forecast::{self, AnnualRuler};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

// --- Um mês em detalhe ------------------------------------------------------------------

#[derive(Serialize)]
struct BucketsDto {
    fixed_out_cents: i64,
    daily_out_cents: i64,
    cartao_cents: i64,
    economia_cents: i64,
    patrimonio_cents: i64,
}

#[derive(Serialize)]
struct MonthDto {
    month: String,
    /// `complete` (mês vivido) · `current` (em curso) · `future` (só o que já foi pré-lançado).
    status: &'static str,
    income_cents: i64,
    /// Fixas + diário realizado + cartão. Economia e Patrimônio ficam de FORA: são dinheiro que
    /// saiu da conta, não o custo de viver.
    cost_of_living_cents: i64,
    cost_of_living_within_income: bool,
    buckets: BucketsDto,
    performance_cents: i64,
    /// Diário que o mês ainda deve gastar até o fim (teto dos dias futuros + pré-lançados).
    daily_projected_cents: i64,
    /// Diário realizado ÷ dias decorridos.
    daily_average_cents: i64,
    /// Economizado% do mês. Serve de série histórica; o veredito da faixa é ANUAL.
    economizado_bps: i64,
}

#[derive(Serialize)]
struct MonthDeltaDto {
    income: Delta,
    cost_of_living: Delta,
    fixed_out: Delta,
    daily_out: Delta,
    cartao: Delta,
    economia: Delta,
    patrimonio: Delta,
    performance: Delta,
    /// Diferença em pontos-base entre os dois Economizado% — percentual não varia sobre
    /// percentual, ele se subtrai.
    economizado_bps: i64,
}

#[derive(Serialize)]
struct DayGridDto {
    date: String,
    income_cents: i64,
    fixed_out_cents: i64,
    daily_out_cents: i64,
    balance_cents: Option<i64>,
}

#[derive(Serialize)]
struct OwnerDto {
    owner_name: String,
    total_cents: i64,
}

pub(crate) async fn month_analysis(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let (year, month) = args
        .month("month")?
        .unwrap_or((today.year(), today.month()));
    let figures = month_figures(pool, year, month, today).await?;

    let mut data = serde_json::to_value(&figures).expect("figuras do mês são serializáveis");
    if let Some((base_year, base_month)) = args.month("compare_to")? {
        let base = month_figures(pool, base_year, base_month, today).await?;
        let delta = MonthDeltaDto {
            income: Delta::between(figures.income_cents, base.income_cents),
            cost_of_living: Delta::between(figures.cost_of_living_cents, base.cost_of_living_cents),
            fixed_out: Delta::between(
                figures.buckets.fixed_out_cents,
                base.buckets.fixed_out_cents,
            ),
            daily_out: Delta::between(
                figures.buckets.daily_out_cents,
                base.buckets.daily_out_cents,
            ),
            cartao: Delta::between(figures.buckets.cartao_cents, base.buckets.cartao_cents),
            economia: Delta::between(figures.buckets.economia_cents, base.buckets.economia_cents),
            patrimonio: Delta::between(
                figures.buckets.patrimonio_cents,
                base.buckets.patrimonio_cents,
            ),
            performance: Delta::between(figures.performance_cents, base.performance_cents),
            economizado_bps: figures.economizado_bps - base.economizado_bps,
        };
        insert(&mut data, "compare_to", base);
        insert(&mut data, "delta", delta);
    }

    if args.wants("days") {
        let grid = month_grid_at(pool, year, month, today)
            .await
            .map_err(ToolError::read_failed)?;
        let days: Vec<DayGridDto> = grid.into_iter().map(day_grid).collect();
        insert(&mut data, "days", Listing::capped(days));
    }

    if args.wants("owners") {
        let owners = crate::splits::owner_totals_for_month(pool, year, month)
            .await
            .map_err(ToolError::read_failed)?;
        let owners: Vec<OwnerDto> = owners
            .into_iter()
            .map(|o| OwnerDto {
                owner_name: o.owner_name,
                total_cents: o.total_cents,
            })
            .collect();
        insert(&mut data, "owners", Listing::capped(owners));
    }

    Ok(ToolOutput {
        period: month_period(year, month)?,
        data,
    })
}

async fn month_figures(
    pool: &SqlitePool,
    year: i32,
    month: u32,
    today: NaiveDate,
) -> Result<MonthDto, ToolError> {
    let m = annual_month_metrics(pool, year, today)
        .await
        .map_err(ToolError::read_failed)?
        .into_iter()
        .find(|m| m.month == month)
        .ok_or_else(|| {
            ToolError::read_failed(format!("o motor não devolveu o mês {year:04}-{month:02}"))
        })?;

    Ok(MonthDto {
        month: format!("{year:04}-{month:02}"),
        status: match (year, month).cmp(&(today.year(), today.month())) {
            std::cmp::Ordering::Less => "complete",
            std::cmp::Ordering::Equal => "current",
            std::cmp::Ordering::Greater => "future",
        },
        income_cents: m.income_cents,
        cost_of_living_cents: m.cost_of_living_cents,
        cost_of_living_within_income: m.cost_of_living_cents <= m.income_cents,
        buckets: BucketsDto {
            fixed_out_cents: m.fixed_out_cents,
            daily_out_cents: m.daily_out_cents,
            cartao_cents: m.cartao_cents,
            economia_cents: m.economia_cents,
            patrimonio_cents: m.patrimonio_cents,
        },
        performance_cents: m.performance_cents,
        daily_projected_cents: m.daily_projected_cents,
        daily_average_cents: m.real_daily_avg_cents,
        economizado_bps: m.savings_rate_bps,
    })
}

// --- Um ano na régua anual --------------------------------------------------------------

#[derive(Serialize)]
struct BandDto {
    floor_bps: i64,
    target_bps: i64,
    ceiling_bps: i64,
}

#[derive(Serialize)]
struct EconomizadoDto {
    /// `verdict` (a régua fecha o ano) · `estimate` (há mês sem lastro, o número vale pelo
    /// vivido) · `no_record` (não há renda registrada para dividir).
    state: DataState,
    bps: Option<i64>,
    /// `lived` (recorte dos meses vividos) · `year` (o ano inteiro).
    scope: &'static str,
    lived_bps: Option<i64>,
    projected_bps: Option<i64>,
    verdict: &'static str,
    band: BandDto,
}

#[derive(Serialize)]
struct YearDto {
    year: i32,
    is_current_year: bool,
    lived_months: u32,
    future_months: u32,
    income_lived_cents: i64,
    economia_lived_cents: i64,
    /// A sobra dos meses vividos — o colchão, distinto da Economia registrada.
    surplus_lived_cents: i64,
    income_year_cents: i64,
    economia_year_cents: i64,
    /// Meses vividos com renda registrada e a média por mês deles — a leitura que compara um ano
    /// com outro sem que o calendário finja uma queda de renda.
    recorded_months: u32,
    avg_income_cents: i64,
    /// Mediana das saídas dos meses vividos: o mês típico contra o qual o futuro é medido.
    typical_spend_cents: i64,
    /// Meses à frente cuja saída lançada não alcança o piso de lastro.
    suspect_months: Vec<u32>,
    economizado: EconomizadoDto,
    /// Quanto falta para o piso de 20% no ano. Negativo = o piso já foi passado.
    shortfall_to_floor_cents: i64,
    /// A falta anual dividida pelos meses que restam; nulo em ano sem futuro.
    per_month_shortfall_cents: Option<i64>,
}

#[derive(Serialize)]
struct YearMonthDto {
    month: String,
    income_cents: i64,
    outflow_cents: i64,
    economia_cents: i64,
    performance_cents: i64,
    economizado_bps: i64,
    lived: bool,
    /// Mês à frente sem lastro: tem lançamento, mas abaixo do piso do gasto típico.
    suspect: bool,
}

pub(crate) async fn year_analysis(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let year = args.year("year")?.unwrap_or(today.year());
    let reserve = reserve_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let reserve_months = (reserve.state != "no_record").then_some(reserve.months);

    let metrics = annual_month_metrics(pool, year, today)
        .await
        .map_err(ToolError::read_failed)?;
    let ruler = forecast::annual_ruler(&metrics, year, today);
    let figures = year_dto(year, today, &ruler, reserve_months);

    let mut data = serde_json::to_value(&figures).expect("figuras do ano são serializáveis");

    if let Some(base_year) = args.year("compare_to")? {
        let base_metrics = annual_month_metrics(pool, base_year, today)
            .await
            .map_err(ToolError::read_failed)?;
        let base_ruler = forecast::annual_ruler(&base_metrics, base_year, today);
        let base = year_dto(base_year, today, &base_ruler, reserve_months);
        let delta = json!({
            // A comparação entre anos é de RENDA MÉDIA por mês com registro, nunca de totais: um
            // ano em curso contra um ano fechado acusaria uma queda que é só o calendário.
            "avg_income": Delta::between(ruler.avg_income_cents, base_ruler.avg_income_cents),
            // Sem régua de um dos lados não há diferença de percentual — nula, nunca zero.
            "economizado_bps": match (ruler.bps, base_ruler.bps) {
                (Some(now), Some(before)) => Some(now - before),
                _ => None,
            },
        });
        insert(&mut data, "compare_to", base);
        insert(&mut data, "delta", delta);
    }

    if args.wants("months") {
        let months: Vec<YearMonthDto> = (1..=12)
            .map(|month| {
                let m = metrics.iter().find(|m| m.month == month);
                YearMonthDto {
                    month: format!("{year:04}-{month:02}"),
                    income_cents: m.map_or(0, |m| m.income_cents),
                    outflow_cents: m.map_or(0, |m| m.income_cents - m.performance_cents),
                    economia_cents: m.map_or(0, |m| m.economia_cents),
                    performance_cents: m.map_or(0, |m| m.performance_cents),
                    economizado_bps: m.map_or(0, |m| m.savings_rate_bps),
                    lived: ruler.months.iter().any(|m| m.month == month && m.lived),
                    suspect: ruler.months.iter().any(|m| m.month == month && m.suspect),
                }
            })
            .collect();
        insert(&mut data, "months", Listing::capped(months));
    }

    if args.wants("year_end") {
        insert(
            &mut data,
            "year_end",
            year_end(pool, year, &ruler, today).await?,
        );
    }

    Ok(ToolOutput {
        period: Period::between(
            NaiveDate::from_ymd_opt(year, 1, 1).ok_or_else(|| invalid_year(year))?,
            NaiveDate::from_ymd_opt(year, 12, 31).ok_or_else(|| invalid_year(year))?,
        ),
        data,
    })
}

fn year_dto(
    year: i32,
    today: NaiveDate,
    ruler: &AnnualRuler,
    reserve_months: Option<f64>,
) -> YearDto {
    YearDto {
        year,
        is_current_year: year == today.year(),
        lived_months: ruler.lived_months,
        future_months: ruler.future_months,
        income_lived_cents: ruler.income_lived_cents,
        economia_lived_cents: ruler.economia_lived_cents,
        surplus_lived_cents: ruler.surplus_lived_cents,
        income_year_cents: ruler.income_year_cents,
        economia_year_cents: ruler.economia_year_cents,
        recorded_months: ruler.recorded_months,
        avg_income_cents: ruler.avg_income_cents,
        typical_spend_cents: ruler.typical_spend_cents,
        suspect_months: ruler.suspect_months(),
        economizado: EconomizadoDto {
            state: match (ruler.bps, ruler.scope_lived) {
                (None, _) => DataState::NoRecord,
                (Some(_), true) => DataState::Estimate,
                (Some(_), false) => DataState::Verdict,
            },
            bps: ruler.bps,
            scope: if ruler.scope_lived { "lived" } else { "year" },
            lived_bps: ruler.lived_bps,
            projected_bps: ruler.projected_bps,
            verdict: forecast::band_verdict(ruler, reserve_months).as_str(),
            band: BandDto {
                floor_bps: SAVINGS_FLOOR_BPS,
                target_bps: SAVINGS_TARGET_BPS,
                ceiling_bps: SAVINGS_CEILING_BPS,
            },
        },
        shortfall_to_floor_cents: ruler.shortfall_year_cents,
        per_month_shortfall_cents: ruler.per_month_shortfall_cents,
    }
}

/// Onde o ano termina: o saldo do último mês com projeção e o cenário em que cada mês sem
/// lastro custasse o típico — a mesma leitura que a tela do ano publica, do mesmo motor.
async fn year_end(
    pool: &SqlitePool,
    year: i32,
    ruler: &AnnualRuler,
    today: NaiveDate,
) -> Result<Value, ToolError> {
    let month_end = annual_month_end(pool, year, today)
        .await
        .map_err(ToolError::read_failed)?;
    let end = forecast::year_end_scenario(ruler, &month_end);

    let Some(end_month) = end.end_month else {
        return Ok(json!({ "end_month": Value::Null, "end_balance_cents": Value::Null }));
    };
    Ok(json!({
        "end_month": format!("{year:04}-{end_month:02}"),
        "end_balance_cents": end.end_balance_cents,
        "end_balance_typical_cents": end.end_balance_typical_cents,
    }))
}

// --- A projeção à frente ----------------------------------------------------------------

#[derive(Serialize)]
struct MonthEndDto {
    month: String,
    balance_cents: i64,
}

#[derive(Serialize)]
struct ScenarioMonthEndDto {
    month: String,
    balance_cents: i64,
    delta_cents: i64,
}

/// Um dia e o saldo que ele deixa — a linha da projeção e o ponto que a resposta destaca são a
/// mesma leitura, então são o mesmo tipo.
#[derive(Serialize, Clone)]
struct DayPointDto {
    date: String,
    balance_cents: i64,
}

/// Cobertura de um mês futuro no vocabulário do envelope. Vive aqui, e não em cada ferramenta,
/// porque a leitura é uma só: a projeção do mês é crível ou não é.
#[derive(Serialize)]
pub(super) struct CoverageDto {
    month: String,
    coverage_bps: i64,
    is_complete: bool,
    estimated_missing_cents: i64,
}

pub(super) fn coverage_listing<'a>(
    rows: impl Iterator<Item = &'a MonthCoverageDto>,
) -> Listing<CoverageDto> {
    Listing::capped(
        rows.map(|c| CoverageDto {
            month: format!("{:04}-{:02}", c.year, c.month),
            coverage_bps: c.coverage_bps,
            is_complete: c.is_complete,
            estimated_missing_cents: c.estimated_missing_cents,
        })
        .collect(),
    )
}

pub(crate) async fn forecast(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let dto = forecast_dto(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let horizon_end = iso(&dto.horizon_end)?;

    let (start, end) = match args.range("range")? {
        None => (today, horizon_end),
        Some((start, end)) => {
            if end < today {
                return Err(ToolError::new(
                    super::ErrorCode::InvalidArgument,
                    "A projeção começa hoje, e o recorte pedido terminou antes disso.",
                    "Para um mês já vivido use get_month_analysis; para o saldo dia a dia do \
                     passado, get_cashflow_calendar.",
                ));
            }
            if start > horizon_end {
                return Err(ToolError::new(
                    super::ErrorCode::InvalidArgument,
                    format!("A projeção vai até {}.", dto.horizon_end),
                    format!(
                        "Peça um recorte que comece até {} — mais à frente que isso não há nada \
                         lançado para projetar.",
                        dto.horizon_end
                    ),
                ));
            }
            (start.max(today), end.min(horizon_end))
        }
    };

    let daily: Vec<DayPointDto> = dto
        .daily
        .iter()
        .filter_map(|d| {
            let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok()?;
            (date >= start && date <= end).then(|| DayPointDto {
                date: d.date.clone(),
                balance_cents: d.balance_cents,
            })
        })
        .collect();
    let lowest = daily.iter().min_by_key(|d| d.balance_cents).cloned();
    let month_end: Vec<MonthEndDto> = dto
        .month_end
        .iter()
        .filter(|m| in_range(m.year, m.month, start, end))
        .map(|m| MonthEndDto {
            month: format!("{:04}-{:02}", m.year, m.month),
            balance_cents: m.balance_cents,
        })
        .collect();

    let mut data = json!({
        "today": dto.today,
        "horizon_end": dto.horizon_end,
        "safe_to_spend_today_cents": dto.safe_to_spend_today_cents,
        "binding": dto.binding_guardrail,
        "cash_headroom_cents": dto.cash_headroom_cents,
        "savings_headroom_cents": dto.savings_headroom_cents,
        "end_balance_cents": daily.last().map(|d| d.balance_cents),
        "lowest_balance": lowest,
        // O fundo do poço do horizonte INTEIRO, que é o que a régua de caixa usa — separado do
        // menor saldo do recorte, para que a resposta nunca troque um pelo outro.
        "horizon_lowest_balance": dto.deepest_deficit.as_ref().map(|p| DayPointDto {
            date: p.date.clone(),
            balance_cents: p.balance_cents,
        }),
        "typical_spend_cents": dto.baseline_outflow_cents,
        "trusted_through_month": dto.trusted_through_month,
        "total_missing_cents": dto.total_missing_cents,
        "month_end": Listing::capped(month_end),
    });

    if let Some(scenario_id) = args.text("scenario_id")? {
        insert(
            &mut data,
            "scenario",
            scenario_block(pool, scenario_id, start, end, today).await?,
        );
    }

    if args.wants("daily") {
        insert(&mut data, "daily", Listing::capped(daily));
    }

    if args.wants("coverage") {
        let inside = dto
            .coverage
            .iter()
            .filter(|c| in_range(c.year, c.month, start, end));
        insert(&mut data, "coverage", coverage_listing(inside));
    }

    Ok(ToolOutput {
        period: Period::between(start, end),
        data,
    })
}

/// A hipótese guardada contra o mundo real, com as diferenças já feitas. O cenário é lido, nunca
/// criado: esta ferramenta projeta o que já existe.
async fn scenario_block(
    pool: &SqlitePool,
    scenario_id: &str,
    start: NaiveDate,
    end: NaiveDate,
    today: NaiveDate,
) -> Result<Value, ToolError> {
    let exists: Option<(String,)> = sqlx::query_as("SELECT name FROM scenario WHERE id = ?1")
        .bind(scenario_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ToolError::read_failed(format!("cenário: {e}")))?;
    let Some((name,)) = exists else {
        return Err(ToolError::new(
            super::ErrorCode::NotFound,
            format!("Não existe o cenário \"{scenario_id}\"."),
            "Chame de novo sem scenario_id para a projeção real.".to_string(),
        ));
    };

    let compare = crate::scenarios::get_scenario_forecast_inner(pool, scenario_id, today)
        .await
        .map_err(ToolError::read_failed)?;
    let month_end: Vec<ScenarioMonthEndDto> = compare
        .month_end
        .iter()
        .filter(|m| in_range(m.year, m.month, start, end))
        .map(|m| ScenarioMonthEndDto {
            month: format!("{:04}-{:02}", m.year, m.month),
            balance_cents: m.scenario_balance_cents,
            delta_cents: m.scenario_balance_cents - m.real_balance_cents,
        })
        .collect();

    Ok(json!({
        "id": scenario_id,
        "name": name,
        "month_end": Listing::capped(month_end),
        "performance_delta_cents": compare.performance_delta_cents,
        "safe_to_spend_delta_cents": compare.safe_to_spend_delta_cents,
        "cost_of_living_delta_cents": compare.cost_of_living_delta_cents,
        "deepest_deficit_delta_cents": compare.deepest_deficit_delta_cents,
    }))
}

// --- O calendário de caixa --------------------------------------------------------------

#[derive(Serialize)]
struct CalendarDayDto {
    date: String,
    income_cents: i64,
    fixed_out_cents: i64,
    daily_out_cents: i64,
    /// Passo da corrente contra a véspera; nulo quando qualquer uma das duas pontas não tem
    /// saldo conhecido. É o movimento que enxerga tudo — inclusive uma Economia, que as três
    /// colunas da planilha não imprimem.
    movement_cents: Option<i64>,
    balance_cents: Option<i64>,
    is_future: bool,
}

pub(crate) async fn cashflow_calendar(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let (start, end) = match args.range("range")? {
        Some(range) => range,
        None => {
            let period = month_period(today.year(), today.month())?;
            (iso(&period.start)?, iso(&period.end)?)
        }
    };

    // A véspera do primeiro dia entra na corrente (e sai da resposta): sem ela o movimento do
    // primeiro dia seria nulo por construção, não por falta de dado.
    let eve = start.pred_opt().unwrap_or(start);
    let chain = chain_of(pool, eve, end, today).await?;

    let days: Vec<CalendarDayDto> = chain
        .iter()
        .filter(|day| day.date >= start)
        .map(|day| CalendarDayDto {
            date: day.date.format("%Y-%m-%d").to_string(),
            income_cents: day.income_cents,
            fixed_out_cents: day.fixed_out_cents,
            daily_out_cents: day.daily_out_cents,
            movement_cents: match (day.balance_cents, day.previous_balance_cents) {
                (Some(now), Some(before)) => Some(now - before),
                _ => None,
            },
            balance_cents: day.balance_cents,
            is_future: day.date > today,
        })
        .collect();

    // Os agregados cobrem o RECORTE inteiro; a paginação corta só a lista.
    let totals = json!({
        "income_cents": days.iter().map(|d| d.income_cents).sum::<i64>(),
        "fixed_out_cents": days.iter().map(|d| d.fixed_out_cents).sum::<i64>(),
        "daily_out_cents": days.iter().map(|d| d.daily_out_cents).sum::<i64>(),
    });
    let lowest = days
        .iter()
        .filter_map(|d| d.balance_cents.map(|b| (d.date.clone(), b)))
        .min_by_key(|(_, balance)| *balance)
        .map(|(date, balance_cents)| DayPointDto {
            date,
            balance_cents,
        });

    let scope = format!("{start}..{end}");
    let offset = match args.text("cursor")? {
        None => 0,
        Some(raw) => {
            let from = Cursor::decode(raw, &scope)?;
            // Cursor cuja âncora sumiu do recorte é recusado, nunca reiniciado em silêncio: o
            // modelo leria a primeira página como se fosse a segunda.
            days.iter()
                .position(|d| d.date == from)
                .ok_or_else(Cursor::refused)?
        }
    };
    let page = Page::from(days, offset, &scope, |d| d.date.clone());

    Ok(ToolOutput {
        period: Period::between(start, end),
        data: json!({
            "days": page,
            "totals": totals,
            "lowest_balance": lowest,
        }),
    })
}

/// Um dia da corrente, com o saldo da véspera ao lado para o movimento sair pronto.
struct ChainDay {
    date: NaiveDate,
    income_cents: i64,
    fixed_out_cents: i64,
    daily_out_cents: i64,
    balance_cents: Option<i64>,
    previous_balance_cents: Option<i64>,
}

/// A costura das duas correntes: até a véspera de hoje vale o realizado da planilha; de hoje em
/// diante, a projeção. É a mesma emenda que o calendário da tela faz — sem ela, o passado
/// apareceria projetado e o futuro, vazio.
async fn chain_of(
    pool: &SqlitePool,
    start: NaiveDate,
    end: NaiveDate,
    today: NaiveDate,
) -> Result<Vec<ChainDay>, ToolError> {
    // Cada corrente só é carregada se o recorte a alcança: recorte todo no futuro não lê a
    // planilha, recorte todo no passado não roda a projeção.
    let mut realized: std::collections::HashMap<String, MonthGridDayDto> =
        std::collections::HashMap::new();
    let mut month = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap_or(start);
    while month <= end && month < today {
        for day in month_grid_at(pool, month.year(), month.month(), today)
            .await
            .map_err(ToolError::read_failed)?
        {
            realized.insert(day.date.clone(), day);
        }
        month = month + chrono::Months::new(1);
    }

    let mut projected: std::collections::HashMap<String, crate::commands::ForecastDayDto> =
        std::collections::HashMap::new();
    if end >= today {
        projected = forecast_dto(pool, today)
            .await
            .map_err(ToolError::read_failed)?
            .daily
            .into_iter()
            .map(|d| (d.date.clone(), d))
            .collect();
    }

    let mut chain = Vec::new();
    let mut previous_balance_cents = None;
    let mut date = start;
    while date <= end {
        let iso = date.format("%Y-%m-%d").to_string();
        let day = if date < today {
            realized.get(&iso).map(|d| {
                (
                    d.income_cents,
                    d.fixed_out_cents,
                    d.daily_out_cents,
                    d.balance_cents,
                )
            })
        } else {
            projected.get(&iso).map(|d| {
                (
                    d.income_cents,
                    d.fixed_out_cents,
                    d.daily_out_cents,
                    Some(d.balance_cents),
                )
            })
        };
        let (income_cents, fixed_out_cents, daily_out_cents, balance_cents) =
            day.unwrap_or((0, 0, 0, None));
        chain.push(ChainDay {
            date,
            income_cents,
            fixed_out_cents,
            daily_out_cents,
            balance_cents,
            previous_balance_cents,
        });
        previous_balance_cents = balance_cents;
        date = match date.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    Ok(chain)
}

// --- Costura ----------------------------------------------------------------------------

fn day_grid(day: MonthGridDayDto) -> DayGridDto {
    DayGridDto {
        date: day.date,
        income_cents: day.income_cents,
        fixed_out_cents: day.fixed_out_cents,
        daily_out_cents: day.daily_out_cents,
        balance_cents: day.balance_cents,
    }
}

fn month_period(year: i32, month: u32) -> Result<Period, ToolError> {
    let start = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        ToolError::new(
            super::ErrorCode::InvalidArgument,
            format!("Não existe o mês {year:04}-{month:02}."),
            "Chame de novo com um mês entre 01 e 12.".to_string(),
        )
    })?;
    Ok(Period::between(
        start,
        forecast::last_day_of_month(year, month),
    ))
}

fn invalid_year(year: i32) -> ToolError {
    ToolError::new(
        super::ErrorCode::InvalidArgument,
        format!("Não existe o ano {year}."),
        "Chame de novo com um ano de quatro dígitos, como 2026.".to_string(),
    )
}

fn iso(date: &str) -> Result<NaiveDate, ToolError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| ToolError::read_failed(format!("data do motor ({date}): {e}")))
}

/// O mês (ano+mês) cai dentro do recorte de dias?
fn in_range(year: i32, month: u32, start: NaiveDate, end: NaiveDate) -> bool {
    (year, month) >= (start.year(), start.month()) && (year, month) <= (end.year(), end.month())
}
