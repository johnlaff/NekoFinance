//! O inventário declarado da leitura do dia.
//!
//! `ForecastInputs` responde, com nome e tipo, à pergunta "que janela de dados alimenta que
//! argumento do motor". É um checklist: um insumo que não está aqui não existe para a composição,
//! e uma régua nova declara sua dependência acrescentando um campo em vez de abrir uma consulta
//! própria no meio da conta.

use crate::cards::CardLexicon;
use crate::forecast::{self, CashflowEvent};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};

/// Tudo que a projeção do dia precisa, carregado uma vez.
#[derive(Debug, Clone)]
pub(crate) struct ForecastInputs {
    /// O dia da leitura. Parâmetro, nunca relógio ambiente — é o que torna a composição
    /// determinística.
    pub today: NaiveDate,
    /// Último dia com dado pré-lançado (transação, Saldo importado ou fatura) ≥ hoje; piso no fim
    /// do mês corrente.
    pub horizon_end: NaiveDate,
    /// Semente da projeção: o saldo de partida do qual a engine encadeia o futuro.
    pub seed_cents: i64,
    /// Eventos de CAIXA do horizonte, sem máscara — o Saldo conta todo dinheiro que se move,
    /// mesmo o excluído de alguma régua. Só o que foi LANÇADO: o Diário típico dos dias restantes
    /// é regra, e a composição o injeta.
    pub cash_events: Vec<CashflowEvent>,
    /// Eventos de MÉTRICA do mês corrente ao horizonte, cada um com a máscara de réguas herdada
    /// das tags do lançamento-pai. A máscara vem da origem: nenhum consumidor a reconstrói.
    pub metric_events: Vec<forecast::MetricEvent>,
    /// Anotação da aba Economia por `(ano, mês)`, cobrindo os anos do horizonte e o ano corrente.
    pub economia_annotation: HashMap<(i32, u32), i64>,
    pub ceiling: CeilingInputs,
    pub annual: AnnualInputs,
    pub baseline: BaselineInputs,
    pub reserve: ReserveInputs,
    pub spending_mode: SpendingModeInputs,
    pub cards: CardInputs,
    pub today_spend: DailySpendInputs,
    pub ledger: LedgerInputs,
}

/// As mudanças hipotéticas de um cenário, já classificadas em eventos pela casca — a mesma
/// classificação que os insumos reais atravessaram.
///
/// Um cenário é sempre a mesma gramática: o que SAI da leitura (as células que uma supressão
/// atinge, como estão hoje) e o que ENTRA (as linhas hipotéticas e o que sobra de uma célula
/// parcialmente suprimida). Os dois streams aparecem separados porque as janelas deles são
/// diferentes na origem: o encadeamento de caixa parte de hoje (a semente já embute o passado) e a
/// métrica cobre o mês inteiro.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScenarioChanges {
    /// Eventos de CAIXA que o cenário retira do encadeamento.
    pub chain_removed: Vec<CashflowEvent>,
    /// Eventos de CAIXA que o cenário acrescenta ao encadeamento.
    pub chain_added: Vec<CashflowEvent>,
    /// Eventos de MÉTRICA que o cenário retira, com a máscara de réguas do lançamento-pai.
    pub metric_removed: Vec<forecast::MetricEvent>,
    /// Eventos de MÉTRICA que o cenário acrescenta. Linha hipotética não tem tag → conta em todas
    /// as réguas.
    pub metric_added: Vec<forecast::MetricEvent>,
    /// Horizonte esticado pela linha hipotética mais distante. `None` mantém o horizonte da
    /// leitura — sem ele, uma parcela além do horizonte sairia da projeção sem aviso.
    pub horizon_end: Option<NaiveDate>,
}

/// Aplica as mudanças de um cenário SOBRE os insumos já carregados, devolvendo outros insumos.
///
/// Pura: mesma entrada, mesma saída, sem banco e sem relógio. É essa pureza que faz o "antes" de um
/// cenário ser literalmente a leitura de produção — a comparação inteira é
/// `diff(compose(inputs), compose(apply_scenario(inputs, changes)))` — e que torna um cenário
/// testável ajustando campos de um construtor de insumos, sem fabricar um banco.
pub(crate) fn apply_scenario(inputs: &ForecastInputs, changes: &ScenarioChanges) -> ForecastInputs {
    let mut out = inputs.clone();
    out.horizon_end = changes
        .horizon_end
        .filter(|end| *end > inputs.horizon_end)
        .unwrap_or(inputs.horizon_end);

    out.cash_events = suppress(out.cash_events, &changes.chain_removed);
    out.metric_events = suppress(out.metric_events, &changes.metric_removed);

    // Precedência da fatura, a mesma da leitura real: com cartão cadastrado, uma compra de cartão
    // FUTURA já está contida no lump da fatura do ciclo — deixá-la entrar cobraria duas vezes.
    let has_card = inputs.cards.has_card;
    let today = inputs.today;
    let is_future_cartao =
        |e: &CashflowEvent| e.kind == forecast::EventKind::Cartao && e.date > today;
    out.cash_events.extend(
        changes
            .chain_added
            .iter()
            .filter(|e| !(has_card && is_future_cartao(e)))
            .cloned(),
    );
    out.metric_events.extend(
        changes
            .metric_added
            .iter()
            .filter(|me| !(has_card && is_future_cartao(&me.event)))
            .cloned(),
    );

    out
}

/// Retira magnitudes da série, em duas passagens: primeiro casando data, tipo E máscara de réguas;
/// depois, com o que sobrou da dívida, só data e tipo.
///
/// A supressão chega como os EVENTOS da célula atingida, não como um ponteiro para a linha: a
/// leitura já classificou (nota decomposta em itens, compra coberta por fatura descartada), e o
/// que sobreviveu à classificação é o que existe para ser retirado. Uma dívida sem par — a compra
/// que a fatura já substituiu — não retira nada, porque aquele dinheiro já não estava sendo contado
/// por este evento.
///
/// A máscara vem primeiro porque duas saídas do mesmo dia podem contar em réguas diferentes:
/// descontar da errada tiraria a conta suprimida do Saldo certo e do custo de vida errado.
fn suppress<T: Suppressible>(events: Vec<T>, removed: &[T]) -> Vec<T> {
    if removed.is_empty() {
        return events;
    }
    let mut debt: HashMap<SuppressionKey, i64> = HashMap::new();
    for item in removed {
        *debt.entry(item.key()).or_insert(0) += item.event().amount_cents.abs();
    }
    let (events, debt) = take_debt(events, debt, |item| item.key());

    // O resto da dívida — a célula cuja classificação mudou de máscara entre a leitura e a
    // reconstrução — cai na régua de quem tem a mesma data e o mesmo tipo.
    let loose: HashMap<SuppressionKey, i64> =
        debt.into_iter()
            .fold(HashMap::new(), |mut acc, (key, owed)| {
                *acc.entry(SuppressionKey { mask: None, ..key }).or_insert(0) += owed;
                acc
            });
    take_debt(events, loose, |item| SuppressionKey {
        mask: None,
        ..item.key()
    })
    .0
}

/// Uma passagem da supressão: desconta o que a chave deve, derruba o evento que zera e devolve a
/// dívida que sobrou.
fn take_debt<T: Suppressible>(
    events: Vec<T>,
    mut debt: HashMap<SuppressionKey, i64>,
    key_of: impl Fn(&T) -> SuppressionKey,
) -> (Vec<T>, HashMap<SuppressionKey, i64>) {
    let mut out = Vec::with_capacity(events.len());
    for mut item in events {
        let amount = item.event().amount_cents;
        match debt.get_mut(&key_of(&item)) {
            Some(owed) if *owed > 0 && amount > 0 => {
                let taken = (*owed).min(amount);
                *owed -= taken;
                if amount == taken {
                    continue;
                }
                item.set_amount(amount - taken);
                out.push(item);
            }
            _ => out.push(item),
        }
    }
    debt.retain(|_, owed| *owed > 0);
    (out, debt)
}

/// O que identifica um evento para a supressão. `mask` é `None` no encadeamento de caixa (que não
/// tem máscara — o Saldo sempre conta) e na segunda passagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SuppressionKey {
    date: NaiveDate,
    kind: forecast::EventKind,
    mask: Option<forecast::RulerMask>,
}

/// Os dois envelopes de evento da leitura — caixa e métrica — vistos pela supressão.
trait Suppressible {
    fn event(&self) -> &CashflowEvent;
    fn key(&self) -> SuppressionKey;
    fn set_amount(&mut self, amount_cents: i64);
}

impl Suppressible for CashflowEvent {
    fn event(&self) -> &CashflowEvent {
        self
    }
    fn key(&self) -> SuppressionKey {
        SuppressionKey {
            date: self.date,
            kind: self.kind,
            mask: None,
        }
    }
    fn set_amount(&mut self, amount_cents: i64) {
        self.amount_cents = amount_cents;
    }
}

impl Suppressible for forecast::MetricEvent {
    fn event(&self) -> &CashflowEvent {
        &self.event
    }
    fn key(&self) -> SuppressionKey {
        SuppressionKey {
            date: self.event.date,
            kind: self.event.kind,
            mask: Some(self.mask),
        }
    }
    fn set_amount(&mut self, amount_cents: i64) {
        self.event.amount_cents = amount_cents;
    }
}

#[cfg(test)]
impl ForecastInputs {
    /// Insumos MÍNIMOS e válidos de um dia sem dado nenhum: horizonte de um dia, sem eventos, sem
    /// teto, sem renda, sem reserva mapeada. Cada teste de regra ajusta os campos da sua pergunta
    /// e ignora o resto — é isso que torna barato cobrir bordas que hoje exigiriam um banco.
    pub(crate) fn minimal(today: NaiveDate) -> Self {
        ForecastInputs {
            today,
            horizon_end: today,
            seed_cents: 0,
            cash_events: Vec::new(),
            metric_events: Vec::new(),
            economia_annotation: HashMap::new(),
            ceiling: CeilingInputs {
                per_day_cents: 0,
                source: CeilingSource::None,
                estimate_basis: None,
                projection_per_day_cents: 0,
                proposal_pending: false,
            },
            annual: AnnualInputs {
                year_metrics: Vec::new(),
                registered_income_cents: 0,
                registered_net_cents: 0,
                registered_economia_cents: 0,
                registered_patrimonio_cents: 0,
                projected_income_cents: 0,
                projected_net_cents: 0,
            },
            baseline: BaselineInputs {
                monthly_cents: 0,
                months: 0,
                typical_income_cents: 0,
                typical_economia_cents: 0,
            },
            reserve: ReserveInputs {
                balance_cents: 0,
                has_accounts: false,
                trend: "flat".to_string(),
            },
            spending_mode: SpendingModeInputs {
                samples: [forecast::MonthSpendSample {
                    daily_days: 0,
                    daily_total_cents: 0,
                    cartao_present: false,
                }; 3],
                cartao_month_cents: 0,
                next_fatura: None,
            },
            cards: CardInputs {
                has_card: false,
                active_invoices: Vec::new(),
                invoiced_cycles: HashSet::new(),
                alias_index: CardLexicon::from_entries(std::iter::empty()),
            },
            today_spend: DailySpendInputs {
                daily_avg_cents: 0,
                card_cents: 0,
            },
            ledger: LedgerInputs {
                transaction_count: 0,
                last_real_tx_date: None,
            },
        }
    }
}

/// O teto do diário: o número exibido, sua procedência, os operandos da estimativa e o teto que
/// dirige a projeção — que é outro número no modo cartão.
#[derive(Debug, Clone)]
pub(crate) struct CeilingInputs {
    /// Teto EFETIVO por dia: orçamento escolhido, senão o Diário médio do mês anterior.
    pub per_day_cents: i64,
    /// `chosen` (veredito do dono) · `estimate` (média do mês anterior) · `none` (sem registro).
    pub source: CeilingSource,
    /// Operandos da média do mês anterior. Presente só quando a procedência é `estimate`: número
    /// digitado não tem conta a mostrar.
    pub estimate_basis: Option<CeilingEstimateBasis>,
    /// Teto usado como DRIVER da projeção, re-roteado pelo modo de gasto: no modo cartão o gasto
    /// variável já vive nas faturas, e injetar o Diário típico dobraria a saída projetada.
    pub projection_per_day_cents: i64,
    /// Overlay da cerimônia: existe proposta aguardando o dono. Nunca a procedência do número.
    pub proposal_pending: bool,
}

/// As figuras anuais, cada uma na janela que a sua régua exige. A janela de meses COMPLETOS do
/// guardrail de poupança é campo próprio, distinta do recorte vivido (que inclui o mês em curso)
/// da régua anual — as duas convivem sem se confundir porque cada uma tem nome.
#[derive(Debug, Clone)]
pub(crate) struct AnnualInputs {
    /// Os doze meses `MonthMetric` do ano corrente, varridos UMA vez. A régua anual, o gate do
    /// cartão e a poupança anual do DTO leem daqui.
    pub year_metrics: Vec<forecast::MonthMetric>,
    /// Renda-base do guardrail 20–30%: soma de `income_cents` dos meses da janela de meses
    /// completos (`forecast::registered_window`), que em janeiro desloca para dezembro.
    pub registered_income_cents: i64,
    /// Net `renda − saída` (view Performance) da MESMA janela — o "colchão", distinto da Economia
    /// registrada que o guardrail divide. Nenhum recorte atual publica o colchão desta janela.
    #[allow(dead_code)]
    pub registered_net_cents: i64,
    /// Economia REGISTRADA da janela do guardrail: o numerador do Economizado% que julga.
    pub registered_economia_cents: i64,
    /// Patrimônio realizado da mesma janela. Publicado ao lado da régua, nunca somado a ela.
    pub registered_patrimonio_cents: i64,
    /// Renda do ano INTEIRO projetado (todas as linhas do ano) — contraste otimista, fora do
    /// guardrail.
    pub projected_income_cents: i64,
    /// Net do ano inteiro projetado, par do `projected_income_cents`.
    pub projected_net_cents: i64,
}

/// O "mês típico" contra o qual o futuro é medido: custo de vida, renda e economia medianos dos
/// últimos 6 meses de calendário completos.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BaselineInputs {
    /// Mediana do custo de vida (fixas + diário + cartão). Divisor da reserva e régua da cobertura.
    pub monthly_cents: i64,
    /// Quantos meses sustentam a mediana — o que separa veredito (6) de retrato vivo (1–5).
    pub months: i64,
    /// Mediana da renda dos meses ativos da janela — o denominador da perna de poupança do gate de
    /// financiamento.
    pub typical_income_cents: i64,
    /// Mediana da economia dos meses ativos, já reconciliada com a anotação da aba: o numerador da
    /// mesma perna.
    pub typical_economia_cents: i64,
}

/// A reserva crua: saldo, se há conta mapeada e a tendência registrada. Os meses de cobertura e o
/// estado epistêmico nascem da composição, dividindo pelo custo de vida do [`BaselineInputs`].
#[derive(Debug, Clone)]
pub(crate) struct ReserveInputs {
    pub balance_cents: i64,
    /// Existe conta de reserva mapeada. Sem ela não há número honesto — e é o que distingue
    /// "sem registro" de "contas zeradas", que é alerta legítimo.
    pub has_accounts: bool,
    pub trend: String,
}

/// Procedência do teto exibido. `chosen` é o único veredito; `estimate` é a média do mês
/// anterior COM selo (o fallback silencioso morre na exibição — o motor de projeção continua
/// usando o teto efetivo); `none` = travessão + CTA da cerimônia. A proposta pendente da
/// cerimônia é um OVERLAY (banner de confirmação), nunca a procedência do número exibido — o
/// valor proposto não entra em progresso/projeção antes do aceite explícito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingSource {
    Chosen,
    Estimate,
    None,
}

impl CeilingSource {
    pub fn as_str(self) -> &'static str {
        match self {
            CeilingSource::Chosen => "chosen",
            CeilingSource::Estimate => "estimate",
            CeilingSource::None => "none",
        }
    }
}

/// Os operandos que produzem a média do mês anterior, não só o resultado. A tela IMPRIME esta
/// conta — uma frase que a descrevesse poderia divergir do que a leitura faz.
#[derive(Debug, Clone)]
pub(crate) struct CeilingEstimateBasis {
    /// Mês da base, `YYYY-MM`.
    pub month: String,
    /// Gasto variável somado do mês anterior (magnitude).
    pub variable_cents: i64,
    /// Dias do mês anterior — o divisor.
    pub days: i64,
    pub per_day_cents: i64,
}

/// Uma fatura a vencer, com o total efetivo já resolvido entre o declarado e as compras.
#[derive(Debug, Clone)]
pub(crate) struct CardInvoiceEvent {
    pub account_id: String,
    pub card_name: String,
    pub owner_name: String,
    pub closing_date: NaiveDate,
    pub due_date: NaiveDate,
    pub amount_cents: i64,
    /// Existe Entrada vinculada (`refund_invoice_id`) — a expectativa de reembolso da fatura.
    pub has_refund_expectation: bool,
    pub refund_expected_cents: i64,
}

/// Os sinais que decidem o modo de gasto e o que o dia do modo cartão lê.
#[derive(Debug, Clone)]
pub(crate) struct SpendingModeInputs {
    /// Os três meses da janela de detecção (2 completos + corrente), em ordem cronológica.
    pub samples: [forecast::MonthSpendSample; 3],
    /// Cartão do mês corrente (realizado + projetado), magnitude.
    pub cartao_month_cents: i64,
    /// Próximo dia de fatura declarado pela planilha a partir de hoje: `(data, total do dia)`.
    /// A leitura por FATURA persistida vive em [`CardInputs`]; esta é o fallback de quem ainda
    /// não cadastrou cartão.
    pub next_fatura: Option<(NaiveDate, i64)>,
}

/// Os cartões: faturas a vencer, ciclos já faturados e o índice de apelidos que resolve a conta
/// de uma linha da nota.
#[derive(Debug, Clone)]
pub(crate) struct CardInputs {
    /// Existe conta de cartão cadastrada. Sem isso a precedência do lump não se aplica.
    pub has_card: bool,
    /// Faturas com vencimento ≥ hoje, em ordem de vencimento e nome do cartão.
    pub active_invoices: Vec<CardInvoiceEvent>,
    /// `(account_id, cycle_month)` com fatura persistida — o conjunto que decide se uma linha de
    /// nota que nomeia um cartão vira evento Cartão ou permanece Saída fixa. Já aplicado dentro
    /// de `cash_events`/`metric_events` pelo loader — nenhum recorte atual o lê deste campo.
    #[allow(dead_code)]
    pub invoiced_cycles: HashSet<(String, String)>,
    /// Apelido normalizado → `account_id` das contas de cartão. Mesma situação: já resolvido nos
    /// eventos carregados.
    #[allow(dead_code)]
    pub alias_index: CardLexicon<String>,
}

/// O gasto de HOJE, por régua: o que a régua do Diário mede e o que o modo cartão mostra.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DailySpendInputs {
    /// Diário realizado de hoje com a máscara `daily_avg` aplicada na origem — a mesma régua do
    /// teto, sem contabilidade paralela.
    pub daily_avg_cents: i64,
    /// Compras de cartão realizadas hoje (magnitude). O pagamento da fatura não entra.
    pub card_cents: i64,
}

/// As contagens do livro-razão que o dashboard publica.
#[derive(Debug, Clone)]
pub(crate) struct LedgerInputs {
    /// Lançamentos com data ≤ hoje.
    pub transaction_count: i64,
    /// Data do lançamento REAL mais recente (`YYYY-MM-DD`); `None` enquanto não houver nenhum.
    pub last_real_tx_date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast::{EventKind, MetricEvent, RulerMask};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn cash(date: &str, kind: EventKind, amount_cents: i64) -> CashflowEvent {
        CashflowEvent {
            date: d(date),
            kind,
            amount_cents,
            realized: false,
        }
    }

    fn metric(date: &str, kind: EventKind, amount_cents: i64) -> MetricEvent {
        MetricEvent {
            event: cash(date, kind, amount_cents),
            mask: RulerMask::ALL,
        }
    }

    /// Insumos com uma conta fixa futura nos dois streams — a célula que os cenários abaixo
    /// suprimem, adiam ou acompanham.
    fn inputs_with_a_fixed_bill() -> ForecastInputs {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-07-31");
        inputs.seed_cents = 1_000_000;
        inputs.cash_events = vec![cash("2026-07-10", EventKind::FixedOut, 150_000)];
        inputs.metric_events = vec![metric("2026-07-10", EventKind::FixedOut, 150_000)];
        inputs
    }

    // Suprimir uma conta retira a magnitude dos DOIS streams: o Saldo deixa de perder o dinheiro e
    // a régua deixa de contar a saída. Os insumos originais seguem intocados — a transformação
    // devolve outros insumos em vez de reescrever os que a leitura real usa.
    #[test]
    fn suppressing_a_bill_removes_it_from_both_streams_without_touching_the_original() {
        let inputs = inputs_with_a_fixed_bill();
        let changes = ScenarioChanges {
            chain_removed: inputs.cash_events.clone(),
            metric_removed: inputs.metric_events.clone(),
            ..ScenarioChanges::default()
        };

        let scenario = apply_scenario(&inputs, &changes);

        assert!(scenario.cash_events.is_empty());
        assert!(scenario.metric_events.is_empty());
        assert_eq!(
            inputs.cash_events.len(),
            1,
            "os insumos da leitura real não são reescritos"
        );
    }

    // Supressão PARCIAL de uma célula com irmãos: o que sobra continua pesando. A célula não é
    // derrubada inteira só porque um item dela saiu.
    #[test]
    fn a_partial_suppression_keeps_the_rest_of_the_cell() {
        let inputs = inputs_with_a_fixed_bill();
        let changes = ScenarioChanges {
            chain_removed: vec![cash("2026-07-10", EventKind::FixedOut, 40_000)],
            metric_removed: vec![metric("2026-07-10", EventKind::FixedOut, 40_000)],
            ..ScenarioChanges::default()
        };

        let scenario = apply_scenario(&inputs, &changes);

        assert_eq!(scenario.cash_events.len(), 1);
        assert_eq!(scenario.cash_events[0].amount_cents, 110_000);
        assert_eq!(scenario.metric_events[0].event.amount_cents, 110_000);
    }

    // Uma supressão sem par não retira nada: a compra que a fatura já substituiu saiu da leitura
    // na classificação, e aquele dinheiro não está sendo contado por este evento. Descontá-la de
    // outro evento do dia cobraria a mudança duas vezes.
    #[test]
    fn a_suppression_without_a_match_removes_nothing() {
        let inputs = inputs_with_a_fixed_bill();
        let changes = ScenarioChanges {
            chain_removed: vec![cash("2026-07-10", EventKind::Cartao, 90_000)],
            ..ScenarioChanges::default()
        };

        let scenario = apply_scenario(&inputs, &changes);

        assert_eq!(scenario.cash_events.len(), 1);
        assert_eq!(scenario.cash_events[0].amount_cents, 150_000);
    }

    // Uma parcela além do horizonte estica a janela — senão ela sairia da projeção sem aviso. Um
    // horizonte hipotético mais CURTO não encolhe a leitura.
    #[test]
    fn a_line_beyond_the_horizon_stretches_it_and_never_shortens_it() {
        let inputs = inputs_with_a_fixed_bill();

        let stretched = apply_scenario(
            &inputs,
            &ScenarioChanges {
                horizon_end: Some(d("2028-01-10")),
                ..ScenarioChanges::default()
            },
        );
        let untouched = apply_scenario(
            &inputs,
            &ScenarioChanges {
                horizon_end: Some(d("2026-06-20")),
                ..ScenarioChanges::default()
            },
        );

        assert_eq!(stretched.horizon_end, d("2028-01-10"));
        assert_eq!(untouched.horizon_end, inputs.horizon_end);
    }

    // Precedência da fatura: com cartão cadastrado, uma compra de cartão FUTURA já está contida no
    // lump da fatura do ciclo — a linha hipotética de cartão não entra por cima dele. Sem cartão
    // cadastrado não há lump nenhum, e a compra é uma saída como qualquer outra.
    #[test]
    fn a_future_card_purchase_yields_to_the_invoice_only_when_a_card_exists() {
        let mut inputs = inputs_with_a_fixed_bill();
        let changes = ScenarioChanges {
            chain_added: vec![cash("2026-07-05", EventKind::Cartao, 30_000)],
            metric_added: vec![metric("2026-07-05", EventKind::Cartao, 30_000)],
            ..ScenarioChanges::default()
        };

        inputs.cards.has_card = true;
        let with_card = apply_scenario(&inputs, &changes);
        inputs.cards.has_card = false;
        let without_card = apply_scenario(&inputs, &changes);

        assert_eq!(with_card.cash_events.len(), 1, "a compra cede ao lump");
        assert_eq!(with_card.metric_events.len(), 1);
        assert_eq!(without_card.cash_events.len(), 2);
        assert_eq!(without_card.metric_events.len(), 2);
    }

    // Duas saídas do mesmo dia e do mesmo tipo, mas em réguas diferentes: a supressão desconta da
    // que a tag identifica. Descontar da outra tiraria a conta do custo de vida errado.
    #[test]
    fn a_suppression_takes_from_the_event_of_its_own_ruler() {
        let mut inputs = ForecastInputs::minimal(d("2026-06-15"));
        inputs.horizon_end = d("2026-07-31");
        let out_of_cost = RulerMask {
            cost_of_living: false,
            ..RulerMask::ALL
        };
        inputs.metric_events = vec![
            metric("2026-07-10", EventKind::FixedOut, 100_000),
            MetricEvent {
                event: cash("2026-07-10", EventKind::FixedOut, 100_000),
                mask: out_of_cost,
            },
        ];

        let scenario = apply_scenario(
            &inputs,
            &ScenarioChanges {
                metric_removed: vec![MetricEvent {
                    event: cash("2026-07-10", EventKind::FixedOut, 100_000),
                    mask: out_of_cost,
                }],
                ..ScenarioChanges::default()
            },
        );

        assert_eq!(scenario.metric_events.len(), 1);
        assert!(
            scenario.metric_events[0].mask.cost_of_living,
            "quem sobra é a conta que conta no custo de vida"
        );
    }

    // O resto dos insumos atravessa a transformação intacto: um cenário reprojeta o caixa e as
    // réguas do mês, nunca reescreve o ano já realizado nem a reserva.
    #[test]
    fn a_scenario_never_rewrites_the_realized_year_or_the_reserve() {
        let mut inputs = inputs_with_a_fixed_bill();
        inputs.annual.registered_income_cents = 400_000;
        inputs.annual.registered_economia_cents = 100_000;
        inputs.reserve.balance_cents = 900_000;

        let scenario = apply_scenario(
            &inputs,
            &ScenarioChanges {
                chain_added: vec![cash("2026-07-20", EventKind::Daily, 500_000)],
                ..ScenarioChanges::default()
            },
        );

        assert_eq!(scenario.annual.registered_income_cents, 400_000);
        assert_eq!(scenario.annual.registered_economia_cents, 100_000);
        assert_eq!(scenario.reserve.balance_cents, 900_000);
    }
}
