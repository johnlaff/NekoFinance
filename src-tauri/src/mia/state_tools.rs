//! As quatro perguntas de estado: como estou agora, o que falta de dado, qual meu teto, onde
//! está meu dinheiro.
//!
//! Nenhuma régua nasce aqui. Cada ferramenta chama o helper que a tela correspondente já chama
//! e traduz o resultado para o vocabulário do envelope — em especial os estados epistêmicos,
//! que o domínio expressa em dois dialetos (`chosen`/`none` no teto, `verdict`/`no_record` nas
//! demais) e a fachada publica num só.

use super::Args;
use super::envelope::{DataState, Listing, Period, Reading, ToolError, ToolOutput, ToolResult};
use crate::commands::{
    CeilingSource, RESERVE_MIN_MONTHS, SAVINGS_FLOOR_BPS, SAVINGS_TARGET_BPS,
    daily_ceiling_reading, dashboard_summary, economia_ruler_reading, forecast_dto,
    get_ceiling_proposal_inner, get_daily_budget_inner, last_sync_at_query, pockets,
    reserve_reading, spending_mode_summary,
};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// Teto da faixa de economia do método (20–30% ao ano). O piso e o centro já vivem no motor
/// (guardrail e gate do cartão decidem por eles); o teto é didático — nunca julga.
const SAVINGS_BAND_CEILING_BPS: i64 = 3_000;

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
    let summary = dashboard_summary(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let reserve = reserve_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;

    let months_tenths = (reserve.state != "no_record").then(|| tenths(reserve.months));
    let mut data = json!({
        "spending_mode": summary.spending_mode,
        "projected_month_end_balance_cents": summary.balance,
        "daily_ceiling_cents": ceiling_reading(summary.daily_ceiling_source.as_str(), summary.daily_budget),
        "ceiling_proposal_pending": summary.ceiling_proposal_pending,
        "daily_spend_today_cents": summary.daily_spend_today,
        "card_spend_today_cents": summary.card_spend_today_cents,
        "cartao_month_cents": summary.cartao_month_cents,
        "next_invoice": summary.next_fatura_date.as_ref().map(|due_date| NextInvoiceDto {
            due_date: due_date.clone(),
            amount_cents: summary.next_fatura_amount_cents,
        }),
        "reserve": ReserveDto {
            state: state_of(reserve.state),
            months_tenths,
            months_display: months_tenths.map(tenths_display),
            balance_cents: reserve.balance_cents,
            basis_months: reserve.basis_months,
            target_months: RESERVE_MIN_MONTHS,
            trend: reserve.trend,
        },
        "card_gate": CardGateDto {
            verdict: summary.card_gate,
            economy: summary.card_gate_economy,
            economy_bps: summary.card_gate_economy_bps,
            reserve: summary.card_gate_reserve,
        },
        "realized_transactions": summary.transaction_count,
        "last_real_transaction_date": summary.last_real_tx_date,
    });

    if args.wants("upcoming_invoices") {
        let invoices: Vec<InvoiceDto> = summary
            .upcoming_invoices
            .into_iter()
            .map(|i| InvoiceDto {
                card_name: i.card_name,
                owner_name: i.owner_name,
                due_date: i.due_date,
                amount_cents: i.amount_cents,
                status: i.status,
                has_refund_expectation: i.has_refund_expectation,
            })
            .collect();
        insert(&mut data, "upcoming_invoices", Listing::capped(invoices));
    }

    if args.wants("guardrail") {
        let forecast = forecast_dto(pool, today)
            .await
            .map_err(ToolError::read_failed)?;
        insert(
            &mut data,
            "guardrail",
            GuardrailDto {
                safe_to_spend_today_cents: forecast.safe_to_spend_today_cents,
                binding: forecast.binding_guardrail,
                cash_headroom_cents: forecast.cash_headroom_cents,
                savings_headroom_cents: forecast.savings_headroom_cents,
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

#[derive(Serialize)]
struct CoverageDto {
    month: String,
    coverage_bps: i64,
    is_complete: bool,
    estimated_missing_cents: i64,
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

    let ceiling = daily_ceiling_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let reserve = reserve_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let economia = economia_ruler_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let mode = spending_mode_summary(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let last_sync_at = last_sync_at_query(pool)
        .await
        .map_err(ToolError::read_failed)?;

    let days_since_last_entry = last_real_date.as_deref().and_then(|d| {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|date| (today - date).num_days())
    });
    let card_mode = matches!(mode.mode, crate::forecast::SpendingMode::Card);

    let mut gaps = Vec::new();
    if total == 0 {
        gaps.push(Gap {
            code: "no_transactions",
            what: "Não há nenhum lançamento no app.".into(),
            fix: "Importe a planilha em Configurações ou registre o primeiro lançamento.",
        });
    }
    if ceiling.source == CeilingSource::None {
        gaps.push(Gap {
            code: "daily_ceiling_missing",
            what: "O teto do Diário não está estipulado, e sem ele o dia não tem contra o que ser \
                   comparado."
                .into(),
            fix: "Faça a cerimônia do teto na tela Teto do diário.",
        });
    }
    if reserve.state == "no_record" {
        gaps.push(Gap {
            code: "reserve_unmapped",
            what: if reserve.baseline_cents <= 0 {
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
    if economia.state == "no_record" {
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
            "daily_ceiling": state_of(ceiling.source.as_str()),
            "reserve": state_of(reserve.state),
            "annual_economia": state_of(economia.state),
        },
        "pending": {
            "ceiling_proposals": ceiling_proposals,
            "card_proposals": card_proposals,
            "import_conflicts": import_conflicts,
        },
        "gaps": gaps,
    });

    if args.wants("future_coverage") {
        let forecast = forecast_dto(pool, today)
            .await
            .map_err(ToolError::read_failed)?;
        let coverage: Vec<CoverageDto> = forecast
            .coverage
            .iter()
            .map(|c| CoverageDto {
                month: format!("{:04}-{:02}", c.year, c.month),
                coverage_bps: c.coverage_bps,
                is_complete: c.is_complete,
                estimated_missing_cents: c.estimated_missing_cents,
            })
            .collect();
        insert(
            &mut data,
            "future_coverage",
            json!({
                "months": Listing::capped(coverage),
                "baseline_outflow_cents": forecast.baseline_outflow_cents,
                "total_missing_cents": forecast.total_missing_cents,
                "trusted_through_month": forecast.trusted_through_month,
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
    let ceiling = daily_ceiling_reading(pool, today)
        .await
        .map_err(ToolError::read_failed)?;
    let proposal = get_ceiling_proposal_inner(pool)
        .await
        .map_err(ToolError::read_failed)?;

    let monthly_total: i64 = budget.categories.iter().map(|c| c.amount_cents).sum();
    let mut data = json!({
        "daily_ceiling_cents": ceiling_reading(ceiling.source.as_str(), ceiling.per_day_cents),
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
            "economia_target_bps": SAVINGS_TARGET_BPS,
            "economia_ceiling_bps": SAVINGS_BAND_CEILING_BPS,
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

/// Acrescenta um campo ao objeto de dados. A serialização de um tipo próprio não falha; um
/// `json!` mal formado seria erro de programação, não de dado.
fn insert(data: &mut Value, key: &str, value: impl Serialize) {
    let object = data
        .as_object_mut()
        .expect("dados de ferramenta são sempre um objeto");
    object.insert(
        key.to_string(),
        serde_json::to_value(value).expect("dados de ferramenta são serializáveis"),
    );
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
