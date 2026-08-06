//! O núcleo puro: os insumos entram, a leitura do dia sai.
//!
//! `compose` é a única receita de "que janela alimenta que argumento". Não recebe pool, não lê
//! relógio e não abre consulta: tudo o que decide vem de [`ForecastInputs`], e por isso toda regra
//! do dia é testável montando insumos em memória.
//!
//! O que a composição decide, decide UMA vez: a projeção do horizonte (com métricas, nunca só
//! caixa), a régua anual sobre os doze meses que já chegaram varridos, o `safe_to_spend` e o
//! guardrail que mordeu, a cobertura dos meses futuros, os estados epistêmicos (reserva, economia,
//! procedência do teto), a detecção do modo de gasto e as pernas do gate do cartão. Um consumidor
//! recorta campos — nenhum recalcula um estado a partir de valores.

use super::inputs::*;
use crate::cards::{self, GateLeg, InvoiceStatus};
use crate::commands::forecast_cmds::{DayPointDto, MonthEndDto};
use crate::forecast;
use crate::scenarios::ScenarioMonthEnd;
use chrono::{Datelike, NaiveDate};

/// Limiar de cobertura: um mês futuro com menos de 60% do gasto típico já lançado é INCOMPLETO —
/// projeção otimista demais para sustentar o "confiável até".
pub(crate) const COVERAGE_COMPLETE_BPS: i64 = 6_000;

/// A leitura financeira do dia, inteira. O critério de inclusão é a pergunta da spec: se a figura
/// é lida por mais de uma superfície, ou julga o dia, ela é campo daqui.
#[derive(Debug, Clone)]
pub(crate) struct ForecastReading {
    pub today: NaiveDate,
    pub horizon_end: NaiveDate,
    /// A projeção do horizonte, feita uma vez com caixa E métricas. Trajetória diária, saldos de
    /// fim de mês, déficit mais profundo e métricas mensais saem todos daqui.
    pub forecast: forecast::Forecast,
    /// O saldo projetado do fim do mês corrente — o herói do dashboard. Campo ÚNICO: a projeção
    /// só de caixa não participa da leitura, então não há um segundo número a divergir.
    pub projected_month_end_cents: i64,
    pub annual: AnnualReading,
    pub safe_to_spend: forecast::SafeToSpend,
    pub coverage: CoverageReading,
    pub reserve: ReserveReading,
    pub ceiling: CeilingReading,
    pub spending_mode: SpendingModeReading,
    pub cards: CardReading,
    /// Diário realizado de hoje na régua do Diário e compras de cartão do dia. Vem mascarado da
    /// origem: a leitura não mantém contabilidade paralela do dia.
    pub today_spend: DailySpendInputs,
    pub ledger: LedgerInputs,
}

/// As figuras do ano, todas derivadas da MESMA varredura dos doze meses.
#[derive(Debug, Clone)]
pub(crate) struct AnnualReading {
    /// A régua anual do ano corrente: recorte vivido, faixa, lastro dos meses à frente.
    pub ruler: forecast::AnnualRuler,
    /// O Economizado% que JULGA, truncado uma vez. Régua anual, gate do cartão e DTO leem deste
    /// campo — não existe segunda divisão a arredondar diferente. `None` sem renda vivida: a
    /// régua não fabrica zero.
    pub economia_bps: Option<i64>,
    /// `verdict` (Economia registrada viva) · `no_record` (nada registrado).
    pub economia_state: &'static str,
    /// Economia REGISTRADA da janela de meses completos — o retrato do que já fechou.
    pub registered_economia_cents: i64,
    /// Patrimônio realizado da mesma janela, publicado ao lado da régua e nunca somado a ela.
    pub registered_patrimonio_cents: i64,
    /// Renda da mesma janela de meses COMPLETOS. Publicada ao lado das figuras registradas;
    /// nenhum recorte atual a lê — quem julga o ano usa o recorte da régua.
    #[allow(dead_code)]
    pub registered_income_cents: i64,
    pub projected_income_cents: i64,
    pub projected_savings_cents: i64,
    /// Os doze meses no tipo do motor, para quem lista o ano mês a mês. Nenhum recorte atual
    /// publica a listagem completa — fica pronto para o consumidor que precisar dela.
    #[allow(dead_code)]
    pub year_metrics: Vec<forecast::MonthMetric>,
}

/// Previsibilidade: quanto de cada mês futuro já está lançado e até onde a projeção é confiável.
#[derive(Debug, Clone)]
pub(crate) struct CoverageReading {
    pub months: Vec<forecast::MonthCoverage>,
    /// Gasto típico mensal (mediana realizada). `0` = sem histórico → previsibilidade indeterminada.
    pub baseline_outflow_cents: i64,
    /// Último mês cuja projeção é crível (`YYYY-MM`); `None` sem baseline para avaliar.
    pub trusted_through_month: Option<String>,
    pub total_missing_cents: i64,
}

/// A reserva em meses de custo de vida, com o estado epistêmico já decidido.
#[derive(Debug, Clone)]
pub(crate) struct ReserveReading {
    pub balance_cents: i64,
    /// Custo de vida mensal que serve de divisor. `0` = sem histórico para dividir.
    pub baseline_cents: i64,
    /// Meses completos que sustentam o divisor — o que separa veredito de retrato vivo.
    pub basis_months: i64,
    pub months: f64,
    /// `verdict` · `estimate` · `zero` (contas mapeadas e zeradas) · `no_record`.
    pub state: &'static str,
    pub trend: String,
    /// Alvo do método: custo de vida × meses mínimos. `0` sem base para calcular.
    pub target_cents: i64,
    /// Quanto passa do alvo; `None` enquanto a reserva está sendo construída.
    pub surplus_cents: Option<i64>,
}

/// O teto do dia exibido, com procedência e operandos.
#[derive(Debug, Clone)]
pub(crate) struct CeilingReading {
    pub per_day_cents: i64,
    pub source: CeilingSource,
    pub estimate_basis: Option<CeilingEstimateBasis>,
    pub proposal_pending: bool,
}

/// O modo de gasto e sua procedência — a copy muda conforme o veredito seja leitura da janela ou
/// o default de dado insuficiente.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SpendingModeReading {
    pub mode: forecast::SpendingMode,
    /// `false` quando o modo é o default de dado insuficiente, não uma leitura da janela.
    pub detected: bool,
    pub cartao_month_cents: i64,
}

/// O cartão: gate de legitimidade e as faturas a vencer.
#[derive(Debug, Clone)]
pub(crate) struct CardReading {
    pub gate: GateLeg,
    pub gate_economy: GateLeg,
    pub gate_reserve: GateLeg,
    /// O percentual por trás da perna de economia — o mesmo campo que a régua anual publica.
    pub gate_economy_bps: Option<i64>,
    /// Próxima fatura de cada cartão, em ordem de vencimento e nome, com o status do calendário.
    pub upcoming_invoices: Vec<UpcomingInvoice>,
    /// Próximo dia de fatura e o total que vence nele.
    pub next_fatura: Option<(NaiveDate, i64)>,
}

/// Uma fatura a vencer com o status já classificado pelo calendário do dia da leitura.
#[derive(Debug, Clone)]
pub(crate) struct UpcomingInvoice {
    pub account_id: String,
    pub card_name: String,
    pub owner_name: String,
    pub due_date: NaiveDate,
    pub amount_cents: i64,
    pub status: InvoiceStatus,
    pub has_refund_expectation: bool,
    pub refund_expected_cents: i64,
}

/// A leitura do dia, composta de uma vez a partir do inventário.
pub(crate) fn compose(inputs: &ForecastInputs) -> ForecastReading {
    let today = inputs.today;

    // Diário típico dos dias restantes do mês: REGRA, não carga. Nasce aqui, depois de qualquer
    // transformação sobre os insumos — é o que faz um gasto hipotético num dia futuro ocupar a vaga
    // do teto daquele dia em vez de somar por cima dele.
    let cash_events = with_projected_daily(inputs);
    let metric_events = with_projected_daily_metrics(inputs);

    // UMA projeção, com métricas. O saldo do fim do mês, a trajetória, o déficit mais profundo e
    // as métricas mensais são recortes deste mesmo `Forecast` — nenhum campo abaixo projeta de novo.
    let forecast = forecast::project_with_metrics(
        inputs.seed_cents,
        today,
        &cash_events,
        &metric_events,
        inputs.horizon_end,
        &inputs.economia_annotation,
    );

    // O herói do dashboard: o fim do mês corrente da mesma projeção. Sem mês fechado no horizonte
    // (horizonte de um dia no fim do mês), o último ponto da trajetória; sem trajetória, a semente.
    let projected_month_end_cents = forecast
        .month_end
        .iter()
        .find(|m| m.year == today.year() && m.month == today.month())
        .map(|m| m.balance_cents)
        .or_else(|| forecast.daily.last().map(|p| p.balance_cents))
        .unwrap_or(inputs.seed_cents);

    // UMA varredura do ano: a régua, o gate do cartão e a poupança anual leem destes doze meses.
    let ruler = forecast::annual_ruler(&inputs.annual.year_metrics, today.year(), today);
    let annual = AnnualReading {
        economia_bps: ruler.lived_bps,
        economia_state: if inputs.annual.registered_economia_cents > 0 {
            "verdict"
        } else {
            "no_record"
        },
        registered_economia_cents: inputs.annual.registered_economia_cents,
        registered_patrimonio_cents: inputs.annual.registered_patrimonio_cents,
        registered_income_cents: inputs.annual.registered_income_cents,
        projected_income_cents: inputs.annual.projected_income_cents,
        projected_savings_cents: inputs.annual.projected_net_cents,
        year_metrics: inputs.annual.year_metrics.clone(),
        ruler,
    };

    let coverage = compose_coverage(&forecast, today, inputs.baseline.monthly_cents);
    let reserve = compose_reserve(&inputs.reserve, &inputs.baseline);
    // Os meses de reserva, resolvidos UMA vez: o guardrail e o gate do cartão leem o mesmo
    // `Option` — sem reserva conhecida, nenhum dos dois inventa um número.
    let reserve_months = (reserve.state != "no_record").then_some(reserve.months);

    // O guardrail duplo mora no motor, e a régua da economia que ele consulta é a MESMA que a
    // tela do ano julga: entra a régua inteira, não uma renda e uma economia recompostas aqui.
    let safe_to_spend = forecast::safe_to_spend_today(&forecast, &annual.ruler, reserve_months);

    let spending_mode = SpendingModeReading {
        mode: forecast::detect_spending_mode(&inputs.spending_mode.samples),
        detected: forecast::spending_mode_is_detected(&inputs.spending_mode.samples),
        cartao_month_cents: inputs.spending_mode.cartao_month_cents,
    };

    let cards = compose_cards(inputs, annual.economia_bps, reserve_months);

    ForecastReading {
        today,
        horizon_end: inputs.horizon_end,
        forecast,
        projected_month_end_cents,
        annual,
        safe_to_spend,
        coverage,
        reserve,
        ceiling: CeilingReading {
            per_day_cents: inputs.ceiling.per_day_cents,
            source: inputs.ceiling.source,
            estimate_basis: inputs.ceiling.estimate_basis.clone(),
            proposal_pending: inputs.ceiling.proposal_pending,
        },
        spending_mode,
        cards,
        today_spend: inputs.today_spend,
        ledger: inputs.ledger.clone(),
    }
}

/// O encadeamento de caixa com o Diário típico dos dias restantes do mês por cima. O teto/dia entra
/// como DRIVER da projeção, para o saldo projetado e a Performance não nascerem otimistas — e só
/// nos dias que ainda não têm Diário lançado, para nunca dobrar o gasto do dia.
fn with_projected_daily(inputs: &ForecastInputs) -> Vec<forecast::CashflowEvent> {
    let days_with_daily: std::collections::HashSet<NaiveDate> = inputs
        .cash_events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    let mut events = inputs.cash_events.clone();
    events.extend(forecast::project_daily_ceiling(
        inputs.ceiling.projection_per_day_cents,
        inputs.today,
        inputs.horizon_end,
        &days_with_daily,
    ));
    events
}

/// O mesmo teto no stream de MÉTRICAS. A cobertura de dias é fato COMPORTAMENTAL (o dia teve
/// registro de Diário), sem máscara: um dia coberto por gasto excluído de alguma régua não recebe
/// dupla projeção. O teto projetado é evento SINTÉTICO → conta em todas as réguas.
fn with_projected_daily_metrics(inputs: &ForecastInputs) -> Vec<forecast::MetricEvent> {
    let days_with_daily: std::collections::HashSet<NaiveDate> = inputs
        .metric_events
        .iter()
        .filter(|me| me.event.kind == forecast::EventKind::Daily)
        .map(|me| me.event.date)
        .collect();
    let mut events = inputs.metric_events.clone();
    events.extend(forecast::lift_all(&forecast::project_daily_ceiling(
        inputs.ceiling.projection_per_day_cents,
        inputs.today,
        inputs.horizon_end,
        &days_with_daily,
    )));
    events
}

/// Cobertura dos meses futuros e o "confiável até" — uma leitura só, para a previsibilidade não
/// contar duas histórias. Sem baseline não há o que afirmar: `None`, nunca um mês fabricado.
fn compose_coverage(
    forecast: &forecast::Forecast,
    today: NaiveDate,
    baseline_outflow_cents: i64,
) -> CoverageReading {
    let months = forecast::month_coverage(
        &forecast.months,
        today,
        baseline_outflow_cents,
        COVERAGE_COMPLETE_BPS,
    );
    // Com baseline, o mês corrente é sempre confiável (tem o realizado) e a confiança estende
    // pelos meses futuros completos até o primeiro incompleto.
    let trusted_through_month = (baseline_outflow_cents > 0).then(|| {
        let mut trusted = format!("{:04}-{:02}", today.year(), today.month());
        for c in months.iter().take_while(|c| c.is_complete) {
            trusted = format!("{:04}-{:02}", c.year, c.month);
        }
        trusted
    });
    let total_missing_cents = months
        .iter()
        .filter(|c| !c.is_complete)
        .map(|c| c.estimated_missing_cents)
        .sum();

    CoverageReading {
        months,
        baseline_outflow_cents,
        trusted_through_month,
        total_missing_cents,
    }
}

/// A reserva em meses e seu estado epistêmico. Sem conta mapeada ou sem custo de vida não há
/// número honesto (sem registro); contas mapeadas e zeradas são o alerta legítimo.
fn compose_reserve(reserve: &ReserveInputs, baseline: &BaselineInputs) -> ReserveReading {
    let balance_cents = reserve.balance_cents;
    let baseline_cents = baseline.monthly_cents;
    let months = if baseline_cents > 0 {
        balance_cents as f64 / baseline_cents as f64
    } else {
        0.0
    };
    let state = if !reserve.has_accounts || baseline_cents <= 0 {
        "no_record"
    } else if balance_cents == 0 {
        "zero"
    } else if baseline.months >= forecast::RESERVE_MIN_MONTHS {
        "verdict"
    } else {
        "estimate"
    };
    // O alvo é custo de vida × meses mínimos; o excedente só existe depois de alcançado.
    let target_cents = baseline_cents * forecast::RESERVE_MIN_MONTHS;
    let surplus_cents =
        (target_cents > 0 && balance_cents > target_cents).then(|| balance_cents - target_cents);

    ReserveReading {
        balance_cents,
        baseline_cents,
        basis_months: baseline.months,
        months,
        state,
        trend: reserve.trend.clone(),
        target_cents,
        surplus_cents,
    }
}

/// O gate do cartão e as faturas. As pernas leem evidência JÁ resolvida — o percentual da régua
/// anual e os meses de reserva da leitura — para o gate nunca declarar viva uma economia que a
/// tela do ano mostra abaixo da faixa.
fn compose_cards(
    inputs: &ForecastInputs,
    economia_bps: Option<i64>,
    reserve_months: Option<f64>,
) -> CardReading {
    let gate_economy = cards::economy_gate_leg(economia_bps);
    let gate_reserve = cards::reserve_gate_leg(reserve_months);

    // Fatura zerada preserva a estrutura mensal, não um compromisso: nem ocupa a vaga da real na
    // lista, nem entra no total do próximo vencimento. Um predicado só, lido pelos dois.
    let committed = || {
        inputs
            .cards
            .active_invoices
            .iter()
            .filter(|invoice| invoice.amount_cents != 0)
    };

    let mut seen_accounts = std::collections::HashSet::new();
    let upcoming_invoices: Vec<UpcomingInvoice> = committed()
        .filter(|invoice| seen_accounts.insert(invoice.account_id.clone()))
        .map(|invoice| UpcomingInvoice {
            account_id: invoice.account_id.clone(),
            card_name: invoice.card_name.clone(),
            owner_name: invoice.owner_name.clone(),
            due_date: invoice.due_date,
            amount_cents: invoice.amount_cents,
            status: cards::invoice_status(inputs.today, invoice.closing_date, invoice.due_date),
            has_refund_expectation: invoice.has_refund_expectation,
            refund_expected_cents: invoice.refund_expected_cents,
        })
        .collect();

    // Com cartão cadastrado, o próximo vencimento é o das faturas persistidas (somando as que
    // vencem no mesmo dia). Sem cartão, o fallback é o dia de fatura declarado pela planilha —
    // quem gasta tudo no crédito e ainda não cadastrou o cartão não pode ser lido como débito.
    let next_fatura = if inputs.cards.has_card {
        committed().next().map(|first| {
            let amount_cents = committed()
                .filter(|invoice| invoice.due_date == first.due_date)
                .map(|invoice| invoice.amount_cents)
                .sum();
            (first.due_date, amount_cents)
        })
    } else {
        inputs.spending_mode.next_fatura
    };

    CardReading {
        gate: cards::compose_card_gate(gate_economy, gate_reserve),
        gate_economy,
        gate_reserve,
        gate_economy_bps: economia_bps,
        upcoming_invoices,
        next_fatura,
    }
}

/// A comparação entre duas leituras: o mundo como está e o mundo com a mudança, com as diferenças
/// já subtraídas.
///
/// Nasce de um `diff` entre duas `ForecastReading`, e por isso o conjunto de campos comparados é
/// completo por construção: cada figura do "depois" tem o "antes" que a leitura de produção
/// publica, não um número recalculado por outro caminho.
pub(crate) struct ProjectionComparison {
    pub real_horizon_end: NaiveDate,
    pub real_month_end: Vec<MonthEndDto>,
    pub real_deepest_deficit: Option<DayPointDto>,
    pub real_performance_cents: i64,
    pub real_safe_to_spend_today_cents: i64,
    pub real_binding_guardrail: String,
    pub real_cost_of_living_cents: i64,
    pub real_income_cents: i64,

    pub scenario_month_end: Vec<MonthEndDto>,
    pub scenario_deepest_deficit: Option<DayPointDto>,
    pub scenario_performance_cents: i64,
    pub scenario_safe_to_spend_today_cents: i64,
    pub scenario_binding_guardrail: String,
    pub scenario_cost_of_living_cents: i64,
    pub scenario_income_cents: i64,

    pub month_end: Vec<ScenarioMonthEnd>,
    pub deepest_deficit_delta_cents: Option<i64>,
    pub performance_delta_cents: i64,
    pub safe_to_spend_delta_cents: i64,
    pub cost_of_living_delta_cents: i64,
}

/// Compara duas leituras compostas pela MESMA função. Pura: dois recortes entram, as diferenças
/// saem — nenhuma projeção acontece aqui.
pub(crate) fn diff(real: &ForecastReading, scenario: &ForecastReading) -> ProjectionComparison {
    let scenario_month_end = month_end_dtos(scenario);
    // O horizonte do cenário pode ultrapassar o real (uma parcela distante). Depois do último dia
    // pré-lançado não existe evento real nenhum, então o saldo real dos meses extras permanece o do
    // último fim de mês projetado — é o que o par velho→novo precisa para não perder linhas.
    let real_month_end = carry_through(month_end_dtos(real), &scenario_month_end);

    let month_end = real_month_end
        .iter()
        .filter_map(|r| {
            scenario_month_end
                .iter()
                .find(|s| s.year == r.year && s.month == r.month)
                .map(|s| ScenarioMonthEnd {
                    year: r.year,
                    month: r.month,
                    real_balance_cents: r.balance_cents,
                    scenario_balance_cents: s.balance_cents,
                    delta_cents: s.balance_cents - r.balance_cents,
                })
        })
        .collect();

    let real_cost_of_living_cents = current_month(real, |m| m.cost_of_living_cents);
    let scenario_cost_of_living_cents = current_month(scenario, |m| m.cost_of_living_cents);
    let real_performance_cents = current_month(real, |m| m.performance_cents);
    let scenario_performance_cents = current_month(scenario, |m| m.performance_cents);

    ProjectionComparison {
        real_horizon_end: real.horizon_end,
        real_month_end,
        real_deepest_deficit: deepest_deficit_dto(real),
        real_performance_cents,
        real_safe_to_spend_today_cents: real.safe_to_spend.amount_cents,
        real_binding_guardrail: real.safe_to_spend.binding.as_str().to_string(),
        real_cost_of_living_cents,
        real_income_cents: current_month(real, |m| m.income_cents),

        scenario_month_end,
        scenario_deepest_deficit: deepest_deficit_dto(scenario),
        scenario_performance_cents,
        scenario_safe_to_spend_today_cents: scenario.safe_to_spend.amount_cents,
        scenario_binding_guardrail: scenario.safe_to_spend.binding.as_str().to_string(),
        scenario_cost_of_living_cents,
        scenario_income_cents: current_month(scenario, |m| m.income_cents),

        month_end,
        deepest_deficit_delta_cents: match (
            &real.forecast.deepest_deficit,
            &scenario.forecast.deepest_deficit,
        ) {
            (Some(r), Some(s)) => Some(s.balance_cents - r.balance_cents),
            _ => None,
        },
        performance_delta_cents: scenario_performance_cents - real_performance_cents,
        safe_to_spend_delta_cents: scenario.safe_to_spend.amount_cents
            - real.safe_to_spend.amount_cents,
        cost_of_living_delta_cents: scenario_cost_of_living_cents - real_cost_of_living_cents,
    }
}

/// Uma figura do mês CORRENTE (custo de vida, performance, renda) na definição canônica do motor.
/// `0` se o mês corrente não aparece nos meses do horizonte — o que não acontece, já que `today`
/// sempre inicia a projeção.
fn current_month(reading: &ForecastReading, figure: impl Fn(&forecast::MonthMetric) -> i64) -> i64 {
    reading
        .forecast
        .months
        .iter()
        .find(|m| m.year == reading.today.year() && m.month == reading.today.month())
        .map(figure)
        .unwrap_or(0)
}

fn month_end_dtos(reading: &ForecastReading) -> Vec<MonthEndDto> {
    reading
        .forecast
        .month_end
        .iter()
        .map(|m| MonthEndDto {
            year: m.year,
            month: m.month,
            balance_cents: m.balance_cents,
        })
        .collect()
}

/// Estende a série mais curta até cobrir os meses da outra, repetindo o último saldo conhecido.
fn carry_through(mut months: Vec<MonthEndDto>, reference: &[MonthEndDto]) -> Vec<MonthEndDto> {
    for month in reference {
        if months
            .iter()
            .any(|m| m.year == month.year && m.month == month.month)
        {
            continue;
        }
        let balance_cents = months.last().map(|m| m.balance_cents).unwrap_or(0);
        months.push(MonthEndDto {
            year: month.year,
            month: month.month,
            balance_cents,
        });
    }
    months
}

fn deepest_deficit_dto(reading: &ForecastReading) -> Option<DayPointDto> {
    reading.forecast.deepest_deficit.map(|p| DayPointDto {
        date: p.date.format("%Y-%m-%d").to_string(),
        balance_cents: p.balance_cents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast::{CashflowEvent, EventKind, MetricEvent, MonthMetric, RulerMask};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// Um mês do ano com renda e Economia — o par que a régua anual divide.
    fn month(year: i32, month: u32, income_cents: i64, economia_cents: i64) -> MonthMetric {
        MonthMetric {
            year,
            month,
            income_cents,
            economia_cents,
            ..Default::default()
        }
    }

    fn cash(date: &str, kind: EventKind, amount_cents: i64) -> CashflowEvent {
        CashflowEvent {
            date: d(date),
            kind,
            amount_cents,
            // A data decide realizado × previsão nas métricas; o flag só marca projeção sintética.
            realized: false,
        }
    }

    fn metric(date: &str, kind: EventKind, amount_cents: i64, mask: RulerMask) -> MetricEvent {
        MetricEvent {
            event: cash(date, kind, amount_cents),
            mask,
        }
    }

    // --- Regressões nomeadas ---

    // O saldo projetado do fim do mês é UM campo: o recorte do dashboard e a trajetória do
    // forecast leem o mesmo `Forecast`. Uma segunda projeção — só de caixa, para o mesmo mês —
    // seria outro motor respondendo à mesma pergunta, e o lugar onde os dois números divergiriam.
    #[test]
    fn projected_month_end_is_one_field_shared_by_dashboard_and_forecast() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-07-31");
        inputs.seed_cents = 500_000;
        inputs.cash_events = vec![cash("2026-06-20", EventKind::Daily, 100_000)];

        let reading = compose(&inputs);

        let from_trajectory = reading
            .forecast
            .month_end
            .iter()
            .find(|m| (m.year, m.month) == (2026, 6))
            .expect("o mês corrente fecha dentro do horizonte")
            .balance_cents;
        assert_eq!(reading.projected_month_end_cents, 400_000);
        assert_eq!(
            reading.projected_month_end_cents, from_trajectory,
            "o herói do dashboard É o fim de mês da trajetória, não uma segunda projeção"
        );
    }

    // O teto do dia e a tela do ano leem a MESMA régua: a folga da economia é o inverso do
    // déficit até o piso que `annual_ruler` já calculou, sobre o recorte que ele julga. As
    // figuras da janela de meses completos são retrato publicado ao lado — não alimentam o teto.
    #[test]
    fn the_ceiling_derives_from_the_same_ruler_the_year_screen_judges() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 1_000_000;
        // Recorte vivido da régua: renda 600.000, Economia 150.000 (25%) — o mês em curso conta.
        inputs.annual.year_metrics = vec![month(2026, 6, 600_000, 150_000)];
        // A janela de meses COMPLETOS diz outra coisa, e o teto não a escuta.
        inputs.annual.registered_income_cents = 400_000;
        inputs.annual.registered_economia_cents = 0;

        let reading = compose(&inputs);

        assert_eq!(
            reading.safe_to_spend.savings_headroom_cents,
            Some(-reading.annual.ruler.judged_shortfall_cents()),
            "a folga É o déficit da régua com o sinal trocado, no mesmo recorte"
        );
        // 20% de 600.000 = 120.000 contra 150.000 guardados.
        assert_eq!(reading.safe_to_spend.savings_headroom_cents, Some(30_000));
        assert_eq!(
            reading.safe_to_spend.binding,
            forecast::Guardrail::Savings,
            "com a faixa viva e o caixa folgado, quem limita o dia é a economia"
        );
        assert_eq!(reading.safe_to_spend.amount_cents, 30_000);
    }

    // O Economizado% do ano é um campo só, truncado uma vez. Régua anual, gate do cartão e DTO
    // leem dele — nenhum deles redivide a economia pela renda com outro arredondamento.
    #[test]
    fn annual_savings_percentage_is_one_truncated_field() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        // 21,9% → 2190 bps truncados. Um arredondamento paralelo publicaria 22%.
        inputs.annual.year_metrics = vec![month(2026, 3, 100_000, 21_900)];

        let reading = compose(&inputs);

        assert_eq!(reading.annual.economia_bps, Some(2_190));
        assert_eq!(
            reading.annual.economia_bps, reading.annual.ruler.lived_bps,
            "a régua anual e o campo publicado são a mesma divisão"
        );
        assert_eq!(
            reading.cards.gate_economy_bps, reading.annual.economia_bps,
            "o gate do cartão julga o percentual que a tela do ano mostra"
        );
        assert_eq!(reading.cards.gate_economy, GateLeg::Alive);
    }

    // O gasto do dia obedece à máscara de réguas: ele chega mascarado da origem e a composição o
    // publica como está. Uma linha fora da régua do Diário não conta no dia e continua pesando no
    // caixa — sem contabilidade paralela em lugar nenhum.
    #[test]
    fn today_spend_obeys_the_ruler_mask_without_parallel_accounting() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 100_000;
        inputs.today_spend.daily_avg_cents = 4_000;
        // A linha excluída da régua do Diário: fora da máscara `daily_avg`, dentro do caixa.
        inputs.cash_events = vec![cash("2026-06-15", EventKind::Daily, 9_000)];
        inputs.metric_events = vec![metric(
            "2026-06-15",
            EventKind::Daily,
            9_000,
            RulerMask {
                daily_avg: false,
                ..RulerMask::ALL
            },
        )];

        let reading = compose(&inputs);

        assert_eq!(
            reading.today_spend.daily_avg_cents, 4_000,
            "o dia conta só o que a régua do Diário mede"
        );
        let june = reading
            .forecast
            .months
            .iter()
            .find(|m| (m.year, m.month) == (2026, 6))
            .expect("o mês corrente tem métricas");
        assert_eq!(
            june.daily_avg_out_cents, 0,
            "a linha excluída some da régua do Diário"
        );
        assert_eq!(
            reading.forecast.daily[0].balance_cents, 91_000,
            "e continua pesando no caixa"
        );
    }

    // --- Bordas, todas sem banco ---

    // Ano sem renda: a régua não fabrica zero. Sem denominador não há percentual, o guardrail de
    // poupança fica inativo e só o caixa decide — e o gate do cartão não aprova por omissão.
    #[test]
    fn a_year_without_income_yields_no_percentage_and_an_inactive_savings_guardrail() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 300_000;

        let reading = compose(&inputs);

        assert_eq!(reading.annual.economia_bps, None);
        assert_eq!(reading.annual.economia_state, "no_record");
        assert_eq!(reading.safe_to_spend.savings_headroom_cents, None);
        assert_eq!(reading.safe_to_spend.binding, forecast::Guardrail::Cash);
        assert_eq!(reading.cards.gate_economy, GateLeg::Unknown);
        assert_eq!(reading.cards.gate, GateLeg::Unknown);
    }

    // Mês corrente sem lastro: sem gasto típico não há cobertura a julgar nem "confiável até" a
    // afirmar. A previsibilidade fica indeterminada em vez de otimista.
    #[test]
    fn a_month_without_baseline_leaves_predictability_undetermined() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-08-31");
        inputs.baseline.monthly_cents = 0;
        inputs.baseline.months = 0;

        let reading = compose(&inputs);

        assert!(reading.coverage.months.is_empty());
        assert_eq!(reading.coverage.trusted_through_month, None);
        assert_eq!(reading.coverage.total_missing_cents, 0);
    }

    // Reserva sem conta mapeada é falta de registro, não zero. E a perna de reserva do gate fica
    // desconhecida — ausência de dado nunca vira aprovação.
    #[test]
    fn a_reserve_without_mapped_accounts_is_no_record_not_zero() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.baseline.monthly_cents = 200_000;
        inputs.baseline.months = 6;
        inputs.reserve.has_accounts = false;

        let reading = compose(&inputs);

        assert_eq!(reading.reserve.state, "no_record");
        assert_eq!(reading.reserve.months, 0.0);
        assert_eq!(reading.reserve.surplus_cents, None);
        assert_eq!(reading.cards.gate_reserve, GateLeg::Unknown);
    }

    // Contas de reserva mapeadas e zeradas: o alerta legítimo, distinto da falta de registro.
    #[test]
    fn mapped_and_empty_reserve_accounts_are_the_legitimate_alert() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.baseline.monthly_cents = 200_000;
        inputs.baseline.months = 6;
        inputs.reserve.has_accounts = true;
        inputs.reserve.balance_cents = 0;

        let reading = compose(&inputs);

        assert_eq!(reading.reserve.state, "zero");
        assert_eq!(reading.reserve.target_cents, 200_000 * 6);
        assert_eq!(reading.cards.gate_reserve, GateLeg::Below);
    }

    // Horizonte de um dia (o piso do fim do mês): sem mês fechado na trajetória, o saldo do fim do
    // mês é o último ponto projetado — nunca a semente sem os eventos do dia.
    #[test]
    fn a_one_day_horizon_still_publishes_a_projected_month_end() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-30"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 250_000;
        inputs.cash_events = vec![cash("2026-06-30", EventKind::Income, 50_000)];

        let reading = compose(&inputs);

        assert_eq!(reading.forecast.daily.len(), 1);
        assert_eq!(reading.projected_month_end_cents, 300_000);
    }

    // Teto sem registro: travessão e CTA da cerimônia, não um zero apresentado como escolha.
    #[test]
    fn a_ceiling_without_record_keeps_its_absent_provenance() {
        let inputs = ForecastInputs::minimal(d("2026-06-15"));

        let reading = compose(&inputs);

        assert_eq!(reading.ceiling.source, CeilingSource::None);
        assert_eq!(reading.ceiling.per_day_cents, 0);
        assert!(reading.ceiling.estimate_basis.is_none());
        assert!(!reading.ceiling.proposal_pending);
    }

    // Sem sinal nenhum na janela, o modo de gasto é o DEFAULT do gesto-base — e a leitura diz que
    // não foi detectado, para a tela não apresentar o default como leitura dos dados.
    #[test]
    fn an_empty_window_yields_the_default_spending_mode_marked_undetected() {
        let inputs = ForecastInputs::minimal(d("2026-06-15"));

        let reading = compose(&inputs);

        assert_eq!(reading.spending_mode.mode, forecast::SpendingMode::Debit);
        assert!(!reading.spending_mode.detected);
    }

    // A cobertura estende o "confiável até" pelos meses futuros completos e para no primeiro
    // incompleto — o que falta lançar é somado só dos incompletos.
    #[test]
    fn coverage_extends_trust_until_the_first_incomplete_month() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-08-31");
        inputs.baseline.monthly_cents = 100_000;
        inputs.baseline.months = 6;
        // Julho lançado acima do limiar; agosto quase vazio.
        inputs.metric_events = vec![
            metric("2026-07-10", EventKind::FixedOut, 90_000, RulerMask::ALL),
            metric("2026-08-10", EventKind::FixedOut, 10_000, RulerMask::ALL),
        ];

        let reading = compose(&inputs);

        assert_eq!(
            reading.coverage.trusted_through_month.as_deref(),
            Some("2026-07")
        );
        assert_eq!(reading.coverage.total_missing_cents, 90_000);
    }

    // O gate do cartão só aprova com as DUAS pernas vivas, e a leitura publica a matemática por
    // trás da perna de economia, não só o veredito.
    #[test]
    fn the_card_gate_needs_both_legs_alive() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.annual.year_metrics = vec![month(2026, 5, 100_000, 25_000)];
        inputs.baseline.monthly_cents = 100_000;
        inputs.baseline.months = 6;
        inputs.reserve.has_accounts = true;
        inputs.reserve.balance_cents = 700_000;

        let reading = compose(&inputs);

        assert_eq!(reading.annual.economia_bps, Some(2_500));
        assert_eq!(reading.reserve.state, "verdict");
        assert_eq!(reading.cards.gate_economy, GateLeg::Alive);
        assert_eq!(reading.cards.gate_reserve, GateLeg::Alive);
        assert_eq!(reading.cards.gate, GateLeg::Alive);
        assert_eq!(reading.reserve.surplus_cents, Some(100_000));
    }

    // Faturas: uma vaga por cartão, zerada não ocupa vaga, e o próximo vencimento soma as faturas
    // que vencem no mesmo dia.
    #[test]
    fn upcoming_invoices_keep_one_slot_per_card_and_sum_the_same_due_date() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.cards.has_card = true;
        inputs.cards.active_invoices = vec![
            invoice("acc-a", "2026-06-20", "2026-06-28", 0),
            invoice("acc-a", "2026-06-20", "2026-06-28", 120_000),
            invoice("acc-b", "2026-06-20", "2026-06-28", 30_000),
            invoice("acc-a", "2026-07-20", "2026-07-28", 90_000),
        ];

        let reading = compose(&inputs);

        assert_eq!(reading.cards.upcoming_invoices.len(), 2);
        assert_eq!(reading.cards.upcoming_invoices[0].account_id, "acc-a");
        assert_eq!(reading.cards.upcoming_invoices[0].amount_cents, 120_000);
        assert_eq!(
            reading.cards.upcoming_invoices[0].status,
            InvoiceStatus::Aberta
        );
        assert_eq!(
            reading.cards.next_fatura,
            Some((d("2026-06-28"), 150_000)),
            "o dia do vencimento soma as faturas que vencem nele"
        );
    }

    // Sem cartão cadastrado, o próximo dia de fatura vem do sinal declarado pela planilha — quem
    // gasta tudo no crédito e ainda não cadastrou o cartão não é lido como usuário de débito.
    #[test]
    fn without_a_registered_card_the_next_invoice_falls_back_to_the_sheet_signal() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.cards.has_card = false;
        inputs.spending_mode.next_fatura = Some((d("2026-06-25"), 80_000));

        let reading = compose(&inputs);

        assert!(reading.cards.upcoming_invoices.is_empty());
        assert_eq!(reading.cards.next_fatura, Some((d("2026-06-25"), 80_000)));
    }

    // --- Cenário: a comparação é um diff entre duas leituras compostas ---

    // O "antes" de um cenário é a leitura de PRODUÇÃO, campo a campo: mesma função, mesmos
    // insumos. É o que garante que o dono lê o "depois" como consequência da mudança, e não como
    // consequência de outra conta.
    #[test]
    fn the_before_of_a_scenario_is_the_production_reading_field_by_field() {
        let inputs = scenario_inputs();
        let real = compose(&inputs);
        let scenario = compose(&apply_scenario(&inputs, &a_new_bill()));

        let comparison = diff(&real, &scenario);

        assert_eq!(comparison.real_horizon_end, real.horizon_end);
        assert_eq!(
            comparison.real_safe_to_spend_today_cents,
            real.safe_to_spend.amount_cents
        );
        assert_eq!(
            comparison.real_binding_guardrail,
            real.safe_to_spend.binding.as_str()
        );
        assert_eq!(
            comparison.real_month_end[0].balance_cents,
            real.projected_month_end_cents
        );
        assert_eq!(
            comparison.real_cost_of_living_cents,
            real.forecast.months[0].cost_of_living_cents
        );
    }

    // A economia anual do guardrail atravessa o cenário intacta: um "e se" reprojeta o caixa e as
    // réguas do mês, nunca reescreve o ano já realizado. Os dois lados julgam pela MESMA janela de
    // meses completos que os insumos declaram — a mesma que o forecast usa.
    #[test]
    fn the_guardrail_savings_window_is_the_same_on_both_sides_of_the_diff() {
        let mut inputs = scenario_inputs();
        inputs.annual.registered_income_cents = 400_000;
        inputs.annual.registered_economia_cents = 100_000;

        let real = compose(&inputs);
        let scenario = compose(&apply_scenario(&inputs, &a_new_bill()));

        assert_eq!(
            scenario.annual.registered_economia_cents,
            real.annual.registered_economia_cents
        );
        assert_eq!(
            scenario.safe_to_spend.savings_headroom_cents,
            real.safe_to_spend.savings_headroom_cents,
            "a folga da poupança não muda: o cenário não reescreve o ano realizado"
        );
    }

    // As diferenças são a subtração dos dois recortes, e um gasto novo empobrece o mês: o saldo do
    // fim do mês cai, o custo de vida sobe e o dia encolhe.
    #[test]
    fn the_deltas_are_the_subtraction_of_the_two_readings() {
        let inputs = scenario_inputs();
        let real = compose(&inputs);
        let scenario = compose(&apply_scenario(&inputs, &a_new_bill()));

        let comparison = diff(&real, &scenario);

        assert_eq!(comparison.month_end[0].delta_cents, -80_000);
        assert_eq!(
            comparison.month_end[0].scenario_balance_cents
                - comparison.month_end[0].real_balance_cents,
            comparison.month_end[0].delta_cents
        );
        assert_eq!(comparison.cost_of_living_delta_cents, 80_000);
        assert_eq!(comparison.performance_delta_cents, -80_000);
        assert!(comparison.safe_to_spend_delta_cents <= 0);
    }

    // Uma parcela distante estica o horizonte do cenário. Depois do último dia pré-lançado não
    // existe evento real nenhum, então o saldo do "antes" desses meses é o do último fim de mês
    // projetado — o par velho→novo não perde linhas por o real terminar antes.
    #[test]
    fn months_beyond_the_real_horizon_pair_with_the_carried_real_balance() {
        let inputs = scenario_inputs();
        let mut changes = a_new_bill();
        changes.horizon_end = Some(d("2026-08-31"));
        changes
            .chain_added
            .push(cash("2026-08-10", EventKind::FixedOut, 20_000));
        changes.metric_added.push(metric(
            "2026-08-10",
            EventKind::FixedOut,
            20_000,
            RulerMask::ALL,
        ));

        let comparison = diff(
            &compose(&inputs),
            &compose(&apply_scenario(&inputs, &changes)),
        );

        let august = comparison
            .month_end
            .iter()
            .find(|m| (m.year, m.month) == (2026, 8))
            .expect("o mês da parcela distante entra na comparação");
        let june = &comparison.month_end[0];
        assert_eq!(
            august.real_balance_cents, june.real_balance_cents,
            "sem evento real depois do horizonte, o saldo do antes permanece"
        );
        assert_eq!(august.delta_cents, -100_000);
    }

    // Um gasto hipotético num dia futuro OCUPA a vaga do teto daquele dia em vez de somar por
    // cima dele: o Diário típico é regra da composição, refeita depois da transformação. Somar os
    // dois cobraria o dia duas vezes.
    #[test]
    fn a_hypothetical_daily_takes_the_ceiling_slot_of_its_day() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 1_000_000;
        inputs.ceiling.projection_per_day_cents = 2_000;

        let real = compose(&inputs);
        let scenario = compose(&apply_scenario(
            &inputs,
            &ScenarioChanges {
                chain_added: vec![cash("2026-06-20", EventKind::Daily, 50_000)],
                metric_added: vec![metric(
                    "2026-06-20",
                    EventKind::Daily,
                    50_000,
                    RulerMask::ALL,
                )],
                ..ScenarioChanges::default()
            },
        ));

        let comparison = diff(&real, &scenario);
        assert_eq!(
            comparison.month_end[0].delta_cents,
            -(50_000 - 2_000),
            "o dia troca o teto pelo gasto hipotético, não acumula os dois"
        );
    }

    /// Insumos de um mês com renda lançada e uma conta fixa futura — o mundo "antes" dos cenários.
    fn scenario_inputs() -> ForecastInputs {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 1_000_000;
        inputs.cash_events = vec![cash("2026-06-25", EventKind::FixedOut, 150_000)];
        inputs.metric_events = vec![metric(
            "2026-06-25",
            EventKind::FixedOut,
            150_000,
            RulerMask::ALL,
        )];
        inputs
    }

    /// Uma conta nova de 800,00 no fim do mês: a mudança hipotética dos casos acima.
    fn a_new_bill() -> ScenarioChanges {
        ScenarioChanges {
            chain_added: vec![cash("2026-06-28", EventKind::FixedOut, 80_000)],
            metric_added: vec![metric(
                "2026-06-28",
                EventKind::FixedOut,
                80_000,
                RulerMask::ALL,
            )],
            ..ScenarioChanges::default()
        }
    }

    fn invoice(
        account_id: &str,
        closing_date: &str,
        due_date: &str,
        amount_cents: i64,
    ) -> CardInvoiceEvent {
        CardInvoiceEvent {
            account_id: account_id.to_string(),
            card_name: account_id.to_string(),
            owner_name: "Tester".to_string(),
            closing_date: d(closing_date),
            due_date: d(due_date),
            amount_cents,
            has_refund_expectation: false,
            refund_expected_cents: 0,
        }
    }
}
