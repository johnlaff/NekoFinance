//! Forecast core — projected running balance ("saldo projetado", the methodology's heart).
//!
//! Pure functional core: NO IO, NO ambient clock, NO DB. Every input (seed, events, horizon,
//! "today") arrives as an argument, so the engine is deterministic and trivially testable. The
//! imperative shell (`commands.rs`) loads/maps rows and supplies the seed. The engine owns the
//! daily chain, month-end balances, deepest deficit, safe-to-spend guardrails and monthly totals.

use crate::calendar::last_day_of_month;
use chrono::{Datelike, NaiveDate};

/// A dated cash-flow event in the projection. Amounts are always positive; the sign is implied by
/// `kind`. `realized = false` marks a future projection (vs a realized transaction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// Entrada — income (salary, reimbursement, freela…).
    Income,
    /// Saída — fixed outflow, excluding credit-card bucket once item/transaction classification knows it.
    FixedOut,
    /// Diário — variable daily débito/cash spend (Régua 1).
    Daily,
    /// Cartão — credit-card bill/purchase bucket. It is inside custo de vida but visible apart.
    Cartao,
    /// Economia — guardar em reserva acessível. Leaves the spending balance (signed −), feeds
    /// Economia%, and is excluded from custo de vida.
    Economia,
    /// Patrimônio — long-term/illiquid investment. Leaves the spending balance, but is excluded
    /// from custo de vida and from accessible Economia%.
    Patrimonio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashflowEvent {
    pub date: NaiveDate,
    pub kind: EventKind,
    pub amount_cents: i64,
    pub realized: bool,
}

/// Em quais réguas do método um evento conta. A tag do lançamento é um interruptor de
/// contabilidade: cada flag desligado tira o evento dos insumos DAQUELA régua, e só dela.
/// O Saldo (encadeamento de caixa) não tem máscara por definição — dinheiro que entrou e
/// saiu de verdade sempre conta; por isso a máscara vive no stream de MÉTRICAS
/// ([`MetricEvent`]) e nunca no de caixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RulerMask {
    pub performance: bool,
    pub cost_of_living: bool,
    pub savings: bool,
    pub daily_avg: bool,
}

impl RulerMask {
    /// Conta em todas as réguas (lançamento sem tag; eventos sintéticos).
    pub const ALL: RulerMask = RulerMask {
        performance: true,
        cost_of_living: true,
        savings: true,
        daily_avg: true,
    };

    /// Interseção: um lançamento com várias tags fica fora de uma régua se QUALQUER
    /// tag o excluir dela (mesma semântica do flag único que esta máscara substitui).
    pub fn and(self, other: RulerMask) -> RulerMask {
        RulerMask {
            performance: self.performance && other.performance,
            cost_of_living: self.cost_of_living && other.cost_of_living,
            savings: self.savings && other.savings,
            daily_avg: self.daily_avg && other.daily_avg,
        }
    }
}

/// Evento do stream de métricas: o evento de caixa + a máscara de réguas herdada das
/// tags do lançamento-pai (itens de nota e resíduo herdam a máscara do pai). O tipo
/// separado é o lockstep em forma de código: todo call site de métricas é obrigado
/// pelo compilador a declarar de onde vem a máscara.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricEvent {
    pub event: CashflowEvent,
    pub mask: RulerMask,
}

/// Eleva eventos de caixa a eventos de métrica contando em todas as réguas — para
/// eventos sintéticos (teto projetado, hipotéticos de cenário) e testes.
pub fn lift_all(events: &[CashflowEvent]) -> Vec<MetricEvent> {
    events
        .iter()
        .map(|&event| MetricEvent {
            event,
            mask: RulerMask::ALL,
        })
        .collect()
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

/// Métricas de decisão de um mês. Como a máscara por régua faz cada régua enxergar o
/// próprio conjunto de eventos, cada campo declara a VIEW que serve — as equações
/// exibidas nas telas fecham com o motor porque leem campos da mesma view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MonthMetric {
    pub year: i32,
    pub month: u32,
    /// Renda do mês na view ECONOMIA — a "sua renda" do método, denominador do
    /// Economizado% exibido (o que um terceiro devolve e está fora da Economia não
    /// entra aqui).
    pub income_cents: i64,
    /// Renda do mês na view PERFORMANCE (a perna positiva de `performance_cents`).
    pub income_performance_cents: i64,
    /// View PERFORMANCE: renda − (fixas + diário + previsão + cartão + economia +
    /// patrimônio), tudo filtrado pela máscara de Performance.
    pub performance_cents: i64,
    /// View CUSTO DE VIDA: fixas + diário realizado + cartão.
    pub cost_of_living_cents: i64,
    /// Saídas FIXAS realizadas (view CUSTO DE VIDA — componente exibido).
    pub fixed_out_cents: i64,
    /// Diário REALIZADO (view CUSTO DE VIDA — componente exibido).
    pub daily_out_cents: i64,
    /// Diário REALIZADO na view DIÁRIO MÉDIO — numerador de `real_daily_avg_cents`.
    pub daily_avg_out_cents: i64,
    /// Previsão de diário do mês (teto dos dias futuros + diários pré-lançados), view
    /// PERFORMANCE. Entra na Performance — o mês corrente considera o que ainda vai
    /// ser gasto — mas fica fora do custo de vida, que reporta só o realizado.
    pub daily_projected_cents: i64,
    /// Gastos com cartão (view CUSTO DE VIDA — componente exibido).
    pub cartao_cents: i64,
    /// Diário médio = `daily_avg_out_cents` ÷ dias decorridos.
    pub real_daily_avg_cents: i64,
    /// Economizado% = economia ÷ renda, ambos na view ECONOMIA.
    pub savings_rate_bps: i64,
    /// Economia lançada no mês (view ECONOMIA), reconciliada com a anotação da aba —
    /// numerador do Economizado%. Excluída do custo de vida, mas descontada da
    /// Performance (na view de lá) como todo dinheiro que saiu.
    pub economia_cents: i64,
    /// Patrimônio/long-term/illiquid (view PERFORMANCE). Excluído de custo de vida e
    /// Economia% acessível, mas reduz Performance/Saldo como saída.
    pub patrimonio_cents: i64,
    /// Saída TOTAL lançada no mês para cobertura ("quanto do viver já está lançado") =
    /// view CUSTO DE VIDA + diário projetado da mesma view. Não inclui
    /// economia/patrimônio.
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
    /// guardrail real (duplo: caixa × economia) é [`safe_to_spend_today`]; o DTO expõe o dele.
    /// Nome explícito para não ser confundido com o número do dashboard.
    pub cash_floor_cents: i64,
    /// Per-month decision metrics (Totais).
    pub months: Vec<MonthMetric>,
}

/// Qual régua limita o "pode gastar hoje".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guardrail {
    /// Limitado pelo caixa: gastar mais empurraria algum dia futuro abaixo do piso de reserva.
    Cash,
    /// Limitado pela régua da economia: gastar mais tiraria o ano do piso da faixa 20–30%.
    Savings,
}

impl Guardrail {
    /// Valor estável exposto nas fronteiras que representam qual régua limita o gasto.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Savings => "savings",
        }
    }
}

/// "Pode gastar hoje" decomposto nas duas réguas do método.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeToSpend {
    /// O número honesto exibido: o MAIS APERTADO das duas réguas, nunca negativo.
    pub amount_cents: i64,
    /// Folga de caixa: menor saldo projetado no horizonte − piso de reserva. Pode ser a reserva
    /// inteira (alta) mesmo quando a poupança do mês já estourou — é o "Caixa ≠ Performance".
    pub cash_headroom_cents: i64,
    /// Folga da economia: o inverso do déficit até o piso de 20% que a régua anual já calcula,
    /// no recorte que ela julga. Negativa = a janela já está abaixo do piso. `None` = régua
    /// INATIVA (janela sem renda) → só o caixa decide. `Option` obriga o tratamento explícito da
    /// régua inativa e impede o vazamento de sentinelas numéricos.
    pub savings_headroom_cents: Option<i64>,
    /// Qual régua manda.
    pub binding: Guardrail,
}

/// Meses COMPLETOS do ano corrente: janeiro até o mês anterior a `today`, em ordem. É a janela
/// das figuras REGISTRADAS que a tela publica ao lado da régua (Economia registrada, Patrimônio,
/// colchão) — o que já fechou, sem o mês em curso pela metade. Em janeiro ela cai vazia (nenhum
/// mês do ano corrente terminou) e recua para dezembro do ano anterior, para o retrato não sumir
/// na virada.
///
/// Não é a janela que JULGA: o veredito da faixa e o teto do dia leem o recorte de
/// [`annual_ruler`], que inclui o mês em curso. Única definição desta: os consumidores leem
/// daqui, nunca recompõem os limites por conta própria.
pub fn registered_window(today: NaiveDate) -> Vec<(i32, u32)> {
    if today.month() == 1 {
        return vec![(today.year() - 1, 12)];
    }
    (1..today.month()).map(|m| (today.year(), m)).collect()
}

/// O "pode gastar hoje" fiel ao método: o mais apertado de duas réguas.
///
/// 1. **Caixa** — `menor saldo projetado no horizonte`. É o Saldo da planilha e o termômetro:
///    a régua é não abrir o bico. A reserva NÃO entra aqui. No método ela é o amortecedor que
///    se ACIONA quando o saldo fica negativo ("você vai usar a sua reserva quando a sua planilha
///    ficar negativa") — usá-la como piso invertia o papel: o instrumento que socorre virava o
///    que proíbe, e quem ainda não completou a reserva ficava com teto zero por anos. O estoque
///    da reserva é leitura patrimonial (meses de custo de vida + excedente), não trava de fluxo.
/// 2. **Economia** — a MESMA régua anual que a tela do ano julga: quanto cabe mantendo o
///    Economizado% da janela ≥ o piso de 20% (`SAVINGS_FLOOR_BPS`). A folga é o inverso do
///    déficit que [`AnnualRuler`] já calculou, no recorte que ele julga (vividos, ou o ano
///    inteiro quando todo mês à frente tem lastro) — uma derivação, não uma segunda divisão.
///    O critério é o PISO da faixa, nunca um alvo intermediário: 20–30% é média ANUAL (tem mês
///    que é mais, tem mês que é menos), e é o piso que diz se o ano ainda está dentro dela.
///    `None` = janela sem renda → só o caixa.
///
/// Espelha o gate determinístico do método: só pode gastar se o saldo não abre o bico **E** a
/// economia 20–30% (no ano) se mantém.
pub fn safe_to_spend_today(
    fc: &Forecast,
    ruler: &AnnualRuler,
    reserve_months: Option<f64>,
) -> SafeToSpend {
    let cash_headroom_cents = fc.deepest_deficit.map(|p| p.balance_cents).unwrap_or(0);
    let savings_headroom_cents = ruler.savings_headroom_cents();

    // A fronteira "morde / não morde" é o VEREDITO da faixa, não o sinal de um número: a régua
    // protege a faixa enquanto ela está viva — é a pergunta do método sobre uma decisão nova
    // ("essa parcela vai me IMPEDIR de economizar de 20 a 30%?"), portanto prospectiva. Com a
    // faixa rompida ela solta: o déficit acumulado é passado, nenhum gasto de hoje o desfaz, e
    // travar o dia puniria o que não volta. Economia zerada com a reserva de pé solta pelo mesmo
    // caminho — é a ordem do método cumprida —, e ano sem registro não tem o que proteger. Aí a
    // orientação passa a ser o diagnóstico (a economia do ano, visível na tela) e a régua do
    // caixa, que é a do presente.
    let band_alive = matches!(
        band_verdict(ruler, reserve_months),
        BandVerdict::InBand | BandVerdict::AboveBand
    );
    let savings_binds =
        band_alive && savings_headroom_cents.is_some_and(|s| s < cash_headroom_cents);
    let binding = if savings_binds {
        Guardrail::Savings
    } else {
        Guardrail::Cash
    };
    let limit = if savings_binds {
        savings_headroom_cents.unwrap_or(cash_headroom_cents)
    } else {
        cash_headroom_cents
    };
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
    // sinalizar "sem histórico", em vez de marcar tudo como completo.
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

/// Um mês futuro só sustenta o veredito do ano se a saída lançada for compatível com a vida
/// real: piso de 60% do gasto típico. Abaixo disso o mês é SEM LASTRO — tem lançamento, só tem
/// pouco: pode ser mês barato de verdade ou pode faltar lançar.
pub const LASTRO_FLOOR_BPS: i64 = 6_000;

/// Piso da faixa de economia do método (20%): abaixo dele a economia não está "viva" — é o
/// vermelho da escada das réguas, o gate de legitimidade do modo cartão, o badge "Dentro do
/// ideal", a cor da visão anual, o gate da fase "operar" e o critério ÚNICO do guardrail do dia.
/// Uma barra só para todas as réguas: a faixa 20–30% é média ANUAL, e é o piso que diz se o ano
/// ainda está dentro dela. O espelho da tela é `SAVINGS_MIN_BPS` (`src/screens/totaisStatus.ts`).
pub const SAVINGS_FLOOR_BPS: i64 = 2_000;

/// Teto da faixa de economia do método (30%): acima dele o ano guardou além do ideal, e o
/// convite é gastar um pouco mais se quiser — nunca uma reprovação. Fecha o trio da faixa
/// canônica 20–30% com piso e meta, numa casa só para que não possam divergir.
pub const SAVINGS_CEILING_BPS: i64 = 3_000;

/// Meses de reserva mínimos do método: abaixo disso a liquidez ainda está sendo construída, e é
/// ela que vem primeiro na ordem do método — economia zerada só é escolha com a reserva de pé.
pub const RESERVE_MIN_MONTHS: i64 = 6;

/// Um mês do ano na ótica do método: o que saiu, se já foi vivido, se tem lastro para sustentar o
/// veredito e quanto faltaria lançar para ele parecer um mês típico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnualRulerMonth {
    pub month: u32,
    /// Saída total do mês (renda − performance) — a mesma figura que alimenta o gasto típico.
    pub outflow_cents: i64,
    pub lived: bool,
    /// Mês à frente cuja saída lançada não alcança o piso de lastro.
    pub suspect: bool,
    /// Quanto faltaria lançar para o mês custar o típico. Zero em mês vivido ou lastreado: o que
    /// já foi vivido não deve nada ao futuro.
    pub missing_cents: i64,
}

/// A régua ANUAL do método sobre os doze meses de um ano: o Economizado% que julga, o recorte
/// que o sustenta e os meses à frente que ainda não têm lastro.
///
/// Sem o teste de lastro, um dezembro vazio inflaria o percentual do ano inteiro e o veredito
/// diria "dentro da faixa" sobre um ano que ninguém viveu. Enquanto houver mês sem lastro, a
/// régua recua ao que já foi vivido — e o recorte vai junto, para que o número nunca se
/// apresente como se fosse do ano fechado.
///
/// É a única definição da régua: a tela O ano e a conversa leem esta função, nenhuma das duas
/// recompõe a faixa por conta própria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualRuler {
    pub lived_months: u32,
    pub future_months: u32,
    /// Gasto típico do ano = mediana das saídas dos meses vividos.
    pub typical_spend_cents: i64,
    /// Os doze meses na ótica do método, em ordem.
    pub months: Vec<AnnualRulerMonth>,
    pub income_lived_cents: i64,
    pub economia_lived_cents: i64,
    /// Sobra dos meses vividos (Performance somada) — o colchão do ano.
    pub surplus_lived_cents: i64,
    pub income_year_cents: i64,
    pub economia_year_cents: i64,
    /// Meses vividos COM renda registrada. É o denominador honesto para comparar um ano com
    /// outro: dividir um ano em curso por doze inventaria uma queda que não aconteceu.
    pub recorded_months: u32,
    /// Renda média por mês com registro.
    pub avg_income_cents: i64,
    /// Economizado% do recorte vivido; `None` sem renda para dividir.
    pub lived_bps: Option<i64>,
    /// Economizado% do ano inteiro (vivido + lançado à frente).
    pub projected_bps: Option<i64>,
    /// O percentual que JULGA: o vivido enquanto houver mês sem lastro, senão o do ano.
    pub bps: Option<i64>,
    /// A régua fala do recorte vivido (e não do ano fechado).
    pub scope_lived: bool,
    /// Algum mês vivido teve movimento — sem isso, o ano não tem o que julgar.
    pub has_data: bool,
    /// Quanto falta guardar para o recorte vivido fechar no piso de 20%. Negativo = já passou.
    pub shortfall_lived_cents: i64,
    /// Quanto falta guardar para o ANO fechar no piso de 20%. É o denominador que a tela usa no
    /// convite: a falta dos meses vividos fecharia o ano num número menor.
    pub shortfall_year_cents: i64,
    /// A falta do ano dividida pelos meses que restam; nula em ano sem futuro.
    pub per_month_shortfall_cents: Option<i64>,
}

impl AnnualRuler {
    /// A renda da janela que a régua JULGA — a mesma que produziu [`AnnualRuler::bps`].
    pub fn judged_income_cents(&self) -> i64 {
        if self.scope_lived {
            self.income_lived_cents
        } else {
            self.income_year_cents
        }
    }

    /// Quanto falta guardar para a janela julgada fechar no piso de 20%.
    pub fn judged_shortfall_cents(&self) -> i64 {
        if self.scope_lived {
            self.shortfall_lived_cents
        } else {
            self.shortfall_year_cents
        }
    }

    /// A folga da economia: o inverso do déficit até o piso, sobre a janela julgada. É o número
    /// que o guardrail do dia consome — reusar o déficit da régua é o que impede o teto e a tela
    /// de divergirem por um arredondamento. `None` sem renda na janela: régua inativa, não zero.
    pub fn savings_headroom_cents(&self) -> Option<i64> {
        (self.judged_income_cents() > 0).then(|| -self.judged_shortfall_cents())
    }

    /// Os meses à frente sem lastro, em ordem.
    pub fn suspect_months(&self) -> Vec<u32> {
        self.months
            .iter()
            .filter(|m| m.suspect)
            .map(|m| m.month)
            .collect()
    }
}

/// O veredito do ano contra a faixa 20–30%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandVerdict {
    /// O ano não tem movimento vivido para julgar.
    NoRecord,
    /// Economia zerada com a reserva de pé: a ordem do método cumprida, não uma falta.
    ZeroByChoice,
    BelowBand,
    InBand,
    AboveBand,
}

impl BandVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            BandVerdict::NoRecord => "no_record",
            BandVerdict::ZeroByChoice => "zero_by_choice",
            BandVerdict::BelowBand => "below_band",
            BandVerdict::InBand => "in_band",
            BandVerdict::AboveBand => "above_band",
        }
    }
}

/// O veredito da faixa: ano sem registro não julga; economia zerada com a reserva já protegida é
/// a troca CERTA na ordem do método (proteger a reserva vem antes de guardar mais), nunca uma
/// falta; o resto lê a régua contra a faixa.
pub fn band_verdict(ruler: &AnnualRuler, reserve_months: Option<f64>) -> BandVerdict {
    if !ruler.has_data {
        return BandVerdict::NoRecord;
    }
    if ruler.economia_lived_cents == 0
        && reserve_months.is_some_and(|m| m >= RESERVE_MIN_MONTHS as f64)
    {
        return BandVerdict::ZeroByChoice;
    }
    match ruler.bps {
        None => BandVerdict::NoRecord,
        Some(bps) if bps < SAVINGS_FLOOR_BPS => BandVerdict::BelowBand,
        Some(bps) if bps <= SAVINGS_CEILING_BPS => BandVerdict::InBand,
        Some(_) => BandVerdict::AboveBand,
    }
}

/// Onde o ano termina: o saldo do último mês projetado e o cenário em que cada mês sem lastro
/// custasse o típico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct YearEnd {
    pub end_month: Option<u32>,
    pub end_balance_cents: Option<i64>,
    /// O saldo se os meses sem lastro até o fim custassem o típico. `None` quando não há mês sem
    /// lastro na janela — sem silêncio a descontar, o cenário alternativo não existe.
    pub end_balance_typical_cents: Option<i64>,
}

/// O fim do ano a partir da régua e dos saldos de fim de mês (`(mês, saldo)`, esparso).
///
/// O ano termina no último mês que tem saldo: dezembro quando a projeção alcança, senão o mês
/// mais distante do horizonte — e só os meses sem lastro DENTRO dessa janela entram no cenário,
/// porque descontar um dezembro que o saldo nem alcança inventaria um rombo.
pub fn year_end_scenario(ruler: &AnnualRuler, month_end: &[(u32, i64)]) -> YearEnd {
    let Some(&(end_month, end_balance)) = month_end.iter().max_by_key(|(month, _)| *month) else {
        return YearEnd::default();
    };
    let missing: i64 = ruler
        .months
        .iter()
        .filter(|m| m.suspect && m.month <= end_month)
        .map(|m| m.missing_cents)
        .sum();
    YearEnd {
        end_month: Some(end_month),
        end_balance_cents: Some(end_balance),
        end_balance_typical_cents: (missing > 0).then_some(end_balance - missing),
    }
}

/// Saída total de um mês = tudo que deixou a conta = renda − performance. Inclui Economia e
/// Patrimônio de propósito: para o teste de lastro o que importa é se o mês parece vivido.
fn month_outflow(m: &MonthMetric) -> i64 {
    m.income_cents - m.performance_cents
}

pub fn annual_ruler(months: &[MonthMetric], year: i32, today: NaiveDate) -> AnnualRuler {
    let by_month = |month: u32| months.iter().find(|m| m.year == year && m.month == month);
    let lived_of = |month: u32| match year.cmp(&today.year()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => month <= today.month(),
    };

    // Mês ausente na entrada vale zero em toda figura — o motor pode vir esparso, e um mês sem
    // evento é um mês de zeros, não um buraco.
    let rows: Vec<(u32, Option<&MonthMetric>)> = (1..=12).map(|m| (m, by_month(m))).collect();
    let figure = |m: Option<&MonthMetric>, pick: fn(&MonthMetric) -> i64| m.map_or(0, pick);

    let lived_outflows: Vec<i64> = rows
        .iter()
        .filter(|(month, _)| lived_of(*month))
        .map(|(_, m)| figure(*m, month_outflow))
        .collect();
    let typical_spend_cents = median_cents(lived_outflows);
    // O piso trunca em centavos: a fronteira é determinística, e um centavo não decide lastro.
    let lastro_threshold = typical_spend_cents * LASTRO_FLOOR_BPS / 10_000;

    let ruler_months: Vec<AnnualRulerMonth> = rows
        .iter()
        .map(|(month, m)| {
            let outflow_cents = figure(*m, month_outflow);
            let lived = lived_of(*month);
            let suspect = !lived && typical_spend_cents > 0 && outflow_cents < lastro_threshold;
            AnnualRulerMonth {
                month: *month,
                outflow_cents,
                lived,
                suspect,
                // `.max(0)`: um mês sem lastro pode ter saída negativa (estorno) — o que falta
                // lançar nunca é menos que zero.
                missing_cents: if suspect {
                    (typical_spend_cents - outflow_cents).max(0)
                } else {
                    0
                },
            }
        })
        .collect();

    let sum = |pick: fn(&MonthMetric) -> i64, lived_only: bool| -> i64 {
        rows.iter()
            .filter(|(month, _)| !lived_only || lived_of(*month))
            .map(|(_, m)| figure(*m, pick))
            .sum()
    };
    let income_lived_cents = sum(|m| m.income_cents, true);
    let economia_lived_cents = sum(|m| m.economia_cents, true);
    let income_year_cents = sum(|m| m.income_cents, false);
    let economia_year_cents = sum(|m| m.economia_cents, false);

    // TRUNCA, como o motor mensal e como a exibição: um percentual arredondado para cima faria a
    // tela mostrar 21% onde a conversa diria 22%.
    let rate = |economia: i64, income: i64| (income > 0).then(|| economia * 10_000 / income);
    let lived_bps = rate(economia_lived_cents, income_lived_cents);
    let projected_bps = rate(economia_year_cents, income_year_cents);
    let scope_lived = ruler_months.iter().any(|m| m.suspect);

    // A falta ARREDONDA (não trunca): é a conta que a tela imprime como convite, e meio centavo
    // a menos apareceria como um "falta guardar" que não fecha o piso.
    let shortfall =
        |income: i64, economia: i64| round_half_up(income * SAVINGS_FLOOR_BPS, 10_000) - economia;
    let shortfall_year_cents = shortfall(income_year_cents, economia_year_cents);

    let lived_months = rows.iter().filter(|(month, _)| lived_of(*month)).count() as u32;
    let future_months = 12 - lived_months;
    let recorded: Vec<i64> = rows
        .iter()
        .filter(|(month, _)| lived_of(*month))
        .map(|(_, m)| figure(*m, |m| m.income_cents))
        .filter(|income| *income > 0)
        .collect();
    AnnualRuler {
        lived_months,
        future_months,
        typical_spend_cents,
        months: ruler_months,
        income_lived_cents,
        economia_lived_cents,
        surplus_lived_cents: sum(|m| m.performance_cents, true),
        income_year_cents,
        economia_year_cents,
        recorded_months: recorded.len() as u32,
        avg_income_cents: if recorded.is_empty() {
            0
        } else {
            recorded.iter().sum::<i64>() / recorded.len() as i64
        },
        lived_bps,
        projected_bps,
        bps: if scope_lived {
            lived_bps
        } else {
            projected_bps
        },
        scope_lived,
        has_data: rows.iter().any(|(month, m)| {
            lived_of(*month)
                && (figure(*m, |m| m.income_cents) != 0
                    || figure(*m, |m| m.economia_cents) != 0
                    || figure(*m, month_outflow) != 0)
        }),
        shortfall_lived_cents: shortfall(income_lived_cents, economia_lived_cents),
        shortfall_year_cents,
        per_month_shortfall_cents: (future_months > 0)
            .then(|| round_half_up(shortfall_year_cents, future_months as i64)),
    }
}

/// Divisão que arredonda meio para CIMA (o `Math.round` da tela), em inteiros: `floor(n/d + 0.5)`.
/// Divisão inteira em Rust trunca em direção ao zero, o que faria um valor negativo arredondar
/// para o lado oposto ao da tela.
fn round_half_up(numerator: i64, denominator: i64) -> i64 {
    (2 * numerator + denominator).div_euclid(2 * denominator)
}

/// Mediana em centavos (par ⇒ média dos dois centrais, truncada) — o estimador de "mês típico"
/// das réguas do método, robusto a um mês atípico ao contrário da média. Janela vazia vale 0,
/// e é o chamador que decide o que "sem histórico" significa na régua dele.
pub fn median_cents(mut vals: Vec<i64>) -> i64 {
    if vals.is_empty() {
        return 0;
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2
    }
}

/// Net signed effect of an event on the balance (income adds, outflows subtract).
fn signed(e: &CashflowEvent) -> i64 {
    match e.kind {
        EventKind::Income => e.amount_cents,
        EventKind::FixedOut
        | EventKind::Daily
        | EventKind::Cartao
        | EventKind::Economia
        | EventKind::Patrimonio => -e.amount_cents,
    }
}

/// Row→event classification rule (the shell maps DB rows through this).
/// `income` → Entrada; a credit `expense` → Cartão; fixed `expense` → Saída; any other `expense`
/// → Diário. A `transfer` is **Economia** only when its destination is accessible reserve;
/// `illiquid` destinations are Patrimônio. `restricted` (vale-refeição) is restricted spending,
/// not savings. Other transfers are net-zero for the method → ignored.
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
            if payment_method == Some("credit") {
                Some(EventKind::Cartao)
            } else if is_fixed {
                Some(EventKind::FixedOut)
            } else {
                Some(EventKind::Daily)
            }
        }
        "transfer" => match to_liquidity {
            Some("reserve") => Some(EventKind::Economia),
            Some("illiquid") => Some(EventKind::Patrimonio),
            // Vale-refeição (restricted) é gasto restrito, não poupança; e transferências entre
            // contas líquidas são net-zero → nenhuma conta como Economia.
            _ => None,
        },
        _ => None,
    }
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
    events: &[MetricEvent],
    months: &[MonthEnd],
    annotation: &std::collections::HashMap<(i32, u32), i64>,
) -> Vec<MonthMetric> {
    months
        .iter()
        .map(|me| {
            let (year, month) = (me.year, me.month);
            // Os buckets alimentam réguas CRUZADAS (o diário entra em Performance,
            // Custo de vida e Diário médio; a renda em Performance e na base do
            // Economizado%), então cada régua acumula a própria view do bucket,
            // filtrada pela sua máscara. Sufixos: _p Performance · _c Custo de vida ·
            // _s Economia · _d Diário médio.
            let mut income_p = 0i64;
            let mut income_s = 0i64;
            let mut fixed_p = 0i64;
            let mut fixed_c = 0i64;
            let mut daily_real_p = 0i64;
            let mut daily_real_c = 0i64;
            let mut daily_real_d = 0i64;
            let mut daily_proj_p = 0i64;
            let mut daily_proj_c = 0i64;
            let mut cartao_p = 0i64;
            let mut cartao_c = 0i64;
            let mut economia_p = 0i64;
            let mut economia_s = 0i64;
            let mut patrimonio_p = 0i64;
            for me in events
                .iter()
                .filter(|me| me.event.date.year() == year && me.event.date.month() == month)
            {
                let (e, m) = (&me.event, me.mask);
                match e.kind {
                    EventKind::Income => {
                        if m.performance {
                            income_p += e.amount_cents;
                        }
                        if m.savings {
                            income_s += e.amount_cents;
                        }
                    }
                    EventKind::FixedOut => {
                        if m.performance {
                            fixed_p += e.amount_cents;
                        }
                        if m.cost_of_living {
                            fixed_c += e.amount_cents;
                        }
                    }
                    EventKind::Daily => {
                        // A DATA decide realizado × previsão (o flag `realized` vem de
                        // `is_projection`, que fica congelado no import e vira stale quando o
                        // dia passa): dia já vivido é realizado; dia futuro é previsão (teto dos
                        // dias restantes + diários pré-lançados). Mês passado nunca tem previsão.
                        if e.date <= today {
                            if m.performance {
                                daily_real_p += e.amount_cents;
                            }
                            if m.cost_of_living {
                                daily_real_c += e.amount_cents;
                            }
                            if m.daily_avg {
                                daily_real_d += e.amount_cents;
                            }
                        } else {
                            if m.performance {
                                daily_proj_p += e.amount_cents;
                            }
                            if m.cost_of_living {
                                daily_proj_c += e.amount_cents;
                            }
                        }
                    }
                    EventKind::Economia => {
                        if m.performance {
                            economia_p += e.amount_cents;
                        }
                        if m.savings {
                            economia_s += e.amount_cents;
                        }
                    }
                    EventKind::Cartao => {
                        if m.performance {
                            cartao_p += e.amount_cents;
                        }
                        if m.cost_of_living {
                            cartao_c += e.amount_cents;
                        }
                    }
                    EventKind::Patrimonio => {
                        if m.performance {
                            patrimonio_p += e.amount_cents;
                        }
                    }
                }
            }
            // Anotação da aba Economia para este mês (import via store_economia_entries). A
            // anotação e os eventos podem representar o MESMO dinheiro após o round-trip; somar
            // dobraria. O mês usa o MAIOR entre o derivado (eventos acima) e a anotação.
            // Mês só-planilha usa a anotação; excedente digitado à mão ainda conta. Trade-off
            // deliberado: dinheiro GENUINAMENTE disjunto (anotação só-planilha + transfer manual
            // ainda não escrito de volta) fica subcontado até o próximo write-back alinhar a aba —
            // preferível à dupla contagem permanente que o `+=` causava após cada round-trip.
            // A reconciliação é POR VIEW (a anotação da aba não tem tag, então alcança as duas):
            // se a anotação dominar o `max`, desligar a Economia de uma tag não muda o número —
            // fronteira honesta, e o efeito computado mostra R$ 0.
            let annotation_cents = annotation.get(&(year, month)).copied().unwrap_or(0);
            let economia_p = economia_p.max(annotation_cents);
            let economia_s = economia_s.max(annotation_cents);
            // Custo de vida = Saídas fixas + Diário realizado + Cartão. Economia e Patrimônio são
            // outflows reais, mas não são custo de vida.
            let cost_of_living_cents = fixed_c + daily_real_c + cartao_c;
            // Performance = Entradas − (fixas + diário + cartão + economia + patrimônio + previsão
            // de diário restante). A previsão entra: o mês corrente já considera o que ainda vai
            // ser gasto até o fim do mês e melhora conforme o gasto real fica abaixo do teto.
            let performance_cents = income_p
                - (fixed_p + daily_real_p + daily_proj_p + cartao_p + economia_p + patrimonio_p);

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
                daily_real_d / elapsed
            } else {
                0
            };

            // Economizado% = economia lançada ÷ entradas (não mais o superávit/performance).
            let savings_rate_bps = if income_s > 0 {
                economia_s * 10_000 / income_s
            } else {
                0
            };

            MonthMetric {
                year,
                month,
                income_cents: income_s,
                income_performance_cents: income_p,
                performance_cents,
                cost_of_living_cents,
                fixed_out_cents: fixed_c,
                daily_out_cents: daily_real_c,
                daily_avg_out_cents: daily_real_d,
                daily_projected_cents: daily_proj_p,
                cartao_cents: cartao_c,
                real_daily_avg_cents,
                savings_rate_bps,
                economia_cents: economia_s,
                patrimonio_cents: patrimonio_p,
                total_outflow_cents: fixed_c + daily_real_c + daily_proj_c + cartao_c,
            }
        })
        .collect()
}

/// Métricas por mês para uma lista arbitrária de `(ano, mês)` — usada pela visão ANUAL (todos os 12
/// meses do ano, realizado + projetado), independente do horizonte do forecast. O `balance_cents`
/// do `MonthEnd` não importa aqui (as métricas só usam os eventos do mês).
pub fn month_metrics_for(
    today: NaiveDate,
    events: &[MetricEvent],
    months: &[(i32, u32)],
    annotation: &std::collections::HashMap<(i32, u32), i64>,
) -> Vec<MonthMetric> {
    let ends: Vec<MonthEnd> = months
        .iter()
        .map(|&(year, month)| MonthEnd {
            year,
            month,
            balance_cents: 0,
        })
        .collect();
    month_metrics(today, events, &ends, annotation)
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
///
/// A rota de leitura projeta sempre com `project_with_metrics` (caixa + métricas juntos, uma
/// projeção só). Esta função cash-only não tem mais chamador de produção — fica como o núcleo
/// mínimo que os testes do motor de caixa exercitam isoladamente.
#[allow(dead_code)]
pub fn project(
    seed_cents: i64,
    today: NaiveDate,
    events: &[CashflowEvent],
    horizon_end: NaiveDate,
) -> Forecast {
    // Sem anotação da aba Economia: só os eventos (inclui transfers de reserva REAIS) decidem o
    // Economizado%. As métricas de produção que precisam da anotação chamam `project_with_metrics`.
    // Conveniência sem máscara: todos os eventos contam em todas as réguas.
    project_with_metrics(
        seed_cents,
        today,
        events,
        &lift_all(events),
        horizon_end,
        &std::collections::HashMap::new(),
    )
}

/// Como [`project`], mas as MÉTRICAS por mês (performance/poupança) usam um conjunto de eventos
/// SEPARADO do encadeamento de caixa.
///
/// O encadeamento diário parte da semente (que já embute todo o passado) e por isso só consome
/// `chain_events` com `date > hoje` — somar o realizado de novo dobraria. Mas a performance do
/// mês corrente PRECISA do realizado de hoje-pra-trás no mês (renda e saídas já lançadas), senão
/// junho aparece com sinal trocado e o guardrail decide sobre o mês pela metade.
/// Por isso `metric_events` cobre o mês inteiro (realizado + projetado).
///
/// `metric_events` carrega a máscara de réguas por evento ([`MetricEvent`]); o
/// encadeamento de caixa (`chain_events`) segue sem máscara — o Saldo sempre conta.
pub fn project_with_metrics(
    seed_cents: i64,
    today: NaiveDate,
    chain_events: &[CashflowEvent],
    metric_events: &[MetricEvent],
    horizon_end: NaiveDate,
    annotation: &std::collections::HashMap<(i32, u32), i64>,
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
    let months = month_metrics(today, metric_events, &month_end, annotation);

    Forecast {
        daily,
        month_end,
        deepest_deficit,
        cash_floor_cents,
        months,
    }
}

/// Modo de gasto global: por onde vive o gasto variável do dia a dia. Re-roteia quais insumos
/// alimentam as réguas do dia ANTES do julgamento — no modo cartão, o Diário zerado é
/// zero-legítimo por design e o velocímetro lê as faturas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendingMode {
    /// Gasto variável em débito/PIX/dinheiro — o gesto-base do método (registrar no Diário).
    Debit,
    /// Gasto variável dentro das faturas de cartão — protocolo reconhecido pelo método
    /// (cada compra soma na fatura em aberto; a fatura crescendo é o instrumento de consciência).
    Card,
}

impl SpendingMode {
    /// Valor estável exposto nas fronteiras que representam o protocolo de gasto detectado.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debit => "debit",
            Self::Card => "card",
        }
    }
}

/// Um mês avulso de Diário (1 dia, ou total dentro do ruído) não flipa o modo.
pub const DAILY_NOISE_CENTS: i64 = 5_000;
/// Constância mínima do gesto diário: dias distintos com Diário realizado no mês.
pub const DAILY_ACTIVE_MIN_DAYS: u32 = 4;

/// Sinais de um mês para a detecção de modo de gasto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthSpendSample {
    /// Dias distintos com Diário realizado (> 0) no mês.
    pub daily_days: u32,
    /// Total de Diário realizado no mês (magnitude, cents).
    pub daily_total_cents: i64,
    /// Existe evento Cartão (realizado ou projetado) no mês.
    pub cartao_present: bool,
}

/// Constância do gesto diário: dias suficientes E volume acima do ruído.
fn diario_ativo(m: &MonthSpendSample) -> bool {
    m.daily_days >= DAILY_ACTIVE_MIN_DAYS && m.daily_total_cents > DAILY_NOISE_CENTS
}

/// Detecção automática pura do modo de gasto sobre a janela móvel (2 últimos meses de calendário
/// completos + mês corrente, ordem cronológica). Sem configuração e sem estado persistido.
///
/// A histerese é assimétrica por construção: entrar no modo cartão exige a janela INTEIRA sem
/// constância de débito (um mês avulso não entra); voltar ao débito exige apenas UM mês com
/// constância — a migração cartão→débito flipa sozinha assim que o débito ganha regularidade,
/// enquanto um lançamento avulso não tira ninguém do modo cartão.
pub fn detect_spending_mode(samples: &[MonthSpendSample]) -> SpendingMode {
    if samples.iter().any(diario_ativo) {
        return SpendingMode::Debit;
    }
    if samples.iter().any(|m| m.cartao_present) {
        return SpendingMode::Card;
    }
    // Sem constância de débito e sem fatura viva: usuário novo / dado insuficiente — o
    // gesto-base do método é o default.
    SpendingMode::Debit
}

/// A janela SUSTENTA o veredito de modo, ou ele é o default de dado insuficiente?
///
/// Existe para a tela não afirmar "detectado dos seus dados" sobre o valor que sobra quando o
/// motor não sabe: os dois casos devolvem `Debit`, e só este os distingue.
pub fn spending_mode_is_detected(samples: &[MonthSpendSample]) -> bool {
    samples.iter().any(|m| diario_ativo(m) || m.cartao_present)
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

    // ---- Janela de meses COMPLETOS: as figuras registradas que a tela publica ----

    // Meio de ano: janeiro até o mês anterior a `today`.
    #[test]
    fn registered_window_mid_year_is_january_through_previous_month() {
        assert_eq!(
            registered_window(d("2026-06-15")),
            vec![(2026, 1), (2026, 2), (2026, 3), (2026, 4), (2026, 5)]
        );
    }

    // Fevereiro: só janeiro sustenta a janela.
    #[test]
    fn registered_window_february_is_january_only() {
        assert_eq!(registered_window(d("2026-02-10")), vec![(2026, 1)]);
    }

    // Janeiro: a janela do ano corrente está vazia, recua para dezembro do ano anterior.
    #[test]
    fn registered_window_january_falls_back_to_prior_december() {
        assert_eq!(registered_window(d("2026-01-05")), vec![(2025, 12)]);
    }

    // ---- Guardrail duplo: caixa × a MESMA régua da economia que a tela do ano julga ----

    /// Um horizonte de caixa largo (folga de 700.000): nestes testes quem decide é a régua da
    /// economia, e o número do caixa serve de contraste.
    fn roomy_cash() -> Forecast {
        let events = [ev("2026-06-02", EventKind::FixedOut, 100_000)];
        project(800_000, d("2026-06-01"), &events, d("2026-06-30"))
    }

    /// Meio de ano com seis meses vividos guardando `rate_bps` da renda — o ano em curso que a
    /// tela do ano julga, com os meses à frente ainda em branco (sem lastro).
    fn year_saving(rate_bps: i64) -> AnnualRuler {
        let months: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 800_000 * rate_bps / 10_000))
            .collect();
        annual_ruler(&months, 2026, d("2026-06-15"))
    }

    /// Entre o piso e o antigo centro da faixa (22%) a régua está VIVA e mais apertada que o
    /// caixa: é ela que manda, e o teto do dia é a folga até o piso — não até um alvo médio.
    #[test]
    fn savings_binds_between_the_floor_and_the_old_mid_target() {
        let s = safe_to_spend_today(&roomy_cash(), &year_saving(2_200), None);

        assert_eq!(s.cash_headroom_cents, 700_000);
        // 20% de 4.800.000 = 960.000 contra 1.056.000 guardados.
        assert_eq!(s.savings_headroom_cents, Some(96_000));
        assert_eq!(s.binding, Guardrail::Savings);
        assert!(s.amount_cents < s.cash_headroom_cents);
        assert_eq!(s.amount_cents, 96_000);
    }

    /// No piso exato a folga é zero: gastar mais tiraria o ano da faixa. A régua ainda morde —
    /// é a fronteira, não a ruptura.
    #[test]
    fn savings_headroom_is_zero_at_the_exact_floor() {
        let s = safe_to_spend_today(&roomy_cash(), &year_saving(2_000), None);

        assert_eq!(s.savings_headroom_cents, Some(0));
        assert_eq!(s.binding, Guardrail::Savings);
        assert_eq!(s.amount_cents, 0);
    }

    /// Abaixo do piso a faixa já está rompida e a régua SOLTA: o déficit é do ano que passou e
    /// nenhum gasto de hoje o desfaz. Travar o dia puniria o que não volta e viraria um zero
    /// perpétuo para quem está mais longe do piso — o diagnóstico sai do teto, não da tela.
    #[test]
    fn savings_releases_below_the_floor() {
        let s = safe_to_spend_today(&roomy_cash(), &year_saving(1_800), None);

        assert_eq!(
            s.savings_headroom_cents,
            Some(-96_000),
            "o diagnóstico continua exposto — some do teto, não da tela"
        );
        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 700_000);
    }

    /// Economia zerada com a reserva de pé é a ordem do método cumprida: o mesmo veredito da
    /// faixa desativa a régua, sem ramo especial no guardrail.
    #[test]
    fn zero_economia_with_a_standing_reserve_disables_the_ruler() {
        let ruler = year_saving(0);
        assert_eq!(band_verdict(&ruler, Some(8.0)), BandVerdict::ZeroByChoice);

        let s = safe_to_spend_today(&roomy_cash(), &ruler, Some(8.0));

        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 700_000);
    }

    /// O mesmo zero SEM reserva é faixa rompida, não escolha — e solta pelo mesmo caminho, mas
    /// com outro veredito por trás.
    #[test]
    fn zero_economia_without_a_reserve_is_a_broken_band() {
        let ruler = year_saving(0);
        assert_eq!(band_verdict(&ruler, Some(3.0)), BandVerdict::BelowBand);

        let s = safe_to_spend_today(&roomy_cash(), &ruler, Some(3.0));

        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.savings_headroom_cents, Some(-960_000));
    }

    /// Sem renda na janela a régua fica INATIVA: ausente, nunca um zero que passaria por
    /// veredito. Só o caixa decide.
    #[test]
    fn savings_is_inactive_without_income_in_the_window() {
        let s = safe_to_spend_today(
            &roomy_cash(),
            &annual_ruler(&[], 2026, d("2026-06-15")),
            None,
        );

        assert_eq!(s.savings_headroom_cents, None);
        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 700_000);
    }

    /// O mês CORRENTE é vivido e conta na janela — com saída lançada no nível do gasto típico,
    /// a renda e a economia dele passam a pesar no piso e o teto do dia se move. É a diferença
    /// para a janela de meses fechados, que só o admitiria no mês seguinte.
    #[test]
    fn the_current_month_with_lastro_moves_the_ceiling() {
        let until_may: Vec<MonthMetric> = (1..=5)
            .map(|m| month(m, 800_000, 500_000, 176_000))
            .collect();
        let with_june: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 176_000))
            .collect();

        let before = safe_to_spend_today(
            &roomy_cash(),
            &annual_ruler(&until_may, 2026, d("2026-06-15")),
            None,
        );
        let after = safe_to_spend_today(
            &roomy_cash(),
            &annual_ruler(&with_june, 2026, d("2026-06-15")),
            None,
        );

        assert_eq!(before.amount_cents, 80_000); // 880.000 − 20% de 4.000.000
        assert_eq!(after.amount_cents, 96_000); // 1.056.000 − 20% de 4.800.000
    }

    /// Mês à frente apenas projetado, sem lastro, não altera nada: a régua julga o recorte
    /// vivido enquanto houver silêncio à frente, e o guardrail lê a mesma janela.
    #[test]
    fn a_projected_month_without_lastro_does_not_move_the_ceiling() {
        let lived: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 176_000))
            .collect();
        let mut with_july = lived.clone();
        with_july.push(month(7, 900_000, 50_000, 0));

        let without = safe_to_spend_today(
            &roomy_cash(),
            &annual_ruler(&lived, 2026, d("2026-06-15")),
            None,
        );
        let with = safe_to_spend_today(
            &roomy_cash(),
            &annual_ruler(&with_july, 2026, d("2026-06-15")),
            None,
        );

        assert_eq!(with.savings_headroom_cents, without.savings_headroom_cents);
        assert_eq!(with.amount_cents, without.amount_cents);
    }

    /// A folga publicada É o negativo do déficit até o piso que a régua anual já calcula, no
    /// mesmo recorte — uma derivação, nunca uma segunda divisão. Vale nos dois recortes: o
    /// vivido (ano em curso) e o ano inteiro (todo mês lastreado).
    #[test]
    fn the_headroom_is_exactly_the_negated_shortfall_of_the_judged_window() {
        let open = year_saving(2_200);
        assert!(open.scope_lived);
        assert_eq!(
            safe_to_spend_today(&roomy_cash(), &open, None).savings_headroom_cents,
            Some(-open.shortfall_lived_cents)
        );

        let closed: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 500_000, 176_000))
            .collect();
        let closed = annual_ruler(&closed, 2026, d("2026-12-31"));
        assert!(!closed.scope_lived);
        assert_eq!(
            safe_to_spend_today(&roomy_cash(), &closed, None).savings_headroom_cents,
            Some(-closed.shortfall_year_cents)
        );
    }

    /// Os arredondamentos do método sobrevivem à derivação: o percentual que JULGA trunca, e a
    /// falta até o piso arredonda meio para cima — a folga herda essa conta, sem refazê-la.
    #[test]
    fn the_headroom_keeps_the_rulers_truncation_and_rounding() {
        let mut months: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 183_334))
            .collect();
        months[0] = month(1, 800_003, 500_000, 183_334);
        let ruler = annual_ruler(&months, 2026, d("2026-06-15"));

        // Renda vivida 4.800.003: o piso é 960.000,6 → arredonda para 960.001, não trunca.
        assert_eq!(ruler.income_lived_cents, 4_800_003);
        assert_eq!(ruler.economia_lived_cents, 1_100_004);
        assert_eq!(ruler.bps, Some(2_291), "o percentual que julga TRUNCA");

        let s = safe_to_spend_today(&roomy_cash(), &ruler, None);
        assert_eq!(s.savings_headroom_cents, Some(140_003)); // 1.100.004 − 960.001
    }

    // Cobertura: meses futuros esparsos (só fixas) vs gasto típico → sinaliza incompleto.
    #[test]
    fn month_coverage_flags_sparse_future_months() {
        let mm = |year, month, cost: i64| MonthMetric {
            year,
            month,
            income_cents: 0,
            income_performance_cents: 0,
            performance_cents: 0,
            cost_of_living_cents: cost,
            fixed_out_cents: cost,
            daily_out_cents: 0,
            daily_avg_out_cents: 0,
            daily_projected_cents: 0,
            cartao_cents: 0,
            real_daily_avg_cents: 0,
            savings_rate_bps: 0,
            economia_cents: 0,
            patrimonio_cents: 0,
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

    // Sem baseline (nenhum mês realizado) → cobertura VAZIA, não "tudo completo".
    #[test]
    fn month_coverage_empty_without_baseline() {
        let mm = |year, month, cost: i64| MonthMetric {
            year,
            month,
            income_cents: 0,
            income_performance_cents: 0,
            performance_cents: 0,
            cost_of_living_cents: cost,
            fixed_out_cents: cost,
            daily_out_cents: 0,
            daily_avg_out_cents: 0,
            daily_projected_cents: 0,
            cartao_cents: 0,
            real_daily_avg_cents: 0,
            savings_rate_bps: 0,
            economia_cents: 0,
            patrimonio_cents: 0,
            total_outflow_cents: cost,
        };
        let months = [mm(2026, 7, 1_000), mm(2026, 8, 380)];
        let cov = month_coverage(&months, d("2026-06-13"), 0, 6_000);
        assert!(cov.is_empty());
    }

    /// Conta futura pré-lançada (fatura/salário) num mês à frente limita o gasto de HOJE pelo
    /// caixa — só visível porque o horizonte varre além do mês corrente. A régua de caixa é o
    /// Saldo da planilha: não abrir o bico, e o estoque da reserva não a aperta (no método a
    /// reserva é o amortecedor acionado QUANDO o saldo fica negativo).
    #[test]
    fn safe_to_spend_cash_binds_on_future_month_commitment() {
        let events = [
            ev("2026-06-01", EventKind::Income, 1_000_000),
            ev("2026-07-15", EventKind::FixedOut, 950_000), // fatura lá na frente
        ];
        let f = project(0, d("2026-06-01"), &events, d("2026-07-31"));
        let s = safe_to_spend_today(&f, &year_saving(2_200), Some(8.0));

        // Caixa cai para 50.000 em 15/jul (o "buraco do futuro") — mais apertado que a economia.
        assert_eq!(s.cash_headroom_cents, 50_000);
        assert_eq!(s.savings_headroom_cents, Some(96_000));
        assert_eq!(s.binding, Guardrail::Cash);
        assert_eq!(s.amount_cents, 50_000);
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
        // Visão retrospectiva (hoje ≥ todos os eventos): tudo é realizado.
        let f = project(0, d("2026-03-12"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.cost_of_living_cents, 600000); // 400 + 200
        assert_eq!(m.performance_cents, 400000); // 1000 - 600
    }

    // Modelo canônico de 5 tipos. Cartão entra no custo de vida como bucket próprio;
    // Economia sai do custo de vida, mas continua reduzindo Performance porque o dinheiro saiu.
    #[test]
    fn five_type_worked_example_matches_target_table() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1_000_000),
            ev("2026-03-10", EventKind::FixedOut, 300_000),
            ev("2026-03-12", EventKind::Daily, 200_000),
            ev("2026-03-15", EventKind::Cartao, 150_000),
            ev("2026-03-20", EventKind::Economia, 100_000),
        ];
        // Visão prospectiva (hoje antes dos eventos): o mês inteiro é pré-lançado. O Diário
        // futuro entra como PREVISÃO (mesma coluna da planilha; a data decide), com o mesmo
        // desconto na Performance; custo de vida reporta só o realizado (0 aqui).
        let f = project(0, d("2026-03-01"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();

        assert_eq!(m.cost_of_living_cents, 450_000); // fixas 300 + cartão 150 (diário ainda é previsão)
        assert_eq!(m.daily_projected_cents, 200_000);
        assert_eq!(m.total_outflow_cents, 650_000); // 300 + 200 + 150 (lançado, p/ cobertura)
        assert_eq!(m.cartao_cents, 150_000);
        assert_eq!(m.economia_cents, 100_000);
        assert_eq!(m.patrimonio_cents, 0);
        assert_eq!(m.savings_rate_bps, 1_000); // 100 / 1000 = 10%
        assert_eq!(m.performance_cents, 250_000); // 1000 - (300 + 200 + 150 + 100)
        assert_eq!(f.month_end[0].balance_cents, 250_000);

        // A MESMA tabela, vista do fim do mês (hoje ≥ todos os eventos): tudo realizado.
        let f = project(0, d("2026-03-20"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.cost_of_living_cents, 650_000); // 300 + 200 + 150
        assert_eq!(m.daily_projected_cents, 0);
        assert_eq!(m.performance_cents, 250_000); // idêntica: a previsão virou realizado
    }

    // Quando a anotação e os eventos representam o MESMO dinheiro após o round-trip, o mês usa o
    // maior dos dois; somá-los duplicaria o Economizado% e a queda na Performance.
    #[test]
    fn annotation_equal_to_derived_economia_counts_once() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1_000_000),
            ev("2026-03-20", EventKind::Economia, 100_000),
        ];
        let mut annotation = std::collections::HashMap::new();
        annotation.insert((2026i32, 3u32), 100_000i64);
        let f = project_with_metrics(
            0,
            d("2026-03-01"),
            &events,
            &lift_all(&events),
            d("2026-03-31"),
            &annotation,
        );
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.economia_cents, 100_000); // uma vez, não 200_000
        assert_eq!(m.savings_rate_bps, 1_000); // 10%, não 20%
        assert_eq!(m.performance_cents, 900_000); // 1000 − 100, não 1000 − 200
    }

    // Excedente digitado à mão na aba Economia (acima do derivado) ainda conta: a aba é o
    // registro do método; o mês vale o MAIOR entre derivado e anotação.
    #[test]
    fn annotation_excess_over_derived_still_counts() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1_000_000),
            ev("2026-03-20", EventKind::Economia, 100_000),
        ];
        let mut annotation = std::collections::HashMap::new();
        annotation.insert((2026i32, 3u32), 160_000i64);
        let f = project_with_metrics(
            0,
            d("2026-03-01"),
            &events,
            &lift_all(&events),
            d("2026-03-31"),
            &annotation,
        );
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.economia_cents, 160_000);
        assert_eq!(m.performance_cents, 840_000);
    }

    // Mês só-planilha (sem eventos derivados): a anotação continua valendo sozinha.
    #[test]
    fn annotation_only_month_still_counts() {
        let events = [ev("2026-03-05", EventKind::Income, 1_000_000)];
        let mut annotation = std::collections::HashMap::new();
        annotation.insert((2026i32, 3u32), 50_000i64);
        let f = project_with_metrics(
            0,
            d("2026-03-01"),
            &events,
            &lift_all(&events),
            d("2026-03-31"),
            &annotation,
        );
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.economia_cents, 50_000);
        assert_eq!(m.performance_cents, 950_000);
    }

    // A Performance do mês CORRENTE desconta também a previsão de diário restante (teto dos dias
    // futuros + diários pré-lançados): o mês nasce mostrando o cenário cheio e melhora conforme o
    // gasto real fica abaixo do teto. Custo de vida segue só com o realizado.
    #[test]
    fn performance_includes_remaining_daily_forecast() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1_000_000),
            ev("2026-03-10", EventKind::FixedOut, 300_000),
            ev("2026-03-12", EventKind::Daily, 200_000), // realizado
            CashflowEvent {
                date: d("2026-03-20"),
                kind: EventKind::Daily,
                amount_cents: 150_000, // previsão (dias futuros × teto)
                realized: false,
            },
        ];
        let f = project(0, d("2026-03-12"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.daily_projected_cents, 150_000);
        assert_eq!(
            m.daily_out_cents, 200_000,
            "realizado não mistura com previsão"
        );
        assert_eq!(
            m.cost_of_living_cents, 500_000,
            "custo de vida = realizado (300 + 200)"
        );
        assert_eq!(
            m.performance_cents, 350_000,
            "1000 − (300 + 200 + 150 de previsão)"
        );
    }

    // Flag `is_projection` congelado (stale) num mês JÁ ENCERRADO: a data decide — o diário
    // conta como realizado e o mês passado não carrega previsão nenhuma (nem muda de valor
    // retroativamente por falta de re-import).
    #[test]
    fn past_month_ignores_stale_projection_flag() {
        let events = [
            ev("2026-06-05", EventKind::Income, 1_000_000),
            CashflowEvent {
                date: d("2026-06-20"),
                kind: EventKind::Daily,
                amount_cents: 80_000,
                realized: false, // stale: importado como futuro e nunca re-importado
            },
        ];
        let metrics = month_metrics_for(
            d("2026-07-02"),
            &lift_all(&events),
            &[(2026, 6)],
            &std::collections::HashMap::new(),
        );
        let m = metrics.iter().find(|m| m.month == 6).unwrap();
        assert_eq!(m.daily_projected_cents, 0, "mês encerrado não tem previsão");
        assert_eq!(m.daily_out_cents, 80_000);
        assert_eq!(m.performance_cents, 920_000);
    }

    // ---- Máscara por régua (interruptores de contabilidade por tag) ----

    /// Evento de métrica com máscara custom (helper local dos testes de máscara).
    fn mev(date: &str, kind: EventKind, amount_cents: i64, mask: RulerMask) -> MetricEvent {
        MetricEvent {
            event: ev(date, kind, amount_cents),
            mask,
        }
    }

    fn no_annotation() -> std::collections::HashMap<(i32, u32), i64> {
        std::collections::HashMap::new()
    }

    // Um gasto fora do CUSTO DE VIDA (e só dele) some do custo e dos componentes
    // exibidos, mas segue contando em Performance e no Diário médio — as views do
    // mesmo bucket divergem, e cada campo serve a régua que declara.
    #[test]
    fn mask_excludes_single_ruler_only() {
        let off_cost = RulerMask {
            cost_of_living: false,
            ..RulerMask::ALL
        };
        let events = [
            mev("2026-03-05", EventKind::Income, 1_000_000, RulerMask::ALL),
            mev("2026-03-10", EventKind::Daily, 200_000, RulerMask::ALL),
            mev("2026-03-12", EventKind::Daily, 100_000, off_cost),
        ];
        let m = &month_metrics_for(d("2026-03-31"), &events, &[(2026, 3)], &no_annotation())[0];
        assert_eq!(
            m.cost_of_living_cents, 200_000,
            "custo sem o gasto mascarado"
        );
        assert_eq!(
            m.daily_out_cents, 200_000,
            "componente exibido = view do custo"
        );
        assert_eq!(
            m.daily_avg_out_cents, 300_000,
            "diário médio segue contando"
        );
        assert_eq!(m.real_daily_avg_cents, 300_000 / 31);
        assert_eq!(m.performance_cents, 700_000, "performance segue contando");
        assert_eq!(
            m.total_outflow_cents, 200_000,
            "cobertura acompanha o custo"
        );
    }

    // A renda fora da ECONOMIA sai da base do Economizado% ("o que devolvem não é sua
    // renda"), mas permanece na Performance — o denominador menor SOBE o percentual.
    #[test]
    fn mask_income_diverges_between_savings_base_and_performance() {
        let off_savings = RulerMask {
            savings: false,
            ..RulerMask::ALL
        };
        let events = [
            mev("2026-03-05", EventKind::Income, 500_000, RulerMask::ALL),
            mev("2026-03-06", EventKind::Income, 500_000, off_savings),
            mev("2026-03-20", EventKind::Economia, 100_000, RulerMask::ALL),
        ];
        let m = &month_metrics_for(d("2026-03-31"), &events, &[(2026, 3)], &no_annotation())[0];
        assert_eq!(
            m.income_cents, 500_000,
            "base do Economizado% = view Economia"
        );
        assert_eq!(m.income_performance_cents, 1_000_000);
        assert_eq!(m.savings_rate_bps, 2_000, "100/500 = 20%, não 10%");
        assert_eq!(
            m.performance_cents, 900_000,
            "1000 − 100 na view Performance"
        );
    }

    // O teste da mentira aritmética (decisão do desenho): a MESMA exclusão move a
    // Performance pelo LÍQUIDO (entrou − saiu) e o custo de vida pela SAÍDA — quem
    // devolve mais do que gasta PIORA a performance ao sair, enquanto o custo cai.
    #[test]
    fn mask_performance_moves_by_net_cost_by_outflow() {
        let base = [
            mev("2026-03-05", EventKind::Income, 1_000_000, RulerMask::ALL),
            mev("2026-03-08", EventKind::Income, 497_764, RulerMask::ALL),
            mev("2026-03-10", EventKind::Daily, 407_764, RulerMask::ALL),
        ];
        let with = &month_metrics_for(d("2026-03-31"), &base, &[(2026, 3)], &no_annotation())[0];

        let off_perf_cost = RulerMask {
            performance: false,
            cost_of_living: false,
            ..RulerMask::ALL
        };
        let masked = [
            base[0],
            mev("2026-03-08", EventKind::Income, 497_764, off_perf_cost),
            mev("2026-03-10", EventKind::Daily, 407_764, off_perf_cost),
        ];
        let without =
            &month_metrics_for(d("2026-03-31"), &masked, &[(2026, 3)], &no_annotation())[0];

        // Custo cai pela saída inteira; performance PIORA em (entrou − saiu) = 90.000.
        assert_eq!(
            with.cost_of_living_cents - without.cost_of_living_cents,
            407_764
        );
        assert_eq!(with.performance_cents - without.performance_cents, 90_000);
    }

    // Fronteira da anotação: a aba Economia não tem tag, então quando ela domina o
    // `max`, desligar a Economia de uma tag não muda o número — o efeito honesto é 0.
    #[test]
    fn mask_annotation_floor_survives_savings_exclusion() {
        let off_savings = RulerMask {
            savings: false,
            ..RulerMask::ALL
        };
        let events = [
            mev("2026-03-05", EventKind::Income, 1_000_000, RulerMask::ALL),
            mev("2026-03-20", EventKind::Economia, 100_000, off_savings),
        ];
        let mut annotation = std::collections::HashMap::new();
        annotation.insert((2026i32, 3u32), 100_000i64);
        let m = &month_metrics_for(d("2026-03-31"), &events, &[(2026, 3)], &annotation)[0];
        assert_eq!(
            m.economia_cents, 100_000,
            "anotação sustenta o piso mesmo com o evento fora da Economia"
        );
        assert_eq!(m.savings_rate_bps, 1_000);
    }

    // Interseção de tags: fora de uma régua se QUALQUER tag excluir (semântica do
    // flag único preservada na composição).
    #[test]
    fn mask_and_composes_by_intersection() {
        let off_cost = RulerMask {
            cost_of_living: false,
            ..RulerMask::ALL
        };
        let off_savings = RulerMask {
            savings: false,
            ..RulerMask::ALL
        };
        let composed = off_cost.and(off_savings);
        assert!(!composed.cost_of_living);
        assert!(!composed.savings);
        assert!(composed.performance);
        assert!(composed.daily_avg);
        assert_eq!(RulerMask::ALL.and(RulerMask::ALL), RulerMask::ALL);
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
        // Performance = renda (1000) − diário (200) − economia (250) = 550.
        // Economia fica fora do custo de vida, mas reduz Performance uma vez.
        assert_eq!(m.performance_cents, 550_000);
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
            Some(EventKind::Cartao)
        ); // credit bucket
        assert_eq!(
            classify("expense", false, Some("debit"), None),
            Some(EventKind::Daily)
        ); // variable débito
        assert_eq!(
            classify("expense", false, None, None),
            Some(EventKind::Daily)
        );
    }

    // Economia/Patrimônio: reserve = Economia; illiquid = Patrimônio; entre líquidos = net-zero.
    #[test]
    fn classify_transfer_to_reserve_is_economia() {
        // Poupança real (reserva) → Economia.
        assert_eq!(
            classify("transfer", false, None, Some("reserve")),
            Some(EventKind::Economia)
        );
        // FGTS/previdência (illiquid) = Patrimônio, não Economia acessível.
        assert_eq!(
            classify("transfer", false, None, Some("illiquid")),
            Some(EventKind::Patrimonio)
        );
        // Vale-refeição (restricted) é gasto restrito, NÃO poupança → não conta como Economia.
        assert_eq!(classify("transfer", false, None, Some("restricted")), None);
        // Entre contas líquidas (ou destino desconhecido) → net-zero, ignorado.
        assert_eq!(classify("transfer", false, None, Some("liquid")), None);
        assert_eq!(classify("transfer", false, None, None), None);
    }

    // ---- Economia + previsão de diário como driver ----

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

    // A previsão de diário (teto dos dias restantes) desconta a Performance do mês corrente,
    // mas fica FORA do custo de vida e do diário médio (que reportam só o realizado).
    #[test]
    fn daily_ceiling_feeds_performance_not_cost_of_living() {
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
        assert_eq!(m.daily_projected_cents, 210_000);
        // Performance = 1_000_000 − 210_000 de previsão restante.
        assert_eq!(m.performance_cents, 790_000);
    }

    // Regressão: economia e previsão de diário descontam a Performance uma vez cada;
    // custo de vida e diário médio seguem só com o realizado.
    #[test]
    fn performance_subtracts_economia_and_projected_once() {
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
        // performance = income(1_000) − fixed(300) − diário(50) − previsão(30) − economia(200).
        assert_eq!(m.performance_cents, 420_000);
        // economia still feeds savings_rate
        assert_eq!(m.economia_cents, 200_000);
        assert_eq!(m.savings_rate_bps, 2_000); // 200/1000 = 20%
    }

    // Economia reduz Performance uma vez, mas fica fora de custo de vida.
    #[test]
    fn performance_counts_economia_as_outflow_once() {
        // Arrange: renda 5_000_000, Saída fixa 1_000_000, Diário realizado 500_000.
        // Economia 800_000 representa o transfer da aba Economia (anotação de taxa).
        // A poupança real já está no custo de vida como expense row (FixedOut ou Daily).
        let events = [
            ev("2026-05-01", EventKind::Income, 5_000_000),
            ev("2026-05-10", EventKind::FixedOut, 1_000_000),
            ev("2026-05-15", EventKind::Daily, 500_000), // realized
            ev("2026-05-20", EventKind::Economia, 800_000), // savings-rate annotation
        ];
        // Visão retrospectiva (hoje ≥ todos os eventos): tudo é realizado.
        let f = project(0, d("2026-05-20"), &events, d("2026-05-31"));
        let m = f.months.iter().find(|m| m.month == 5).unwrap();

        // cost_of_living = FixedOut(1_000) + Daily(500) = 1_500_000 (Economia NOT in here)
        assert_eq!(m.cost_of_living_cents, 1_500_000);
        // economia_cents is reported separately and feeds savings_rate_bps.
        assert_eq!(m.economia_cents, 800_000);
        // performance = income(5_000) − fixed(1_000) − daily(500) − economia(800) = 2_700_000
        assert_eq!(m.performance_cents, 2_700_000);
        // savings_rate_bps = 800_000 / 5_000_000 = 1600 bps (16%) — unaffected
        assert_eq!(m.savings_rate_bps, 1_600);
    }

    // --- detect_spending_mode -------------------------------------------------------------

    fn sample(daily_days: u32, daily_total_cents: i64, cartao_present: bool) -> MonthSpendSample {
        MonthSpendSample {
            daily_days,
            daily_total_cents,
            cartao_present,
        }
    }

    // Sem dado nenhum na janela, o modo é o gesto-base do método.
    #[test]
    fn spending_mode_separates_a_verdict_from_the_default() {
        // Janela vazia e janela só com ruído devolvem Debit — mas nenhuma das duas o SUSTENTA.
        assert!(!spending_mode_is_detected(&[]));
        let noise = [MonthSpendSample {
            daily_days: 1,
            daily_total_cents: 900,
            cartao_present: false,
        }];
        assert!(!spending_mode_is_detected(&noise));

        let constant = [MonthSpendSample {
            daily_days: 5,
            daily_total_cents: 30_000,
            cartao_present: false,
        }];
        assert!(spending_mode_is_detected(&constant));

        let cards = [MonthSpendSample {
            daily_days: 0,
            daily_total_cents: 0,
            cartao_present: true,
        }];
        assert!(spending_mode_is_detected(&cards));
    }

    #[test]
    fn spending_mode_defaults_to_debit_without_data() {
        assert_eq!(detect_spending_mode(&[]), SpendingMode::Debit);
        let window = [
            sample(0, 0, false),
            sample(0, 0, false),
            sample(0, 0, false),
        ];
        assert_eq!(detect_spending_mode(&window), SpendingMode::Debit);
    }

    // Diário morto na janela inteira + fatura viva = o perfil que o método reconhece como
    // modo cartão (o gasto variável vive nas faturas).
    #[test]
    fn spending_mode_card_when_daily_dead_and_fatura_alive() {
        let window = [sample(0, 0, true), sample(0, 0, true), sample(0, 0, false)];
        assert_eq!(detect_spending_mode(&window), SpendingMode::Card);
    }

    // Um lançamento avulso não flipa o modo: 1 dia (mesmo acima do ruído), ou vários dias
    // dentro do ruído, não contam como constância de débito.
    #[test]
    fn spending_mode_stray_daily_purchase_does_not_flip_card_mode() {
        // Um único dia com R$ 130,60 (acima do ruído, mas sem constância).
        let one_day = [
            sample(0, 0, true),
            sample(1, 13_060, true),
            sample(0, 0, true),
        ];
        assert_eq!(detect_spending_mode(&one_day), SpendingMode::Card);
        // Cinco dias que somam R$ 40,00 (constância de dias, volume dentro do ruído).
        let low_volume = [
            sample(0, 0, true),
            sample(5, 4_000, true),
            sample(0, 0, false),
        ];
        assert_eq!(detect_spending_mode(&low_volume), SpendingMode::Card);
    }

    // Fronteira exata da constância: 4 dias e um centavo acima do ruído já é débito;
    // 4 dias com total exatamente no ruído ainda não é.
    #[test]
    fn spending_mode_constancy_boundary() {
        let at_noise = [
            sample(0, 0, true),
            sample(4, 5_000, true),
            sample(0, 0, false),
        ];
        assert_eq!(detect_spending_mode(&at_noise), SpendingMode::Card);
        let above_noise = [
            sample(0, 0, true),
            sample(4, 5_001, true),
            sample(0, 0, false),
        ];
        assert_eq!(detect_spending_mode(&above_noise), SpendingMode::Debit);
    }

    // Migração cartão→débito: assim que UM mês ganha constância (mesmo o corrente, mesmo com
    // faturas ainda vivas), o modo volta ao débito — a transição acontece sozinha.
    #[test]
    fn spending_mode_debit_constancy_wins_even_with_faturas_alive() {
        let window = [
            sample(0, 0, true),
            sample(0, 0, true),
            sample(8, 60_000, true),
        ];
        assert_eq!(detect_spending_mode(&window), SpendingMode::Debit);
    }

    // Diário morto SEM fatura viva não é modo cartão — é só ausência de dado (default débito).
    #[test]
    fn spending_mode_debit_when_no_fatura_in_window() {
        let window = [
            sample(0, 0, false),
            sample(1, 2_000, false),
            sample(0, 0, false),
        ];
        assert_eq!(detect_spending_mode(&window), SpendingMode::Debit);
    }

    // --- Régua anual: o teste de lastro ---

    fn month(
        month: u32,
        income_cents: i64,
        outflow_cents: i64,
        economia_cents: i64,
    ) -> MonthMetric {
        MonthMetric {
            year: 2026,
            month,
            income_cents,
            income_performance_cents: income_cents,
            performance_cents: income_cents - outflow_cents,
            cost_of_living_cents: outflow_cents - economia_cents,
            fixed_out_cents: outflow_cents - economia_cents,
            daily_out_cents: 0,
            daily_avg_out_cents: 0,
            daily_projected_cents: 0,
            cartao_cents: 0,
            real_daily_avg_cents: 0,
            savings_rate_bps: 0,
            economia_cents,
            patrimonio_cents: 0,
            total_outflow_cents: outflow_cents - economia_cents,
        }
    }

    // Um mês à frente com saída magra derruba o lastro do ano: a régua recua ao vivido e diz
    // que recuou. Sem isso, um dezembro vazio inflaria o percentual do ano inteiro.
    #[test]
    fn annual_ruler_falls_back_to_the_lived_cut_when_a_future_month_is_thin() {
        let mut months: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 200_000))
            .collect();
        months.push(month(7, 900_000, 50_000, 0));

        let ruler = annual_ruler(&months, 2026, d("2026-06-15"));

        assert_eq!(ruler.lived_months, 6);
        assert_eq!(ruler.typical_spend_cents, 500_000);
        assert_eq!(ruler.suspect_months(), vec![7, 8, 9, 10, 11, 12]);
        assert_eq!(ruler.lived_bps, Some(2_500));
        // 1.200.000 guardados sobre 5.700.000 de renda: o mês magro dilui o ano.
        assert_eq!(ruler.projected_bps, Some(2_105));
        assert_eq!(ruler.bps, Some(2_500));
        assert!(ruler.scope_lived);
    }

    // Com todo mês do ano lastreado, a régua fala do ano inteiro — é o ano fechado que o método
    // julga.
    #[test]
    fn annual_ruler_covers_the_whole_year_when_every_month_has_lastro() {
        let months: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 500_000, 200_000))
            .collect();

        let ruler = annual_ruler(&months, 2026, d("2026-12-31"));

        assert_eq!(ruler.lived_months, 12);
        assert!(ruler.suspect_months().is_empty());
        assert!(!ruler.scope_lived);
        assert_eq!(ruler.bps, Some(2_500));
        assert_eq!(ruler.income_year_cents, 9_600_000);
    }

    // Ano sem renda registrada não tem percentual: nulo, jamais um zero que passaria por
    // veredito de "não guardou nada".
    #[test]
    fn annual_ruler_without_income_has_no_percentage() {
        let ruler = annual_ruler(&[], 2026, d("2026-07-25"));

        assert_eq!(ruler.bps, None);
        assert_eq!(ruler.typical_spend_cents, 0);
        assert!(
            ruler.suspect_months().is_empty(),
            "sem gasto típico não há como acusar falta de lastro"
        );
        assert!(!ruler.has_data);
    }

    /// Seis meses vividos guardando 25%, e um julho magro à frente — a régua do ano em curso.
    fn open_year() -> AnnualRuler {
        let mut months: Vec<MonthMetric> = (1..=6)
            .map(|m| month(m, 800_000, 500_000, 200_000))
            .collect();
        months.push(month(7, 900_000, 50_000, 0));
        annual_ruler(&months, 2026, d("2026-06-15"))
    }

    // Cada mês sem lastro carrega o preço do próprio silêncio: quanto faltaria lançar para ele
    // parecer um mês típico. É o insumo do cenário de fim de ano — e quem o compõe (tela ou
    // conversa) nunca refaz a subtração.
    #[test]
    fn annual_ruler_prices_each_thin_month_by_what_it_is_missing() {
        let ruler = open_year();

        let julho = ruler.months.iter().find(|m| m.month == 7).unwrap();
        assert!(julho.suspect);
        assert_eq!(julho.outflow_cents, 50_000);
        assert_eq!(julho.missing_cents, 450_000); // 500.000 típico − 50.000 lançados

        // Mês ausente da entrada não lançou nada: falta o típico inteiro.
        assert_eq!(
            ruler
                .months
                .iter()
                .find(|m| m.month == 12)
                .unwrap()
                .missing_cents,
            500_000
        );
        // Mês vivido não deve nada ao futuro — o que ele custou já é fato.
        let junho = ruler.months.iter().find(|m| m.month == 6).unwrap();
        assert!(junho.lived && !junho.suspect);
        assert_eq!(junho.missing_cents, 0);
    }

    // A falta para o piso de 20% nos dois recortes, e a parcela por mês que resta — a mesma
    // conta que a tela imprime e que a conversa responde.
    #[test]
    fn annual_ruler_measures_the_shortfall_to_the_floor() {
        let ruler = open_year();

        // Vivido: 20% de 4.800.000 = 960.000, e já guardou 1.200.000 → o piso ficou para trás.
        assert_eq!(ruler.shortfall_lived_cents, -240_000);
        // Ano: 20% de 5.700.000 = 1.140.000 contra os mesmos 1.200.000.
        assert_eq!(ruler.shortfall_year_cents, -60_000);
        assert_eq!(ruler.per_month_shortfall_cents, Some(-10_000)); // ÷ 6 meses à frente

        let closed: Vec<MonthMetric> = (1..=12).map(|m| month(m, 800_000, 800_000, 0)).collect();
        let closed = annual_ruler(&closed, 2026, d("2026-12-31"));
        assert_eq!(closed.shortfall_year_cents, 1_920_000);
        assert_eq!(
            closed.per_month_shortfall_cents, None,
            "ano sem futuro não divide a falta por mês nenhum"
        );
    }

    // --- Régua anual: o veredito da faixa ---

    #[test]
    fn band_verdict_reads_the_ruler_against_the_band() {
        let inside: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 600_000, 200_000))
            .collect();
        let inside = annual_ruler(&inside, 2026, d("2026-12-31"));
        assert_eq!(band_verdict(&inside, None), BandVerdict::InBand);

        let above: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 500_000, 300_000))
            .collect();
        let above = annual_ruler(&above, 2026, d("2026-12-31"));
        assert_eq!(band_verdict(&above, None), BandVerdict::AboveBand);

        let below: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 750_000, 50_000))
            .collect();
        let below = annual_ruler(&below, 2026, d("2026-12-31"));
        assert_eq!(band_verdict(&below, None), BandVerdict::BelowBand);
    }

    // Zerar a economia com a reserva já protegida é a ordem do método cumprida, não uma falta.
    // Sem reserva conhecida, o mesmo zero é "não guardou nada".
    #[test]
    fn band_verdict_reads_zero_economia_against_the_reserve() {
        let months: Vec<MonthMetric> = (1..=12).map(|m| month(m, 800_000, 800_000, 0)).collect();
        let ruler = annual_ruler(&months, 2026, d("2026-12-31"));

        assert_eq!(band_verdict(&ruler, Some(8.0)), BandVerdict::ZeroByChoice);
        assert_eq!(band_verdict(&ruler, Some(3.0)), BandVerdict::BelowBand);
        assert_eq!(band_verdict(&ruler, None), BandVerdict::BelowBand);
    }

    // Ano sem um único movimento vivido não julga ninguém.
    #[test]
    fn band_verdict_without_data_does_not_judge() {
        let ruler = annual_ruler(&[], 2026, d("2026-07-25"));
        assert_eq!(band_verdict(&ruler, Some(8.0)), BandVerdict::NoRecord);
    }

    // --- Onde o ano termina ---

    // O ano termina no último mês com saldo, e o cenário desconta o que os meses sem lastro
    // deixaram de lançar até ali.
    #[test]
    fn year_end_scenario_discounts_the_thin_months_up_to_the_closing_month() {
        let ruler = open_year();
        let balances: Vec<(u32, i64)> = vec![(6, 1_800_000), (7, 1_900_000), (12, 2_000_000)];

        let end = year_end_scenario(&ruler, &balances);

        assert_eq!(end.end_month, Some(12));
        assert_eq!(end.end_balance_cents, Some(2_000_000));
        // 450.000 de julho + 500.000 × 5 meses vazios = 2.950.000 fora da projeção.
        assert_eq!(end.end_balance_typical_cents, Some(-950_000));
    }

    // Horizonte curto: só os meses sem lastro DENTRO da janela que fecha o ano entram na conta —
    // descontar um dezembro que o saldo nem alcança seria inventar um rombo.
    #[test]
    fn year_end_scenario_only_counts_thin_months_inside_the_horizon() {
        let ruler = open_year();

        let end = year_end_scenario(&ruler, &[(6, 1_800_000), (7, 1_900_000)]);

        assert_eq!(end.end_month, Some(7));
        assert_eq!(end.end_balance_typical_cents, Some(1_450_000)); // 1.900.000 − 450.000
    }

    // Ano inteiro lastreado não tem cenário alternativo: a projeção já conta a vida inteira.
    #[test]
    fn year_end_scenario_has_no_alternative_when_every_month_has_lastro() {
        let months: Vec<MonthMetric> = (1..=12)
            .map(|m| month(m, 800_000, 500_000, 200_000))
            .collect();
        let ruler = annual_ruler(&months, 2026, d("2026-12-31"));

        let end = year_end_scenario(&ruler, &[(12, 2_000_000)]);

        assert_eq!(end.end_balance_cents, Some(2_000_000));
        assert_eq!(end.end_balance_typical_cents, None);
    }

    // Sem nenhum saldo, o ano não tem fim para mostrar — nulo, nunca zero.
    #[test]
    fn year_end_scenario_without_balances_has_no_end() {
        let end = year_end_scenario(&open_year(), &[]);

        assert_eq!(end.end_month, None);
        assert_eq!(end.end_balance_cents, None);
        assert_eq!(end.end_balance_typical_cents, None);
    }

    #[test]
    fn guardrail_as_str_covers_every_variant() {
        assert_eq!(Guardrail::Cash.as_str(), "cash");
        assert_eq!(Guardrail::Savings.as_str(), "savings");
    }

    #[test]
    fn spending_mode_as_str_covers_every_variant() {
        assert_eq!(SpendingMode::Debit.as_str(), "debit");
        assert_eq!(SpendingMode::Card.as_str(), "card");
    }
}
