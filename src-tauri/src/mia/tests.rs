//! Suíte da fachada: exercita a PORTA contra pool real com fixtures, nunca a forma interna de
//! cada ferramenta. O que está sob teste é o envelope que sai no fio.

use super::*;
use chrono::DateTime;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;

/// Pool de UMA conexão, como o de produção: pool default esconde deadlock de transação.
async fn pool() -> SqlitePool {
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    p
}

/// 25/07/2026, 09:00 no horário de Brasília — o mundo de todos os testes.
fn clock() -> Clock {
    Clock::at(DateTime::parse_from_rfc3339("2026-07-25T09:00:00-03:00").unwrap())
}

async fn call(pool: &SqlitePool, name: &str, arguments: Value) -> Envelope {
    dispatch(pool, &ToolCall::new(name, arguments), clock()).await
}

/// Envelope de sucesso, com os dados. Falha o teste se a porta recusou.
async fn data(pool: &SqlitePool, name: &str, arguments: Value) -> Value {
    let env = call(pool, name, arguments).await;
    assert!(env.ok, "esperava sucesso, veio {:?}", env.error);
    env.data.expect("envelope de sucesso carrega dados")
}

async fn person(pool: &SqlitePool) -> String {
    let id = "p-eu".to_string();
    sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Eu')")
        .bind(&id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn account(pool: &SqlitePool, id: &str, name: &str, kind: &str, balance: i64) {
    let owner = "p-eu";
    let liquidity = crate::commands::liquidity_for_type(kind);
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity, institution) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Banco Exemplo')",
    )
    .bind(id)
    .bind(name)
    .bind(kind)
    .bind(owner)
    .bind(balance)
    .bind(liquidity)
    .execute(pool)
    .await
    .unwrap();
}

async fn expense(pool: &SqlitePool, id: &str, amount: i64, date: &str, fixed: bool) {
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
                                      payment_method) \
         VALUES (?1, 'expense', ?2, ?3, ?4, 0, 'debit')",
    )
    .bind(id)
    .bind(amount)
    .bind(date)
    .bind(i64::from(fixed))
    .execute(pool)
    .await
    .unwrap();
}

async fn income(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
         VALUES (?1, 'income', ?2, ?3, 0)",
    )
    .bind(id)
    .bind(amount)
    .bind(date)
    .execute(pool)
    .await
    .unwrap();
}

async fn active_budget(pool: &SqlitePool, per_day: i64) {
    sqlx::query(
        "INSERT INTO daily_budget (id, person_id, amount, start_date, status, divisor_days, \
                                   ceremony_month) \
         VALUES ('db-1', 'p-eu', ?1, '2026-06-01', 'active', 30, '2026-06')",
    )
    .bind(per_day)
    .execute(pool)
    .await
    .unwrap();
}

/// Mundo base: uma pessoa, conta corrente e reserva, três meses completos de custo de vida
/// (o "retrato vivo" da reserva) e renda no ano.
async fn world(pool: &SqlitePool) {
    person(pool).await;
    account(pool, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(pool, "acc-reserve", "Reserva", "savings", 900_000).await;
    for (i, month) in ["04", "05", "06"].iter().enumerate() {
        expense(
            pool,
            &format!("fx-{i}"),
            200_000,
            &format!("2026-{month}-05"),
            true,
        )
        .await;
        income(
            pool,
            &format!("in-{i}"),
            800_000,
            &format!("2026-{month}-01"),
        )
        .await;
    }
}

// --- Contrato do envelope ---------------------------------------------------------------

#[tokio::test]
async fn envelope_carries_currency_timezone_period_as_of_and_revision() {
    let p = pool().await;
    world(&p).await;

    let env = call(&p, "get_financial_snapshot", json!({})).await;

    assert_eq!(env.tool, "get_financial_snapshot");
    assert!(env.ok);
    assert_eq!(env.meta.currency, "BRL");
    assert_eq!(env.meta.timezone, "-03:00");
    assert_eq!(env.meta.as_of, "2026-07-25T09:00:00-03:00");
    // Retrato de agora responde pelo MÊS corrente — o saldo que ele traz é o do fim do mês.
    assert_eq!(env.meta.period.start, "2026-07-01");
    assert_eq!(env.meta.period.end, "2026-07-31");
    assert!(env.meta.data_revision.is_some());
    assert_eq!(env.meta.row_limit, envelope::MAX_ROWS);
}

#[tokio::test]
async fn envelope_travels_with_the_error_too() {
    let p = pool().await;
    let env = call(&p, "get_the_answer", json!({})).await;

    assert!(!env.ok);
    assert_eq!(env.meta.currency, "BRL");
    assert!(env.meta.data_revision.is_some());
    assert!(env.data.is_none());
}

#[tokio::test]
async fn data_revision_is_stable_between_reads_and_moves_when_data_lands() {
    let p = pool().await;
    world(&p).await;

    let first = call(&p, "get_data_status", json!({}))
        .await
        .meta
        .data_revision;
    let again = call(&p, "get_data_status", json!({}))
        .await
        .meta
        .data_revision;
    assert_eq!(
        first, again,
        "ler duas vezes o mesmo mundo dá a mesma revisão"
    );

    expense(&p, "novo", 3_000, "2026-07-24", false).await;
    let after = call(&p, "get_data_status", json!({}))
        .await
        .meta
        .data_revision;
    assert_ne!(first, after, "um lançamento novo muda a revisão");
}

/// Dinheiro é centavo exato. Um float no fio significaria que alguma conta passou por ponto
/// flutuante — e centavo perdido em arredondamento é erro que o recibo não pega.
#[tokio::test]
async fn no_number_in_any_envelope_is_a_float() {
    let p = pool().await;
    world(&p).await;
    active_budget(&p, 5_000).await;
    expense(&p, "hoje", 4_300, "2026-07-25", false).await;

    for (tool, args) in [
        (
            "get_financial_snapshot",
            json!({"include": ["upcoming_invoices", "guardrail"]}),
        ),
        ("get_data_status", json!({"include": ["future_coverage"]})),
        ("get_budget_settings", json!({"include": ["ceremony"]})),
        (
            "get_accounts_and_net_worth",
            json!({"include": ["accounts"]}),
        ),
    ] {
        let env = call(&p, tool, args).await;
        let json = serde_json::to_value(&env).unwrap();
        assert_no_float(&json, tool, "");
    }
}

fn assert_no_float(value: &Value, tool: &str, path: &str) {
    match value {
        Value::Number(n) => assert!(
            n.is_i64() || n.is_u64(),
            "{tool}{path} devolveu float ({n}) — dinheiro e derivados saem inteiros"
        ),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                assert_no_float(item, tool, &format!("{path}[{i}]"));
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                assert_no_float(item, tool, &format!("{path}.{key}"));
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn listing_is_capped_and_says_so() {
    let p = pool().await;
    person(&p).await;
    for i in 0..(envelope::MAX_ROWS + 12) {
        account(&p, &format!("acc-{i}"), &format!("Conta {i}"), "bank", 100).await;
    }

    let data = data(
        &p,
        "get_accounts_and_net_worth",
        json!({"include": ["accounts"]}),
    )
    .await;
    let accounts = &data["accounts"];

    assert_eq!(accounts["returned"], envelope::MAX_ROWS);
    assert_eq!(accounts["total"], envelope::MAX_ROWS + 12);
    assert_eq!(accounts["truncated"], true);
    assert_eq!(
        accounts["items"].as_array().unwrap().len(),
        envelope::MAX_ROWS
    );
    // O agregado cobre o filtro inteiro, não a página.
    assert_eq!(data["liquid_cents"], (envelope::MAX_ROWS as i64 + 12) * 100);
}

// --- Erros acionáveis -------------------------------------------------------------------

#[tokio::test]
async fn unknown_tool_names_the_catalog() {
    let p = pool().await;
    let env = call(&p, "get_category_breakdown", json!({})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::UnknownTool);
    assert!(
        err.fix.contains("get_financial_snapshot"),
        "fix: {}",
        err.fix
    );
}

#[tokio::test]
async fn unknown_argument_is_refused_with_the_accepted_list() {
    let p = pool().await;
    let env = call(&p, "get_financial_snapshot", json!({"month": "2026-05"})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::UnknownArgument);
    assert!(err.message.contains("month"), "message: {}", err.message);
    assert!(err.fix.contains("include"), "fix: {}", err.fix);
}

#[tokio::test]
async fn unknown_include_lists_the_available_expansions() {
    let p = pool().await;
    let env = call(
        &p,
        "get_accounts_and_net_worth",
        json!({"include": ["cards"]}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("accounts"), "fix: {}", err.fix);
}

#[tokio::test]
async fn include_must_be_a_list_of_names() {
    let p = pool().await;
    let env = call(
        &p,
        "get_accounts_and_net_worth",
        json!({"include": "accounts"}),
    )
    .await;
    assert_eq!(env.error.unwrap().code, ErrorCode::InvalidArgument);
}

// --- Catálogo ---------------------------------------------------------------------------

#[tokio::test]
async fn every_tool_declares_use_for_and_not_for_and_answers() {
    let p = pool().await;
    world(&p).await;

    for spec in catalog::CATALOG {
        assert!(!spec.use_for.is_empty(), "{}: sem \"use para\"", spec.name);
        assert!(
            !spec.not_for.is_empty(),
            "{}: sem \"não use para\"",
            spec.name
        );
        assert!(!spec.summary.is_empty(), "{}: sem resumo", spec.name);

        let env = call(&p, spec.name, json!({})).await;
        assert!(env.ok, "{} não respondeu: {:?}", spec.name, env.error);

        // Toda expansão declarada é alcançável — catálogo que promete o que não entrega é
        // pior que catálogo curto.
        for include in spec.include_names() {
            let env = call(&p, spec.name, json!({ "include": [include] })).await;
            assert!(
                env.ok,
                "{} com include {include} falhou: {:?}",
                spec.name, env.error
            );
            assert!(
                env.data.unwrap().get(include).is_some(),
                "{}: include {include} pedido e ausente",
                spec.name
            );
        }
    }
}

// --- Defaults enxutos -------------------------------------------------------------------

#[tokio::test]
async fn heavy_fields_only_with_explicit_include() {
    let p = pool().await;
    world(&p).await;

    let snapshot = data(&p, "get_financial_snapshot", json!({})).await;
    assert!(snapshot.get("guardrail").is_none());
    assert!(snapshot.get("upcoming_invoices").is_none());

    let accounts = data(&p, "get_accounts_and_net_worth", json!({})).await;
    assert!(accounts.get("accounts").is_none());
    assert_eq!(accounts["accounts_total"], 2);

    let budget = data(&p, "get_budget_settings", json!({})).await;
    assert!(budget.get("ceremony").is_none());

    let status = data(&p, "get_data_status", json!({})).await;
    assert!(status.get("future_coverage").is_none());
}

// --- Estados epistêmicos ----------------------------------------------------------------

#[tokio::test]
async fn ceiling_without_record_is_no_record_never_zero() {
    let p = pool().await;
    person(&p).await;

    let snapshot = data(&p, "get_financial_snapshot", json!({})).await;
    assert_eq!(snapshot["daily_ceiling_cents"]["state"], "no_record");
    assert_eq!(snapshot["daily_ceiling_cents"]["value"], Value::Null);
}

#[tokio::test]
async fn ceiling_from_active_budget_is_verdict_and_from_last_month_is_estimate() {
    let p = pool().await;
    person(&p).await;
    active_budget(&p, 5_000).await;

    let chosen = data(&p, "get_budget_settings", json!({})).await;
    assert_eq!(chosen["daily_ceiling_cents"]["state"], "verdict");
    assert_eq!(chosen["daily_ceiling_cents"]["value"], 5_000);

    let q = pool().await;
    person(&q).await;
    for day in 1..=6 {
        expense(
            &q,
            &format!("d{day}"),
            3_000,
            &format!("2026-06-{day:02}"),
            false,
        )
        .await;
    }
    let estimated = data(&q, "get_budget_settings", json!({})).await;
    assert_eq!(estimated["daily_ceiling_cents"]["state"], "estimate");
}

#[tokio::test]
async fn reserve_states_walk_the_ladder() {
    // Sem conta de reserva mapeada: sem registro — nunca "0 meses".
    let p = pool().await;
    person(&p).await;
    account(&p, "acc-bank", "Conta", "bank", 100_000).await;
    expense(&p, "fx", 200_000, "2026-06-05", true).await;
    let snapshot = data(&p, "get_financial_snapshot", json!({})).await;
    assert_eq!(snapshot["reserve"]["state"], "no_record");
    assert_eq!(snapshot["reserve"]["months_tenths"], Value::Null);

    // Conta mapeada e zerada: zero legítimo, com a palavra dedicada.
    let q = pool().await;
    person(&q).await;
    account(&q, "acc-reserve", "Reserva", "savings", 0).await;
    expense(&q, "fx", 200_000, "2026-06-05", true).await;
    let snapshot = data(&q, "get_financial_snapshot", json!({})).await;
    assert_eq!(snapshot["reserve"]["state"], "zero");
    assert_eq!(snapshot["reserve"]["months_tenths"], 0);

    // Três meses vividos: retrato vivo (estimativa), com a mesma truncagem da tela.
    let r = pool().await;
    world(&r).await;
    let snapshot = data(&r, "get_financial_snapshot", json!({})).await;
    assert_eq!(snapshot["reserve"]["state"], "estimate");
    assert_eq!(snapshot["reserve"]["basis_months"], 3);
    assert_eq!(snapshot["reserve"]["months_tenths"], 45);
    assert_eq!(snapshot["reserve"]["months_display"], "4,5");
    assert_eq!(snapshot["reserve"]["balance_cents"], 900_000);
    assert_eq!(snapshot["reserve"]["target_months"], 6);

    // Janela cheia de seis meses: veredito. O degrau importa por dois motivos — é o estado que
    // o método persegue, e é o único que exercita a tradução do `verdict` do domínio.
    let s = pool().await;
    person(&s).await;
    account(&s, "acc-reserve", "Reserva", "savings", 1_200_000).await;
    for (i, month) in ["01", "02", "03", "04", "05", "06"].iter().enumerate() {
        expense(
            &s,
            &format!("fx-{i}"),
            200_000,
            &format!("2026-{month}-05"),
            true,
        )
        .await;
    }
    let snapshot = data(&s, "get_financial_snapshot", json!({})).await;
    assert_eq!(snapshot["reserve"]["state"], "verdict");
    assert_eq!(snapshot["reserve"]["basis_months"], 6);
    assert_eq!(snapshot["reserve"]["months_tenths"], 60);
    // Inteiro não ganha casa decimal — a mesma escrita da tela.
    assert_eq!(snapshot["reserve"]["months_display"], "6");
}

// --- As quatro perguntas de estado ------------------------------------------------------

#[tokio::test]
async fn snapshot_reads_the_day_against_the_ceiling() {
    let p = pool().await;
    world(&p).await;
    active_budget(&p, 5_000).await;
    expense(&p, "hoje-1", 3_000, "2026-07-25", false).await;
    expense(&p, "hoje-2", 1_300, "2026-07-25", false).await;

    let snapshot = data(&p, "get_financial_snapshot", json!({})).await;

    assert_eq!(snapshot["daily_ceiling_cents"]["value"], 5_000);
    assert_eq!(snapshot["daily_spend_today_cents"], 4_300);
    assert_eq!(snapshot["spending_mode"], "debit");
    assert_eq!(snapshot["last_real_transaction_date"], "2026-07-25");
    assert!(snapshot["projected_month_end_balance_cents"].is_i64());
}

#[tokio::test]
async fn snapshot_guardrail_says_which_ruler_binds() {
    let p = pool().await;
    world(&p).await;
    active_budget(&p, 5_000).await;

    let snapshot = data(
        &p,
        "get_financial_snapshot",
        json!({"include": ["guardrail"]}),
    )
    .await;
    let guardrail = &snapshot["guardrail"];

    assert!(guardrail["safe_to_spend_today_cents"].is_i64());
    assert!(["cash", "savings"].contains(&guardrail["binding"].as_str().unwrap()));
}

#[tokio::test]
async fn data_status_names_the_gaps_with_a_way_out() {
    let p = pool().await;
    world(&p).await;

    let status = data(&p, "get_data_status", json!({})).await;

    assert_eq!(status["has_data"], true);
    assert_eq!(status["realized_transactions"], 6);
    assert_eq!(status["first_transaction_date"], "2026-04-01");
    assert_eq!(status["last_real_transaction_date"], "2026-06-05");
    assert_eq!(status["days_since_last_entry"], 50);
    assert_eq!(status["readings"]["daily_ceiling"], "no_record");
    assert_eq!(status["readings"]["reserve"], "estimate");
    assert_eq!(status["pending"]["import_conflicts"], 0);

    // A lacuna do teto vem nomeada e com o gesto de saída — é dela que a recusa honesta nasce.
    let gaps = status["gaps"].as_array().unwrap();
    let ceiling = gaps
        .iter()
        .find(|g| g["code"] == "daily_ceiling_missing")
        .expect("lacuna do teto listada");
    assert!(!ceiling["what"].as_str().unwrap().is_empty());
    assert!(!ceiling["fix"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn data_status_without_any_data_is_honest_about_it() {
    let p = pool().await;

    let status = data(&p, "get_data_status", json!({})).await;

    assert_eq!(status["has_data"], false);
    assert_eq!(status["transactions_total"], 0);
    assert_eq!(status["last_real_transaction_date"], Value::Null);
    assert_eq!(status["days_since_last_entry"], Value::Null);
    assert!(
        status["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["code"] == "no_transactions")
    );
}

#[tokio::test]
async fn data_status_counts_what_waits_for_a_gesture() {
    let p = pool().await;
    world(&p).await;
    sqlx::query(
        "INSERT INTO ceiling_proposal (id, per_day_cents, divisor_days, source_month, \
                                       items_json, note_hash, status) \
         VALUES ('cp-1', 5500, 30, '2026-07', '[]', 'h1', 'pending')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO import_conflict (id, transaction_id, field, local_value, sheet_value) \
         VALUES ('ic-1', 'fx-0', 'amount', '100', '200')",
    )
    .execute(&p)
    .await
    .unwrap();

    let status = data(&p, "get_data_status", json!({})).await;
    assert_eq!(status["pending"]["ceiling_proposals"], 1);
    assert_eq!(status["pending"]["import_conflicts"], 1);
    assert_eq!(status["pending"]["card_proposals"], 0);
}

#[tokio::test]
async fn budget_settings_shows_the_ceremony_behind_the_ceiling() {
    let p = pool().await;
    person(&p).await;
    active_budget(&p, 5_000).await;
    for (i, (name, amount)) in [("Mercado", 90_000), ("Transporte", 60_000)]
        .iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO daily_budget_category (id, budget_id, name, amount_cents, position) \
             VALUES (?1, 'db-1', ?2, ?3, ?4)",
        )
        .bind(format!("dbc-{i}"))
        .bind(name)
        .bind(amount)
        .bind(i as i64)
        .execute(&p)
        .await
        .unwrap();
    }

    let lean = data(&p, "get_budget_settings", json!({})).await;
    assert_eq!(lean["divisor_days"], 30);
    assert_eq!(lean["ceremony_month"], "2026-06");
    assert_eq!(lean["monthly_total_cents"], 150_000);
    assert_eq!(lean["method_targets"]["economia_floor_bps"], 2_000);
    assert_eq!(lean["method_targets"]["economia_ceiling_bps"], 3_000);
    assert_eq!(lean["method_targets"]["reserve_months"], 6);
    assert_eq!(lean["pending_proposal"], Value::Null);

    let full = data(&p, "get_budget_settings", json!({"include": ["ceremony"]})).await;
    let items = &full["ceremony"]["items"];
    assert_eq!(items["total"], 2);
    assert_eq!(items["items"][0]["name"], "Mercado");
    assert_eq!(items["items"][0]["amount_cents"], 90_000);
}

#[tokio::test]
async fn accounts_and_net_worth_sums_by_liquidity() {
    let p = pool().await;
    person(&p).await;
    account(&p, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(&p, "acc-reserve", "Reserva", "savings", 900_000).await;
    account(&p, "acc-vr", "Vale", "meal_voucher", 40_000).await;
    account(&p, "acc-prev", "Previdência", "pension", 1_000_000).await;
    // Cartão é passivo, não bolso: fica fora do patrimônio e da contagem de contas.
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, balance, closing_day, due_day) \
         VALUES ('acc-card', 'Cartão', 'credit_card', 'p-eu', 0, 5, 15)",
    )
    .execute(&p)
    .await
    .unwrap();

    let worth = data(
        &p,
        "get_accounts_and_net_worth",
        json!({"include": ["accounts"]}),
    )
    .await;

    assert_eq!(worth["liquid_cents"], 500_000);
    assert_eq!(worth["reserve_cents"], 900_000);
    assert_eq!(worth["restricted_cents"], 40_000);
    assert_eq!(worth["illiquid_cents"], 1_000_000);
    assert_eq!(worth["net_worth_cents"], 2_400_000);
    assert_eq!(worth["accounts_total"], 4);
    assert_eq!(worth["accounts"]["items"][0]["name"], "Conta corrente");
    assert_eq!(worth["accounts"]["items"][0]["liquidity"], "liquid");
    assert_eq!(worth["accounts"]["items"][0]["balance_cents"], 500_000);
}
