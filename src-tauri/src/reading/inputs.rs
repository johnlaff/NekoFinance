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
    /// mesmo o excluído de alguma régua. Inclui o Diário típico projetado dos dias futuros.
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
                guardrail_income_cents: 0,
                guardrail_net_cents: 0,
                guardrail_economia_cents: 0,
                guardrail_patrimonio_cents: 0,
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
    /// variável já vive nas faturas, e injetar o Diário típico dobraria a saída projetada. Já
    /// carregado dentro de `cash_events` pelo loader — nenhum recorte atual o lê deste campo.
    #[allow(dead_code)]
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
    /// completos (`forecast::guardrail_window`), que em janeiro desloca para dezembro.
    pub guardrail_income_cents: i64,
    /// Net `renda − saída` (view Performance) da MESMA janela — o "colchão", distinto da Economia
    /// registrada que o guardrail divide. Nenhum recorte atual publica o colchão desta janela.
    #[allow(dead_code)]
    pub guardrail_net_cents: i64,
    /// Economia REGISTRADA da janela do guardrail: o numerador do Economizado% que julga.
    pub guardrail_economia_cents: i64,
    /// Patrimônio realizado da mesma janela. Publicado ao lado da régua, nunca somado a ela.
    pub guardrail_patrimonio_cents: i64,
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
    /// Mediana da renda dos meses ativos da janela. Nenhum recorte atual a publica.
    #[allow(dead_code)]
    pub typical_income_cents: i64,
    /// Mediana da economia dos meses ativos, já reconciliada com a anotação da aba. Nenhum
    /// recorte atual a publica.
    #[allow(dead_code)]
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
