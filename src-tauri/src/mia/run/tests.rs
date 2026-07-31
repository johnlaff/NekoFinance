//! Suíte das invariantes puras do laço da conversa.

use super::grounding::{FactOrigin, Facts, cites_data, orphans};
use super::redaction::{REDACTED, credentials};
use crate::mia::envelope::{CURRENCY, Envelope, ErrorCode, MAX_ROWS, Meta, Period, ToolError};
use serde_json::{Value, json};

fn envelope(data: Value) -> Envelope {
    Envelope {
        tool: "get_financial_snapshot".to_string(),
        ok: true,
        meta: Meta {
            currency: CURRENCY,
            timezone: "-03:00".to_string(),
            period: Period {
                start: "2026-07-01".to_string(),
                end: "2026-07-31".to_string(),
            },
            as_of: "2026-07-26T09:00:00-03:00".to_string(),
            data_revision: Some("fixture-revision".to_string()),
            row_limit: MAX_ROWS,
        },
        data: Some(data),
        error: None,
    }
}

#[test]
fn redaction_removes_api_key_prefixes() {
    let token = "sk_1234567890abcdef";

    assert_eq!(credentials(token), REDACTED);
    assert_eq!(credentials("SK_1234567890abcdef"), REDACTED);
}

#[test]
fn redaction_removes_bearer_tokens() {
    assert_eq!(
        credentials("bEaReR   token-1234"),
        format!("bEaReR   {REDACTED}")
    );
}

#[test]
fn redaction_removes_api_key_header_values() {
    assert_eq!(
        credentials("X-Api-Key: secret-value\ncontinua"),
        format!("X-Api-Key: {REDACTED}\ncontinua")
    );
}

#[test]
fn redaction_keeps_clean_text_unchanged() {
    let text = "O provedor recusou a chamada por limite de taxa.";

    assert_eq!(credentials(text), text);
}

#[test]
fn redaction_removes_two_credentials_from_a_provider_message() {
    assert_eq!(
        credentials("erro 401: Bearer bearer_token_1234 sk-1234567890abcdef"),
        format!("erro 401: Bearer {REDACTED} {REDACTED}")
    );
}

#[test]
fn grounding_accepts_reais_read_from_cents() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"amount_cents": 810158})), FactOrigin::Data);

    assert!(orphans("Você gastou R$ 8.101,58.", &facts).is_empty());
}

#[test]
fn grounding_accepts_percentages_read_from_basis_points() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"savings_bps": 3012})), FactOrigin::Data);

    assert!(orphans("A economia foi de 30,12%, acima dos 30%.", &facts).is_empty());
}

#[test]
fn grounding_reports_an_invented_number() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"amount_cents": 810158})), FactOrigin::Data);

    assert_eq!(
        orphans("Você gastou R$ 9.999,99.", &facts),
        vec!["9.999,99"]
    );
}

#[test]
fn grounding_ignores_numbers_planted_in_a_tool_error() {
    let mut failed = envelope(json!({"amount_cents": 810158}));
    failed.ok = false;
    failed.data = None;
    failed.error = Some(ToolError::new(
        ErrorCode::UnknownTool,
        "A ferramenta citada pelo modelo trazia o número 999.",
        "Escolha uma ferramenta disponível.",
    ));
    let mut facts = Facts::new();
    facts.absorb_envelope(&failed, FactOrigin::Data);

    assert_eq!(orphans("O valor é 999.", &facts), vec!["999"]);
}

#[test]
fn grounding_accepts_a_number_from_the_method_prefix() {
    let mut facts = Facts::new();
    facts.absorb_text("A faixa anual é 20–30% da renda.");

    assert!(orphans("A faixa recomendada segue entre 20% e 30%.", &facts).is_empty());
}

#[test]
fn grounding_accepts_a_date_from_envelope_metadata() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"status": "consultado"})), FactOrigin::Data);

    assert!(orphans("Os dados são de 2026-07-01.", &facts).is_empty());
}

#[test]
fn grounding_reports_orphans_once_in_appearance_order() {
    let facts = Facts::new();

    assert_eq!(
        orphans("Os valores são 12, 7 e 12 novamente.", &facts),
        vec!["12", "7"]
    );
}

#[test]
fn grounding_reports_scientific_notation_as_an_orphan() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"amount_cents": 300})), FactOrigin::Data);

    assert_eq!(orphans("O saldo é R$ 3e3.", &facts), vec!["3e3"]);
}

#[test]
fn grounding_cites_a_number_only_data_supports() {
    let mut facts = Facts::new();
    facts.absorb_text("A faixa anual recomendada é 20–30%.");
    facts.absorb_envelope(&envelope(json!({"amount_cents": 810158})), FactOrigin::Data);

    assert!(cites_data("O saldo lido é R$ 8.101,58.", &facts));
}

#[test]
fn grounding_does_not_count_a_number_the_method_prefix_also_supports() {
    let mut facts = Facts::new();
    facts.absorb_text("A faixa anual recomendada é 30%.");
    facts.absorb_envelope(&envelope(json!({"amount_cents": 3000})), FactOrigin::Data);

    assert!(!cites_data("A faixa continua em 30%.", &facts));
}

#[test]
fn grounding_does_not_count_a_number_the_method_tool_also_supports() {
    let mut facts = Facts::new();
    facts.absorb_envelope(
        &envelope(json!({"recommended_percentage_bps": 3000})),
        FactOrigin::Method,
    );
    facts.absorb_envelope(&envelope(json!({"amount_cents": 3000})), FactOrigin::Data);

    assert!(!cites_data("A faixa continua em 30%.", &facts));
}

#[test]
fn grounding_does_not_count_envelope_metadata_as_data() {
    let mut facts = Facts::new();
    facts.absorb_envelope(&envelope(json!({"status": "consultado"})), FactOrigin::Data);

    assert!(!cites_data("Os dados são de 2026-07-01.", &facts));
}

use super::{
    AnswerProvenance, CancelToken, ProviderAdapter, RetryDecision, Round, RunErrorCode, RunEvent,
    RunLimits, Runner, StopReason, TraceKind, retry_decision,
};
use crate::mia::Context;
use crate::mia::catalog::{self, METHOD_LAYER_TOOL};
use crate::mia::envelope::Clock;
use crate::mia::key_store::ApiKey;
use crate::mia::method_tools::MethodPack;
use crate::mia::provider::pins::default_pin;
use crate::mia::provider::request::RunSpec;
use crate::mia::provider::stream::{ErrorKind, FinishReason, ProviderError, ProviderEvent, Usage};
use chrono::DateTime;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::VecDeque;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

enum Script {
    Events(Vec<ProviderEvent>),
    OpenError(ProviderError),
    EventsThenFailure {
        events: Vec<ProviderEvent>,
        failure: ProviderError,
    },
    Hang,
}

#[derive(Debug)]
struct ObservedRun {
    messages: Vec<Value>,
}

struct ScriptedAdapter {
    scripts: Mutex<VecDeque<Script>>,
    observed: Mutex<Vec<ObservedRun>>,
    connection_closed: Arc<AtomicBool>,
    open_calls: AtomicUsize,
}

impl ScriptedAdapter {
    fn new(scripts: impl IntoIterator<Item = Script>) -> Self {
        Self {
            scripts: Mutex::new(scripts.into_iter().collect()),
            observed: Mutex::new(vec![]),
            connection_closed: Arc::new(AtomicBool::new(false)),
            open_calls: AtomicUsize::new(0),
        }
    }

    fn transcripts(&self) -> Vec<Vec<Value>> {
        self.observed
            .lock()
            .expect("o adaptador de teste mantém o histórico acessível")
            .iter()
            .map(|run| run.messages.clone())
            .collect()
    }

    fn connection_closed(&self) -> bool {
        self.connection_closed.load(Ordering::SeqCst)
    }

    fn open_calls(&self) -> usize {
        self.open_calls.load(Ordering::SeqCst)
    }

    async fn wait_for_open(&self) {
        tokio::time::timeout(Duration::from_millis(100), async {
            while self.open_calls() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("o adaptador deveria receber uma abertura de turno");
    }

    async fn wait_for_connection_close(&self) {
        tokio::time::timeout(Duration::from_millis(100), async {
            while !self.connection_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("o cancelamento deveria chegar à conexão do provedor");
    }
}

impl ProviderAdapter for ScriptedAdapter {
    fn open(
        &self,
        spec: &RunSpec<'_>,
        cancel: &CancelToken,
    ) -> impl Future<Output = Result<mpsc::Receiver<ProviderEvent>, ProviderError>> + Send {
        self.observed
            .lock()
            .expect("o adaptador de teste guarda os pedidos vistos")
            .push(ObservedRun {
                messages: spec.messages.to_vec(),
            });
        self.open_calls.fetch_add(1, Ordering::SeqCst);

        let script = self
            .scripts
            .lock()
            .expect("o adaptador de teste consome um roteiro por turno")
            .pop_front()
            .unwrap_or_else(|| {
                Script::OpenError(ProviderError {
                    kind: ErrorKind::Permanent,
                    message: "Não há roteiro para este turno.".to_string(),
                    responded: true,
                })
            });
        let cancel = cancel.clone();
        let connection_closed = Arc::clone(&self.connection_closed);

        async move {
            match script {
                Script::OpenError(error) => Err(error),
                Script::Events(events) => {
                    let receiver = spawn_events(events, None, cancel, connection_closed);
                    Ok(receiver)
                }
                Script::EventsThenFailure { events, failure } => {
                    let receiver = spawn_events(events, Some(failure), cancel, connection_closed);
                    Ok(receiver)
                }
                Script::Hang => {
                    let (sender, receiver) = mpsc::channel(1);
                    tokio::spawn(async move {
                        cancel.cancelled().await;
                        drop(sender);
                        connection_closed.store(true, Ordering::SeqCst);
                    });
                    Ok(receiver)
                }
            }
        }
    }
}

fn spawn_events(
    events: Vec<ProviderEvent>,
    failure: Option<ProviderError>,
    cancel: CancelToken,
    connection_closed: Arc<AtomicBool>,
) -> mpsc::Receiver<ProviderEvent> {
    let capacity = (events.len() + usize::from(failure.is_some())).max(1);
    let (sender, receiver) = mpsc::channel(capacity);
    tokio::spawn(async move {
        tokio::select! {
            _ = cancel.cancelled() => connection_closed.store(true, Ordering::SeqCst),
            sent = send_events(sender, events, failure) => {
                if !sent {
                    connection_closed.store(true, Ordering::SeqCst);
                }
            }
        }
    });
    receiver
}

async fn send_events(
    sender: mpsc::Sender<ProviderEvent>,
    events: Vec<ProviderEvent>,
    failure: Option<ProviderError>,
) -> bool {
    for event in events {
        if sender.send(event).await.is_err() {
            return false;
        }
    }
    failure.is_none_or(|error| sender.try_send(ProviderEvent::Failed(error)).is_ok())
}

struct TestPack {
    root: PathBuf,
}

impl TestPack {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("neko-finance-run-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("o diretório temporário do pack deve existir");
        std::fs::create_dir_all(root.join("chapters"))
            .expect("o diretório de capítulos do pack deve existir");
        std::fs::write(
            root.join("chapters/metodo.md"),
            "# Método\n\nOrientação sintética do método para a suíte.\n",
        )
        .expect("o capítulo do método deve existir");
        std::fs::write(
            root.join("forbidden-fixture.txt"),
            "termo-ausente-da-fixture\n",
        )
        .expect("a deny-list do pack deve existir");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TestPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("o pool SQLite em memória deve abrir");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("as migrações devem preparar o pool de teste");
    pool
}

fn clock() -> Clock {
    Clock::at(
        DateTime::parse_from_rfc3339("2026-07-25T09:00:00-03:00")
            .expect("o relógio fixo da suíte é RFC 3339"),
    )
}

fn limits() -> RunLimits {
    RunLimits {
        retry_backoff: Duration::ZERO,
        ..RunLimits::default()
    }
}

fn round() -> Round<'static> {
    Round {
        system: "Responda usando apenas fatos da rodada.",
        history: &[],
        question: "Como estou?",
    }
}

fn usage(cost_micro_usd: Option<i64>) -> ProviderEvent {
    ProviderEvent::Usage(Usage {
        prompt_tokens: 10,
        completion_tokens: 5,
        cost_micro_usd,
    })
}

fn finished(reason: FinishReason) -> ProviderEvent {
    ProviderEvent::Finished {
        reason,
        native: None,
    }
}

fn tool_call(id: &str, name: &str, arguments: &str) -> ProviderEvent {
    ProviderEvent::ToolCallComplete {
        id: id.to_string(),
        name: name.to_string(),
        arguments: arguments.to_string(),
    }
}

/// Erro COM resposta do servidor — o caso que não mexe na contagem de dinheiro. Falha sem
/// resposta se constrói explícita no teste que a exercita.
fn provider_error(kind: ErrorKind, message: &str) -> ProviderError {
    ProviderError {
        kind,
        message: message.to_string(),
        responded: true,
    }
}

async fn execute(
    adapter: &ScriptedAdapter,
    limits: RunLimits,
    cancel: CancelToken,
) -> (super::RunOutcome, Vec<RunEvent>) {
    execute_round(adapter, limits, cancel, round()).await
}

async fn execute_round(
    adapter: &ScriptedAdapter,
    limits: RunLimits,
    cancel: CancelToken,
    round: Round<'_>,
) -> (super::RunOutcome, Vec<RunEvent>) {
    let pool = pool().await;
    crate::mia::consent::grant(&pool, default_pin(), "2026-07-25T12:00:00Z")
        .await
        .expect("o consentimento do roteiro deve gravar");
    let pack = TestPack::new();
    let ctx = Context {
        clock: clock(),
        pack: MethodPack::at(pack.path()),
    };
    let (events, mut receiver) = mpsc::channel(32);
    let runner = Runner {
        pool: &pool,
        ctx: &ctx,
        adapter,
        pin: default_pin(),
        limits,
        cancel,
        events,
    };
    let outcome = runner.run(round).await;
    let mut published = vec![];
    while let Ok(event) = receiver.try_recv() {
        published.push(event);
    }
    (outcome, published)
}

async fn execute_sem_consentimento<A: ProviderAdapter>(
    adapter: &A,
    limits: RunLimits,
    cancel: CancelToken,
) -> (super::RunOutcome, Vec<RunEvent>) {
    let pool = pool().await;
    execute_com_pool(&pool, adapter, limits, cancel).await
}

async fn execute_com_pool<A: ProviderAdapter>(
    pool: &SqlitePool,
    adapter: &A,
    limits: RunLimits,
    cancel: CancelToken,
) -> (super::RunOutcome, Vec<RunEvent>) {
    let pack = TestPack::new();
    let ctx = Context {
        clock: clock(),
        pack: MethodPack::at(pack.path()),
    };
    let (events, mut receiver) = mpsc::channel(32);
    let runner = Runner {
        pool,
        ctx: &ctx,
        adapter,
        pin: default_pin(),
        limits,
        cancel,
        events,
    };
    let outcome = runner.run(round()).await;
    let mut published = vec![];
    while let Ok(event) = receiver.try_recv() {
        published.push(event);
    }
    (outcome, published)
}

struct AdaptadorQueNaoDeveAbrir;

impl ProviderAdapter for AdaptadorQueNaoDeveAbrir {
    async fn open(
        &self,
        _spec: &RunSpec<'_>,
        _cancel: &CancelToken,
    ) -> Result<mpsc::Receiver<ProviderEvent>, ProviderError> {
        panic!("o gate de consentimento não pode abrir o adaptador")
    }
}

/// Emula quem revoga com a conversa em curso: o primeiro turno pede uma ferramenta, e a revogação
/// acontece entre ele e o turno seguinte.
struct AdaptadorQueRevogaNoPrimeiroTurno {
    pool: SqlitePool,
    aberturas: AtomicUsize,
}

impl ProviderAdapter for AdaptadorQueRevogaNoPrimeiroTurno {
    async fn open(
        &self,
        _spec: &RunSpec<'_>,
        _cancel: &CancelToken,
    ) -> Result<mpsc::Receiver<ProviderEvent>, ProviderError> {
        let aberturas = self.aberturas.fetch_add(1, Ordering::SeqCst);
        assert_eq!(aberturas, 0, "o segundo turno não pode chegar ao provedor");

        crate::mia::consent::revoke(&self.pool)
            .await
            .expect("a revogação deve funcionar");

        let (sender, receiver) = mpsc::channel(8);
        sender
            .send(tool_call("call-1", "get_financial_snapshot", "{}"))
            .await
            .expect("o canal do roteiro aceita a chamada");
        sender
            .send(finished(FinishReason::ToolCalls))
            .await
            .expect("o canal do roteiro aceita o fim do turno");
        Ok(receiver)
    }
}

fn event_names(events: &[RunEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            RunEvent::RunStarted { .. } => "RunStarted",
            RunEvent::ToolStarted { .. } => "ToolStarted",
            RunEvent::ToolFinished { .. } => "ToolFinished",
            RunEvent::ProposalReady { .. } => "ProposalReady",
            RunEvent::AnswerReady { .. } => "AnswerReady",
            RunEvent::Usage(_) => "Usage",
            RunEvent::Error(_) => "Error",
            RunEvent::RunFinished { .. } => "RunFinished",
        })
        .collect()
}

#[test]
fn method_layer_tool_is_declared_in_catalog() {
    assert!(catalog::is_method_layer(METHOD_LAYER_TOOL));
    assert!(catalog::spec(METHOD_LAYER_TOOL).is_some());
}

#[tokio::test]
async fn answer_without_any_tool_is_method_provenance() {
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        ProviderEvent::TextDelta("O método começa pela leitura das réguas.".to_string()),
        finished(FinishReason::Stop),
    ])]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
}

#[tokio::test]
async fn answer_after_method_guidance_is_method_provenance() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", METHOD_LAYER_TOOL, "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A régua mostra o papel de cada movimento.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::ToolFinished {
            tool,
            ok: true,
            ..
        } if tool == METHOD_LAYER_TOOL
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
}

#[tokio::test]
async fn answer_after_a_data_tool_is_calculation_provenance() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A meta de reserva lida é 6 meses.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Calculo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Calculo,
            ..
        }
    )));
}

#[tokio::test]
async fn an_explanation_after_a_data_tool_is_method_provenance() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A consulta orienta a próxima leitura.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
}

#[tokio::test]
async fn a_failed_data_tool_does_not_turn_an_explanation_into_a_calculation() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call(
                "call-1",
                "get_financial_snapshot",
                r#"{"not_declared":"value"}"#,
            ),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            tool_call("call-2", METHOD_LAYER_TOOL, "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A explicação do método continua disponível.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::ToolFinished {
            tool,
            ok: false,
            ..
        } if tool == "get_financial_snapshot"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
}

#[tokio::test]
async fn a_data_tool_before_the_method_layer_keeps_the_calculation_provenance() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            tool_call("call-2", METHOD_LAYER_TOOL, "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta(
                "A meta de reserva lida é 6 meses; a leitura combina fatos e método.".to_string(),
            ),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Calculo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Calculo,
            ..
        }
    )));
}

#[tokio::test]
async fn a_regenerated_answer_without_data_numbers_is_an_explanation() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("O valor é 999.".to_string()),
            finished(FinishReason::Stop),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A régua orienta a próxima leitura.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(
        outcome.answer.as_deref(),
        Some("A régua orienta a próxima leitura.")
    );
    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
    assert!(
        outcome
            .trace
            .iter()
            .any(|entry| entry.kind == TraceKind::Regeneration)
    );
}

#[tokio::test]
async fn a_prefix_number_does_not_turn_an_explanation_into_a_calculation() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("O limiar didático é 47.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);
    let round = Round {
        system: "O limiar didático é 47.",
        history: &[],
        question: "Como estou?",
    };

    let (outcome, events) = execute_round(&adapter, limits(), CancelToken::new(), round).await;

    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::AnswerReady {
            provenance: AnswerProvenance::Metodo,
            ..
        }
    )));
}

#[tokio::test]
async fn sem_consentimento_a_rodada_nunca_abre_o_adaptador() {
    let (outcome, events) =
        execute_sem_consentimento(&AdaptadorQueNaoDeveAbrir, limits(), CancelToken::new()).await;

    assert_eq!(outcome.answer, None);
    assert_eq!(outcome.stop, StopReason::ConsentMissing);
    assert_eq!(outcome.turns, 0);
    assert_eq!(outcome.tool_calls, 0);
    assert_eq!(outcome.cost_micro_usd, 0);
    assert_eq!(outcome.attempts, 0);
    assert!(outcome.transcript.is_empty());
    assert!(events.iter().any(
        |event| matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::ConsentMissing)
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::RunStarted { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::RunFinished {
            stop: StopReason::ConsentMissing
        }
    )));
    assert!(
        outcome
            .trace
            .iter()
            .any(|entry| entry.kind == TraceKind::Stopped)
    );
}

/// Revogar com a conversa em curso precisa PARAR a conversa, não deixá-la terminar o que começou:
/// entre um turno e o próximo, o consentimento é relido, e o que ele diz agora é o que vale.
#[tokio::test]
async fn revogar_no_meio_da_rodada_fecha_antes_do_turno_seguinte() {
    let pool = pool().await;
    crate::mia::consent::grant(&pool, default_pin(), "2026-07-25T12:00:00Z")
        .await
        .expect("o consentimento deve gravar");
    let adapter = AdaptadorQueRevogaNoPrimeiroTurno {
        pool: pool.clone(),
        aberturas: AtomicUsize::new(0),
    };

    let (outcome, events) = execute_com_pool(&pool, &adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::ConsentMissing);
    assert_eq!(outcome.answer, None);
    // O que prova a garantia é a contagem de aberturas, não a de turnos: o turno seguinte chega a
    // abrir no laço e morre no gate, sem nunca alcançar o provedor.
    assert_eq!(adapter.aberturas.load(Ordering::SeqCst), 1);
    assert!(events.iter().any(
        |event| matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::ConsentMissing)
    ));
}

#[tokio::test]
async fn revogar_o_consentimento_volta_a_recusar_a_rodada() {
    let pool = pool().await;
    crate::mia::consent::grant(&pool, default_pin(), "2026-07-25T12:00:00Z")
        .await
        .expect("o consentimento deve gravar");
    crate::mia::consent::revoke(&pool)
        .await
        .expect("a revogação deve apagar o registro");

    let (outcome, _) = execute_com_pool(
        &pool,
        &AdaptadorQueNaoDeveAbrir,
        limits(),
        CancelToken::new(),
    )
    .await;

    assert_eq!(outcome.stop, StopReason::ConsentMissing);
    assert_eq!(outcome.turns, 0);
}

#[tokio::test]
async fn consentimento_de_versao_anterior_recusa_a_rodada() {
    let pool = pool().await;
    sqlx::query("INSERT INTO app_setting (key, value) VALUES (?1, ?2)")
        .bind("mia_consent")
        .bind(r#"{"version":0,"granted_at":"2026-07-25T12:00:00Z"}"#)
        .execute(&pool)
        .await
        .expect("o fixture deve gravar");

    let (outcome, events) = execute_com_pool(
        &pool,
        &AdaptadorQueNaoDeveAbrir,
        limits(),
        CancelToken::new(),
    )
    .await;

    assert_eq!(outcome.stop, StopReason::ConsentMissing);
    assert!(events.iter().any(
        |event| matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::ConsentMissing)
    ));
}

#[tokio::test]
async fn consentimento_registrado_desbloqueia_a_rodada() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            usage(Some(21)),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A consulta foi concluída.".to_string()),
            usage(Some(8)),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.answer.as_deref(), Some("A consulta foi concluída."));
    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(
        event_names(&events),
        [
            "RunStarted",
            "Usage",
            "ToolStarted",
            "ToolFinished",
            "Usage",
            "AnswerReady",
            "RunFinished",
        ]
    );
    let transcripts = adapter.transcripts();
    let tool_result = transcripts[1]
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("o turno seguinte recebe o resultado da ferramenta");
    let content = tool_result["content"]
        .as_str()
        .expect("o resultado da ferramenta é texto no transcript");
    assert!(content.starts_with("<dados_de_ferramenta confiavel=\"nao\">"));
    assert!(content.contains("\"tool\":\"get_financial_snapshot\""));
}

#[tokio::test]
async fn credential_never_reaches_events_or_trace() {
    let secret = "sk-or-v1-1234567890abcdef";
    let bearer = "Authorization: Bearer token-1234567890";
    let adapter = ScriptedAdapter::new([Script::OpenError(provider_error(
        ErrorKind::Permanent,
        &format!("Falha com {secret}. {bearer}"),
    ))]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    let published = format!("{events:?}");
    let trace = format!("{:?}", outcome.trace);
    assert!(!published.contains(secret));
    assert!(!published.contains("token-1234567890"));
    assert!(!trace.contains(secret));
    assert!(!trace.contains("token-1234567890"));
    assert!(trace.contains(REDACTED));
}

/// A chave guardada no cofre não tem caminho até o laço, e o único jeito de ela reaparecer é o
/// outro lado devolvê-la num erro que ecoa o cabeçalho enviado. O que se prova aqui é que o
/// formato da chave que o app guarda é justamente um dos que o redator reconhece: um prefixo novo
/// que ele não pegasse passaria direto para evento e rastro.
#[tokio::test]
async fn a_chave_guardada_no_cofre_sai_redigida_quando_o_provedor_a_ecoa() {
    let key = ApiKey::new("sk-or-v1-fixture1234567890".to_string());
    let adapter = ScriptedAdapter::new([Script::OpenError(provider_error(
        ErrorKind::Permanent,
        &format!("401 rejeitado. Authorization: Bearer {}", key.expose()),
    ))]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert!(!format!("{events:?}").contains(key.expose()));
    assert!(!format!("{:?}", outcome.trace).contains(key.expose()));
    assert!(format!("{:?}", outcome.trace).contains(REDACTED));
}

#[tokio::test]
async fn answer_text_is_redacted_before_publication() {
    let secret = "sk-or-v1-1234567890abcdef";
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        ProviderEvent::TextDelta(format!("O provedor ecoou {secret}.")),
        finished(FinishReason::Stop),
    ])]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    let event_text = events
        .iter()
        .find_map(|event| match event {
            RunEvent::AnswerReady { text, .. } => Some(text),
            _ => None,
        })
        .expect("a resposta redigida deve ser publicada");
    let transcript =
        serde_json::to_string(&outcome.transcript).expect("o transcript da rodada é serializável");
    assert!(!event_text.contains(secret));
    assert!(event_text.contains(REDACTED));
    assert_eq!(outcome.answer.as_deref(), Some(event_text.as_str()));
    assert!(!transcript.contains(secret));
    assert!(transcript.contains(REDACTED));
}

#[tokio::test]
async fn grounding_rejects_a_number_that_only_the_question_mentions() {
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        ProviderEvent::TextDelta("Você tem R$ 999.".to_string()),
        finished(FinishReason::Stop),
    ])]);
    let limits = RunLimits {
        max_regenerations: 0,
        ..limits()
    };
    let round = Round {
        system: "Responda usando apenas fatos da rodada.",
        history: &[],
        question: "Tenho R$ 999?",
    };

    let (outcome, events) = execute_round(&adapter, limits, CancelToken::new(), round).await;

    assert_eq!(outcome.stop, StopReason::Ungrounded);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::AnswerReady { .. }))
    );
}

#[tokio::test]
async fn tool_call_is_validated_before_running() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call(
                "call-1",
                "get_financial_snapshot",
                r#"{"not_declared":"value"}"#,
            ),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("Corrigi a chamada.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let _ = execute(&adapter, limits(), CancelToken::new()).await;
    let transcripts = adapter.transcripts();
    let tool_result = transcripts[1]
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("a recusa deve voltar ao modelo");
    let content = tool_result["content"]
        .as_str()
        .expect("a recusa é serializada no transcript");
    let envelope: Value = serde_json::from_str(
        content
            .strip_prefix("<dados_de_ferramenta confiavel=\"nao\">\n")
            .and_then(|value| value.strip_suffix("\n</dados_de_ferramenta>"))
            .expect("a recusa mantém a moldura de dado não confiável"),
    )
    .expect("o conteúdo da moldura é JSON");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "unknown_argument");
    assert!(envelope.get("data").is_none());
}

#[tokio::test]
async fn parallel_tool_calls_close_the_run() {
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        tool_call("call-1", "get_financial_snapshot", "{}"),
        tool_call("call-2", "get_data_status", "{}"),
        finished(FinishReason::ToolCalls),
    ])]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::Failed);
    assert_eq!(outcome.tool_calls, 0);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::ToolStarted { .. }))
    );
    assert!(events.iter().any(|event| {
        matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::ProtocolViolation)
    }));
}

#[tokio::test]
async fn turn_cap_closes_the_run() {
    // O roteiro nunca conclui: cada turno pede outra ferramenta. O que fecha a rodada é o teto,
    // e é por isso que o teste conta os turnos gastos — um teto de zero provaria só que o laço
    // não abre turno nenhum, que é outra coisa.
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            tool_call("call-2", "get_data_status", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            tool_call("call-3", "get_budget_settings", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
    ]);
    let limits = RunLimits {
        max_turns: 2,
        ..limits()
    };

    let (outcome, events) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::TurnCap);
    assert_eq!(outcome.provenance, None);
    assert_eq!(outcome.turns, 2);
    assert_eq!(adapter.open_calls(), 2);
    assert!(events.iter().any(
        |event| matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::TurnCap)
    ));
}

#[tokio::test]
async fn tool_call_cap_closes_the_run() {
    // A primeira leitura acontece; é a segunda que o teto recusa. Com teto zero o laço recusaria
    // antes de despachar qualquer coisa, e o teste não distinguiria "parou no teto" de "nunca
    // chamou ferramenta".
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            tool_call("call-2", "get_data_status", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
    ]);
    let limits = RunLimits {
        max_tool_calls: 1,
        ..limits()
    };

    let (outcome, events) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::ToolCallCap);
    assert_eq!(outcome.tool_calls, 1);
    let started: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            RunEvent::ToolStarted { tool, .. } => Some(tool.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(started, ["get_financial_snapshot"]);
}

#[tokio::test]
async fn cost_cap_closes_the_run() {
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        ProviderEvent::TextDelta("Esta resposta não pode aparecer.".to_string()),
        usage(Some(51)),
        finished(FinishReason::Stop),
    ])]);
    let limits = RunLimits {
        max_cost_micro_usd: 50,
        ..limits()
    };

    let (outcome, events) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::CostCap);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::AnswerReady { .. }))
    );
}

#[tokio::test]
async fn cost_cap_closes_the_run_before_retrying() {
    let adapter = ScriptedAdapter::new([Script::EventsThenFailure {
        events: vec![usage(Some(51))],
        failure: provider_error(ErrorKind::Transient, "A conexão caiu."),
    }]);
    let limits = RunLimits {
        max_cost_micro_usd: 50,
        ..limits()
    };

    let (outcome, _) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::CostCap);
    assert_eq!(adapter.open_calls(), 1);
}

#[tokio::test]
async fn time_cap_closes_the_run() {
    let adapter = ScriptedAdapter::new([Script::Hang]);
    let limits = RunLimits {
        max_duration: Duration::from_millis(20),
        ..limits()
    };

    let (outcome, events) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::TimeCap);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::AnswerReady { .. }))
    );
}

#[tokio::test]
async fn cancellation_closes_the_provider_connection() {
    let adapter = ScriptedAdapter::new([Script::Hang]);
    let pool = pool().await;
    crate::mia::consent::grant(&pool, default_pin(), "2026-07-25T12:00:00Z")
        .await
        .expect("o consentimento do roteiro deve gravar");
    let pack = TestPack::new();
    let ctx = Context {
        clock: clock(),
        pack: MethodPack::at(pack.path()),
    };
    let (events, mut receiver) = mpsc::channel(32);
    let cancel = CancelToken::new();
    let runner = Runner {
        pool: &pool,
        ctx: &ctx,
        adapter: &adapter,
        pin: default_pin(),
        limits: limits(),
        cancel: cancel.clone(),
        events,
    };

    let cancel_after_open = async {
        adapter.wait_for_open().await;
        cancel.cancel();
    };
    let (outcome, ()) = tokio::join!(runner.run(round()), cancel_after_open);
    adapter.wait_for_connection_close().await;
    let mut published = vec![];
    while let Ok(event) = receiver.try_recv() {
        published.push(event);
    }

    assert_eq!(outcome.stop, StopReason::Cancelled);
    assert!(adapter.connection_closed());
    assert!(
        !published
            .iter()
            .any(|event| matches!(event, RunEvent::AnswerReady { .. }))
    );
}

#[tokio::test]
async fn event_backpressure_never_outlives_the_time_cap() {
    let adapter = ScriptedAdapter::new([Script::Events(vec![
        ProviderEvent::TextDelta("A resposta foi concluída.".to_string()),
        finished(FinishReason::Stop),
    ])]);
    let pool = pool().await;
    crate::mia::consent::grant(&pool, default_pin(), "2026-07-25T12:00:00Z")
        .await
        .expect("o consentimento do roteiro deve gravar");
    let pack = TestPack::new();
    let ctx = Context {
        clock: clock(),
        pack: MethodPack::at(pack.path()),
    };
    let (events, _receiver) = mpsc::channel(1);
    let runner = Runner {
        pool: &pool,
        ctx: &ctx,
        adapter: &adapter,
        pin: default_pin(),
        limits: RunLimits {
            max_duration: Duration::from_millis(20),
            ..limits()
        },
        cancel: CancelToken::new(),
        events,
    };

    let outcome = tokio::time::timeout(Duration::from_secs(1), runner.run(round()))
        .await
        .expect("a publicação não deve prender a rodada além do teto de tempo");

    assert_eq!(outcome.stop, StopReason::TimeCap);
}

#[tokio::test]
async fn cancellation_wakes_a_waiter_registered_after_the_flag() {
    let cancelled_before_wait = CancelToken::new();
    cancelled_before_wait.cancel();
    tokio::time::timeout(Duration::from_millis(50), cancelled_before_wait.cancelled())
        .await
        .expect("o cancelamento anterior à espera deve acordar a tarefa");

    let cancelled_after_wait = CancelToken::new();
    let wait = cancelled_after_wait.cancelled();
    tokio::pin!(wait);
    tokio::select! {
        _ = &mut wait => panic!("o token não foi cancelado"),
        _ = tokio::task::yield_now() => {}
    }
    cancelled_after_wait.cancel();
    tokio::time::timeout(Duration::from_millis(50), wait)
        .await
        .expect("a espera já registrada deve acordar após o cancelamento");
}

#[tokio::test]
async fn partial_arguments_never_run_a_tool() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", r#"{"include": "#),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("Corrigi a chamada.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.tool_calls, 0);
    assert_eq!(outcome.provenance, Some(AnswerProvenance::Metodo));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::ToolStarted { .. }))
    );
    let transcripts = adapter.transcripts();
    let tool_result = transcripts[1]
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("a recusa de JSON volta ao modelo");
    assert!(
        tool_result["content"]
            .as_str()
            .expect("a recusa é texto")
            .contains("invalid_argument")
    );
}

#[test]
fn retry_decision_distinguishes_the_four_failures() {
    let limits = limits();
    let remaining = Duration::from_secs(2);

    assert_eq!(
        retry_decision(&ErrorKind::Transient, false, 1, &limits, remaining),
        RetryDecision::RetryPreResponse {
            after: Duration::ZERO
        }
    );
    assert_eq!(
        retry_decision(&ErrorKind::Malformed, true, 1, &limits, remaining),
        RetryDecision::RetryMidStream {
            after: Duration::ZERO
        }
    );
    assert_eq!(
        retry_decision(
            &ErrorKind::RateLimited {
                retry_after_secs: Some(1)
            },
            false,
            1,
            &limits,
            remaining,
        ),
        RetryDecision::RetryAfterRateLimit {
            after: Duration::from_secs(1)
        }
    );
    assert_eq!(
        retry_decision(&ErrorKind::Permanent, false, 1, &limits, remaining),
        RetryDecision::GiveUp {
            code: RunErrorCode::ProviderRefused
        }
    );
}

#[tokio::test]
async fn pre_response_failure_is_retried_and_answers() {
    let adapter = ScriptedAdapter::new([
        Script::OpenError(provider_error(ErrorKind::Transient, "A conexão caiu.")),
        Script::Events(vec![
            ProviderEvent::TextDelta("A resposta chegou na segunda tentativa.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, _) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::Answered);
    assert_eq!(adapter.open_calls(), 2);
    assert_eq!(outcome.attempts, 2);
    assert!(
        outcome
            .trace
            .iter()
            .any(|entry| entry.kind == TraceKind::Retry)
    );
}

#[tokio::test]
async fn permanent_failure_is_not_retried() {
    let adapter = ScriptedAdapter::new([Script::OpenError(provider_error(
        ErrorKind::Permanent,
        "O provedor recusou a chamada.",
    ))]);

    let (outcome, _) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::Failed);
    assert_eq!(adapter.open_calls(), 1);
    assert_eq!(outcome.attempts, 1);
}

#[tokio::test]
async fn ungrounded_number_is_discarded_and_regenerated() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            ProviderEvent::TextDelta("O valor é 999.".to_string()),
            finished(FinishReason::Stop),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("Não há número para informar.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, events) = execute(&adapter, limits(), CancelToken::new()).await;

    assert_eq!(
        outcome.answer.as_deref(),
        Some("Não há número para informar.")
    );
    let answers: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            RunEvent::AnswerReady { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(answers, ["Não há número para informar."]);
    assert!(
        outcome
            .trace
            .iter()
            .any(|entry| entry.kind == TraceKind::Regeneration)
    );
}

#[tokio::test]
async fn stopped_trace_records_the_last_attempt_in_its_turn() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            tool_call("call-1", "get_financial_snapshot", "{}"),
            finished(FinishReason::ToolCalls),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("A resposta foi concluída.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);

    let (outcome, _) = execute(&adapter, limits(), CancelToken::new()).await;
    let stopped = outcome
        .trace
        .iter()
        .find(|entry| entry.kind == TraceKind::Stopped)
        .expect("a rodada registra o encerramento no rastro");

    assert_eq!(stopped.turn, 2);
    assert_eq!(stopped.attempt, 1);
}

#[tokio::test]
async fn ungrounded_answer_is_refused_after_the_regeneration_cap() {
    let adapter = ScriptedAdapter::new([
        Script::Events(vec![
            ProviderEvent::TextDelta("O valor é 999.".to_string()),
            finished(FinishReason::Stop),
        ]),
        Script::Events(vec![
            ProviderEvent::TextDelta("O valor ainda é 998.".to_string()),
            finished(FinishReason::Stop),
        ]),
    ]);
    let limits = RunLimits {
        max_regenerations: 1,
        ..limits()
    };

    let (outcome, events) = execute(&adapter, limits, CancelToken::new()).await;

    assert_eq!(outcome.stop, StopReason::Ungrounded);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RunEvent::AnswerReady { .. }))
    );
    assert!(events.iter().any(
        |event| matches!(event, RunEvent::Error(error) if error.code == RunErrorCode::Ungrounded)
    ));
}

/// A proposta vira evento próprio só quando a ferramenta a validou. Uma recusa é correção para o
/// modelo, e um cartão de aprovação sobre ela ofereceria um gesto sobre lançamento inexistente.
#[test]
fn so_a_proposta_validada_vira_evento_da_tela() {
    let mut proposta = envelope(json!({"transaction": {"amount_cents": 5_000}}));
    proposta.tool = catalog::PROPOSAL_TOOL.to_string();

    let event = super::proposal_event(catalog::PROPOSAL_TOOL, "call-2", &proposta)
        .expect("a proposta validada publica o evento");

    match event {
        RunEvent::ProposalReady { id, proposal } => {
            assert_eq!(id, "call-2");
            assert_eq!(proposal["tool"], json!(catalog::PROPOSAL_TOOL));
            assert_eq!(
                proposal["data"]["transaction"]["amount_cents"],
                json!(5_000)
            );
        }
        outro => panic!("a proposta validada deveria publicar ProposalReady, veio {outro:?}"),
    }

    let mut recusada = proposta;
    recusada.ok = false;
    recusada.data = None;
    assert!(super::proposal_event(catalog::PROPOSAL_TOOL, "call-2", &recusada).is_none());

    let leitura = envelope(json!({"amount_cents": 810_158}));
    assert!(super::proposal_event("get_financial_snapshot", "call-1", &leitura).is_none());
}
