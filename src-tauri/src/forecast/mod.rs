//! Forecast core — projected running balance ("saldo projetado", the methodology's heart).
//!
//! Pure functional core: NO IO, NO ambient clock, NO DB. Every input (seed, events, horizon,
//! "today") arrives as an argument, so the engine is deterministic and trivially testable. The
//! imperative shell (`commands.rs`) loads/maps rows and supplies the seed. See
//! `specs/003-forecast-core/`.
//!
//! The pure engine is complete (daily chain, month-end, deepest deficit, safe-to-spend, monthly
//! Totais). Remaining slice work is in the shell: wire the row→event mapping + seed into
//! `get_dashboard_summary` (Phase 7) and add a demo fixture (Phase 8).

use chrono::{Datelike, NaiveDate};

/// A dated cash-flow event in the projection. Amounts are always positive; the sign is implied by
/// `kind`. `realized = false` marks a future projection (vs a realized transaction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Entrada — income (salary, reimbursement, freela…).
    Income,
    /// Saída — fixed outflow: fixed bills + the fatura lump at the card due date (credit settles as one lump, not per-purchase).
    FixedOut,
    /// Diário — variable daily débito/cash spend (Régua 1).
    Daily,
    /// Economia — guardar (transfer to real savings: reserve, ou illiquid p/ FGTS/previdência).
    /// NÃO inclui `restricted` (vale-refeição = gasto restrito). Leaves the spending balance
    /// (signed −, mirroring the app's single conta), but is "saved" not "spent": it is the
    /// numerator of Economizado% and a term of Performance, yet is NOT part of Custo de vida.
    Economia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashflowEvent {
    pub date: NaiveDate,
    pub kind: EventKind,
    pub amount_cents: i64,
    pub realized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayPoint {
    pub date: NaiveDate,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthEnd {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthMetric {
    pub year: i32,
    pub month: u32,
    /// Renda do mês (Entradas).
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas FIXAS realizadas (coluna Saída da planilha; cartão entra como lump aqui). Exposto à
    /// parte de `cost_of_living_cents` para o rodapé mensal ENTRADAS | SAÍDAS | DIÁRIO.
    pub fixed_out_cents: i64,
    /// Diário REALIZADO (coluna Diário). `cost_of_living = fixed_out + daily_out`.
    pub daily_out_cents: i64,
    pub real_daily_avg_cents: i64,
    pub savings_rate_bps: i64,
    /// Economia lançada no mês (numerador do Economizado%). Não afeta performance (planilha-parity).
    pub economia_cents: i64,
    /// Saída TOTAL lançada no mês = fixas + diário (realizado + projetado/pré-lançado). É o que a
    /// [`month_coverage`] usa para julgar "mês completo" (quanto do gasto típico já está lançado),
    /// distinto de `cost_of_living_cents` (só realizado). Não inclui economia (a baseline é de
    /// despesas, não de transferências).
    pub total_outflow_cents: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forecast {
    /// Projected balance for each day from `today` to `horizon_end` inclusive.
    pub daily: Vec<DayPoint>,
    /// Projected balance on the last day of each month in the horizon.
    pub month_end: Vec<MonthEnd>,
    /// Lowest projected balance in the horizon and the day it occurs.
    pub deepest_deficit: Option<DayPoint>,
    /// SÓ o piso de caixa (menor saldo do horizonte, ≥ 0) — NÃO é o "pode gastar" exibido. O
    /// guardrail real (duplo: caixa × poupança) é [`safe_to_spend_today`]; o DTO expõe o dele.
    /// Nome explícito para não ser confundido com o número do dashboard (review P2).
    pub cash_floor_cents: i64,
    /// Per-month decision metrics (Totais).
    pub months: Vec<MonthMetric>,
}

/// Qual régua limita o "pode gastar hoje".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guardrail {
    /// Limitado pelo caixa: gastar mais empurraria algum dia futuro abaixo do piso de reserva.
    Cash,
    /// Limitado pela meta de poupança: o mês corrente já está no limite (ou abaixo) dos 20–30%.
    Savings,
}

/// "Pode gastar hoje" decomposto nas duas réguas do método.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeToSpend {
    /// O número honesto exibido: o MAIS APERTADO das duas réguas, nunca negativo.
    pub amount_cents: i64,
    /// Folga de caixa: menor saldo projetado no horizonte − piso de reserva. Pode ser a reserva
    /// inteira (alta) mesmo quando a poupança do mês já estourou — é o "Caixa ≠ Performance".
    pub cash_headroom_cents: i64,
    /// Folga de poupança do mês corrente: `performance − meta×renda`. Negativa = já abaixo da
    /// meta (gastar mais afunda a performance). `None` = régua de poupança INATIVA (mês sem
    /// renda) → só o caixa decide. Tipado como Option para o compilador forçar o tratamento
    /// (antes era um sentinela `i64::MAX` que vazava — review P1/P2).
    pub savings_headroom_cents: Option<i64>,
    /// Qual régua manda.
    pub binding: Guardrail,
}

/// O "pode gastar hoje" fiel ao método: o mais apertado de duas réguas.
///
/// 1. **Caixa** — `menor saldo projetado no horizonte − piso de reserva`. É o padrão de mercado
///    ("não fique negativo"), mas frouxo para quem tem colchão: o caixa pode crescer enquanto a
///    poupança despenca.
/// 2. **Poupança** — quanto cabe mantendo a taxa de poupança **do ANO** ≥ `savings_target_bps`:
///    `poupança_ano − meta×renda_ano`. A meta de 20–30% é **média ANUAL** (o ano todo fica na
///    faixa; tem mês que é mais, tem mês que é menos), então um mês isolado não pode mandar.
///    As figuras anuais são do REALIZADO (o ano projetado
///    mente quando os meses futuros estão incompletos). `None` = sem renda no ano → só o caixa.
///
/// Espelha o gate determinístico do método: só pode gastar se a reserva continua acima do piso
/// **E** a poupança 20–30% (no ano) se mantém.
pub fn safe_to_spend_today(
    fc: &Forecast,
    annual_income_cents: i64,
    annual_savings_cents: i64,
    savings_target_bps: i64,
    reserve_floor_cents: i64,
) -> SafeToSpend {
    let cash_headroom_cents =
        fc.deepest_deficit.map(|p| p.balance_cents).unwrap_or(0) - reserve_floor_cents;

    // Folga de poupança ANUAL = `poupança_ano − meta×renda_ano`. `None` sem renda (régua inativa).
    let savings_headroom_cents = (annual_income_cents > 0)
        .then(|| annual_savings_cents - savings_target_bps * annual_income_cents / 10_000);

    // Régua de poupança só morde se estiver ativa E for mais apertada que o caixa.
    let binding = match savings_headroom_cents {
        Some(s) if s < cash_headroom_cents => Guardrail::Savings,
        _ => Guardrail::Cash,
    };
    let limit = savings_headroom_cents
        .unwrap_or(i64::MAX)
        .min(cash_headroom_cents);
    let amount_cents = limit.max(0);

    SafeToSpend {
        amount_cents,
        cash_headroom_cents,
        savings_headroom_cents,
        binding,
    }
}

/// Cobertura de um mês FUTURO: quanto do gasto típico já está lançado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthCoverage {
    pub year: i32,
    pub month: u32,
    /// Saída já lançada para o mês (fixas + diário + fatura).
    pub projected_outflow_cents: i64,
    /// Gasto típico de um mês (mediana dos meses realizados).
    pub baseline_outflow_cents: i64,
    /// `projected / baseline` em basis points (10000 = 100% do típico).
    pub coverage_bps: i64,
    /// Acima do limiar de confiança → a projeção do mês é crível.
    pub is_complete: bool,
    /// Quanto provavelmente FALTA lançar (fatura/variáveis) para o mês ficar realista.
    pub estimated_missing_cents: i64,
}

/// Cobertura de cada mês FUTURO (estritamente após o mês corrente): quanto do gasto típico
/// (`baseline`, a mediana dos meses realizados) já está lançado. É o "chá revelação" do método
/// virado em métrica: um mês quase vazio projeta saldo otimista demais. O mercado não faz isso —
/// é o diferencial. `complete_threshold_bps` = a partir de quanto o mês é confiável (ex.: 7000 =
/// 70% do gasto típico). O mês corrente é parcial (realizado+projetado) e fica de fora.
pub fn month_coverage(
    months: &[MonthMetric],
    today: NaiveDate,
    baseline_outflow_cents: i64,
    complete_threshold_bps: i64,
) -> Vec<MonthCoverage> {
    // Sem baseline (nenhum mês realizado) não dá para julgar nada — devolve vazio para o caller
    // sinalizar "sem histórico", em vez de marcar tudo como completo (review P1).
    if baseline_outflow_cents <= 0 {
        return Vec::new();
    }
    months
        .iter()
        .filter(|m| (m.year, m.month) > (today.year(), today.month()))
        .map(|m| {
            // Cobertura usa a saída TOTAL lançada (realizado + projetado), não só o custo de vida
            // realizado — um mês futuro pré-lançado já tem dados, mesmo que `realized=false`.
            let projected_outflow_cents = m.total_outflow_cents;
            // `.max(0)`: crédito/estorno pode deixar a saída do mês negativa; cobertura nunca < 0.
            let coverage_bps =
                (projected_outflow_cents.max(0) * 10_000 / baseline_outflow_cents).max(0);
            MonthCoverage {
                year: m.year,
                month: m.month,
                projected_outflow_cents,
                baseline_outflow_cents,
                coverage_bps,
                is_complete: coverage_bps >= complete_threshold_bps,
                estimated_missing_cents: (baseline_outflow_cents - projected_outflow_cents).max(0),
            }
        })
        .collect()
}

/// Net signed effect of an event on the balance (income adds, outflows subtract).
fn signed(e: &CashflowEvent) -> i64 {
    match e.kind {
        EventKind::Income => e.amount_cents,
        // Economia leaves the spending balance too (guardar reduz o disponível), mirroring the
        // app's single conta — só que é poupança, não gasto (ver Economizado% nas métricas).
        EventKind::FixedOut | EventKind::Daily | EventKind::Economia => -e.amount_cents,
    }
}

/// Row→event classification rule (the shell maps DB rows through this).
/// `income` → Entrada; an `expense` on credit or marked fixed → Saída (a fatura lump or fixed bill);
/// any other `expense` → Diário (variable débito/cash). A `transfer` is **Economia** only when its
/// destination is real savings — `reserve` (reserva) or `illiquid` (FGTS/previdência = poupança
/// forçada). `restricted` (vale-refeição) é dinheiro de gasto RESTRITO, **não** poupança: contá-lo
/// como Economia inflaria o Economizado% (= Economia/Entradas) sem respaldo no método. Demais
/// transferências (entre contas líquidas, ou para vale) são net-zero para a poupança → ignoradas.
/// `to_liquidity` é a `liquidity` da conta-destino (None p/ não-transfers ou contas sem classe).
pub fn classify(
    txn_type: &str,
    is_fixed: bool,
    payment_method: Option<&str>,
    to_liquidity: Option<&str>,
) -> Option<EventKind> {
    match txn_type {
        "income" => Some(EventKind::Income),
        "expense" => {
            if is_fixed || payment_method == Some("credit") {
                Some(EventKind::FixedOut)
            } else {
                Some(EventKind::Daily)
            }
        }
        "transfer" => match to_liquidity {
            // Poupar de verdade: reserva ou poupança forçada (FGTS/previdência) = Economia.
            Some("reserve") | Some("illiquid") => Some(EventKind::Economia),
            // Vale-refeição (restricted) é gasto restrito, não poupança; e transferências entre
            // contas líquidas são net-zero → nenhuma conta como Economia.
            _ => None,
        },
        _ => None,
    }
}

/// Last calendar day of the given month.
pub fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|first_of_next| first_of_next.pred_opt())
        .expect("valid month")
}

/// Given a checkin date and a card's closing/due days, compute the due date for that cycle.
/// The cycle closes on `closing_day` of the checkin's month (or previous month if checkin is before
/// closing_day), and the invoice is due on `due_day` of the following month.
pub fn cycle_due_date(checkin_date: NaiveDate, closing_day: u32, due_day: u32) -> NaiveDate {
    // Clamp closing_day to a calendar day present in every month. `closing_day = 0` is not a valid
    // cycle boundary (the `day() <= closing_day` guard would never hold, always routing to the
    // prior month); treat it as 1. `closing_day > 28` could skip February; cap at 28.
    let closing_day = closing_day.clamp(1, 28);
    let (cycle_close_year, cycle_close_month) = if checkin_date.day() <= closing_day {
        // Checkin is before or on closing_day → cycle closes this month
        (checkin_date.year(), checkin_date.month())
    } else {
        // Checkin is after closing_day → cycle closed last month
        if checkin_date.month() == 1 {
            (checkin_date.year() - 1, 12)
        } else {
            (checkin_date.year(), checkin_date.month() - 1)
        }
    };

    // Due date is in the month AFTER the cycle closes
    let (due_year, due_month) = if cycle_close_month == 12 {
        (cycle_close_year + 1, 1)
    } else {
        (cycle_close_year, cycle_close_month + 1)
    };

    let last_day = last_day_of_month(due_year, due_month);
    let due_day_clamped = due_day.min(last_day.day());
    NaiveDate::from_ymd_opt(due_year, due_month, due_day_clamped).expect("valid due date")
}

/// Projected balance on each month's last day within the (chronological) daily series.
fn month_end_points(daily: &[DayPoint]) -> Vec<MonthEnd> {
    let mut out: Vec<MonthEnd> = Vec::new();
    for p in daily {
        let (year, month) = (p.date.year(), p.date.month());
        match out.last_mut() {
            Some(last) if last.year == year && last.month == month => {
                last.balance_cents = p.balance_cents;
            }
            _ => out.push(MonthEnd {
                year,
                month,
                balance_cents: p.balance_cents,
            }),
        }
    }
    out
}

/// Lowest projected balance and its earliest date (None if the series is empty).
fn deepest(daily: &[DayPoint]) -> Option<DayPoint> {
    daily.iter().copied().reduce(|a, b| {
        if b.balance_cents < a.balance_cents {
            b
        } else {
            a
        }
    })
}

/// Per-month decision metrics (Totais). Metrics cover the **whole month** (realized so far +
/// projected), so they filter `events` by month, not by horizon. `today` bounds "elapsed days"
/// for the real daily average (kept as an argument — no ambient clock).
fn month_metrics(
    today: NaiveDate,
    events: &[CashflowEvent],
    months: &[MonthEnd],
) -> Vec<MonthMetric> {
    months
        .iter()
        .map(|me| {
            let (year, month) = (me.year, me.month);
            let mut income = 0i64;
            let mut fixed_out = 0i64;
            let mut daily_realized = 0i64;
            let mut daily_projected = 0i64;
            let mut economia = 0i64;
            for e in events
                .iter()
                .filter(|e| e.date.year() == year && e.date.month() == month)
            {
                match e.kind {
                    EventKind::Income => income += e.amount_cents,
                    EventKind::FixedOut => fixed_out += e.amount_cents,
                    EventKind::Daily => {
                        if e.realized {
                            daily_realized += e.amount_cents;
                        } else {
                            // Previsão de diário (teto dos dias futuros) + diários futuros pré-lançados.
                            daily_projected += e.amount_cents;
                        }
                    }
                    EventKind::Economia => economia += e.amount_cents,
                }
            }
            // Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
            let cost_of_living_cents = fixed_out + daily_realized;
            // Performance = Entradas − (Saídas + Diário) — fórmula da planilha (linha Performance).
            // DECISÃO DO DONO (2026-06-20): paridade com planilha preferida sobre a fórmula do App.
            // Economia e previsão de diário restante NÃO são descontadas aqui (afetam só o guardrail
            // de poupança e o forecast de caixa, que têm suas próprias entradas).
            let performance_cents = income - cost_of_living_cents;

            let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
            let last = last_day_of_month(year, month);
            let elapsed = if today < first {
                0
            } else {
                let end = if today > last { last } else { today };
                (end - first).num_days() + 1
            };
            // Diário médio = Σ Diário REALIZADO ÷ dias decorridos (D/N). A previsão não entra.
            let real_daily_avg_cents = if elapsed > 0 {
                daily_realized / elapsed
            } else {
                0
            };

            // Economizado% = economia lançada ÷ entradas (não mais o superávit/performance).
            let savings_rate_bps = if income > 0 {
                economia * 10_000 / income
            } else {
                0
            };

            MonthMetric {
                year,
                month,
                income_cents: income,
                performance_cents,
                cost_of_living_cents,
                fixed_out_cents: fixed_out,
                daily_out_cents: daily_realized,
                real_daily_avg_cents,
                savings_rate_bps,
                economia_cents: economia,
                total_outflow_cents: fixed_out + daily_realized + daily_projected,
            }
        })
        .collect()
}

/// Métricas por mês para uma lista arbitrária de `(ano, mês)` — usada pela visão ANUAL (todos os 12
/// meses do ano, realizado + projetado), independente do horizonte do forecast. O `balance_cents`
/// do `MonthEnd` não importa aqui (as métricas só usam os eventos do mês).
pub fn month_metrics_for(
    today: NaiveDate,
    events: &[CashflowEvent],
    months: &[(i32, u32)],
) -> Vec<MonthMetric> {
    let ends: Vec<MonthEnd> = months
        .iter()
        .map(|&(year, month)| MonthEnd {
            year,
            month,
            balance_cents: 0,
        })
        .collect();
    month_metrics(today, events, &ends)
}

/// Eventos `Daily` projetados da **previsão de diário**: para cada dia do MÊS CORRENTE após `today`
/// (até o fim do mês ou `horizon_end`, o que vier antes) que ainda não tem um Daily lançado, injeta
/// o teto/dia (`per_day_cents`) como `Daily { realized: false }`. Faz o saldo projetado e a
/// Performance assumirem o gasto típico até o fim do mês ("nasce no vermelho e esverdeia"), sem
/// inflar o diário médio (que só conta realizado) nem o custo de vida. A previsão dos meses
/// FUTUROS fica a cargo de [`month_coverage`]; aqui é só o mês corrente ("restante").
pub fn project_daily_ceiling(
    per_day_cents: i64,
    today: NaiveDate,
    horizon_end: NaiveDate,
    days_with_daily: &std::collections::HashSet<NaiveDate>,
) -> Vec<CashflowEvent> {
    if per_day_cents <= 0 {
        return Vec::new();
    }
    let cap = last_day_of_month(today.year(), today.month()).min(horizon_end);
    let mut out = Vec::new();
    let mut day = match today.succ_opt() {
        Some(d) => d,
        None => return out,
    };
    while day <= cap {
        if !days_with_daily.contains(&day) {
            out.push(CashflowEvent {
                date: day,
                kind: EventKind::Daily,
                amount_cents: per_day_cents,
                realized: false,
            });
        }
        day = match day.succ_opt() {
            Some(d) => d,
            None => break,
        };
    }
    out
}

/// Project the running cash balance day by day from `today` to `horizon_end` (inclusive).
///
/// `seed_cents` is the opening balance carried into `today` (before today's events); thus
/// `daily[0].balance = seed + net(events on today)`, mirroring the spreadsheet's
/// `Saldo[d] = Saldo[d-1] + Entrada − (Saída + Diário)`.
pub fn project(
    seed_cents: i64,
    today: NaiveDate,
    events: &[CashflowEvent],
    horizon_end: NaiveDate,
) -> Forecast {
    project_with_metrics(seed_cents, today, events, events, horizon_end)
}

/// Como [`project`], mas as MÉTRICAS por mês (performance/poupança) usam um conjunto de eventos
/// SEPARADO do encadeamento de caixa.
///
/// O encadeamento diário parte da semente (que já embute todo o passado) e por isso só consome
/// `chain_events` com `date > hoje` — somar o realizado de novo dobraria. Mas a performance do
/// mês corrente PRECISA do realizado de hoje-pra-trás no mês (renda e saídas já lançadas), senão
/// junho aparece com sinal trocado e o guardrail de poupança decide sobre o mês pela metade
/// (P0 do review adversarial). Por isso `metric_events` cobre o mês inteiro (realizado + projetado).
pub fn project_with_metrics(
    seed_cents: i64,
    today: NaiveDate,
    chain_events: &[CashflowEvent],
    metric_events: &[CashflowEvent],
    horizon_end: NaiveDate,
) -> Forecast {
    let mut daily = Vec::new();
    let mut balance = seed_cents;
    let mut day = today;
    while day <= horizon_end {
        let net: i64 = chain_events
            .iter()
            .filter(|e| e.date == day)
            .map(signed)
            .sum();
        balance += net;
        daily.push(DayPoint {
            date: day,
            balance_cents: balance,
        });
        day = match day.succ_opt() {
            Some(next) => next,
            None => break, // chrono's max representable date; horizons never reach this
        };
    }
    let month_end = month_end_points(&daily);
    let deepest_deficit = deepest(&daily);
    let cash_floor_cents = deepest_deficit.map(|p| p.balance_cents.max(0)).unwrap_or(0);
    let months = month_metrics(today, metric_events, &month_end);

    Forecast {
        daily,
        month_end,
        deepest_deficit,
        cash_floor_cents,
        months,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn ev(date: &str, kind: EventKind, amount_cents: i64) -> CashflowEvent {
        CashflowEvent {
            date: d(date),
            kind,
            amount_cents,
            realized: true,
        }
    }

    // T2.4 — empty events yield a flat line at the seed for every day in the horizon.
    #[test]
    fn empty_events_flat_seed_line() {
        let f = project(100000, d("2026-01-01"), &[], d("2026-01-03"));
        assert_eq!(f.daily.len(), 3);
        assert!(f.daily.iter().all(|p| p.balance_cents == 100000));
        assert_eq!(f.daily[0].date, d("2026-01-01"));
        assert_eq!(f.daily[2].date, d("2026-01-03"));
    }

    // T2.1 — single-month chain: saldo[d] = saldo[d-1] + income − (fixed_out + daily).
    #[test]
    fn single_month_chain() {
        let events = [
            ev("2026-01-01", EventKind::Income, 200000),
            ev("2026-01-02", EventKind::FixedOut, 50000),
            ev("2026-01-02", EventKind::Daily, 30000),
            ev("2026-01-03", EventKind::Daily, 20000),
        ];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-03"));
        assert_eq!(f.daily[0].balance_cents, 1200000); // 1000 + 200
        assert_eq!(f.daily[1].balance_cents, 1120000); // 1200 - 50 - 30
        assert_eq!(f.daily[2].balance_cents, 1100000); // 1120 - 20
    }

    // T2.2 — month boundary: last day of a month seeds the first day of the next.
    #[test]
    fn month_boundary_carries() {
        let events = [
            ev("2026-01-31", EventKind::Daily, 100000),
            ev("2026-02-01", EventKind::Income, 500000),
        ];
        let f = project(300000, d("2026-01-31"), &events, d("2026-02-01"));
        assert_eq!(f.daily[0].date, d("2026-01-31"));
        assert_eq!(f.daily[0].balance_cents, 200000); // 300 - 100
        assert_eq!(f.daily[1].date, d("2026-02-01"));
        assert_eq!(f.daily[1].balance_cents, 700000); // 200 + 500
    }

    // T2.3 — year boundary (Dec → Jan) continuity.
    #[test]
    fn year_boundary_carries() {
        let events = [
            ev("2025-12-31", EventKind::FixedOut, 80000),
            ev("2026-01-01", EventKind::Income, 600000),
        ];
        let f = project(1000000, d("2025-12-31"), &events, d("2026-01-01"));
        assert_eq!(f.daily.len(), 2);
        assert_eq!(f.daily[0].balance_cents, 920000); // Dec 31 2025: 1000 - 80
        assert_eq!(f.daily[1].date, d("2026-01-01"));
        assert_eq!(f.daily[1].balance_cents, 1520000); // Jan 1 2026: 920 + 600
    }

    // T2.4 — determinism: identical inputs yield identical output.
    #[test]
    fn deterministic() {
        let events = [ev("2026-01-02", EventKind::Daily, 42000)];
        let a = project(500000, d("2026-01-01"), &events, d("2026-01-05"));
        let b = project(500000, d("2026-01-01"), &events, d("2026-01-05"));
        assert_eq!(a.daily, b.daily);
        assert_eq!(a.daily.len(), 5);
    }

    // ---- Phase 3: month-end (US3) + deepest deficit (US4) ----

    // T3.1 — month_end is the projected balance on each month's last day within the horizon.
    #[test]
    fn month_end_per_month() {
        let events = [
            ev("2026-01-31", EventKind::Income, 200000),
            ev("2026-02-02", EventKind::FixedOut, 50000),
        ];
        let f = project(1000000, d("2026-01-30"), &events, d("2026-02-02"));
        assert_eq!(f.month_end.len(), 2);
        assert_eq!((f.month_end[0].year, f.month_end[0].month), (2026, 1));
        assert_eq!(f.month_end[0].balance_cents, 1200000); // Jan 31: 1000 + 200
        assert_eq!((f.month_end[1].year, f.month_end[1].month), (2026, 2));
        assert_eq!(f.month_end[1].balance_cents, 1150000); // Feb 2: 1200 - 50
    }

    // T3.2 — deepest deficit = min projected balance + its (earliest) date, negative trough.
    #[test]
    fn deepest_deficit_negative() {
        let events = [ev("2026-01-02", EventKind::FixedOut, 1500000)];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-04"));
        let dd = f.deepest_deficit.unwrap();
        assert_eq!(dd.balance_cents, -500000);
        assert_eq!(dd.date, d("2026-01-02"));
    }

    // T3.3 — all-positive horizon → deepest deficit is the minimum positive trough.
    #[test]
    fn deepest_deficit_positive_trough() {
        let events = [
            ev("2026-01-02", EventKind::Daily, 300000),
            ev("2026-01-03", EventKind::Income, 500000),
        ];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-03"));
        let dd = f.deepest_deficit.unwrap();
        assert_eq!(dd.balance_cents, 700000);
        assert_eq!(dd.date, d("2026-01-02"));
    }

    // ---- Phase 4: safe-to-spend today (US5) ----

    // T4.1 / T4.3 — safe-to-spend equals the min future balance (spending it makes the trough touch 0).
    #[test]
    fn safe_to_spend_equals_min_balance() {
        let events = [ev("2026-01-03", EventKind::FixedOut, 200000)];
        let f = project(500000, d("2026-01-01"), &events, d("2026-01-04"));
        assert_eq!(f.cash_floor_cents, 300000); // min over horizon
    }

    // T4.2 — already negative ahead → safe-to-spend clamps to 0, never negative.
    #[test]
    fn safe_to_spend_zero_when_negative() {
        let events = [ev("2026-01-02", EventKind::FixedOut, 800000)];
        let f = project(500000, d("2026-01-01"), &events, d("2026-01-03"));
        assert_eq!(f.cash_floor_cents, 0);
    }

    // ---- Guardrail duplo (poupança ANUAL 25% + caixa) ----

    // Caixa cheio (colchão) MAS a poupança do ANO já estourou → pode gastar = 0, limitado pela
    // POUPANÇA, não pelo caixa. É o "Caixa ≠ Performance" do método.
    #[test]
    fn safe_to_spend_savings_binds_when_cash_is_high() {
        let events = [
            ev("2026-06-01", EventKind::Income, 1_000_000),
            ev("2026-06-02", EventKind::FixedOut, 1_100_000),
        ];
        let f = project(800_000, d("2026-06-01"), &events, d("2026-06-30"));
        // Poupança do ANO (realizado) = renda 1.000.000, sobra −100.000 (dissaving).
        let s = safe_to_spend_today(&f, 1_000_000, -100_000, 2500, 0);

        // Caixa positivo e alto: 800.000 + 1.000.000 − 1.100.000 = 700.000, estável até fim do mês.
        assert_eq!(s.cash_headroom_cents, 700_000);
        // Meta 25% × renda anual = 250.000. Folga = −100.000 − 250.000 = −350.000 (abaixo da meta).
        assert_eq!(s.savings_headroom_cents, Some(-350_000));
        assert_eq!(s.binding, Guardrail::Savings);
        assert_eq!(s.amount_cents, 0); // honesto: 0, não o caixa disponível
    }

    // Conta futura pré-lançada (fatura/salário) num mês à frente limita o gasto de HOJE pelo
    // caixa — só visível porque o horizonte varre além do mês corrente.
    #[test]
    fn safe_to_spend_cash_binds_on_future_month_commitment() {
        let events = [
            ev("2026-06-01", EventKind::Income, 1_000_000),
            ev("2026-07-15", EventKind::FixedOut, 900_000), // fatura lá na frente
        ];
        let f = project(0, d("2026-06-01"), &events, d("2026-07-31"));
        // Poupança do ano folgada: renda 1.000.000, sobra 1.000.000 → folga +750.000.
        let s = safe_to_spend_today(&f, 1_000_000, 1_000_000, 2500, 0);

        // Caixa cai para 100.000 em 15/jul (o "buraco do futuro").
        assert_eq!(s.cash_headroom_cents, 100_000);
        // Poupança de junho está folgada (só renda no mês): 1.000.000 − 250.000 = 750.000.
        assert_eq!(s.savings_headroom_cents, Some(750_000));
        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 100_000);
    }

    // O piso de reserva reduce a folga de caixa (não pode comer a reserva).
    #[test]
    fn safe_to_spend_reserve_floor_subtracts_from_cash() {
        let events = [
            ev("2026-06-01", EventKind::Income, 1_000_000),
            ev("2026-07-15", EventKind::FixedOut, 900_000),
        ];
        let f = project(0, d("2026-06-01"), &events, d("2026-07-31"));
        let s = safe_to_spend_today(&f, 1_000_000, 1_000_000, 2500, 50_000);
        assert_eq!(s.cash_headroom_cents, 50_000); // 100.000 − 50.000 de piso
        assert_eq!(s.amount_cents, 50_000);
    }

    // Cobertura: meses futuros esparsos (só fixas) vs gasto típico → sinaliza incompleto.
    #[test]
    fn month_coverage_flags_sparse_future_months() {
        let mm = |year, month, cost: i64| MonthMetric {
            year,
            month,
            income_cents: 0,
            performance_cents: 0,
            cost_of_living_cents: cost,
            fixed_out_cents: cost,
            daily_out_cents: 0,
            real_daily_avg_cents: 0,
            savings_rate_bps: 0,
            economia_cents: 0,
            total_outflow_cents: cost,
        };
        // Mês corrente (jun) ignorado; jul completo (R$ 1.000), ago esparso (R$ 380).
        let months = [mm(2026, 6, 900), mm(2026, 7, 1_000), mm(2026, 8, 380)];
        let cov = month_coverage(&months, d("2026-06-13"), 1_000, 7_000);
        assert_eq!(cov.len(), 2); // só jul e ago (jun corrente fora)
        assert_eq!(cov[0].month, 7);
        assert_eq!(cov[0].coverage_bps, 10_000); // 100%
        assert!(cov[0].is_complete);
        assert_eq!(cov[0].estimated_missing_cents, 0);
        assert_eq!(cov[1].month, 8);
        assert_eq!(cov[1].coverage_bps, 3_800); // 38% do típico
        assert!(!cov[1].is_complete);
        assert_eq!(cov[1].estimated_missing_cents, 620); // 1000 − 380
    }

    // Sem baseline (nenhum mês realizado) → cobertura VAZIA, não "tudo completo" (review P1).
    #[test]
    fn month_coverage_empty_without_baseline() {
        let mm = |year, month, cost: i64| MonthMetric {
            year,
            month,
            income_cents: 0,
            performance_cents: 0,
            cost_of_living_cents: cost,
            fixed_out_cents: cost,
            daily_out_cents: 0,
            real_daily_avg_cents: 0,
            savings_rate_bps: 0,
            economia_cents: 0,
            total_outflow_cents: cost,
        };
        let months = [mm(2026, 7, 1_000), mm(2026, 8, 380)];
        let cov = month_coverage(&months, d("2026-06-13"), 0, 6_000);
        assert!(cov.is_empty());
    }

    // Sem renda no ano: régua de poupança INATIVA (None), só o caixa decide.
    #[test]
    fn safe_to_spend_savings_inactive_without_income() {
        let events = [ev("2026-06-10", EventKind::Daily, 30_000)];
        let f = project(200_000, d("2026-06-01"), &events, d("2026-06-30"));
        let s = safe_to_spend_today(&f, 0, -30_000, 2500, 0);
        assert_eq!(s.savings_headroom_cents, None);
        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 170_000); // 200.000 − 30.000, só caixa
    }

    // ---- Phase 5: monthly metrics / Totais (US6) ----

    // T5.1 — performance = income − all_out; cost_of_living = fixed_out + daily (+ card via FixedOut).
    #[test]
    fn month_performance_and_cost() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1000000),
            ev("2026-03-10", EventKind::FixedOut, 400000),
            ev("2026-03-12", EventKind::Daily, 200000),
        ];
        let f = project(0, d("2026-03-01"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.cost_of_living_cents, 600000); // 400 + 200
        assert_eq!(m.performance_cents, 400000); // 1000 - 600
    }

    // T5.3 — cash ≠ performance: month ends negative in cash while performance is positive.
    #[test]
    fn cash_differs_from_performance() {
        let events = [
            ev("2026-03-01", EventKind::Income, 100000),
            ev("2026-03-02", EventKind::FixedOut, 20000),
        ];
        let f = project(-200000, d("2026-03-01"), &events, d("2026-03-02"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.performance_cents, 80000); // 100 - 20 (positive)
        assert_eq!(f.month_end[0].balance_cents, -120000); // cash ends negative
    }

    // T5.2 — real daily average = realized daily ÷ elapsed days; Economizado% = economia ÷ renda.
    #[test]
    fn real_daily_avg_and_savings() {
        let mut events = vec![ev("2026-03-01", EventKind::Income, 1000000)];
        for day in ["2026-03-02", "2026-03-04", "2026-03-06", "2026-03-08"] {
            events.push(ev(day, EventKind::Daily, 50000)); // realized daily, 4 × 50 = 200
        }
        events.push(ev("2026-03-09", EventKind::Economia, 250000)); // guardou 250
        let f = project(0, d("2026-03-10"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.real_daily_avg_cents, 20000); // 200.00 / 10 elapsed days (economia não conta)
        assert_eq!(m.economia_cents, 250000);
        // Economizado% = economia (250) ÷ renda (1000) = 25% = 2500 bps (não mais o superávit).
        assert_eq!(m.savings_rate_bps, 2500);
        // Performance = renda (1000) − custo de vida (diário 200) = 800 (economia não desconta).
        assert_eq!(m.performance_cents, 800000);
        assert_eq!(m.cost_of_living_cents, 200000); // só diário realizado (sem economia)
    }

    // ---- Phase 6: credit dual-tracking (US7) ----

    // T6.1 — a Daily event (Régua 1, débito) reduces the balance on its own day.
    #[test]
    fn regua1_daily_hits_its_day() {
        let events = [ev("2026-01-02", EventKind::Daily, 70000)];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-02"));
        assert_eq!(f.daily[0].balance_cents, 1000000); // Jan 1 untouched
        assert_eq!(f.daily[1].balance_cents, 930000); // Jan 2: −70
    }

    // T6.2 / T6.3 — a fatura lump lands as one FixedOut on the card due day, depressing the
    // future month, while débito/PIX daily spend only touches its own day.
    #[test]
    fn credit_fatura_lump_lands_at_due_day() {
        let events = [
            ev("2026-01-10", EventKind::Daily, 20000), // débito/PIX daily spend
            ev("2026-02-15", EventKind::FixedOut, 600000), // fatura lump at card due date
        ];
        let f = project(1000000, d("2026-01-10"), &events, d("2026-02-15"));
        let jan = f.month_end.iter().find(|m| m.month == 1).unwrap();
        let feb = f.month_end.iter().find(|m| m.month == 2).unwrap();
        assert_eq!(jan.balance_cents, 980000); // 1000 − 20 (only daily)
        assert_eq!(feb.balance_cents, 380000); // 980 − 600 at Feb 15
    }

    // ---- Phase 7: row→event classification (US8 mapping) ----

    // T7.1 — classify maps raw transaction rows to the right event kind.
    #[test]
    fn classify_maps_rows_to_kinds() {
        assert_eq!(
            classify("income", false, None, None),
            Some(EventKind::Income)
        );
        assert_eq!(
            classify("expense", true, Some("debit"), None),
            Some(EventKind::FixedOut)
        ); // fixed bill
        assert_eq!(
            classify("expense", false, Some("credit"), None),
            Some(EventKind::FixedOut)
        ); // credit lump
        assert_eq!(
            classify("expense", false, Some("debit"), None),
            Some(EventKind::Daily)
        ); // variable débito
        assert_eq!(
            classify("expense", false, None, None),
            Some(EventKind::Daily)
        );
    }

    // Economia: transfer p/ bolso não-líquido = Economia; entre líquidos = net-zero (skip).
    #[test]
    fn classify_transfer_to_reserve_is_economia() {
        // Poupança real (reserva) → Economia.
        assert_eq!(
            classify("transfer", false, None, Some("reserve")),
            Some(EventKind::Economia)
        );
        // FGTS/previdência (illiquid) = poupança forçada → Economia.
        assert_eq!(
            classify("transfer", false, None, Some("illiquid")),
            Some(EventKind::Economia)
        );
        // Vale-refeição (restricted) é gasto restrito, NÃO poupança → não conta como Economia.
        assert_eq!(classify("transfer", false, None, Some("restricted")), None);
        // Entre contas líquidas (ou destino desconhecido) → net-zero, ignorado.
        assert_eq!(classify("transfer", false, None, Some("liquid")), None);
        assert_eq!(classify("transfer", false, None, None), None);
    }

    // ---- Phase 7: credit cycle aggregation (T7.2) ----

    // T7.2a — cycle_due_date: checkin before closing_day → due next month
    #[test]
    fn cycle_due_date_before_closing() {
        // Card closes on day 20, due on day 10
        // Checkin on Jan 15 (before closing) → cycle closes Jan 20 → due Feb 10
        let checkin = d("2026-01-15");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-02-10"));
    }

    // T7.2b — cycle_due_date: checkin after closing_day → due in 2 months
    #[test]
    fn cycle_due_date_after_closing() {
        // Card closes on day 20, due on day 10
        // Checkin on Jan 25 (after closing) → cycle closed Dec 20 → due Jan 10
        let checkin = d("2026-01-25");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-01-10"));
    }

    // T7.2c — cycle_due_date: year boundary
    #[test]
    fn cycle_due_date_year_boundary() {
        // Card closes on day 20, due on day 10
        // Checkin on Dec 15 → cycle closes Dec 20 → due Jan 10 next year
        let checkin = d("2025-12-15");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-01-10"));
    }

    // T7.2d — cycle_due_date: due_day clamped to last day of month
    #[test]
    fn cycle_due_date_clamped() {
        // Card closes on day 20, due on day 31
        // Checkin on Jan 15 → cycle closes Jan 20 → due Feb 28 (clamped)
        let checkin = d("2026-01-15");
        let due = cycle_due_date(checkin, 20, 31);
        assert_eq!(due, d("2026-02-28"));
    }

    // T7.2e — cycle_due_date: closing_day = 0 is clamped to 1 (not "always prior month").
    // Before the clamp, `day() <= 0` was always false → the cycle was always treated as closed
    // last month regardless of checkin. After clamping to 1: checkin day 15 > closing day 1, so
    // the cycle closed last month (December) → due January 10.
    #[test]
    fn cycle_due_date_closing_day_zero() {
        let checkin = d("2026-01-15");
        let due = cycle_due_date(checkin, 0, 10);
        assert_eq!(due, d("2026-01-10"));
        // Sanity: a same-month checkin on day 1 (≤ clamped closing_day 1) closes this month → Feb.
        let due_early = cycle_due_date(d("2026-01-01"), 0, 10);
        assert_eq!(due_early, d("2026-02-10"));
    }

    // ---- Slice 011: Economia + previsão de diário como driver ----

    use std::collections::HashSet;

    // Economia sai do saldo de gasto como qualquer saída (guardar reduz o disponível).
    #[test]
    fn economia_reduces_spending_balance() {
        let events = [ev("2026-03-02", EventKind::Economia, 150000)];
        let f = project(1000000, d("2026-03-01"), &events, d("2026-03-03"));
        assert_eq!(f.daily[0].balance_cents, 1000000); // dia 1 intacto
        assert_eq!(f.daily[1].balance_cents, 850000); // dia 2: −150 (economia)
        assert_eq!(f.daily[2].balance_cents, 850000);
    }

    // project_daily_ceiling: injeta o teto nos dias futuros do MÊS CORRENTE (realized=false).
    #[test]
    fn daily_ceiling_fills_current_month_future_days() {
        let ev = project_daily_ceiling(12000, d("2026-02-20"), d("2026-02-28"), &HashSet::new());
        assert_eq!(ev.len(), 8); // 21..28 de fev
        assert!(
            ev.iter()
                .all(|e| e.kind == EventKind::Daily && !e.realized && e.amount_cents == 12000)
        );
        assert_eq!(ev[0].date, d("2026-02-21"));
        assert_eq!(ev[7].date, d("2026-02-28"));
    }

    // Pula dias que já têm um Daily lançado (não dobra a previsão).
    #[test]
    fn daily_ceiling_skips_days_with_daily() {
        let mut taken = HashSet::new();
        taken.insert(d("2026-02-22"));
        let ev = project_daily_ceiling(10000, d("2026-02-20"), d("2026-02-28"), &taken);
        assert_eq!(ev.len(), 7); // 8 − 1 (dia 22 já tem)
        assert!(ev.iter().all(|e| e.date != d("2026-02-22")));
    }

    // Só o mês corrente: horizonte que avança para março não recebe teto (isso é da coverage).
    #[test]
    fn daily_ceiling_only_current_month() {
        let ev = project_daily_ceiling(10000, d("2026-02-25"), d("2026-03-10"), &HashSet::new());
        assert!(ev.iter().all(|e| e.date.month() == 2));
        assert_eq!(ev.last().unwrap().date, d("2026-02-28"));
    }

    // Teto zero → sem eventos (orçamento não configurado).
    #[test]
    fn daily_ceiling_zero_budget_is_empty() {
        assert!(
            project_daily_ceiling(0, d("2026-02-20"), d("2026-02-28"), &HashSet::new()).is_empty()
        );
    }

    // Confirma que a previsão de diário (daily_projected) NÃO desconta a Performance
    // (paridade com planilha — DECISÃO DO DONO 2026-06-20).
    // Custo de vida = 0 (sem diário realizado); previsão é só para o saldo de caixa.
    #[test]
    fn performance_excludes_daily_projected_ceiling() {
        let mut events = vec![ev("2026-03-01", EventKind::Income, 1000000)];
        // 11..31 de março = 21 dias × 100.00 = 210.00 de previsão restante.
        events.extend(project_daily_ceiling(
            10000,
            d("2026-03-10"),
            d("2026-03-31"),
            &HashSet::new(),
        ));
        let f = project(0, d("2026-03-10"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.cost_of_living_cents, 0); // previsão NÃO entra no custo de vida
        assert_eq!(m.real_daily_avg_cents, 0); // previsão NÃO conta como realizado
        // Performance = income − cost_of_living = 1_000_000 − 0 = 1_000_000.
        assert_eq!(m.performance_cents, 1000000); // previsão NÃO desconta performance
    }

    // Regressão: economia e previsão de diário NÃO afetam performance (planilha-parity 2026-06-20).
    #[test]
    fn performance_excludes_economia_and_projected() {
        let events = [
            ev("2026-04-01", EventKind::Income, 1_000_000),
            ev("2026-04-05", EventKind::FixedOut, 300_000),
            ev("2026-04-08", EventKind::Daily, 50_000), // realized
            ev("2026-04-09", EventKind::Economia, 200_000),
            // projected daily (realized=false) — normally injected by project_daily_ceiling
            CashflowEvent {
                date: d("2026-04-15"),
                kind: EventKind::Daily,
                amount_cents: 30_000,
                realized: false,
            },
        ];
        let f = project(0, d("2026-04-10"), &events, d("2026-04-30"));
        let m = f.months.iter().find(|m| m.month == 4).unwrap();
        // cost_of_living = fixed_out(300) + daily_realized(50) = 350_000
        assert_eq!(m.cost_of_living_cents, 350_000);
        // performance = income(1_000) − cost_of_living(350) = 650_000 (NOT 420_000 old formula)
        assert_eq!(m.performance_cents, 650_000);
        // economia still feeds savings_rate
        assert_eq!(m.economia_cents, 200_000);
        assert_eq!(m.savings_rate_bps, 2_000); // 200/1000 = 20%
    }
}
