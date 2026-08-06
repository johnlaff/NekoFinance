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
use crate::forecast;
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
    /// Meta do guardrail de poupança em bps — publicada ao lado do número que ela produziu.
    pub savings_target_bps: i64,
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
    /// Economia REGISTRADA da janela de meses completos — o numerador do guardrail.
    pub guardrail_economia_cents: i64,
    /// Patrimônio realizado da mesma janela, publicado ao lado da régua e nunca somado a ela.
    pub guardrail_patrimonio_cents: i64,
    /// Renda-base do guardrail: a janela de meses COMPLETOS, distinta do recorte vivido da régua.
    pub guardrail_income_cents: i64,
    pub projected_income_cents: i64,
    pub projected_savings_cents: i64,
    /// Os doze meses no tipo do motor, para quem lista o ano mês a mês.
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

    // UMA projeção, com métricas. O saldo do fim do mês, a trajetória, o déficit mais profundo e
    // as métricas mensais são recortes deste mesmo `Forecast` — nenhum campo abaixo projeta de novo.
    let forecast = forecast::project_with_metrics(
        inputs.seed_cents,
        today,
        &inputs.cash_events,
        &inputs.metric_events,
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
        economia_state: if inputs.annual.guardrail_economia_cents > 0 {
            "verdict"
        } else {
            "no_record"
        },
        guardrail_economia_cents: inputs.annual.guardrail_economia_cents,
        guardrail_patrimonio_cents: inputs.annual.guardrail_patrimonio_cents,
        guardrail_income_cents: inputs.annual.guardrail_income_cents,
        projected_income_cents: inputs.annual.projected_income_cents,
        projected_savings_cents: inputs.annual.projected_net_cents,
        year_metrics: inputs.annual.year_metrics.clone(),
        ruler,
    };

    // O guardrail duplo mora no motor; aqui só chegam a renda-base da janela de meses completos e
    // a Economia registrada da MESMA janela — uma derivação, não duas.
    let safe_to_spend = forecast::safe_to_spend_today(
        &forecast,
        annual.guardrail_income_cents,
        annual.guardrail_economia_cents,
        forecast::SAVINGS_TARGET_BPS,
    );

    let coverage = compose_coverage(&forecast, today, inputs.baseline.monthly_cents);
    let reserve = compose_reserve(&inputs.reserve, &inputs.baseline);

    let spending_mode = SpendingModeReading {
        mode: forecast::detect_spending_mode(&inputs.spending_mode.samples),
        detected: forecast::spending_mode_is_detected(&inputs.spending_mode.samples),
        cartao_month_cents: inputs.spending_mode.cartao_month_cents,
    };

    let cards = compose_cards(inputs, annual.economia_bps, &reserve);

    ForecastReading {
        today,
        horizon_end: inputs.horizon_end,
        forecast,
        projected_month_end_cents,
        annual,
        safe_to_spend,
        savings_target_bps: forecast::SAVINGS_TARGET_BPS,
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
    reserve: &ReserveReading,
) -> CardReading {
    let reserve_months = (reserve.state != "no_record").then_some(reserve.months);
    let gate_economy = cards::economy_gate_leg(economia_bps);
    let gate_reserve = cards::reserve_gate_leg(reserve_months);

    let mut seen_accounts = std::collections::HashSet::new();
    let upcoming_invoices: Vec<UpcomingInvoice> = inputs
        .cards
        .active_invoices
        .iter()
        // Fatura zerada preserva a estrutura mensal, não um compromisso: não ocupa a vaga da real.
        .filter(|invoice| invoice.amount_cents != 0)
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
        inputs
            .cards
            .active_invoices
            .iter()
            .find(|invoice| invoice.amount_cents != 0)
            .map(|first| {
                let amount_cents = inputs
                    .cards
                    .active_invoices
                    .iter()
                    .filter(|invoice| {
                        invoice.due_date == first.due_date && invoice.amount_cents != 0
                    })
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

    // O saldo projetado do fim do mês é UM campo. O dashboard lia de uma projeção só de caixa e o
    // forecast de uma projeção com métricas: dois motores respondendo à mesma pergunta. Agora o
    // recorte de dashboard e a trajetória do forecast saem do mesmo `Forecast`, e não há segunda
    // projeção onde divergir.
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

    // A economia anual do guardrail tem UMA derivação: a janela de meses completos que chega pelo
    // campo próprio dos insumos. O ramo real do cenário lê o mesmo campo que o forecast — não há
    // como o "antes" de um cenário nascer de outra conta.
    #[test]
    fn guardrail_savings_come_from_the_dedicated_window_field() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-06-30");
        inputs.seed_cents = 1_000_000;
        // Janela de meses COMPLETOS: renda 400.000, Economia 100.000 (25%).
        inputs.annual.guardrail_income_cents = 400_000;
        inputs.annual.guardrail_economia_cents = 100_000;
        // Recorte VIVIDO da régua (o mês em curso incluído) é outra janela, de propósito.
        inputs.annual.year_metrics = vec![month(2026, 6, 600_000, 100_000)];

        let reading = compose(&inputs);

        assert_eq!(reading.annual.guardrail_income_cents, 400_000);
        assert_eq!(reading.annual.guardrail_economia_cents, 100_000);
        assert_eq!(
            reading.safe_to_spend.savings_headroom_cents,
            Some(100_000 - 400_000 * forecast::SAVINGS_TARGET_BPS / 10_000),
            "o guardrail divide a Economia da janela de meses completos pela renda da MESMA janela"
        );
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
