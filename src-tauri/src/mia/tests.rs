//! Suíte da fachada: exercita a PORTA contra pool real com fixtures, nunca a forma interna de
//! cada ferramenta. O que está sob teste é o envelope que sai no fio.

use super::*;
use chrono::DateTime;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    let pack_root = pack_fixture();
    call_with_pack(pool, &pack_root, name, arguments).await
}

async fn call_with_pack(
    pool: &SqlitePool,
    pack_root: &Path,
    name: &str,
    arguments: Value,
) -> Envelope {
    let ctx = Context {
        clock: clock(),
        pack: method_tools::MethodPack::at(pack_root),
    };
    dispatch(pool, &ToolCall::new(name, arguments), &ctx).await
}

/// O pack da suíte é isolado para que o contrato da fachada não dependa de conteúdo privado.
fn pack_fixture() -> PathBuf {
    static PACK: OnceLock<PathBuf> = OnceLock::new();

    PACK.get_or_init(|| {
        let root = std::env::temp_dir().join("neko-finance-mia-pack-fixture");
        std::fs::create_dir_all(root.join("chapters")).unwrap();
        for topic in method_tools::TOPICS {
            std::fs::write(
                root.join("chapters").join(format!("{topic}.md")),
                // O título difere do nome do tópico de propósito: os dois coincidindo, o teste do
                // título passaria também com o parser quebrado, porque o tópico é o fallback.
                format!("# Capítulo de {topic}\n\nOrientação sintética do método para a suíte.\n"),
            )
            .unwrap();
        }
        std::fs::write(
            root.join("forbidden-extra.txt"),
            "termo-ausente-da-fixture\n",
        )
        .unwrap();
        root
    })
    .clone()
}

/// Pack temporário para provar os limites de privacidade sem depender do material privado.
struct TempPack {
    root: PathBuf,
}

impl TempPack {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "neko-finance-mia-test-pack-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn absent() -> Self {
        Self {
            root: std::env::temp_dir().join(format!(
                "neko-finance-mia-test-pack-{}",
                uuid::Uuid::new_v4()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn chapter(&self, topic: &str, content: &str) {
        let chapters = self.root.join("chapters");
        std::fs::create_dir_all(&chapters).unwrap();
        std::fs::write(chapters.join(format!("{topic}.md")), content).unwrap();
    }

    fn root_file(&self, name: &str, content: &str) {
        std::fs::write(self.root.join(name), content).unwrap();
    }
}

impl Drop for TempPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn parity_manifest_rows() -> Vec<[String; 4]> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("mia-tool-parity.md");
    let manifest = std::fs::read_to_string(path).unwrap();

    manifest
        .lines()
        .filter(|line| line.starts_with('|'))
        .filter_map(|line| {
            let mut columns: Vec<String> = line
                .split('|')
                .skip(1)
                .map(str::trim)
                .map(str::to_string)
                .collect();
            if columns.last().is_some_and(|column| column.is_empty()) {
                columns.pop();
            }
            let is_header = columns.first().is_some_and(|column| column == "Tela");
            let is_separator = !columns.is_empty()
                && columns.iter().all(|column| {
                    let dashes = column.trim_matches(':');
                    !dashes.is_empty() && dashes.chars().all(|character| character == '-')
                });
            (!is_header && !is_separator).then_some(columns)
        })
        .map(|columns| {
            columns.try_into().unwrap_or_else(|columns: Vec<String>| {
                panic!(
                    "linha do manifesto precisa ter quatro colunas, encontrou {}",
                    columns.len()
                )
            })
        })
        .collect()
}

/// Envelope de sucesso, com os dados. Falha o teste se a porta recusou.
async fn data(pool: &SqlitePool, name: &str, arguments: Value) -> Value {
    let env = call(pool, name, arguments).await;
    assert!(env.ok, "esperava sucesso, veio {:?}", env.error);
    env.data.expect("envelope de sucesso carrega dados")
}

async fn user_table_row_counts(pool: &SqlitePool) -> BTreeMap<String, i64> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();

    let mut counts = BTreeMap::new();
    for (table,) in tables {
        // O nome vem do catálogo interno do SQLite e é escapado como identificador antes de
        // formar a consulta, portanto não transporta entrada externa para o SQL dinâmico.
        let escaped_table = table.replace('"', "\"\"");
        let query = format!("SELECT COUNT(*) FROM \"{escaped_table}\"");
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.insert(table, count);
    }
    counts
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

/// Uma segunda pessoa — a divisão de despesas só existe quando há com quem dividir.
async fn another_person(pool: &SqlitePool, id: &str, name: &str) {
    sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
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

async fn card_expense(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
                                      payment_method) \
         VALUES (?1, 'expense', ?2, ?3, 0, 0, 'credit')",
    )
    .bind(id)
    .bind(amount)
    .bind(date)
    .execute(pool)
    .await
    .unwrap();
}

/// Transferência para um bolso: destino de reserva é Economia, destino ilíquido é Patrimônio.
async fn transfer(pool: &SqlitePool, id: &str, amount: i64, date: &str, to_account: &str) {
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
         VALUES (?1, 'transfer', ?2, ?3, ?4, 0)",
    )
    .bind(id)
    .bind(amount)
    .bind(date)
    .bind(to_account)
    .execute(pool)
    .await
    .unwrap();
}

/// Uma linha do Livro-razão com tudo o que o recorte filtra. Os helpers curtos acima bastam para
/// as réguas; o recorte precisa de descrição, conta, forma de pagamento e natureza no mesmo gesto.
struct Line<'a> {
    id: &'a str,
    kind: &'a str,
    amount: i64,
    date: &'a str,
    description: &'a str,
    method: &'a str,
    fixed: bool,
    from_account: Option<&'a str>,
    to_account: Option<&'a str>,
}

impl Default for Line<'_> {
    fn default() -> Self {
        Self {
            id: "",
            kind: "expense",
            amount: 10_000,
            date: "2026-07-10",
            description: "",
            method: "debit",
            fixed: false,
            from_account: None,
            to_account: None,
        }
    }
}

async fn line(pool: &SqlitePool, l: Line<'_>) {
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, description, payment_method, \
                                      is_fixed, is_projection, from_account_id, to_account_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)",
    )
    .bind(l.id)
    .bind(l.kind)
    .bind(l.amount)
    .bind(l.date)
    .bind(l.description)
    .bind(l.method)
    .bind(i64::from(l.fixed))
    .bind(l.from_account)
    .bind(l.to_account)
    .execute(pool)
    .await
    .unwrap();
}

async fn tag(pool: &SqlitePool, id: &str, name: &str) {
    sqlx::query("INSERT INTO tag (id, name, color) VALUES (?1, ?2, 'var(--cat-jade)')")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
}

/// Uma tag que desliga o interruptor do custo de vida — o gasto que sai da conta mas não é meu
/// custo de viver (dinheiro de terceiro é o caso canônico).
async fn tag_outside_cost(pool: &SqlitePool, id: &str, name: &str) {
    tag(pool, id, name).await;
    sqlx::query("UPDATE tag SET exclude_from_cost_of_living = 1 WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn tag_on(pool: &SqlitePool, transaction: &str, tag: &str) {
    sqlx::query("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)")
        .bind(transaction)
        .bind(tag)
        .execute(pool)
        .await
        .unwrap();
}

/// Quem respondeu por (parte de) um lançamento — a divisão de despesas do multi-titular.
async fn split_on(pool: &SqlitePool, id: &str, transaction: &str, amount: i64, person: &str) {
    sqlx::query(
        "INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(id)
    .bind(transaction)
    .bind(amount)
    .bind(person)
    .execute(pool)
    .await
    .unwrap();
}

/// Saldo encadeado da planilha para um dia — a corrente que o calendário lê no passado e a
/// semente de onde a projeção parte.
async fn sheet_balance(pool: &SqlitePool, date: &str, cents: i64) {
    sqlx::query(
        "INSERT INTO sheet_daily_balance (sheet_name, date, balance_cents) VALUES ('2026', ?1, ?2)",
    )
    .bind(date)
    .bind(cents)
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

/// Dois meses vividos com os cinco tipos do método na mesa (entrada, fixa, diário, cartão e
/// economia): o mundo das análises temporais.
async fn timeline(pool: &SqlitePool) {
    person(pool).await;
    account(pool, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(pool, "acc-reserve", "Reserva", "savings", 900_000).await;

    income(pool, "in-mai", 800_000, "2026-05-01").await;
    expense(pool, "fx-mai", 200_000, "2026-05-05", true).await;
    expense(pool, "di-mai", 50_000, "2026-05-10", false).await;
    card_expense(pool, "cc-mai", 30_000, "2026-05-12").await;
    transfer(pool, "ec-mai", 100_000, "2026-05-28", "acc-reserve").await;

    income(pool, "in-jun", 900_000, "2026-06-01").await;
    expense(pool, "fx-jun", 200_000, "2026-06-05", true).await;
    expense(pool, "di-jun", 60_000, "2026-06-10", false).await;
    card_expense(pool, "cc-jun", 30_000, "2026-06-12").await;
    transfer(pool, "ec-jun", 150_000, "2026-06-28", "acc-reserve").await;
}

/// Sete meses vividos de composição estável e um futuro quase vazio — o mundo em que a régua
/// anual precisa escolher entre o recorte vivido e a projeção do ano inteiro.
async fn lived_year(pool: &SqlitePool) {
    person(pool).await;
    account(pool, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(pool, "acc-reserve", "Reserva", "savings", 300_000).await;
    for month in 1..=7 {
        income(
            pool,
            &format!("in-{month:02}"),
            800_000,
            &format!("2026-{month:02}-01"),
        )
        .await;
        expense(
            pool,
            &format!("fx-{month:02}"),
            200_000,
            &format!("2026-{month:02}-05"),
            true,
        )
        .await;
        expense(
            pool,
            &format!("di-{month:02}"),
            100_000,
            &format!("2026-{month:02}-10"),
            false,
        )
        .await;
        transfer(
            pool,
            &format!("ec-{month:02}"),
            200_000,
            &format!("2026-{month:02}-20"),
            "acc-reserve",
        )
        .await;
    }
    // Agosto pré-lançado só com a renda e uma fixa magra: tem lançamento, tem pouco.
    income(pool, "in-08", 900_000, "2026-08-01").await;
    expense(pool, "fx-08", 50_000, "2026-08-05", true).await;
}

/// O mês corrente com variedade de recorte — débito, pix, fixa e variável, com tag, titular e
/// conta — mais uma linha em maio, para que o recorte tenha o que cortar.
async fn ledger(pool: &SqlitePool) {
    person(pool).await;
    another_person(pool, "p-gio", "Gio").await;
    account(pool, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(pool, "acc-reserve", "Reserva", "savings", 900_000).await;
    tag(pool, "tg-mercado", "Mercado").await;

    line(
        pool,
        Line {
            id: "lx-salario",
            kind: "income",
            amount: 800_000,
            date: "2026-07-01",
            description: "Salário",
            to_account: Some("acc-bank"),
            ..Default::default()
        },
    )
    .await;
    line(
        pool,
        Line {
            id: "lx-aluguel",
            amount: 200_000,
            date: "2026-07-05",
            description: "Aluguel",
            fixed: true,
            from_account: Some("acc-bank"),
            ..Default::default()
        },
    )
    .await;
    line(
        pool,
        Line {
            id: "lx-mercado",
            amount: 32_000,
            date: "2026-07-10",
            description: "Mercado",
            ..Default::default()
        },
    )
    .await;
    line(
        pool,
        Line {
            id: "lx-farmacia",
            amount: 8_000,
            date: "2026-07-12",
            description: "Farmácia",
            method: "pix",
            ..Default::default()
        },
    )
    .await;
    line(
        pool,
        Line {
            id: "lx-economia",
            kind: "transfer",
            amount: 150_000,
            date: "2026-07-20",
            description: "Para a reserva",
            to_account: Some("acc-reserve"),
            ..Default::default()
        },
    )
    .await;
    line(
        pool,
        Line {
            id: "lx-maio",
            amount: 45_000,
            date: "2026-05-18",
            description: "Mercado de maio",
            ..Default::default()
        },
    )
    .await;

    tag_on(pool, "lx-mercado", "tg-mercado").await;
    tag_on(pool, "lx-maio", "tg-mercado").await;
    split_on(pool, "sp-gio", "lx-mercado", 12_000, "p-gio").await;
}

/// O mesmo mês com um cartão de verdade: conta emissora, ciclo e uma compra na fatura aberta.
/// Devolve o id da conta do cartão.
async fn ledger_with_card(pool: &SqlitePool) -> String {
    ledger(pool).await;
    let card = crate::commands::card_cmds::create_card_account_inner(
        pool,
        "Cartão",
        Some("Banco Exemplo"),
        Some(5),
        Some(15),
        None,
        Some("Eu"),
        None,
        &[],
    )
    .await
    .unwrap();
    crate::commands::card_cmds::register_card_purchase_with_refund_inner(
        pool,
        &card,
        60_000,
        Some("Restaurante"),
        "2026-07-03",
        None,
        &[],
    )
    .await
    .unwrap();
    card
}

/// O que já está comprometido à frente: um parcelamento de cartão com reembolso vinculado, uma
/// assinatura sem fim e uma série de lançamentos do Livro-razão. Devolve o id do cartão.
async fn commitments(pool: &SqlitePool) -> String {
    let card = ledger_with_card(pool).await;
    crate::commands::card_cmds::create_card_series_with_refund_inner(
        pool,
        &card,
        "Notebook",
        100_000,
        Some(6),
        "2026-07-03",
        Some(50_000),
        &[],
    )
    .await
    .unwrap();
    crate::commands::card_cmds::create_card_series_with_refund_inner(
        pool,
        &card,
        "Streaming",
        3_000,
        None,
        "2026-07-03",
        None,
        &[],
    )
    .await
    .unwrap();
    crate::recurrence::create_recurring_series(
        pool,
        &crate::recurrence::RecurringTemplate {
            txn_type: "expense".into(),
            amount: 120_000,
            description: Some("Academia".into()),
            start: chrono::NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(),
            payment_method: Some("debit".into()),
            is_fixed: true,
        },
        crate::recurrence::Frequency::Mensal,
        6,
    )
    .await
    .unwrap();
    card
}

/// Uma obrigação nomeada sobre o item que se repete na nota — a série que a planilha não guarda.
/// Devolve o id da obrigação.
async fn rent_obligation(pool: &SqlitePool) -> String {
    for (i, month) in ["05", "06", "07"].iter().enumerate() {
        let txn = format!("rent-{month}");
        line(
            pool,
            Line {
                id: &txn,
                amount: 180_000 + (i as i64 * 10_000),
                date: &format!("2026-{month}-10"),
                description: "Contas",
                fixed: true,
                ..Default::default()
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, \
                                    section) \
             VALUES (?1, ?2, ?3, 'Aluguel', 0, 'CONTAS:')",
        )
        .bind(format!("li-{month}"))
        .bind(&txn)
        .bind(180_000 + (i as i64 * 10_000))
        .execute(pool)
        .await
        .unwrap();
    }
    crate::obligations::create_obligation(pool, "Aluguel", "Aluguel", Some("CONTAS:"))
        .await
        .unwrap()
}

/// Futuro pré-lançado sobre a corrente da planilha: semente conhecida no dia 24 e agosto inteiro
/// lançado — o mundo da projeção e do calendário.
async fn projected_future(pool: &SqlitePool) {
    sheet_balance(pool, "2026-07-24", 1_000_000).await;
    income(pool, "in-ago", 900_000, "2026-08-01").await;
    expense(pool, "fx-ago", 300_000, "2026-08-10", true).await;
    expense(pool, "fx-ago-fim", 100_000, "2026-08-31", true).await;
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
    let obligation = rent_obligation(&p).await;

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
        (
            "get_month_analysis",
            json!({"month": "2026-06", "compare_to": "2026-05", "include": ["days", "owners"]}),
        ),
        (
            "get_year_analysis",
            json!({"year": 2026, "compare_to": 2025, "include": ["months", "year_end"]}),
        ),
        ("get_forecast", json!({"include": ["daily", "coverage"]})),
        ("get_cashflow_calendar", json!({})),
        (
            "search_transactions",
            json!({"include": ["tags", "items", "owners"]}),
        ),
        ("get_tags", json!({"include": ["effects", "third_parties"]})),
        ("get_commitments", json!({"include": ["occurrences"]})),
        (
            "get_commitments",
            json!({"obligation_id": obligation, "include": ["occurrences"]}),
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
    // O mundo mais completo da suíte: contas, cartão com fatura, séries, tags e titulares. Uma
    // expansão só se prova alcançável quando existe o dado que ela expande.
    commitments(&p).await;
    rent_obligation(&p).await;

    for spec in catalog::CATALOG {
        assert!(!spec.use_for.is_empty(), "{}: sem \"use para\"", spec.name);
        assert!(
            !spec.not_for.is_empty(),
            "{}: sem \"não use para\"",
            spec.name
        );
        assert!(!spec.summary.is_empty(), "{}: sem resumo", spec.name);

        let env = call(&p, spec.name, Value::Object(minimal_args(spec.name))).await;
        assert!(env.ok, "{} não respondeu: {:?}", spec.name, env.error);

        // Toda expansão declarada é alcançável — catálogo que promete o que não entrega é
        // pior que catálogo curto. A expansão de uma LISTA cai dentro de cada linha (as tags de
        // um lançamento), então a busca é pelo nome em qualquer profundidade.
        for include in spec.include_names() {
            let mut arguments = minimal_args(spec.name);
            arguments.insert("include".to_string(), json!([include]));
            let env = call(&p, spec.name, Value::Object(arguments)).await;
            assert!(
                env.ok,
                "{} com include {include} falhou: {:?}",
                spec.name, env.error
            );
            assert!(
                has_key(&env.data.unwrap(), include),
                "{}: include {include} pedido e ausente",
                spec.name
            );
        }
    }
}

fn minimal_args(tool: &str) -> serde_json::Map<String, Value> {
    let mut arguments = serde_json::Map::new();
    if tool == "simulate_scenario" {
        arguments.insert(
            "changes".to_string(),
            json!([{
                "movement": "saida",
                "amount_cents": 50000,
                "date": "2026-07-28"
            }]),
        );
    }
    arguments
}

/// A chave existe em algum ponto da resposta?
fn has_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => map.contains_key(key) || map.values().any(|item| has_key(item, key)),
        Value::Array(items) => items.iter().any(|item| has_key(item, key)),
        _ => false,
    }
}

#[tokio::test]
async fn manifest_tools_resolve_to_catalog_and_catalog_tools_have_surface() {
    let rows = parity_manifest_rows();
    let manifest_tools: Vec<&str> = rows.iter().map(|row| row[3].trim_matches('`')).collect();

    for tool in &manifest_tools {
        assert!(
            catalog::CATALOG.iter().any(|spec| spec.name == *tool),
            "A ferramenta desconhecida \"{tool}\" foi citada no manifesto de paridade."
        );
    }

    for spec in catalog::CATALOG {
        assert!(
            manifest_tools.contains(&spec.name),
            "A ferramenta \"{}\" está órfã no manifesto: ferramenta sem superfície é ferramenta que ninguém alcança.",
            spec.name
        );
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

// --- Um mês em detalhe ------------------------------------------------------------------

#[tokio::test]
async fn month_analysis_publishes_the_engine_buckets() {
    let p = pool().await;
    timeline(&p).await;

    let mai = data(&p, "get_month_analysis", json!({"month": "2026-05"})).await;

    assert_eq!(mai["month"], "2026-05");
    assert_eq!(mai["income_cents"], 800_000);
    assert_eq!(mai["buckets"]["fixed_out_cents"], 200_000);
    assert_eq!(mai["buckets"]["daily_out_cents"], 50_000);
    assert_eq!(mai["buckets"]["cartao_cents"], 30_000);
    assert_eq!(mai["buckets"]["economia_cents"], 100_000);
    assert_eq!(mai["buckets"]["patrimonio_cents"], 0);
    // Custo de vida = fixas + diário + cartão. Economia e Patrimônio ficam FORA: são dinheiro
    // que saiu da conta, não custo de viver.
    assert_eq!(mai["cost_of_living_cents"], 280_000);
    assert_eq!(mai["performance_cents"], 420_000);
    assert_eq!(mai["economizado_bps"], 1_250);
    assert_eq!(mai["cost_of_living_within_income"], true);
}

/// As buckets são as do motor de projeção, uma a uma. Um balde a mais (ou com outro nome) faria
/// a conversa falar um dialeto que as telas não falam — e duas somas do mesmo mês divergiriam.
#[tokio::test]
async fn month_buckets_speak_the_engine_vocabulary_one_to_one() {
    let p = pool().await;
    timeline(&p).await;

    let mai = data(&p, "get_month_analysis", json!({"month": "2026-05"})).await;
    let buckets: Vec<&str> = mai["buckets"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(
        buckets,
        // Os cinco tipos de SAÍDA do motor, em ordem alfabética como o envelope os serializa.
        vec![
            "cartao_cents",
            "daily_out_cents",
            "economia_cents",
            "fixed_out_cents",
            "patrimonio_cents",
        ]
    );
    // Entrada é o sexto tipo do motor e fica fora do objeto porque não é saída.
    assert!(mai["income_cents"].is_i64());
    let out = &mai["buckets"];
    assert_eq!(
        mai["cost_of_living_cents"].as_i64().unwrap(),
        out["fixed_out_cents"].as_i64().unwrap()
            + out["daily_out_cents"].as_i64().unwrap()
            + out["cartao_cents"].as_i64().unwrap()
    );
}

#[tokio::test]
async fn month_analysis_defaults_to_the_current_month_and_says_where_it_stands() {
    let p = pool().await;
    timeline(&p).await;

    let now = call(&p, "get_month_analysis", json!({})).await;
    assert!(now.ok);
    assert_eq!(now.meta.period.start, "2026-07-01");
    assert_eq!(now.meta.period.end, "2026-07-31");
    assert_eq!(now.data.unwrap()["status"], "current");

    let past = data(&p, "get_month_analysis", json!({"month": "2026-05"})).await;
    assert_eq!(past["status"], "complete");

    let ahead = data(&p, "get_month_analysis", json!({"month": "2026-09"})).await;
    assert_eq!(ahead["status"], "future");
}

#[tokio::test]
async fn month_analysis_hands_the_comparison_already_subtracted() {
    let p = pool().await;
    timeline(&p).await;

    let jun = data(
        &p,
        "get_month_analysis",
        json!({"month": "2026-06", "compare_to": "2026-05"}),
    )
    .await;

    assert_eq!(jun["compare_to"]["month"], "2026-05");
    assert_eq!(jun["compare_to"]["cost_of_living_cents"], 280_000);
    // Nenhuma conta sobra para quem consome: cada figura vem com a diferença em centavos e a
    // variação relativa em basis points.
    assert_eq!(jun["delta"]["income"]["cents"], 100_000);
    assert_eq!(jun["delta"]["income"]["change_bps"], 1_250);
    assert_eq!(jun["delta"]["cost_of_living"]["cents"], 10_000);
    assert_eq!(jun["delta"]["cost_of_living"]["change_bps"], 357);
    assert_eq!(jun["delta"]["economia"]["cents"], 50_000);
    assert_eq!(jun["delta"]["economia"]["change_bps"], 5_000);
    assert_eq!(jun["delta"]["performance"]["cents"], 40_000);
    // Economizado% é percentual: a diferença é de pontos-base, não variação sobre variação.
    assert_eq!(jun["delta"]["economizado_bps"], 416);
}

#[tokio::test]
async fn month_analysis_change_is_null_when_there_is_no_base_to_divide() {
    let p = pool().await;
    timeline(&p).await;

    let mai = data(
        &p,
        "get_month_analysis",
        json!({"month": "2026-05", "compare_to": "2026-04"}),
    )
    .await;

    assert_eq!(mai["delta"]["income"]["cents"], 800_000);
    // Abril não teve renda: variação relativa sobre zero não existe — nula, nunca inventada.
    assert_eq!(mai["delta"]["income"]["change_bps"], Value::Null);
}

#[tokio::test]
async fn month_analysis_day_grid_only_with_include() {
    let p = pool().await;
    timeline(&p).await;

    let lean = data(&p, "get_month_analysis", json!({"month": "2026-05"})).await;
    assert!(lean.get("days").is_none());

    let full = data(
        &p,
        "get_month_analysis",
        json!({"month": "2026-05", "include": ["days"]}),
    )
    .await;
    let days = &full["days"];
    assert_eq!(days["total"], 31);
    assert_eq!(days["items"][0]["date"], "2026-05-01");
    assert_eq!(days["items"][0]["income_cents"], 800_000);
    assert_eq!(days["items"][9]["daily_out_cents"], 50_000);
}

#[tokio::test]
async fn month_analysis_owners_only_with_include() {
    let p = pool().await;
    timeline(&p).await;
    sqlx::query(
        "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
         VALUES ('sp-1', 'di-mai', 50000, 'p-eu')",
    )
    .execute(&p)
    .await
    .unwrap();

    let owners = data(
        &p,
        "get_month_analysis",
        json!({"month": "2026-05", "include": ["owners"]}),
    )
    .await;

    assert_eq!(owners["owners"]["items"][0]["owner_name"], "Eu");
    assert_eq!(owners["owners"]["items"][0]["total_cents"], 50_000);
}

#[tokio::test]
async fn month_analysis_refuses_a_month_that_is_not_a_month() {
    let p = pool().await;
    timeline(&p).await;

    let env = call(&p, "get_month_analysis", json!({"month": "julho"})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    // A recusa diz o formato aceito e mostra um mês escrito nele — corrigir não depende de
    // adivinhar a forma.
    assert!(err.message.contains("YYYY-MM"), "message: {}", err.message);
    assert!(err.fix.contains("2026-07"), "fix: {}", err.fix);
}

// --- Um ano na régua anual --------------------------------------------------------------

#[tokio::test]
async fn year_analysis_publishes_the_annual_economizado_ruler() {
    let p = pool().await;
    lived_year(&p).await;

    let env = call(&p, "get_year_analysis", json!({"year": 2026})).await;
    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-01-01");
    assert_eq!(env.meta.period.end, "2026-12-31");
    let year = env.data.unwrap();

    assert_eq!(year["year"], 2026);
    assert_eq!(year["lived_months"], 7);
    assert_eq!(year["income_lived_cents"], 5_600_000);
    assert_eq!(year["economia_lived_cents"], 1_400_000);
    assert_eq!(year["income_year_cents"], 6_500_000);
    assert_eq!(year["economia_year_cents"], 1_400_000);
    // A faixa do método é ANUAL: 20% de piso, 25% de alvo, 30% de teto.
    assert_eq!(year["economizado"]["lived_bps"], 2_500);
    assert_eq!(year["economizado"]["projected_bps"], 2_153);
    assert_eq!(year["economizado"]["band"]["floor_bps"], 2_000);
    assert_eq!(year["economizado"]["band"]["target_bps"], 2_500);
    assert_eq!(year["economizado"]["band"]["ceiling_bps"], 3_000);
    assert_eq!(year["economizado"]["verdict"], "in_band");
}

#[tokio::test]
async fn year_analysis_falls_back_to_the_lived_cut_when_a_future_month_has_no_lastro() {
    let p = pool().await;
    lived_year(&p).await;

    let year = data(&p, "get_year_analysis", json!({"year": 2026})).await;

    // Gasto típico = mediana das saídas vividas (fixa + diário + economia = 500.000).
    assert_eq!(year["typical_spend_cents"], 500_000);
    // Agosto tem lançamento, só tem pouco; setembro a dezembro nem isso. Enquanto houver mês
    // sem lastro, a régua recua ao vivido e IMPRIME o recorte.
    assert_eq!(year["suspect_months"], json!([8, 9, 10, 11, 12]));
    assert_eq!(year["economizado"]["bps"], 2_500);
    assert_eq!(year["economizado"]["scope"], "lived");
    assert_eq!(year["economizado"]["state"], "estimate");
}

#[tokio::test]
async fn year_analysis_ruler_covers_the_whole_year_when_every_month_has_lastro() {
    let p = pool().await;
    person(&p).await;
    account(&p, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(&p, "acc-reserve", "Reserva", "savings", 300_000).await;
    for month in 1..=12 {
        income(
            &p,
            &format!("in-25-{month:02}"),
            600_000,
            &format!("2025-{month:02}-01"),
        )
        .await;
        transfer(
            &p,
            &format!("ec-25-{month:02}"),
            120_000,
            &format!("2025-{month:02}-20"),
            "acc-reserve",
        )
        .await;
    }

    let year = data(&p, "get_year_analysis", json!({"year": 2025})).await;

    assert_eq!(year["lived_months"], 12);
    assert_eq!(year["suspect_months"], json!([]));
    assert_eq!(year["economizado"]["scope"], "year");
    assert_eq!(year["economizado"]["state"], "verdict");
    assert_eq!(year["economizado"]["bps"], 2_000);
    assert_eq!(year["economizado"]["verdict"], "in_band");
}

#[tokio::test]
async fn year_analysis_reads_zero_economia_as_a_choice_when_the_reserve_is_protected() {
    let p = pool().await;
    person(&p).await;
    account(&p, "acc-bank", "Conta corrente", "bank", 500_000).await;
    account(&p, "acc-reserve", "Reserva", "savings", 2_000_000).await;
    for month in 1..=6 {
        income(
            &p,
            &format!("in-{month:02}"),
            800_000,
            &format!("2026-{month:02}-01"),
        )
        .await;
        expense(
            &p,
            &format!("fx-{month:02}"),
            200_000,
            &format!("2026-{month:02}-05"),
            true,
        )
        .await;
        expense(
            &p,
            &format!("di-{month:02}"),
            100_000,
            &format!("2026-{month:02}-10"),
            false,
        )
        .await;
    }

    let year = data(&p, "get_year_analysis", json!({"year": 2026})).await;

    // Reserva de 6,6 meses com economia zerada é a troca CERTA na ordem do método — nunca o
    // "abaixo da faixa" que puniria a escolha.
    assert_eq!(year["economizado"]["verdict"], "zero_by_choice");
    assert_eq!(year["economia_lived_cents"], 0);
}

#[tokio::test]
async fn year_analysis_without_any_lived_month_has_no_record() {
    let p = pool().await;
    person(&p).await;

    let year = data(&p, "get_year_analysis", json!({"year": 2026})).await;

    assert_eq!(year["economizado"]["verdict"], "no_record");
    assert_eq!(year["economizado"]["state"], "no_record");
    assert_eq!(year["economizado"]["bps"], Value::Null);
}

#[tokio::test]
async fn year_analysis_compares_two_years_with_the_delta_ready() {
    let p = pool().await;
    lived_year(&p).await;
    for month in 1..=12 {
        income(
            &p,
            &format!("in-25-{month:02}"),
            600_000,
            &format!("2025-{month:02}-01"),
        )
        .await;
        transfer(
            &p,
            &format!("ec-25-{month:02}"),
            120_000,
            &format!("2025-{month:02}-20"),
            "acc-reserve",
        )
        .await;
    }

    let year = data(
        &p,
        "get_year_analysis",
        json!({"year": 2026, "compare_to": 2025}),
    )
    .await;

    assert_eq!(year["compare_to"]["year"], 2025);
    assert_eq!(year["compare_to"]["economizado"]["bps"], 2_000);
    assert_eq!(year["compare_to"]["economizado"]["scope"], "year");
    // A comparação entre anos é de renda MÉDIA por mês com registro: 2026 tem sete meses
    // vividos e 2025 tem doze, e comparar os totais acusaria uma queda que é só o calendário.
    assert_eq!(year["recorded_months"], 7);
    assert_eq!(year["avg_income_cents"], 800_000);
    assert_eq!(year["compare_to"]["recorded_months"], 12);
    assert_eq!(year["compare_to"]["avg_income_cents"], 600_000);
    assert_eq!(year["delta"]["avg_income"]["cents"], 200_000);
    assert_eq!(year["delta"]["avg_income"]["change_bps"], 3_333);
    assert_eq!(year["delta"]["economizado_bps"], 500);
}

#[tokio::test]
async fn year_analysis_months_only_with_include() {
    let p = pool().await;
    lived_year(&p).await;

    let lean = data(&p, "get_year_analysis", json!({"year": 2026})).await;
    assert!(lean.get("months").is_none());

    let full = data(
        &p,
        "get_year_analysis",
        json!({"year": 2026, "include": ["months"]}),
    )
    .await;
    let months = &full["months"];

    assert_eq!(months["total"], 12);
    assert_eq!(months["items"][0]["month"], "2026-01");
    assert_eq!(months["items"][0]["income_cents"], 800_000);
    assert_eq!(months["items"][0]["economia_cents"], 200_000);
    assert_eq!(months["items"][0]["lived"], true);
    assert_eq!(months["items"][7]["suspect"], true);
}

// A régua anual tem UMA definição: a porta que a tela O ano consome e a resposta que a conversa
// dá saem da mesma função do motor. Enquanto este teste passar, nenhuma das duas superfícies pode
// afirmar um percentual, um veredito ou um fim de ano que a outra não mostra.
#[tokio::test]
async fn the_screen_and_the_conversation_read_the_same_ruler() {
    let p = pool().await;
    lived_year(&p).await;
    sheet_balance(&p, "2026-06-30", 900_000).await;
    sheet_balance(&p, "2026-07-24", 1_000_000).await;

    let today = chrono::NaiveDate::from_ymd_opt(2026, 7, 25).unwrap();
    let screen = crate::commands::annual_ruler_dto(&p, 2026, today)
        .await
        .unwrap();
    let screen = serde_json::to_value(&screen).unwrap();
    let talk = data(
        &p,
        "get_year_analysis",
        json!({"year": 2026, "include": ["months", "year_end"]}),
    )
    .await;

    // O ano tem o que julgar e o que projetar — sem isto a comparação abaixo passaria vazia.
    assert!(screen["bps"].is_i64(), "régua sem percentual: {screen}");
    assert!(
        screen["year_end"]["end_balance_typical_cents"].is_i64(),
        "cenário do fim do ano ausente: {screen}"
    );

    assert_eq!(screen["bps"], talk["economizado"]["bps"]);
    assert_eq!(screen["verdict"], talk["economizado"]["verdict"]);
    assert_eq!(screen["lived_bps"], talk["economizado"]["lived_bps"]);
    assert_eq!(
        screen["projected_bps"],
        talk["economizado"]["projected_bps"]
    );
    assert_eq!(
        screen["scope_lived"],
        talk["economizado"]["scope"] == "lived"
    );
    assert_eq!(screen["typical_spend_cents"], talk["typical_spend_cents"]);
    assert_eq!(screen["income_lived_cents"], talk["income_lived_cents"]);
    assert_eq!(screen["economia_year_cents"], talk["economia_year_cents"]);
    assert_eq!(
        screen["shortfall_year_cents"],
        talk["shortfall_to_floor_cents"]
    );
    assert_eq!(
        screen["per_month_shortfall_cents"],
        talk["per_month_shortfall_cents"]
    );
    assert_eq!(
        screen["year_end"]["end_balance_cents"],
        talk["year_end"]["end_balance_cents"]
    );
    assert_eq!(
        screen["year_end"]["end_balance_typical_cents"],
        talk["year_end"]["end_balance_typical_cents"]
    );

    let suspects: Vec<Value> = screen["months"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["suspect"] == true)
        .map(|m| m["month"].clone())
        .collect();
    assert_eq!(Value::Array(suspects), talk["suspect_months"]);

    // A linha do mês também: a saída, o vivido e o lastro são a mesma leitura nas duas bocas.
    for (screen_month, talk_month) in screen["months"]
        .as_array()
        .unwrap()
        .iter()
        .zip(talk["months"]["items"].as_array().unwrap())
    {
        let month = screen_month["month"].as_u64().unwrap();
        assert_eq!(talk_month["month"], format!("2026-{month:02}"));
        assert_eq!(screen_month["outflow_cents"], talk_month["outflow_cents"]);
        assert_eq!(screen_month["lived"], talk_month["lived"]);
        assert_eq!(screen_month["suspect"], talk_month["suspect"]);
    }
}

// --- A projeção à frente ----------------------------------------------------------------

#[tokio::test]
async fn forecast_answers_the_range_with_the_month_end_and_the_guardrail() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;

    let env = call(
        &p,
        "get_forecast",
        json!({"range": {"start": "2026-08-01", "end": "2026-08-31"}}),
    )
    .await;
    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-08-01");
    assert_eq!(env.meta.period.end, "2026-08-31");
    let forecast = env.data.unwrap();

    assert_eq!(forecast["today"], "2026-07-25");
    assert_eq!(forecast["month_end"]["items"][0]["month"], "2026-08");
    assert_eq!(
        forecast["month_end"]["items"][0]["balance_cents"],
        1_500_000
    );
    assert_eq!(forecast["end_balance_cents"], 1_500_000);
    // O menor saldo é o do RECORTE pedido; o fundo do poço do horizonte inteiro tem campo
    // próprio, para que a resposta não troque um pelo outro.
    assert_eq!(forecast["lowest_balance"]["balance_cents"], 1_500_000);
    assert_eq!(
        forecast["horizon_lowest_balance"]["balance_cents"],
        1_000_000
    );
    assert!(["cash", "savings"].contains(&forecast["binding"].as_str().unwrap()));
    assert!(forecast["safe_to_spend_today_cents"].is_i64());
}

#[tokio::test]
async fn forecast_daily_only_with_include_and_never_leaves_the_range() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;

    let lean = data(&p, "get_forecast", json!({})).await;
    assert!(lean.get("daily").is_none());

    let full = data(
        &p,
        "get_forecast",
        json!({"range": {"start": "2026-08-01", "end": "2026-08-31"}, "include": ["daily"]}),
    )
    .await;
    let daily = &full["daily"];

    assert_eq!(daily["total"], 31);
    assert_eq!(daily["items"][0]["date"], "2026-08-01");
    assert_eq!(daily["items"][0]["balance_cents"], 1_900_000);
    assert_eq!(daily["items"][30]["date"], "2026-08-31");
}

#[tokio::test]
async fn forecast_default_range_starts_today_and_ends_at_the_horizon() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;

    let env = call(&p, "get_forecast", json!({})).await;

    assert_eq!(env.meta.period.start, "2026-07-25");
    assert_eq!(env.meta.period.end, "2026-08-31");
}

#[tokio::test]
async fn forecast_refuses_a_range_that_ended_before_today() {
    let p = pool().await;
    timeline(&p).await;

    let env = call(
        &p,
        "get_forecast",
        json!({"range": {"start": "2026-05-01", "end": "2026-05-31"}}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    // A recusa entrega a porta certa em vez de só fechar a errada.
    assert!(err.fix.contains("get_month_analysis"), "fix: {}", err.fix);
}

#[tokio::test]
async fn forecast_with_a_scenario_hands_the_comparison_ready() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;
    let scenario = crate::scenarios::create_scenario(&p, "Sem o carro")
        .await
        .unwrap();
    crate::scenarios::add_scenario_transaction(
        &p,
        &scenario.id,
        "expense",
        200_000,
        "Parcela do carro",
        "2026-08-15",
        Some("debit"),
        true,
        None,
        None,
    )
    .await
    .unwrap();

    let forecast = data(
        &p,
        "get_forecast",
        json!({"scenario_id": scenario.id, "range": {"start": "2026-08-01", "end": "2026-08-31"}}),
    )
    .await;
    let s = &forecast["scenario"];

    assert_eq!(s["name"], "Sem o carro");
    assert_eq!(s["month_end"]["items"][0]["month"], "2026-08");
    assert_eq!(s["month_end"]["items"][0]["balance_cents"], 1_300_000);
    assert_eq!(s["month_end"]["items"][0]["delta_cents"], -200_000);
    assert!(s["safe_to_spend_delta_cents"].is_i64());
}

#[tokio::test]
async fn forecast_names_the_scenario_that_does_not_exist() {
    let p = pool().await;
    timeline(&p).await;

    let env = call(&p, "get_forecast", json!({"scenario_id": "sc-fantasma"})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(!err.fix.is_empty());
}

// --- O calendário de caixa --------------------------------------------------------------

#[tokio::test]
async fn cashflow_calendar_walks_the_days_with_movement_and_balance() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;
    expense(&p, "di-27", 20_000, "2026-07-27", false).await;

    let calendar = data(
        &p,
        "get_cashflow_calendar",
        json!({"range": {"start": "2026-07-23", "end": "2026-07-27"}}),
    )
    .await;
    let items = calendar["days"]["items"].as_array().unwrap();

    assert_eq!(items.len(), 5);
    // Antes de hoje a corrente é a da planilha: sem Saldo importado, o dia não inventa número.
    assert_eq!(items[0]["date"], "2026-07-23");
    assert_eq!(items[0]["balance_cents"], Value::Null);
    assert_eq!(items[0]["movement_cents"], Value::Null);
    assert_eq!(items[1]["date"], "2026-07-24");
    assert_eq!(items[1]["balance_cents"], 1_000_000);
    assert_eq!(items[1]["is_future"], false);
    // De hoje em diante quem responde é a projeção, e o movimento é o passo da corrente.
    assert_eq!(items[2]["date"], "2026-07-25");
    assert_eq!(items[2]["balance_cents"], 1_000_000);
    assert_eq!(items[2]["movement_cents"], 0);
    assert_eq!(items[4]["date"], "2026-07-27");
    assert_eq!(items[4]["balance_cents"], 980_000);
    assert_eq!(items[4]["movement_cents"], -20_000);
    assert_eq!(items[4]["daily_out_cents"], 20_000);
    assert_eq!(items[4]["is_future"], true);
    assert_eq!(calendar["lowest_balance"]["date"], "2026-07-27");
}

#[tokio::test]
async fn cashflow_calendar_totals_cover_the_whole_range_not_the_page() {
    let p = pool().await;
    timeline(&p).await;
    projected_future(&p).await;

    let first = data(
        &p,
        "get_cashflow_calendar",
        json!({"range": {"start": "2026-01-01", "end": "2026-12-31"}}),
    )
    .await;

    assert_eq!(first["days"]["returned"], envelope::MAX_ROWS);
    assert_eq!(first["days"]["total"], 365);
    let cursor = first["days"]["next_cursor"].as_str().unwrap().to_string();
    // Entradas do ano inteiro: maio 800.000 + junho 900.000 + agosto 900.000.
    assert_eq!(first["totals"]["income_cents"], 2_600_000);

    let second = data(
        &p,
        "get_cashflow_calendar",
        json!({"range": {"start": "2026-01-01", "end": "2026-12-31"}, "cursor": cursor}),
    )
    .await;

    assert_eq!(second["days"]["returned"], 365 - envelope::MAX_ROWS);
    assert_eq!(second["days"]["next_cursor"], Value::Null);
    assert_eq!(second["days"]["items"][0]["date"], "2026-07-20");
    // O agregado é do RECORTE, não da página: as duas páginas dizem o mesmo total.
    assert_eq!(second["totals"]["income_cents"], 2_600_000);
    assert_eq!(second["totals"], first["totals"]);
}

#[tokio::test]
async fn cashflow_calendar_refuses_a_cursor_from_another_range() {
    let p = pool().await;
    timeline(&p).await;

    let first = data(
        &p,
        "get_cashflow_calendar",
        json!({"range": {"start": "2026-01-01", "end": "2026-12-31"}}),
    )
    .await;
    let cursor = first["days"]["next_cursor"].as_str().unwrap().to_string();

    let env = call(
        &p,
        "get_cashflow_calendar",
        json!({"range": {"start": "2026-02-01", "end": "2026-12-31"}, "cursor": cursor}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("cursor"), "fix: {}", err.fix);
}

#[tokio::test]
async fn cashflow_calendar_defaults_to_the_current_month() {
    let p = pool().await;
    timeline(&p).await;

    let env = call(&p, "get_cashflow_calendar", json!({})).await;

    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-07-01");
    assert_eq!(env.meta.period.end, "2026-07-31");
    assert_eq!(env.data.unwrap()["days"]["total"], 31);
}

// --- O recorte de lançamentos -----------------------------------------------------------

#[tokio::test]
async fn search_lists_the_current_month_with_the_lines_and_the_totals() {
    let p = pool().await;
    ledger(&p).await;

    let env = call(&p, "search_transactions", json!({})).await;
    assert!(env.ok, "{:?}", env.error);
    // Sem recorte pedido, o recorte é o mês corrente — nunca a base inteira.
    assert_eq!(env.meta.period.start, "2026-07-01");
    assert_eq!(env.meta.period.end, "2026-07-31");
    let found = env.data.unwrap();

    let items = found["transactions"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 5);
    // Mais recente primeiro, que é a ordem em que a pessoa lê o próprio mês.
    assert_eq!(items[0]["date"], "2026-07-20");
    assert_eq!(items[0]["description"], "Para a reserva");
    assert_eq!(items[0]["amount_cents"], 150_000);
    // O tipo do método vem classificado pelo motor: transferência para a reserva é Economia.
    assert_eq!(items[0]["movement"], "economia");
    assert_eq!(items[0]["provenance"], "manual");
    assert_eq!(items[4]["date"], "2026-07-01");
    assert_eq!(items[4]["movement"], "entrada");

    let totals = &found["totals"];
    assert_eq!(totals["count"], 5);
    assert_eq!(totals["income_cents"], 800_000);
    assert_eq!(totals["expense_cents"], 240_000);
    assert_eq!(totals["transfer_cents"], 150_000);
}

#[tokio::test]
async fn search_filters_by_range_value_payment_method_and_nature() {
    let p = pool().await;
    ledger(&p).await;

    let may = data(
        &p,
        "search_transactions",
        json!({"range": {"start": "2026-05-01", "end": "2026-05-31"}}),
    )
    .await;
    assert_eq!(may["totals"]["count"], 1);
    assert_eq!(
        may["transactions"]["items"][0]["description"],
        "Mercado de maio"
    );

    let big = data(&p, "search_transactions", json!({"min_cents": 100_000})).await;
    assert_eq!(big["totals"]["count"], 3);

    let band = data(
        &p,
        "search_transactions",
        json!({"min_cents": 10_000, "max_cents": 100_000}),
    )
    .await;
    assert_eq!(band["totals"]["count"], 1);
    assert_eq!(band["transactions"]["items"][0]["description"], "Mercado");

    let pix = data(&p, "search_transactions", json!({"payment_method": "pix"})).await;
    assert_eq!(pix["totals"]["count"], 1);
    assert_eq!(pix["transactions"]["items"][0]["description"], "Farmácia");

    let fixed = data(&p, "search_transactions", json!({"nature": "fixed"})).await;
    assert_eq!(fixed["totals"]["count"], 1);
    assert_eq!(fixed["transactions"]["items"][0]["description"], "Aluguel");
    assert_eq!(fixed["transactions"]["items"][0]["movement"], "saida");

    let variable = data(&p, "search_transactions", json!({"nature": "variable"})).await;
    assert_eq!(variable["totals"]["expense_cents"], 40_000);
}

#[tokio::test]
async fn search_filters_by_tag_and_by_responsible_person() {
    let p = pool().await;
    ledger(&p).await;

    let tagged = data(
        &p,
        "search_transactions",
        json!({"tag_id": "tg-mercado", "range": {"start": "2026-01-01", "end": "2026-12-31"}}),
    )
    .await;
    assert_eq!(tagged["totals"]["count"], 2);

    let gio = data(
        &p,
        "search_transactions",
        json!({"owner_person_id": "p-gio"}),
    )
    .await;
    assert_eq!(gio["totals"]["count"], 1);
    assert_eq!(gio["transactions"]["items"][0]["description"], "Mercado");
}

/// Filtrar por conta alcança a compra no cartão: ela não aponta a conta, aponta a FATURA — e
/// "quanto gastei com este cartão" é a pergunta que a conversa precisa responder.
#[tokio::test]
async fn search_by_account_reaches_the_card_through_the_invoice() {
    let p = pool().await;
    let card = ledger_with_card(&p).await;

    let on_card = data(
        &p,
        "search_transactions",
        json!({"account_id": card, "range": {"start": "2026-07-01", "end": "2026-07-31"}}),
    )
    .await;

    assert_eq!(on_card["totals"]["count"], 1);
    assert_eq!(
        on_card["transactions"]["items"][0]["description"],
        "Restaurante"
    );
    assert_eq!(on_card["transactions"]["items"][0]["movement"], "cartao");

    let on_bank = data(&p, "search_transactions", json!({"account_id": "acc-bank"})).await;
    assert_eq!(on_bank["totals"]["count"], 2);
}

#[tokio::test]
async fn search_sort_speaks_a_controlled_vocabulary() {
    let p = pool().await;
    ledger(&p).await;

    let cheapest = data(&p, "search_transactions", json!({"sort": "amount_asc"})).await;
    let items = cheapest["transactions"]["items"].as_array().unwrap();
    assert_eq!(items[0]["description"], "Farmácia");
    assert_eq!(items[4]["description"], "Salário");

    let oldest = data(&p, "search_transactions", json!({"sort": "date_asc"})).await;
    assert_eq!(oldest["transactions"]["items"][0]["date"], "2026-07-01");

    let env = call(&p, "search_transactions", json!({"sort": "amount"})).await;
    let err = env.error.unwrap();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    // A recusa entrega o vocabulário inteiro: corrigir não depende de adivinhar a palavra.
    assert!(err.fix.contains("amount_asc"), "fix: {}", err.fix);
    assert!(err.fix.contains("date_desc"), "fix: {}", err.fix);
}

/// O agregado responde pelo FILTRO, não pela página. Uma soma da página mentiria — e é
/// exatamente a mentira que uma resposta paginada esconderia bem.
#[tokio::test]
async fn search_totals_cover_the_whole_filter_across_pages() {
    let p = pool().await;
    person(&p).await;
    for i in 0..(envelope::MAX_ROWS + 30) {
        line(
            &p,
            Line {
                id: &format!("many-{i:04}"),
                amount: 1_000,
                date: "2026-07-08",
                description: "Café",
                ..Default::default()
            },
        )
        .await;
    }

    let first = data(&p, "search_transactions", json!({})).await;
    assert_eq!(first["transactions"]["returned"], envelope::MAX_ROWS);
    assert_eq!(first["transactions"]["total"], envelope::MAX_ROWS + 30);
    assert_eq!(
        first["totals"]["expense_cents"],
        (envelope::MAX_ROWS as i64 + 30) * 1_000
    );
    let cursor = first["transactions"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();

    let second = data(&p, "search_transactions", json!({"cursor": cursor})).await;
    assert_eq!(second["transactions"]["returned"], 30);
    assert_eq!(second["transactions"]["next_cursor"], Value::Null);
    assert_eq!(second["totals"], first["totals"]);
}

#[tokio::test]
async fn search_refuses_a_cursor_from_another_filter() {
    let p = pool().await;
    person(&p).await;
    for i in 0..(envelope::MAX_ROWS + 5) {
        line(
            &p,
            Line {
                id: &format!("many-{i:04}"),
                amount: 1_000,
                date: "2026-07-08",
                description: "Café",
                ..Default::default()
            },
        )
        .await;
    }

    let first = data(&p, "search_transactions", json!({})).await;
    let cursor = first["transactions"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_string();

    let env = call(
        &p,
        "search_transactions",
        json!({"cursor": cursor, "sort": "amount_asc"}),
    )
    .await;
    let err = env.error.unwrap();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("cursor"), "fix: {}", err.fix);
}

#[tokio::test]
async fn search_expands_tags_items_and_owners_only_with_include() {
    let p = pool().await;
    ledger(&p).await;
    sqlx::query(
        "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
         VALUES ('li-1', 'lx-mercado', 32000, 'Feira', 0, 'CONTAS:')",
    )
    .execute(&p)
    .await
    .unwrap();

    let lean = data(&p, "search_transactions", json!({"tag_id": "tg-mercado"})).await;
    let line = &lean["transactions"]["items"][0];
    assert!(line.get("tags").is_none());
    assert!(line.get("items").is_none());
    assert!(line.get("owners").is_none());

    let full = data(
        &p,
        "search_transactions",
        json!({"tag_id": "tg-mercado", "include": ["tags", "items", "owners"]}),
    )
    .await;
    let line = &full["transactions"]["items"][0];
    // Tag e pessoa vêm com identidade: é por ela que a próxima pergunta filtra, e nenhuma outra
    // ferramenta entrega o id de uma pessoa.
    assert_eq!(line["tags"][0]["id"], "tg-mercado");
    assert_eq!(line["tags"][0]["name"], "Mercado");
    assert_eq!(line["items"][0]["description"], "Feira");
    assert_eq!(line["items"][0]["amount_cents"], 32_000);
    assert_eq!(line["owners"][0]["person_id"], "p-gio");
    assert_eq!(line["owners"][0]["name"], "Gio");
    assert_eq!(line["owners"][0]["amount_cents"], 12_000);
}

// --- O que já está comprometido ----------------------------------------------------------

#[tokio::test]
async fn commitments_group_by_series_with_the_installment_read_from_the_cycle() {
    let p = pool().await;
    commitments(&p).await;

    let env = call(
        &p,
        "get_commitments",
        json!({"range": {"start": "2026-08-01", "end": "2026-12-31"}}),
    )
    .await;
    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-08-01");
    let ahead = env.data.unwrap();

    let series = ahead["card_series"]["items"].as_array().unwrap();
    let notebook = series
        .iter()
        .find(|s| s["description"] == "Notebook")
        .expect("o parcelamento está entre os compromissos");

    assert_eq!(notebook["kind"], "installment");
    assert_eq!(notebook["card_name"], "Cartão");
    assert_eq!(notebook["amount_cents"], 100_000);
    // A primeira parcela venceu em 15/07, fora do recorte: restam cinco de seis.
    assert_eq!(notebook["occurrences_in_range"], 5);
    assert_eq!(notebook["committed_cents"], 500_000);
    assert_eq!(notebook["installments_total"], 6);
    // A parcela n/N sai do índice do CICLO, nunca de um contador guardado na linha.
    assert_eq!(notebook["next"]["cycle_month"], "2026-08");
    assert_eq!(notebook["next"]["due_date"], "2026-08-15");
    assert_eq!(notebook["next"]["installment_index"], 2);

    // A assinatura não tem fim: dizer "5 de 5" mentiria sobre o que vem depois de dezembro.
    let streaming = series
        .iter()
        .find(|s| s["description"] == "Streaming")
        .expect("a assinatura está entre os compromissos");
    assert_eq!(streaming["kind"], "subscription");
    assert_eq!(streaming["installments_total"], Value::Null);
    assert_eq!(streaming["next"]["installment_index"], Value::Null);
}

#[tokio::test]
async fn commitments_show_the_refund_linked_to_a_series() {
    let p = pool().await;
    commitments(&p).await;

    let ahead = data(
        &p,
        "get_commitments",
        json!({"range": {"start": "2026-07-01", "end": "2026-12-31"}}),
    )
    .await;
    let series = ahead["card_series"]["items"].as_array().unwrap();

    let notebook = series
        .iter()
        .find(|s| s["description"] == "Notebook")
        .unwrap();
    assert_eq!(notebook["refund"]["linked"], true);
    assert_eq!(notebook["refund"]["expected_cents"], 50_000);

    let streaming = series
        .iter()
        .find(|s| s["description"] == "Streaming")
        .unwrap();
    assert_eq!(streaming["refund"]["linked"], false);
    assert_eq!(streaming["refund"]["expected_cents"], 0);
}

#[tokio::test]
async fn commitments_carry_the_recurring_series_of_the_ledger_and_the_total_of_the_range() {
    let p = pool().await;
    commitments(&p).await;

    let ahead = data(
        &p,
        "get_commitments",
        json!({"range": {"start": "2026-08-01", "end": "2026-12-31"}}),
    )
    .await;

    let academia = &ahead["recurring"]["items"][0];
    assert_eq!(academia["description"], "Academia");
    assert_eq!(academia["frequency"], "mensal");
    assert_eq!(academia["amount_cents"], 120_000);
    assert_eq!(academia["occurrences_in_range"], 5);
    assert_eq!(academia["committed_cents"], 600_000);
    assert_eq!(academia["next"]["date"], "2026-08-05");
    assert_eq!(academia["next"]["installment_index"], 1);
    assert_eq!(academia["installments_total"], 6);

    // O total cobre o recorte inteiro: parcelas (500.000) + assinatura (15.000) + série (600.000).
    assert_eq!(ahead["committed_cents"], 1_115_000);
}

#[tokio::test]
async fn commitments_default_to_the_twelve_months_ahead() {
    let p = pool().await;
    commitments(&p).await;

    let env = call(&p, "get_commitments", json!({})).await;

    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-07-25");
    assert_eq!(env.meta.period.end, "2027-07-25");
}

#[tokio::test]
async fn commitments_answer_by_obligation_with_the_history_and_the_typical_month() {
    let p = pool().await;
    ledger(&p).await;
    let obligation = rent_obligation(&p).await;

    let env = call(
        &p,
        "get_commitments",
        json!({"obligation_id": obligation, "range": {"start": "2026-01-01", "end": "2026-12-31"}}),
    )
    .await;
    assert!(env.ok, "{:?}", env.error);
    let rent = env.data.unwrap();

    assert_eq!(rent["obligation"]["name"], "Aluguel");
    assert_eq!(rent["obligation"]["kind"], "saida");
    // 180.000 · 190.000 · 200.000 — o típico é a mediana, o mesmo estimador das réguas.
    assert_eq!(rent["typical_cents"], 190_000);
    assert_eq!(rent["last"]["month"], "2026-07");
    assert_eq!(rent["last"]["total_cents"], 200_000);
    // A diferença contra o típico vem pronta: subir de aluguel é a pergunta, não a conta.
    assert_eq!(rent["delta_vs_typical"]["cents"], 10_000);
    assert_eq!(rent["delta_vs_typical"]["change_bps"], 526);
    assert_eq!(rent["committed_cents"], 570_000);

    let months = rent["months"]["items"].as_array().unwrap();
    assert_eq!(months.len(), 3);
    assert_eq!(months[0]["month"], "2026-05");
    assert_eq!(months[0]["total_cents"], 180_000);
    assert_eq!(months[0]["count"], 1);
}

#[tokio::test]
async fn commitments_list_the_obligations_that_touch_the_range() {
    let p = pool().await;
    ledger(&p).await;
    rent_obligation(&p).await;

    let ahead = data(
        &p,
        "get_commitments",
        json!({"range": {"start": "2026-06-01", "end": "2026-12-31"}}),
    )
    .await;
    let obligations = ahead["obligations"]["items"].as_array().unwrap();

    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0]["name"], "Aluguel");
    // Junho e julho caem no recorte; maio ficou de fora.
    assert_eq!(obligations[0]["occurrences_in_range"], 2);
    assert_eq!(obligations[0]["committed_cents"], 390_000);
}

#[tokio::test]
async fn commitments_occurrences_only_with_include() {
    let p = pool().await;
    commitments(&p).await;

    let range = json!({"start": "2026-08-01", "end": "2026-09-30"});
    let lean = data(&p, "get_commitments", json!({"range": range})).await;
    assert!(lean.get("occurrences").is_none());

    let full = data(
        &p,
        "get_commitments",
        json!({"range": {"start": "2026-08-01", "end": "2026-09-30"}, "include": ["occurrences"]}),
    )
    .await;
    let occurrences = full["occurrences"]["items"].as_array().unwrap();

    // Em ordem de data: a série do dia 5, depois as duas do vencimento do dia 15.
    assert_eq!(occurrences[0]["date"], "2026-08-05");
    assert_eq!(occurrences[0]["description"], "Academia");
    assert_eq!(occurrences[0]["source"], "recurrence");
    assert_eq!(occurrences[0]["installment_index"], 1);
    let notebook = occurrences
        .iter()
        .find(|o| o["description"] == "Notebook")
        .expect("a parcela do notebook está entre as ocorrências");
    assert_eq!(notebook["date"], "2026-08-15");
    assert_eq!(notebook["source"], "card_series");
    assert_eq!(notebook["installment_index"], 2);
    assert_eq!(notebook["amount_cents"], 100_000);
}

#[tokio::test]
async fn commitments_name_the_obligation_that_does_not_exist() {
    let p = pool().await;
    commitments(&p).await;

    let env = call(
        &p,
        "get_commitments",
        json!({"obligation_id": "ob-fantasma"}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(!err.fix.is_empty());
}

// --- As tags como interruptores de régua -------------------------------------------------

#[tokio::test]
async fn tags_publish_the_switches_of_each_ruler_and_what_the_tag_moved() {
    let p = pool().await;
    ledger(&p).await;
    tag_outside_cost(&p, "tg-terceiro", "Da outra pessoa").await;
    tag_on(&p, "lx-aluguel", "tg-terceiro").await;

    let env = call(&p, "get_tags", json!({})).await;
    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-07-01");
    assert_eq!(env.meta.period.end, "2026-07-31");
    let tags = env.data.unwrap();

    assert_eq!(tags["month"], "2026-07");
    assert_eq!(tags["has_exceptions"], true);
    let items = tags["tags"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    // A tag mais pesada do mês vem primeiro, com os quatro interruptores explícitos: a tag
    // decide em quais réguas o lançamento conta — ela não é um envelope de orçamento.
    let outside = &items[0];
    assert_eq!(outside["id"], "tg-terceiro");
    assert_eq!(outside["month_total_cents"], 200_000);
    assert_eq!(outside["transaction_count"], 1);
    assert_eq!(outside["counts_in"]["cost_of_living"], false);
    assert_eq!(outside["counts_in"]["performance"], true);
    assert_eq!(outside["counts_in"]["savings"], true);
    assert_eq!(outside["counts_in"]["daily_avg"], true);

    let mercado = &items[1];
    assert_eq!(mercado["id"], "tg-mercado");
    assert_eq!(mercado["month_total_cents"], 32_000);
    assert_eq!(mercado["counts_in"]["cost_of_living"], true);
}

/// O custo com os interruptores de hoje ao lado do custo se todas as tags contassem: é a conta
/// que mostra o preço das exceções sem que ninguém precise fazê-la.
#[tokio::test]
async fn tags_print_the_cost_with_and_without_the_exceptions() {
    let p = pool().await;
    ledger(&p).await;
    tag_outside_cost(&p, "tg-terceiro", "Da outra pessoa").await;
    tag_on(&p, "lx-aluguel", "tg-terceiro").await;

    let tags = data(&p, "get_tags", json!({})).await;

    // Julho: aluguel 200.000 + mercado 32.000 + farmácia 8.000. O aluguel está fora do custo.
    assert_eq!(tags["cost_of_living_cents"], 40_000);
    assert_eq!(tags["cost_if_every_tag_counted_cents"], 240_000);
}

#[tokio::test]
async fn tags_answer_the_month_asked() {
    let p = pool().await;
    ledger(&p).await;

    let env = call(&p, "get_tags", json!({"month": "2026-05"})).await;
    assert!(env.ok, "{:?}", env.error);
    assert_eq!(env.meta.period.start, "2026-05-01");
    assert_eq!(env.meta.period.end, "2026-05-31");
    let tags = env.data.unwrap();

    assert_eq!(tags["month"], "2026-05");
    assert_eq!(tags["tags"]["items"][0]["month_total_cents"], 45_000);
}

#[tokio::test]
async fn tags_effects_come_computed_only_with_include() {
    let p = pool().await;
    ledger(&p).await;
    tag_outside_cost(&p, "tg-terceiro", "Da outra pessoa").await;
    tag_on(&p, "lx-aluguel", "tg-terceiro").await;

    let lean = data(&p, "get_tags", json!({})).await;
    assert!(lean.get("effects").is_none());

    let full = data(&p, "get_tags", json!({"include": ["effects"]})).await;
    let effects = full["effects"]["items"].as_array().unwrap();
    let outside = effects
        .iter()
        .find(|e| e["tag_id"] == "tg-terceiro")
        .expect("efeitos da tag com exceção");

    // Ligar o interruptor de custo devolveria o aluguel ao custo de vida; a Performance mexe
    // pelo líquido, e as duas contas são diferentes por construção.
    assert_eq!(outside["cost_delta_cents"], 200_000);
    assert!(outside["performance_delta_cents"].is_i64());
}

#[tokio::test]
async fn tags_third_parties_only_with_include_and_carry_the_person() {
    let p = pool().await;
    ledger(&p).await;

    let lean = data(&p, "get_tags", json!({})).await;
    assert!(lean.get("third_parties").is_none());
    // A manchete do dinheiro de terceiros fica no default: são dois números, não uma lista.
    assert_eq!(lean["third_party_people"], 1);

    let full = data(&p, "get_tags", json!({"include": ["third_parties"]})).await;
    let line = &full["third_parties"]["items"][0];

    assert_eq!(line["person_id"], "p-gio");
    assert_eq!(line["name"], "Gio");
    assert_eq!(line["out_cents"], 12_000);
    assert!(!line["state"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn search_refuses_a_payment_method_that_is_not_one() {
    let p = pool().await;
    ledger(&p).await;

    let env = call(
        &p,
        "search_transactions",
        json!({"payment_method": "boleto"}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("credit"), "fix: {}", err.fix);
}

// --- A hipótese efêmera -----------------------------------------------------------------

#[tokio::test]
async fn simulation_does_not_write_to_any_user_table() {
    let p = pool().await;
    timeline(&p).await;

    // A garantia da hipótese é que nada é gravado; fotografar só duas tabelas deixaria uma
    // escrita acidental em qualquer outra tabela passar sem ser percebida.
    let before = user_table_row_counts(&p).await;

    let scenario = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-07-28"
            }]
        }),
    )
    .await;

    let after = user_table_row_counts(&p).await;

    assert_eq!(scenario["ephemeral"], true);
    for table in before.keys().chain(after.keys()) {
        let before_count = before.get(table).copied().unwrap_or_default();
        let after_count = after.get(table).copied().unwrap_or_default();
        let entered = (after_count - before_count).max(0);
        let exited = (before_count - after_count).max(0);
        assert_eq!(
            after_count, before_count,
            "A simulação alterou a tabela \"{table}\": {entered} linha(s) entraram; {exited} saíram."
        );
    }
}

#[tokio::test]
async fn simulation_uses_a_unique_namespace_for_hypothetical_line_ids() {
    let p = pool().await;
    // A linha histórica está fora da projeção, mas seu item continuaria acessível por uma busca
    // de nota que usasse o mesmo id sintético da hipótese.
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
         VALUES ('hipotese:0', 'expense', 50000, '2026-06-10', 1, 0)",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
         VALUES ('item-historico', 'hipotese:0', 50000, 'Reserva', 0, 'ECONOMIA:')",
    )
    .execute(&p)
    .await
    .unwrap();

    let scenario = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-07-28"
            }]
        }),
    )
    .await;

    assert_eq!(scenario["delta"]["cost_of_living_cents"], 50_000);
}

#[tokio::test]
async fn high_outflow_hypothesis_lowers_safe_to_spend_and_lowest_balance() {
    let p = pool().await;
    world(&p).await;
    sqlx::query("UPDATE account SET balance = 2000000 WHERE id = 'acc-bank'")
        .execute(&p)
        .await
        .unwrap();
    transfer(&p, "ec-reserva", 800_000, "2026-06-28", "acc-reserve").await;

    let scenario = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 5_000_000,
                "date": "2026-07-26"
            }]
        }),
    )
    .await;

    let delta = scenario["delta"]["safe_to_spend_cents"]
        .as_i64()
        .expect("a diferença de safe-to-spend sai em centavos");
    let baseline = scenario["baseline"]["lowest_balance"]["balance_cents"]
        .as_i64()
        .expect("o mundo base tem saldo mínimo");
    let hypothesis = scenario["hypothesis"]["lowest_balance"]["balance_cents"]
        .as_i64()
        .expect("a hipótese tem saldo mínimo");

    assert!(delta < 0, "delta: {delta}");
    assert!(
        hypothesis < baseline,
        "hipótese: {hypothesis}; base: {baseline}"
    );
}

#[tokio::test]
async fn repeating_months_materializes_dates_in_the_tool() {
    let p = pool().await;
    world(&p).await;

    let scenario = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-07-31",
                "repeat_months": 3
            }]
        }),
    )
    .await;
    let lines = scenario["lines"]["items"].as_array().unwrap();

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["date"], "2026-07-31");
    assert_eq!(lines[1]["date"], "2026-08-31");
    assert_eq!(lines[2]["date"], "2026-09-30");
}

#[tokio::test]
async fn simulation_refuses_movement_outside_the_vocabulary() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "investimento",
                "amount_cents": 50_000,
                "date": "2026-07-28"
            }]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    for movement in [
        "entrada",
        "saida",
        "diario",
        "cartao",
        "economia",
        "patrimonio",
    ] {
        assert!(err.fix.contains(movement), "fix: {}", err.fix);
    }
}

#[tokio::test]
async fn simulation_refuses_dates_before_current_month() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-06-30"
            }]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("get_month_analysis"), "fix: {}", err.fix);
}

#[tokio::test]
async fn simulation_refuses_a_change_beyond_the_ten_year_horizon() {
    let p = pool().await;
    let limit = "2036-07-25";
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2036-07-26"
            }]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.message.contains(limit), "message: {}", err.message);
}

#[tokio::test]
async fn simulation_refuses_a_repetition_that_crosses_the_ten_year_horizon() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2036-07-25",
                "repeat_months": 2
            }]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(
        err.message.contains("2036-07-25"),
        "message: {}",
        err.message
    );
}

#[tokio::test]
async fn simulation_refuses_an_amount_above_the_line_limit() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 100_000_000_001i64,
                "date": "2026-07-28"
            }]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(
        err.message.contains("R$ 1.000.000.000,00"),
        "message: {}",
        err.message
    );
}

#[tokio::test]
async fn simulation_refuses_a_description_with_the_wrong_type_and_names_its_position() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [
                {
                    "movement": "saida",
                    "amount_cents": 50_000,
                    "date": "2026-07-28"
                },
                {
                    "movement": "saida",
                    "amount_cents": 50_000,
                    "date": "2026-07-29",
                    "description": 42
                }
            ]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(
        err.message.contains("mudança #2"),
        "message: {}",
        err.message
    );
    assert!(err.fix.contains("description"), "fix: {}", err.fix);
}

#[tokio::test]
async fn simulation_refuses_unknown_change_field_with_position_and_accepted_fields() {
    let p = pool().await;
    let env = call(
        &p,
        "simulate_scenario",
        json!({
            "changes": [
                {
                    "movement": "saida",
                    "amount_cents": 50_000,
                    "date": "2026-07-28"
                },
                {
                    "movement": "saida",
                    "amount_cents": 50_000,
                    "date": "2026-07-29",
                    "conta": "acc-bank"
                }
            ]
        }),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(
        err.message.contains("mudança #2"),
        "message: {}",
        err.message
    );
    for field in [
        "movement",
        "amount_cents",
        "date",
        "repeat_months",
        "description",
    ] {
        assert!(err.fix.contains(field), "fix: {}", err.fix);
    }
}

#[tokio::test]
async fn simulation_refuses_empty_hypothesis_with_ready_example() {
    let p = pool().await;
    let env = call(&p, "simulate_scenario", json!({"changes": []})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.fix.contains("changes: [{"), "fix: {}", err.fix);
    assert!(err.fix.contains("saida"), "fix: {}", err.fix);
}

#[tokio::test]
async fn simulation_only_returns_month_end_with_include() {
    let p = pool().await;
    world(&p).await;

    let lean = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-07-28"
            }]
        }),
    )
    .await;
    assert!(lean.get("month_end").is_none());

    let expanded = data(
        &p,
        "simulate_scenario",
        json!({
            "changes": [{
                "movement": "saida",
                "amount_cents": 50_000,
                "date": "2026-07-28"
            }],
            "include": ["month_end"]
        }),
    )
    .await;
    assert!(expanded.get("month_end").is_some());
}

// --- O método explicado -----------------------------------------------------------------

#[tokio::test]
async fn method_guidance_serves_requested_chapter_with_method_provenance() {
    let p = pool().await;

    let guidance = data(&p, "get_method_guidance", json!({"topic": "diario"})).await;

    assert_eq!(guidance["topic"], "diario");
    assert_eq!(guidance["provenance"], "metodo");
    assert_eq!(guidance["title"], "Capítulo de diario");
    assert!(
        guidance["content"]
            .as_str()
            .unwrap()
            .contains("Orientação sintética do método para a suíte."),
    );
}

#[tokio::test]
async fn method_guidance_defaults_to_overview() {
    let p = pool().await;

    let guidance = data(&p, "get_method_guidance", json!({})).await;

    assert_eq!(guidance["topic"], "metodo");
    assert_eq!(guidance["provenance"], "metodo");
}

#[tokio::test]
async fn method_guidance_refuses_topic_outside_vocabulary() {
    let p = pool().await;
    let env = call(&p, "get_method_guidance", json!({"topic": "investimentos"})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::InvalidArgument);
    for topic in method_tools::TOPICS {
        assert!(err.fix.contains(topic), "fix: {}", err.fix);
    }
}

#[tokio::test]
async fn method_guidance_without_pack_hides_absolute_path() {
    let p = pool().await;
    let pack = TempPack::absent();
    let absolute_path = pack.path().display().to_string();
    assert!(!pack.path().exists());

    let env = call_with_pack(&p, pack.path(), "get_method_guidance", json!({})).await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(
        !err.message.contains(&absolute_path),
        "message: {}",
        err.message
    );
}

#[tokio::test]
async fn method_guidance_privacy_gate_blocks_without_echoing_forbidden_term() {
    let p = pool().await;
    let term = "segredo-plantado-da-suíte";
    let pack = TempPack::new();
    pack.chapter("diario", &format!("# Diário\n\n{term}\n"));
    pack.root_file("forbidden-extra.txt", term);

    let env = call_with_pack(
        &p,
        pack.path(),
        "get_method_guidance",
        json!({"topic": "diario"}),
    )
    .await;
    let err = env.error.unwrap();

    assert_eq!(err.code, ErrorCode::PrivacyBlocked);
    assert!(!err.message.contains(term), "message: {}", err.message);
    assert!(!err.fix.contains(term), "fix: {}", err.fix);
}

#[tokio::test]
async fn method_guidance_blocks_a_deny_list_with_a_whitespace_only_entry() {
    let p = pool().await;
    let pack = TempPack::new();
    pack.chapter("metodo", "# Método\n\nOrientação sintética.\n");
    pack.root_file("forbidden-extra.txt", " \t \n");

    let env = call_with_pack(&p, pack.path(), "get_method_guidance", json!({})).await;
    let err = env.error.unwrap();

    assert!(!env.ok);
    assert!(env.data.is_none());
    assert_eq!(err.code, ErrorCode::PrivacyBlocked);
    assert!(
        err.message.contains("forbidden-extra.txt"),
        "message: {}",
        err.message
    );
    assert!(err.message.contains("#1"), "message: {}", err.message);
}

#[tokio::test]
async fn method_guidance_fails_closed_without_deny_list() {
    let p = pool().await;
    let pack = TempPack::new();
    pack.chapter("metodo", "# Método\n\nOrientação sintética.\n");

    let env = call_with_pack(&p, pack.path(), "get_method_guidance", json!({})).await;

    assert!(!env.ok);
    assert!(env.data.is_none());
    assert_eq!(env.error.unwrap().code, ErrorCode::PrivacyBlocked);
}
