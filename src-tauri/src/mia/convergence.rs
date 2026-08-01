//! Eval de convergência do piso offline.
//!
//! Sem chave e sem consentimento, seis contas locais respondem na tela a partir dos DTOs de
//! `get_dashboard_summary` e `get_forecast`. Com o runtime ligado, as mesmas perguntas são
//! respondidas pelas ferramentas da fachada. As duas superfícies precisam chegar ao MESMO
//! número: o veredito da pessoa não pode depender de ela estar conectada.
//!
//! Cada teste amarra uma das seis perguntas: lê o número pelo caminho do piso local (o DTO do
//! comando) e o número pelo caminho do runtime (o envelope da ferramenta), e exige igualdade em
//! centavos inteiros — nunca em float, que converge por arredondamento e esconde divergência.

use super::envelope::Envelope;
use super::{Context, ToolCall, dispatch, method_tools};
use crate::commands::{DashboardSummary, ForecastDto, dashboard_summary, forecast_dto};
use crate::mia::bench::fixtures;
use chrono::Datelike;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

/// Pool de UMA conexão, como o de produção: pool default esconde deadlock de transação.
async fn pool() -> SqlitePool {
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    fixtures::seed(&p, "casa_basica").await.unwrap();
    p
}

/// As duas leituras que alimentam o piso offline — os mesmos DTOs que a tela recebe.
async fn floor(pool: &SqlitePool) -> (DashboardSummary, ForecastDto) {
    let today = fixtures::bench_clock().today();
    let summary = dashboard_summary(pool, today).await.unwrap();
    let forecast = forecast_dto(pool, today).await.unwrap();
    (summary, forecast)
}

/// Chama a fachada no relógio da bancada e devolve os dados do envelope de sucesso.
async fn tool(pool: &SqlitePool, name: &str, arguments: Value) -> Value {
    let ctx = Context {
        clock: fixtures::bench_clock(),
        pack: method_tools::MethodPack::at(std::env::temp_dir()),
        conversation_id: None,
    };
    let env: Envelope = dispatch(pool, &ToolCall::new(name, arguments), &ctx).await;
    assert!(
        env.ok,
        "a fachada recusou {name}: {:?}",
        env.error.map(|e| e.message)
    );
    env.data.expect("envelope de sucesso carrega dados")
}

/// Centavos de um campo do envelope, com a falha nomeando o caminho lido.
fn cents(data: &Value, path: &[&str]) -> i64 {
    let mut node = data;
    for key in path {
        node = node
            .get(key)
            .unwrap_or_else(|| panic!("a fachada não publica {}", path.join(".")));
    }
    node.as_i64()
        .unwrap_or_else(|| panic!("{} não é inteiro: {node}", path.join(".")))
}

#[tokio::test]
async fn quanto_posso_gastar_hoje_converge() {
    let p = pool().await;
    let (summary, forecast) = floor(&p).await;
    let snapshot = tool(
        &p,
        "get_financial_snapshot",
        json!({"include": ["guardrail"]}),
    )
    .await;

    assert_eq!(
        forecast.safe_to_spend_today_cents,
        cents(&snapshot, &["guardrail", "safe_to_spend_today_cents"]),
        "pode gastar hoje: piso offline (get_forecast) × fachada (get_financial_snapshot.guardrail)"
    );
    assert_eq!(
        forecast.cash_headroom_cents,
        cents(&snapshot, &["guardrail", "cash_headroom_cents"]),
        "limite do caixa: piso offline × fachada"
    );
    assert_eq!(
        forecast.savings_headroom_cents,
        snapshot["guardrail"]["savings_headroom_cents"].as_i64(),
        "limite da economia: piso offline × fachada"
    );
    assert_eq!(
        forecast.binding_guardrail,
        snapshot["guardrail"]["binding"].as_str().unwrap(),
        "régua que limita o dia: piso offline × fachada"
    );
    // O teto do diário é o segundo limite que a resposta imprime na nota — ele não entra no
    // guardrail, mas diverge com a mesma gravidade se as duas superfícies o lerem diferente.
    assert_eq!(
        summary.daily_budget,
        cents(&snapshot, &["daily_ceiling_cents", "value"]),
        "teto do diário: piso offline (get_dashboard_summary) × fachada"
    );
    assert_eq!(
        summary.daily_spend_today,
        cents(&snapshot, &["daily_spend_today_cents"]),
        "gasto do dia: piso offline × fachada"
    );
}

#[tokio::test]
async fn como_o_mes_esta_indo_converge() {
    let p = pool().await;
    let (summary, forecast) = floor(&p).await;
    let today = fixtures::bench_clock().today();
    let month = forecast
        .months
        .iter()
        .find(|m| (m.year, m.month) == (today.year(), today.month()))
        .expect("o mês corrente está na projeção");
    let analysis = tool(&p, "get_month_analysis", json!({})).await;

    assert_eq!(
        month.performance_cents,
        cents(&analysis, &["performance_cents"]),
        "performance do mês: piso offline (get_forecast.months) × fachada (get_month_analysis)"
    );
    assert_eq!(
        month.cost_of_living_cents,
        cents(&analysis, &["cost_of_living_cents"]),
        "custo de vida do mês: piso offline × fachada"
    );
    assert_eq!(
        month.income_performance_cents,
        cents(&analysis, &["income_cents"]),
        "entradas do mês: piso offline × fachada"
    );
    assert_eq!(
        summary.balance,
        cents(
            &tool(&p, "get_financial_snapshot", json!({})).await,
            &["projected_month_end_balance_cents"]
        ),
        "saldo previsto para o fim do mês: piso offline × fachada"
    );
}

/// A régua anual que a resposta imprime: o mesmo Economizado% pelas duas superfícies, com os
/// dois operandos do recibo batendo um a um — e o recibo FECHANDO, porque uma conta impressa
/// cujos operandos não produzem o resultado é uma fórmula mentindo em prosa.
#[tokio::test]
async fn como_esta_a_economia_do_ano_converge() {
    let p = pool().await;
    let (_, forecast) = floor(&p).await;
    let year = tool(&p, "get_year_analysis", json!({})).await;
    let annual = &forecast.annual_savings;

    assert_eq!(
        annual.economia_ruler_rate_bps,
        cents(&year, &["economizado", "bps"]),
        "Economizado% do ano: piso offline (get_forecast.annual_savings) × fachada (get_year_analysis)"
    );
    assert_eq!(
        annual.economia_ruler_cents,
        cents(&year, &["economia_lived_cents"]),
        "economia da régua: piso offline × fachada"
    );
    assert_eq!(
        annual.realized_income_cents,
        cents(&year, &["income_lived_cents"]),
        "entradas do ano até aqui: piso offline × fachada"
    );
    // O recibo da conversa imprime economia ÷ entradas = percentual. Truncado, como a régua e
    // como a exibição.
    assert_eq!(
        annual.economia_ruler_rate_bps,
        annual.economia_ruler_cents * 10_000 / annual.realized_income_cents,
        "o recibo do Economizado% não fecha: os operandos impressos não produzem o resultado impresso"
    );
    assert_eq!(
        annual.realized_savings_cents,
        cents(&year, &["surplus_lived_cents"]),
        "colchão do ano: piso offline × fachada"
    );
}

#[tokio::test]
async fn como_esta_a_reserva_converge() {
    let p = pool().await;
    let (summary, _) = floor(&p).await;
    let snapshot = tool(&p, "get_financial_snapshot", json!({})).await;

    // A resposta local trunca os meses em décimos para exibir; a fachada publica os décimos já
    // truncados. Comparar no inteiro de décimos é o que garante que a frase e a ferramenta
    // digam o mesmo número — comparar em float compararia precisões diferentes.
    let floor_tenths = (summary.reserve_months * 10.0).trunc() as i64;
    assert_eq!(
        floor_tenths,
        cents(&snapshot, &["reserve", "months_tenths"]),
        "meses de reserva: piso offline (get_dashboard_summary) × fachada (get_financial_snapshot)"
    );
    assert_eq!(
        summary.reserve_state,
        snapshot["reserve"]["state"].as_str().unwrap(),
        "estado epistêmico da reserva: piso offline × fachada"
    );
}

#[tokio::test]
async fn tem_buraco_na_estrada_converge() {
    let p = pool().await;
    let (_, forecast) = floor(&p).await;
    let projection = tool(&p, "get_forecast", json!({})).await;
    let low = forecast
        .deepest_deficit
        .as_ref()
        .expect("a fixture tem projeção suficiente para o ponto mais baixo");

    assert_eq!(
        low.balance_cents,
        cents(&projection, &["horizon_lowest_balance", "balance_cents"]),
        "ponto mais baixo da estrada: piso offline (get_forecast DTO) × fachada (get_forecast)"
    );
    assert_eq!(
        low.date,
        projection["horizon_lowest_balance"]["date"]
            .as_str()
            .unwrap(),
        "dia do ponto mais baixo: piso offline × fachada"
    );
    assert_eq!(
        forecast.horizon_end,
        projection["horizon_end"].as_str().unwrap(),
        "fim da estrada: piso offline × fachada"
    );
}

#[tokio::test]
async fn quando_vence_a_proxima_fatura_converge() {
    let p = pool().await;
    let (summary, _) = floor(&p).await;
    let snapshot = tool(
        &p,
        "get_financial_snapshot",
        json!({"include": ["upcoming_invoices"]}),
    )
    .await;

    // O piso local agrupa as faturas em aberto pelo vencimento e responde pelo primeiro grupo:
    // é o total DAQUELE vencimento que a frase promete, não o total em aberto.
    let mut open: Vec<_> = summary
        .upcoming_invoices
        .iter()
        .filter(|i| i.status == "aberta" || i.status == "fechada")
        .collect();
    open.sort_by(|a, b| a.due_date.cmp(&b.due_date));
    let next_due = open
        .first()
        .expect("a fixture tem fatura em aberto")
        .due_date
        .clone();
    let floor_total: i64 = open
        .iter()
        .filter(|i| i.due_date == next_due)
        .map(|i| i.amount_cents)
        .sum();

    let rows = snapshot["upcoming_invoices"]["items"]
        .as_array()
        .expect("a fachada lista as faturas");
    let mut facade: Vec<(&str, i64)> = rows
        .iter()
        .filter(|r| {
            let status = r["status"].as_str().unwrap_or_default();
            status == "aberta" || status == "fechada"
        })
        .map(|r| {
            (
                r["due_date"].as_str().unwrap(),
                r["amount_cents"].as_i64().unwrap(),
            )
        })
        .collect();
    facade.sort_by(|a, b| a.0.cmp(b.0));
    let facade_due = facade.first().expect("a fachada tem fatura em aberto").0;
    let facade_total: i64 = facade
        .iter()
        .filter(|(due, _)| *due == facade_due)
        .map(|(_, cents)| *cents)
        .sum();

    assert_eq!(
        next_due, facade_due,
        "próximo vencimento: piso offline (get_dashboard_summary) × fachada (get_financial_snapshot)"
    );
    assert_eq!(
        floor_total, facade_total,
        "total do vencimento: piso offline × fachada"
    );
    assert_eq!(
        summary.next_fatura_date.as_deref(),
        snapshot["next_invoice"]["due_date"].as_str(),
        "vencimento anunciado no retrato: piso offline × fachada"
    );
}
