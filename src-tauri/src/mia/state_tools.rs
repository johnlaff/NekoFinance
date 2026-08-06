//! As quatro perguntas de estado: como estou agora, o que falta de dado, qual meu teto, onde
//! está meu dinheiro.
//!
//! Nenhuma régua nasce aqui. As três primeiras carregam e compõem a mesma `ForecastReading` que
//! a tela lê — UMA composição por chamada, mesmo com várias inclusões opcionais — e só recortam
//! campos dela; a quarta (contas e patrimônio) não pertence a essa leitura e continua lendo seu
//! próprio agregado. A tradução para o vocabulário do envelope cuida em especial dos estados
//! epistêmicos, que o domínio expressa em dois dialetos (`chosen`/`none` no teto,
//! `verdict`/`no_record` nas demais) e a fachada publica num só.

use super::envelope::{DataState, Listing, Period, Reading, ToolError, ToolOutput, ToolResult};
use super::{Args, insert};
use crate::commands::{
    RESERVE_MIN_MONTHS, SAVINGS_CEILING_BPS, SAVINGS_FLOOR_BPS, get_ceiling_proposal_inner,
    get_daily_budget_inner, last_sync_at_query, pockets,
};
use crate::reading::compose::ForecastReading;
use crate::reading::{compose::compose, load::load_inputs};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;

/// A única fronteira de carga das três primeiras ferramentas: carrega e compõe uma vez, para que
/// nenhuma inclusão opcional dispare uma segunda projeção do horizonte.
async fn read(pool: &SqlitePool, today: NaiveDate) -> Result<ForecastReading, String> {
    let inputs = load_inputs(pool, today).await?;
    Ok(compose(&inputs))
}

/// Dias sem lançamento a partir dos quais o dado do Diário deixa de descrever o presente.
const STALE_ENTRY_DAYS: i64 = 7;

// --- Retrato de agora -------------------------------------------------------------------

#[derive(Serialize)]
struct ReserveDto {
    state: DataState,
    /// Décimos de mês, TRUNCADOS — a mesma truncagem da tela, para que o número da conversa
    /// nunca discorde do número que a pessoa está vendo.
    months_tenths: Option<i64>,
    months_display: Option<String>,
    balance_cents: i64,
    /// Meses completos que sustentam o divisor (o que separa veredito de retrato vivo).
    basis_months: i64,
    target_months: i64,
    trend: String,
}

#[derive(Serialize)]
struct CardGateDto {
    verdict: String,
    economy: String,
    /// Percentual bruto por trás da perna de economia. Nulo quando não há renda anual para
    /// dividir — a fachada não fabrica número para preencher campo.
    economy_bps: Option<i64>,
    reserve: String,
}

#[derive(Serialize)]
struct NextInvoiceDto {
    due_date: String,
    amount_cents: i64,
}

#[derive(Serialize)]
struct InvoiceDto {
    card_name: String,
    owner_name: String,
    due_date: String,
    amount_cents: i64,
    status: String,
    has_refund_expectation: bool,
}

#[derive(Serialize)]
struct GuardrailDto {
    safe_to_spend_today_cents: i64,
    /// Qual régua está limitando: `cash` ou `savings`.
    binding: String,
    cash_headroom_cents: i64,
    /// Nulo quando a régua de poupança está inativa (mês sem renda) — só o caixa decide.
    savings_headroom_cents: Option<i64>,
}

pub(crate) async fn financial_snapshot(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let reading = read(pool, today).await.map_err(ToolError::read_failed)?;
    let reserve = &reading.reserve;

    let months_tenths = (reserve.state != "no_record").then(|| tenths(reserve.months));
    let next_fatura = reading.cards.next_fatura;
    let mut data = json!({
        "spending_mode": reading.spending_mode.mode.as_str(),
        "projected_month_end_balance_cents": reading.projected_month_end_cents,
        "daily_ceiling_cents": ceiling_reading(reading.ceiling.source.as_str(), reading.ceiling.per_day_cents),
        "ceiling_proposal_pending": reading.ceiling.proposal_pending,
        "daily_spend_today_cents": reading.today_spend.daily_avg_cents,
        "card_spend_today_cents": reading.today_spend.card_cents,
        "cartao_month_cents": reading.spending_mode.cartao_month_cents,
        "next_invoice": next_fatura.as_ref().map(|(due_date, amount_cents)| NextInvoiceDto {
            due_date: due_date.format("%Y-%m-%d").to_string(),
            amount_cents: *amount_cents,
        }),
        "reserve": ReserveDto {
            state: state_of(reserve.state),
            months_tenths,
            months_display: months_tenths.map(tenths_display),
            balance_cents: reserve.balance_cents,
            basis_months: reserve.basis_months,
            target_months: RESERVE_MIN_MONTHS,
            trend: reserve.trend.clone(),
        },
        "card_gate": CardGateDto {
            verdict: reading.cards.gate.as_str().to_string(),
            economy: reading.cards.gate_economy.as_str().to_string(),
            economy_bps: reading.cards.gate_economy_bps,
            reserve: reading.cards.gate_reserve.as_str().to_string(),
        },
        "realized_transactions": reading.ledger.transaction_count,
        "last_real_transaction_date": reading.ledger.last_real_tx_date,
    });

    if args.wants("upcoming_invoices") {
        let invoices: Vec<InvoiceDto> = reading
            .cards
            .upcoming_invoices
            .into_iter()
            .map(|i| InvoiceDto {
                card_name: i.card_name,
                owner_name: i.owner_name,
                due_date: i.due_date.format("%Y-%m-%d").to_string(),
                amount_cents: i.amount_cents,
                status: i.status.as_str().to_string(),
                has_refund_expectation: i.has_refund_expectation,
            })
            .collect();
        insert(&mut data, "upcoming_invoices", Listing::capped(invoices));
    }

    if args.wants("guardrail") {
        insert(
            &mut data,
            "guardrail",
            GuardrailDto {
                safe_to_spend_today_cents: reading.safe_to_spend.amount_cents,
                binding: reading.safe_to_spend.binding.as_str().to_string(),
                cash_headroom_cents: reading.safe_to_spend.cash_headroom_cents,
                savings_headroom_cents: reading.safe_to_spend.savings_headroom_cents,
            },
        );
    }

    Ok(ToolOutput {
        period: month_period(today),
        data,
    })
}

// --- O que falta de dado ----------------------------------------------------------------

#[derive(Serialize)]
struct Gap {
    code: &'static str,
    what: String,
    fix: &'static str,
}

pub(crate) async fn data_status(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let iso = today.format("%Y-%m-%d").to_string();
    // Realizado é decidido pela DATA (≤ hoje), não pelo `is_projection` congelado — que fica
    // obsoleto quando a pessoa passa dias sem reimportar.
    type CountsRow = (i64, i64, Option<String>, Option<String>);
    let counts: CountsRow = sqlx::query_as(
        "SELECT COUNT(*), \
                COALESCE(SUM(CASE WHEN date <= ?1 THEN 1 ELSE 0 END), 0), \
                MIN(date), \
                MAX(CASE WHEN is_projection = 0 AND date <= ?1 THEN date END) \
         FROM \"transaction\" WHERE scenario_id IS NULL",
    )
    .bind(&iso)
    .fetch_one(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("contagem de lançamentos: {e}")))?;
    let (total, realized, first_date, last_real_date) = counts;

    type PendingRow = (i64, i64, i64);
    let pending: PendingRow = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM ceiling_proposal WHERE status = 'pending'), \
                (SELECT COUNT(*) FROM card_proposal WHERE status = 'pending'), \
                (SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL)",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("pendências: {e}")))?;
    let (ceiling_proposals, card_proposals, import_conflicts) = pending;

    let reading = read(pool, today).await.map_err(ToolError::read_failed)?;
    let last_sync_at = last_sync_at_query(pool)
        .await
        .map_err(ToolError::read_failed)?;

    let days_since_last_entry = last_real_date.as_deref().and_then(|d| {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|date| (today - date).num_days())
    });
    let card_mode = matches!(
        reading.spending_mode.mode,
        crate::forecast::SpendingMode::Card
    );

    let mut gaps = Vec::new();
    if total == 0 {
        gaps.push(Gap {
            code: "no_transactions",
            what: "Não há nenhum lançamento no app.".into(),
            fix: "Importe a planilha em Configurações ou registre o primeiro lançamento.",
        });
    }
    if reading.ceiling.source == crate::reading::inputs::CeilingSource::None {
        gaps.push(Gap {
            code: "daily_ceiling_missing",
            what: "O teto do Diário não está estipulado, e sem ele o dia não tem contra o que ser \
                   comparado."
                .into(),
            fix: "Faça a cerimônia do teto na tela Teto do diário.",
        });
    }
    if reading.reserve.state == "no_record" {
        gaps.push(Gap {
            code: "reserve_unmapped",
            what: if reading.reserve.baseline_cents <= 0 {
                "Não há meses completos suficientes para calcular o custo de vida que divide a \
                 reserva."
                    .into()
            } else {
                "Nenhuma conta está marcada como reserva, então a cobertura em meses não pode ser \
                 calculada."
                    .into()
            },
            fix: "Marque a conta da reserva em Configurações › Bolsos.",
        });
    }
    if reading.annual.economia_state == "no_record" {
        gaps.push(Gap {
            code: "economia_unregistered",
            what: "Não há Economia registrada no ano, então o Economizado% não tem numerador."
                .into(),
            fix: "Registre as transferências para a reserva — ou confira se elas chegaram na \
                  importação.",
        });
    }
    // Silêncio do Diário só é lacuna no modo débito. No modo cartão o gasto vive nas faturas e
    // um Diário parado é o modo funcionando, não descuido — cobrar aqui seria punir a escolha.
    if !card_mode
        && let Some(days) = days_since_last_entry
        && days > STALE_ENTRY_DAYS
    {
        gaps.push(Gap {
            code: "entries_stale",
            what: format!("O último lançamento é de {days} dias atrás."),
            fix: "Reimporte a planilha ou registre o que ficou para trás.",
        });
    }
    if import_conflicts > 0 {
        gaps.push(Gap {
            code: "import_conflicts_open",
            what: format!(
                "{import_conflicts} conflito(s) de importação esperam decisão e bloqueiam o \
                 write-back."
            ),
            fix: "Resolva os conflitos na fila de importação.",
        });
    }

    let mut data = json!({
        "has_data": total > 0,
        "transactions_total": total,
        "realized_transactions": realized,
        "first_transaction_date": first_date,
        "last_real_transaction_date": last_real_date,
        "days_since_last_entry": days_since_last_entry,
        "last_sync_at": last_sync_at,
        "spending_mode": if card_mode { "card" } else { "debit" },
        "readings": {
            "daily_ceiling": state_of(reading.ceiling.source.as_str()),
            "reserve": state_of(reading.reserve.state),
            "annual_economia": state_of(reading.annual.economia_state),
        },
        "pending": {
            "ceiling_proposals": ceiling_proposals,
            "card_proposals": card_proposals,
            "import_conflicts": import_conflicts,
        },
        "gaps": gaps,
    });

    if args.wants("future_coverage") {
        insert(
            &mut data,
            "future_coverage",
            json!({
                "months": month_coverage_listing(reading.coverage.months.iter()),
                "baseline_outflow_cents": reading.coverage.baseline_outflow_cents,
                "total_missing_cents": reading.coverage.total_missing_cents,
                "trusted_through_month": reading.coverage.trusted_through_month,
            }),
        );
    }

    Ok(ToolOutput {
        period: month_period(today),
        data,
    })
}

// --- O teto e as metas do método --------------------------------------------------------

#[derive(Serialize)]
struct CeremonyItemDto {
    name: String,
    amount_cents: i64,
}

#[derive(Serialize)]
struct PendingProposalDto {
    id: String,
    per_day_cents: i64,
    divisor_days: i64,
    source_month: String,
}

pub(crate) async fn budget_settings(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let budget = get_daily_budget_inner(pool)
        .await
        .map_err(ToolError::read_failed)?;
    let reading = read(pool, today).await.map_err(ToolError::read_failed)?;
    let proposal = get_ceiling_proposal_inner(pool)
        .await
        .map_err(ToolError::read_failed)?;

    let monthly_total: i64 = budget.categories.iter().map(|c| c.amount_cents).sum();
    let mut data = json!({
        "daily_ceiling_cents": ceiling_reading(reading.ceiling.source.as_str(), reading.ceiling.per_day_cents),
        "monthly_total_cents": monthly_total,
        "divisor_days": budget.divisor_days,
        "ceremony_month": budget.ceremony_month,
        "pending_proposal": proposal.as_ref().map(|p| PendingProposalDto {
            id: p.id.clone(),
            per_day_cents: p.per_day_cents,
            divisor_days: p.divisor_days,
            source_month: p.source_month.clone(),
        }),
        // A faixa é ANUAL: o método julga a média do ano, nunca um mês isolado.
        "method_targets": {
            "economia_floor_bps": SAVINGS_FLOOR_BPS,
            "economia_ceiling_bps": SAVINGS_CEILING_BPS,
            "reserve_months": RESERVE_MIN_MONTHS,
        },
    });

    if args.wants("ceremony") {
        let items: Vec<CeremonyItemDto> = budget
            .categories
            .iter()
            .map(|c| CeremonyItemDto {
                name: c.name.clone(),
                amount_cents: c.amount_cents,
            })
            .collect();
        let proposal_items: Vec<CeremonyItemDto> = proposal
            .iter()
            .flat_map(|p| p.items.iter())
            .map(|i| CeremonyItemDto {
                name: i.name.clone(),
                amount_cents: i.amount_cents,
            })
            .collect();
        insert(
            &mut data,
            "ceremony",
            json!({
                "items": Listing::capped(items),
                // Nota crua da célula que documenta a cerimônia. Reproduzida, nunca parafraseada.
                "source_note": budget.source_note,
                "proposal_items": Listing::capped(proposal_items),
                "proposal_note": proposal.and_then(|p| p.raw_note),
            }),
        );
    }

    Ok(ToolOutput {
        period: month_period(today),
        data,
    })
}

// --- Onde está o dinheiro ---------------------------------------------------------------

#[derive(Serialize)]
struct AccountDto {
    id: String,
    name: String,
    r#type: String,
    liquidity: Option<String>,
    institution: Option<String>,
    balance_cents: i64,
}

pub(crate) async fn accounts_and_net_worth(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let pockets = pockets(pool).await.map_err(ToolError::read_failed)?;

    let mut data = json!({
        "net_worth_cents": pockets.net_worth_cents,
        "liquid_cents": pockets.liquid_cents,
        "reserve_cents": pockets.reserve_cents,
        // Vale-refeição é rastreado à parte: é saldo, mas não é dinheiro que serve para tudo.
        "restricted_cents": pockets.restricted_cents,
        "illiquid_cents": pockets.illiquid_cents,
        "accounts_total": pockets.accounts.len(),
    });

    if args.wants("accounts") {
        let accounts: Vec<AccountDto> = pockets
            .accounts
            .into_iter()
            .map(|a| AccountDto {
                id: a.id,
                name: a.name,
                r#type: a.r#type,
                liquidity: a.liquidity,
                institution: a.institution,
                balance_cents: a.balance,
            })
            .collect();
        insert(&mut data, "accounts", Listing::capped(accounts));
    }

    Ok(ToolOutput {
        // Saldo é fotografia do dia, não do mês.
        period: Period::day(today),
        data,
    })
}

// --- Costura ----------------------------------------------------------------------------

#[derive(Serialize)]
struct CoverageDto {
    month: String,
    coverage_bps: i64,
    is_complete: bool,
    estimated_missing_cents: i64,
}

/// Cobertura de um mês futuro no vocabulário do envelope, recortada direto do motor — a leitura
/// é uma só: a projeção do mês é crível ou não é.
fn month_coverage_listing<'a>(
    rows: impl Iterator<Item = &'a crate::forecast::MonthCoverage>,
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

fn month_period(today: NaiveDate) -> Period {
    let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("dia 1 existe");
    let end = (start + chrono::Months::new(1))
        .pred_opt()
        .expect("mês seguinte tem véspera");
    Period::between(start, end)
}

/// Traduz o vocabulário do domínio para o do envelope. O teto fala `chosen`/`none`; as demais
/// réguas já falam os quatro estados canônicos.
fn state_of(domain_state: &str) -> DataState {
    match domain_state {
        "verdict" | "chosen" => DataState::Verdict,
        "estimate" => DataState::Estimate,
        "zero" => DataState::Zero,
        _ => DataState::NoRecord,
    }
}

/// O teto exibido com estado. Sem registro, o valor é NULO — nunca o zero que a tela desenha
/// como travessão e o modelo leria como "teto de R$ 0,00".
fn ceiling_reading(source: &str, per_day_cents: i64) -> Reading<i64> {
    let state = state_of(source);
    if state == DataState::NoRecord {
        Reading::no_record()
    } else {
        Reading::new(state, per_day_cents)
    }
}

/// Décimos de mês, truncados (nunca arredondados) — a mesma conta da tela.
fn tenths(months: f64) -> i64 {
    (months * 10.0).trunc() as i64
}

/// Décimos como a tela os escreve: uma casa, vírgula decimal, sem casa quando é inteiro.
fn tenths_display(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    let abs = tenths.abs();
    if abs % 10 == 0 {
        format!("{sign}{}", abs / 10)
    } else {
        format!("{sign}{},{}", abs / 10, abs % 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mia::bench::fixtures;
    use crate::mia::{Context, ToolCall, dispatch, method_tools};
    use crate::reading::load::LOAD_INPUTS_CALLS;
    use std::cell::Cell;

    async fn pool() -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        fixtures::seed(&p, "casa_basica").await.unwrap();
        p
    }

    async fn call(
        pool: &SqlitePool,
        name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        let ctx = Context {
            clock: fixtures::bench_clock(),
            pack: method_tools::MethodPack::at(std::env::temp_dir()),
            conversation_id: None,
        };
        let env = dispatch(pool, &ToolCall::new(name, arguments), &ctx).await;
        assert!(
            env.ok,
            "a fachada recusou {name}: {:?}",
            env.error.map(|e| e.message)
        );
        env.data.expect("envelope de sucesso carrega dados")
    }

    // --- Décimos truncados (a mesma conta da tela) ---

    #[test]
    fn tenths_truncates_never_rounds() {
        assert_eq!(
            tenths(6.19),
            61,
            "6,19 meses trunca em 6,1, não arredonda para 6,2"
        );
        assert_eq!(
            tenths(-1.05),
            -10,
            "negativo também trunca em direção ao zero"
        );
        assert_eq!(tenths(3.0), 30);
    }

    #[test]
    fn tenths_display_omits_the_decimal_place_on_whole_numbers() {
        assert_eq!(tenths_display(61), "6,1");
        assert_eq!(tenths_display(30), "3");
        assert_eq!(tenths_display(-10), "-1");
    }

    // --- Tradução do envelope epistêmico ---

    #[test]
    fn state_of_translates_both_domain_dialects_to_the_same_four_states() {
        assert_eq!(state_of("chosen"), DataState::Verdict);
        assert_eq!(state_of("verdict"), DataState::Verdict);
        assert_eq!(state_of("estimate"), DataState::Estimate);
        assert_eq!(state_of("zero"), DataState::Zero);
        assert_eq!(state_of("none"), DataState::NoRecord);
        assert_eq!(state_of("no_record"), DataState::NoRecord);
    }

    #[test]
    fn ceiling_without_record_is_null_not_a_fabricated_zero() {
        let reading = ceiling_reading("none", 0);
        assert_eq!(reading.state, DataState::NoRecord);
        assert!(reading.value.is_none());
    }

    #[test]
    fn ceiling_with_record_carries_its_value() {
        let reading = ceiling_reading("chosen", 15_000);
        assert_eq!(reading.state, DataState::Verdict);
        assert_eq!(reading.value, Some(15_000));
    }

    // --- Uma composição por chamada, mesmo com várias inclusões opcionais ---

    #[tokio::test]
    async fn financial_snapshot_with_every_inclusion_composes_the_reading_once() {
        LOAD_INPUTS_CALLS
            .scope(Cell::new(0), async {
                let p = pool().await;
                call(
                    &p,
                    "get_financial_snapshot",
                    json!({"include": ["upcoming_invoices", "guardrail"]}),
                )
                .await;

                assert_eq!(
                    LOAD_INPUTS_CALLS.with(Cell::get),
                    1,
                    "duas inclusões opcionais não podem disparar uma segunda projeção do horizonte"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn data_status_with_future_coverage_composes_the_reading_once() {
        LOAD_INPUTS_CALLS
            .scope(Cell::new(0), async {
                let p = pool().await;
                call(
                    &p,
                    "get_data_status",
                    json!({"include": ["future_coverage"]}),
                )
                .await;

                assert_eq!(LOAD_INPUTS_CALLS.with(Cell::get), 1);
            })
            .await;
    }

    // --- Recorte certo: o campo que a fachada publica É o campo da leitura ---

    #[tokio::test]
    async fn financial_snapshot_cites_the_same_reading_the_screen_composes() {
        let p = pool().await;
        let today = fixtures::bench_clock().today();
        let inputs = load_inputs(&p, today).await.unwrap();
        let reading = compose(&inputs);

        let snapshot = call(
            &p,
            "get_financial_snapshot",
            json!({"include": ["guardrail", "upcoming_invoices"]}),
        )
        .await;

        assert_eq!(
            snapshot["projected_month_end_balance_cents"],
            reading.projected_month_end_cents
        );
        assert_eq!(
            snapshot["guardrail"]["safe_to_spend_today_cents"],
            reading.safe_to_spend.amount_cents
        );
        let months_tenths =
            (reading.reserve.state != "no_record").then(|| tenths(reading.reserve.months));
        assert_eq!(snapshot["reserve"]["months_tenths"], json!(months_tenths));
        assert_eq!(
            snapshot["upcoming_invoices"]["items"]
                .as_array()
                .unwrap()
                .len(),
            reading.cards.upcoming_invoices.len()
        );
    }

    #[tokio::test]
    async fn data_status_readings_cite_the_same_ceiling_reserve_and_economia_states() {
        let p = pool().await;
        let today = fixtures::bench_clock().today();
        let inputs = load_inputs(&p, today).await.unwrap();
        let reading = compose(&inputs);

        let status = call(&p, "get_data_status", json!({})).await;

        assert_eq!(
            status["readings"]["daily_ceiling"],
            json!(state_of(reading.ceiling.source.as_str()))
        );
        assert_eq!(
            status["readings"]["reserve"],
            json!(state_of(reading.reserve.state))
        );
        assert_eq!(
            status["readings"]["annual_economia"],
            json!(state_of(reading.annual.economia_state))
        );
    }
}
