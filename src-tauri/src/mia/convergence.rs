//! Eval de convergência do piso offline.
//!
//! Sem chave e sem consentimento, seis contas locais respondem na tela a partir dos DTOs de
//! `get_dashboard_summary` e `get_forecast`. Com o runtime ligado, as mesmas perguntas são
//! respondidas pelas ferramentas da fachada. As duas superfícies precisam chegar ao MESMO
//! número: o veredito da pessoa não pode depender de ela estar conectada.
//!
//! Cada teste amarra uma das perguntas ainda respondidas por DOIS caminhos de composição
//! distintos: lê o número pelo caminho do piso local (o DTO do comando) e o número pelo caminho
//! do runtime (o envelope da ferramenta), e exige igualdade em centavos inteiros — nunca em
//! float, que converge por arredondamento e esconde divergência.
//!
//! `get_financial_snapshot` e `get_data_status` não entram mais aqui: as duas recortam a mesma
//! `ForecastReading` que os DTOs de tela leem (uma composição por chamada, ver
//! `mia::state_tools`), então uma comparação piso×fachada para os campos delas compararia o
//! mesmo campo consigo mesmo — a garantia de origem única virou propriedade estrutural do
//! código, não mais algo que um teste de runtime precisa provar. Os testes de recorte, de
//! tradução do envelope epistêmico e de truncagem dessas duas ferramentas vivem em
//! `mia::state_tools::tests`.

use super::envelope::Envelope;
use super::{Context, ToolCall, dispatch, method_tools};
use crate::commands::{ForecastDto, forecast_dto};
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

/// A leitura que alimenta o piso offline — o mesmo DTO que a tela recebe.
async fn floor(pool: &SqlitePool) -> ForecastDto {
    let today = fixtures::bench_clock().today();
    forecast_dto(pool, today).await.unwrap()
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
async fn como_o_mes_esta_indo_converge() {
    let p = pool().await;
    let forecast = floor(&p).await;
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
}

/// A régua anual que a resposta imprime: o mesmo Economizado% pelas duas superfícies, com os
/// dois operandos do recibo batendo um a um — e o recibo FECHANDO, porque uma conta impressa
/// cujos operandos não produzem o resultado é uma fórmula mentindo em prosa.
#[tokio::test]
async fn como_esta_a_economia_do_ano_converge() {
    let p = pool().await;
    let forecast = floor(&p).await;
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
async fn tem_buraco_na_estrada_converge() {
    let p = pool().await;
    let forecast = floor(&p).await;
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
