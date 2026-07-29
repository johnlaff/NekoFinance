//! O laço da conversa: uma rodada, da pergunta à resposta validada.
//!
//! Uma rodada é uma máquina de estados pequena: monta o pedido, consome o stream, despacha as
//! ferramentas que o modelo pediu, devolve os envelopes ao transcript e recomeça — até haver
//! resposta aterrada, teto estourado ou recusa. Não há framework de agente aqui, e é de
//! propósito: o que separa um agente de um agente em que se pode confiar com dinheiro são as
//! invariantes, e invariante escondida em biblioteca de terceiros não é invariante.
//!
//! O ponto de injeção é o trait do provedor. Um adapter roteirizado emite sequências de eventos
//! do domínio e exercita o laço inteiro sem rede, sem chave e sem gastar saldo.

pub(crate) mod grounding;
pub(crate) mod redaction;

use super::envelope::{ErrorCode, ToolError};
use super::provider::pins::ModelPin;
use super::provider::request::{RunSpec, ToolDeclaration};
use super::provider::stream::{ErrorKind, FinishReason, ProviderError, ProviderEvent, Usage};
use super::{Context, ToolCall, catalog, consent, dispatch, refuse};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, mpsc};

/// Resposta concisa preserva espaço para fatos e envelopes sem induzir texto financeiro longo.
const MAX_TOKENS_PER_TURN: u32 = 1_024;

/// O provedor visto pelo laço: uma rodada aberta, um canal de eventos do domínio.
///
/// A credencial não atravessa este trait. Quem a carrega é a implementação de rede, que a lê do
/// cofre do sistema e a some no cabeçalho — não existe caminho daqui até um evento, um log ou o
/// banco. O trait também não é `dyn`: o laço é genérico sobre ele, o que deixa a chamada direta e
/// o adapter da suíte trivial.
pub(crate) trait ProviderAdapter {
    /// Abre a rodada. `Err` aqui é falha ANTES de qualquer evento — a distinção que a taxonomia
    /// de retentativa precisa fazer. O sinal de cancelamento vai junto porque quem fecha a
    /// conexão HTTP é quem a abriu.
    fn open(
        &self,
        spec: &RunSpec<'_>,
        cancel: &CancelToken,
    ) -> impl Future<Output = Result<mpsc::Receiver<ProviderEvent>, ProviderError>> + Send;
}

/// Tetos de uma rodada. Todos existem pelo mesmo motivo: um laço de agente sem teto gasta o
/// dinheiro e o tempo de quem perguntou até alguém desligar o app.
#[derive(Debug, Clone)]
pub(crate) struct RunLimits {
    pub max_turns: u32,
    pub max_tool_calls: u32,
    /// Limita somente o custo informado pelo provedor; uso sem custo declarado é lacuna, não
    /// gratuidade, e a transparência o publica como ausente em vez de zero.
    pub max_cost_micro_usd: i64,
    pub max_duration: Duration,
    /// Quantas vezes uma resposta com número órfão pode ser refeita antes de a rodada recusar.
    pub max_regenerations: u32,
    /// Tentativas por turno, contando a primeira.
    pub max_attempts_per_turn: u32,
    /// Espera base entre tentativas. Zero na suíte: o que está sob teste é a decisão, não o sono.
    pub retry_backoff: Duration,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            // Oito turnos cobrem consulta, correção e resposta sem deixar o laço derivar.
            max_turns: 8,
            // Doze leituras permitem decompor uma pergunta sem virar uma varredura do banco.
            max_tool_calls: 12,
            // Quinze centavos limitam o custo de uma pergunta sem cortar pergunta legítima. O
            // número saiu de medição, não de estimativa: uma rodada real custa entre 2,6 e 9,1
            // centavos conforme o modelo, e o teto anterior caía no meio dessa distribuição —
            // reprovava por custo metade da matriz, e até o modelo default quando um turno
            // retentava. Teto que corta pela metade não é trava de segurança, é filtro de modelo
            // disfarçado. (verificado 2026-07)
            max_cost_micro_usd: 150_000,
            // Noventa segundos acomodam provedor lento sem prender a interface por tempo aberto.
            max_duration: Duration::from_secs(90),
            // Uma correção dá espaço ao aterramento sem aceitar insistência em número inventado.
            max_regenerations: 1,
            // Três tentativas absorvem instabilidade transitória sem repetir cobrança indefinidamente.
            max_attempts_per_turn: 3,
            // Meio segundo evita repetir a mesma falha em sequência sem tornar a resposta ociosa.
            retry_backoff: Duration::from_millis(500),
        }
    }
}

/// A pergunta e o que veio antes dela. O histórico é do app, não do provedor: a janela da
/// conversa corrente é reenviada inteira a cada rodada.
pub(crate) struct Round<'a> {
    pub system: &'a str,
    /// O transcript da conversa até aqui, no formato nativo do provedor.
    pub history: &'a [Value],
    pub question: &'a str,
}

/// O que a interface recebe. Nunca texto token a token: a resposta financeira publica atômica,
/// depois de validada.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RunEvent {
    RunStarted {
        model: &'static str,
        endpoint: &'static str,
    },
    ToolStarted {
        id: String,
        tool: String,
    },
    ToolFinished {
        id: String,
        tool: String,
        ok: bool,
    },
    AnswerReady {
        text: String,
        provenance: AnswerProvenance,
    },
    Usage(RunUsage),
    Error(RunError),
    RunFinished {
        stop: StopReason,
    },
}

/// Como a resposta se apresenta: explicação do método ou conta sobre os números da pessoa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnswerProvenance {
    /// A resposta explica o método. Nenhum número dos dados da pessoa a sustenta.
    Metodo,
    /// A resposta se apoia em pelo menos um fato lido dos dados da pessoa.
    Calculo,
}

/// A linha de transparência da rodada: para onde foi, quanto custou, quantas tentativas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunUsage {
    pub model: &'static str,
    pub endpoint: &'static str,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Ausente é lacuna declarada, nunca zero, que afirmaria uma rodada gratuita.
    pub cost_micro_usd: Option<i64>,
    pub attempts: u32,
}

/// Por que a rodada terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopReason {
    ConsentMissing,
    Answered,
    TurnCap,
    ToolCallCap,
    CostCap,
    TimeCap,
    Cancelled,
    /// A resposta insistiu em citar número sem origem nos fatos da rodada.
    Ungrounded,
    Failed,
}

/// Erro publicável. Traz texto NOSSO: mensagem crua de provedor é dado não confiável e fica no
/// rastro técnico, redigida — nunca vira o que a pessoa lê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunError {
    pub code: RunErrorCode,
    pub message: String,
    pub fix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunErrorCode {
    ConsentMissing,
    ProviderUnavailable,
    RateLimited,
    ProviderRefused,
    /// O provedor quebrou o contrato da rodada (mais de uma chamada por turno, turno sem
    /// conteúdo, argumento que não fecha).
    ProtocolViolation,
    TurnCap,
    ToolCallCap,
    CostCap,
    TimeCap,
    Cancelled,
    Ungrounded,
}

/// Rastro técnico da rodada. Retenção e persistência não são deste ticket; o que é daqui é a
/// garantia de que nada entra sem passar pelo redator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceEntry {
    pub turn: u32,
    pub attempt: u32,
    pub kind: TraceKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraceKind {
    TurnStarted,
    ToolDispatched,
    ToolRefused,
    ProviderFailure,
    Retry,
    Regeneration,
    Stopped,
}

/// O resultado da rodada inteira.
#[derive(Debug)]
pub(crate) struct RunOutcome {
    pub answer: Option<String>,
    pub provenance: Option<AnswerProvenance>,
    pub stop: StopReason,
    pub turns: u32,
    pub tool_calls: u32,
    pub cost_micro_usd: i64,
    /// Todo uso da rodada veio com custo declarado pelo provedor — inclusive o das tentativas que
    /// falharam. Falso significa que a rodada gastou dinheiro que o total não conta, e é o que
    /// impede a trava de gasto da bancada de ler um total incompleto como total.
    pub cost_declared: bool,
    pub attempts: u32,
    /// O transcript já com a pergunta, as chamadas e os envelopes — o que a persistência grava.
    pub transcript: Vec<Value>,
    pub trace: Vec<TraceEntry>,
}

/// O sinal de cancelamento. Compartilhado com o adapter porque cancelar precisa chegar até a
/// conexão: parar de ler o stream sem fechá-la deixaria o provedor gerando — e cobrando — uma
/// resposta que ninguém vai ler.
#[derive(Clone, Default)]
pub(crate) struct CancelToken {
    inner: Arc<CancelState>,
}

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancelToken {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        self.inner.notify.notify_waiters();
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    /// Espera até o cancelamento. Resolve na hora se já foi cancelado.
    pub(crate) async fn cancelled(&self) {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// O que fazer diante de uma falha do provedor. As quatro situações pedem respostas diferentes, e
/// tratá-las como uma só ou repete o que já custou, ou desiste do que teria funcionado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryDecision {
    /// Falha antes de qualquer evento: nada foi produzido nem cobrado, o turno recomeça já.
    RetryPreResponse { after: Duration },
    /// Falha depois de já ter produzido evento: o parcial morre sem ser publicado e o turno
    /// recomeça — o custo já incorrido continua contando no teto.
    RetryMidStream { after: Duration },
    /// Limite de taxa: a espera é a que o provedor pediu, não a nossa.
    RetryAfterRateLimit { after: Duration },
    /// Definitiva: erro permanente, tentativas esgotadas, ou espera que não cabe no que resta da
    /// rodada — esperar além do teto de tempo seria estourá-lo por outro caminho.
    GiveUp { code: RunErrorCode },
}

/// A decisão isolada do laço, para que a taxonomia seja exercitável sem relógio nem stream.
pub(crate) fn retry_decision(
    kind: &ErrorKind,
    produced_events: bool,
    attempt: u32,
    limits: &RunLimits,
    remaining: Duration,
) -> RetryDecision {
    if attempt >= limits.max_attempts_per_turn {
        return RetryDecision::GiveUp {
            code: failure_code(kind),
        };
    }

    if matches!(kind, ErrorKind::Permanent) {
        return RetryDecision::GiveUp {
            code: RunErrorCode::ProviderRefused,
        };
    }

    if let ErrorKind::RateLimited { retry_after_secs } = kind {
        let after = retry_after_secs
            .map(Duration::from_secs)
            .unwrap_or(limits.retry_backoff);
        return if after > remaining {
            RetryDecision::GiveUp {
                code: RunErrorCode::RateLimited,
            }
        } else {
            RetryDecision::RetryAfterRateLimit { after }
        };
    }

    let multiplier = 2_u32
        .checked_pow(attempt.saturating_sub(1))
        .unwrap_or(u32::MAX);
    let after = limits
        .retry_backoff
        .checked_mul(multiplier)
        .unwrap_or(Duration::MAX);
    if after > remaining {
        RetryDecision::GiveUp {
            code: RunErrorCode::TimeCap,
        }
    } else if produced_events {
        RetryDecision::RetryMidStream { after }
    } else {
        RetryDecision::RetryPreResponse { after }
    }
}

/// As 14 ferramentas como o provedor as declara.
///
/// O schema declarado é a lista FECHADA de nomes aceitos; o tipo de cada argumento é validado
/// localmente, pela fachada, porque é a validação da casa que decide se a ferramenta roda. Por
/// isso o modo estrito fica desligado aqui: schema sem tipo não o satisfaz, e prometê-lo ao
/// provedor sem cumpri-lo seria pior que não pedi-lo.
pub(crate) fn tool_declarations() -> Vec<ToolDeclaration> {
    catalog::CATALOG
        .iter()
        .map(|spec| {
            let mut properties = serde_json::Map::new();
            for parameter in spec.params {
                properties.insert((*parameter).to_string(), json!({"description": parameter}));
            }
            if !spec.includes.is_empty() {
                properties.insert(
                    "include".to_string(),
                    json!({
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": spec.include_names(),
                        },
                    }),
                );
            }

            ToolDeclaration {
                name: spec.name.to_string(),
                description: format!(
                    "{}. Use para: {}. Não use para: {}.",
                    spec.summary, spec.use_for, spec.not_for
                ),
                parameters: json!({
                    "type": "object",
                    "properties": properties,
                    "additionalProperties": false,
                }),
                strict: false,
            }
        })
        .collect()
}

#[derive(Default)]
struct CollectedUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    cost_micro_usd: Option<i64>,
    /// Algum uso chegou sem custo declarado. O turno pode terminar com um total conhecido — a
    /// tentativa seguinte declara o dela — e ainda assim ter consumido dinheiro que ninguém
    /// contou: o total soma o que sabe, e este bit é o que impede de lê-lo como completo.
    missing_cost: bool,
}

impl CollectedUsage {
    fn absorb(&mut self, usage: Usage, total_cost_micro_usd: &mut i64) {
        self.prompt_tokens = self.prompt_tokens.saturating_add(usage.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(usage.completion_tokens);
        match usage.cost_micro_usd {
            Some(cost) => {
                let accumulated = self.cost_micro_usd.get_or_insert(0);
                *accumulated = accumulated.saturating_add(cost);
                *total_cost_micro_usd = total_cost_micro_usd.saturating_add(cost);
            }
            None => self.missing_cost = true,
        }
    }
}

struct CompletedToolCall {
    id: String,
    name: String,
    arguments: String,
}

struct CompletedTurn {
    text: String,
    tool_calls: Vec<CompletedToolCall>,
    reason: FinishReason,
}

enum TurnRead {
    Completed(CompletedTurn),
    ProviderFailure {
        error: ProviderError,
        produced_events: bool,
    },
    Cancelled,
    TimeCap,
}

enum RetryWait {
    Retry,
    Cancelled,
    TimeCap,
    GiveUp(RunErrorCode),
}

#[derive(Clone, Copy)]
enum EventWindow {
    Ordinary,
    Closing,
}

async fn consume_turn(
    receiver: &mut mpsc::Receiver<ProviderEvent>,
    cancel: &CancelToken,
    deadline: Instant,
    usage: &mut CollectedUsage,
    total_cost_micro_usd: &mut i64,
) -> TurnRead {
    let mut text = String::new();
    let mut tool_calls = vec![];
    let mut reason = None;
    let mut produced_events = false;
    // Esta tentativa chegou a receber uma linha de uso?
    //
    // Quem chega aqui já teve o pedido ACEITO — a abertura do stream é que devolve erro antes de
    // qualquer cobrança. Daí em diante, terminar sem a linha de uso é lacuna, tenha ou não chegado
    // conteúdo: o provedor pode ter gerado e cobrado o que a rede não entregou, e o total do
    // turno, fechado pela tentativa seguinte, sairia parecendo completo. Entre parar a bancada à
    // toa e deixar dinheiro fora do contador, a trava precisa do lado fechado.
    let mut saw_usage = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            usage.missing_cost |= !saw_usage;
            return TurnRead::TimeCap;
        }

        tokio::select! {
            // Parar no meio não desfaz o que o provedor já gerou: se houve conteúdo e a linha de
            // uso não chegou, o turno consumiu dinheiro que ninguém contou — e um turno anterior
            // com custo declarado faria o total parecer completo.
            _ = cancel.cancelled() => {
                usage.missing_cost |= !saw_usage;
                return TurnRead::Cancelled;
            }
            _ = tokio::time::sleep(remaining) => {
                usage.missing_cost |= !saw_usage;
                return TurnRead::TimeCap;
            }
            event = receiver.recv() => match event {
                Some(ProviderEvent::TextDelta(delta)) => {
                    produced_events = true;
                    text.push_str(&delta);
                }
                Some(ProviderEvent::ToolCallComplete { id, name, arguments }) => {
                    produced_events = true;
                    tool_calls.push(CompletedToolCall { id, name, arguments });
                }
                Some(ProviderEvent::Usage(provider_usage)) => {
                    produced_events = true;
                    saw_usage = true;
                    usage.absorb(provider_usage, total_cost_micro_usd);
                }
                // O motivo final não encerra a leitura: o uso da rodada chega DEPOIS dele, e sair
                // aqui deixaria o custo de fora justamente do turno que o gastou. Quem fecha o
                // turno é o fim do stream; o teto de tempo cobre o provedor que não o fecha.
                Some(ProviderEvent::Finished { reason: finished, native: _ }) => {
                    produced_events = true;
                    reason = Some(finished);
                }
                Some(ProviderEvent::Failed(error)) => {
                    usage.missing_cost |= !saw_usage;
                    return TurnRead::ProviderFailure { error, produced_events };
                }
                None => match reason {
                    Some(reason) => return TurnRead::Completed(CompletedTurn {
                        text,
                        tool_calls,
                        reason,
                    }),
                    None => {
                        usage.missing_cost |= !saw_usage;
                        return TurnRead::ProviderFailure {
                            error: ProviderError {
                                kind: ErrorKind::Transient,
                                message: "O stream do provedor fechou sem informar como a resposta terminou.".to_string(),
                                // O stream chegou a abrir: a resposta veio, e o dinheiro deste
                                // turno já está contabilizado pela linha de uso (ou pela falta
                                // dela, acima).
                                responded: true,
                            },
                            produced_events,
                        };
                    }
                },
            },
        }
    }
}

async fn wait_for_retry(cancel: &CancelToken, deadline: Instant, after: Duration) -> RetryWait {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return RetryWait::TimeCap;
    }

    tokio::select! {
        _ = cancel.cancelled() => RetryWait::Cancelled,
        _ = tokio::time::sleep(remaining) => RetryWait::TimeCap,
        _ = tokio::time::sleep(after) => RetryWait::Retry,
    }
}

async fn retry_or_stop(
    decision: RetryDecision,
    cancel: &CancelToken,
    deadline: Instant,
    trace: &mut Vec<TraceEntry>,
    turn: u32,
    attempt: u32,
) -> RetryWait {
    let after = match decision {
        RetryDecision::RetryPreResponse { after }
        | RetryDecision::RetryMidStream { after }
        | RetryDecision::RetryAfterRateLimit { after } => after,
        RetryDecision::GiveUp { code } => return RetryWait::GiveUp(code),
    };
    trace.push(TraceEntry {
        turn,
        attempt,
        kind: TraceKind::Retry,
        detail: "Uma nova tentativa foi programada para esta rodada.".to_string(),
    });
    wait_for_retry(cancel, deadline, after).await
}

fn failure_code(kind: &ErrorKind) -> RunErrorCode {
    match kind {
        ErrorKind::Permanent => RunErrorCode::ProviderRefused,
        ErrorKind::RateLimited { .. } => RunErrorCode::RateLimited,
        ErrorKind::Transient | ErrorKind::Malformed => RunErrorCode::ProviderUnavailable,
    }
}

fn run_error(code: RunErrorCode) -> RunError {
    let (message, fix) = match code {
        RunErrorCode::ConsentMissing => (
            "A conversa aberta só roda com o seu consentimento registrado.",
            "Abra Configurações › Conversa e registre o consentimento.",
        ),
        RunErrorCode::ProviderUnavailable => (
            "Não foi possível concluir a conversa com o provedor.",
            "Tente de novo em instantes.",
        ),
        RunErrorCode::RateLimited => (
            "O provedor limitou temporariamente esta conversa.",
            "Aguarde alguns instantes antes de tentar de novo.",
        ),
        RunErrorCode::ProviderRefused => (
            "O provedor recusou concluir esta resposta.",
            "Tente reformular a pergunta ou escolha outro modelo permitido.",
        ),
        RunErrorCode::ProtocolViolation => (
            "A resposta do provedor não respeitou o contrato da conversa.",
            "Tente de novo; se persistir, escolha outro modelo permitido.",
        ),
        RunErrorCode::TurnCap => (
            "A conversa precisaria de mais passos do que o limite permitido.",
            "Faça uma pergunta mais específica.",
        ),
        RunErrorCode::ToolCallCap => (
            "A conversa atingiu o limite de consultas aos seus dados.",
            "Faça uma pergunta mais específica.",
        ),
        RunErrorCode::CostCap => (
            "A conversa atingiu o limite de custo permitido.",
            "Faça uma pergunta mais específica ou tente de novo depois.",
        ),
        RunErrorCode::TimeCap => (
            "A conversa excedeu o tempo permitido.",
            "Tente de novo com uma pergunta mais específica.",
        ),
        RunErrorCode::Cancelled => (
            "A conversa foi cancelada.",
            "Envie outra pergunta quando quiser continuar.",
        ),
        RunErrorCode::Ungrounded => (
            "A resposta citou números sem origem nos seus dados.",
            "Tente de novo para consultar os dados necessários.",
        ),
    };
    RunError {
        code,
        message: message.to_string(),
        fix: fix.to_string(),
    }
}

/// O laço, com tudo o que uma rodada precisa saber antes de começar.
pub(crate) struct Runner<'a, A: ProviderAdapter> {
    pub pool: &'a SqlitePool,
    pub ctx: &'a Context,
    pub adapter: &'a A,
    pub pin: &'static ModelPin,
    pub limits: RunLimits,
    pub cancel: CancelToken,
    pub events: mpsc::Sender<RunEvent>,
}

impl<A: ProviderAdapter> Runner<'_, A> {
    async fn emit(&self, event: RunEvent, deadline: Instant, window: EventWindow) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let window_duration = match window {
            EventWindow::Ordinary => remaining,
            // O encerramento precisa de uma chance própria de chegar à interface mesmo quando a
            // rodada já consumiu seu prazo; sem ele, a tela ficaria sem saber que deve fechar.
            EventWindow::Closing => Duration::from_secs(1),
        };
        if window_duration.is_zero() {
            return;
        }

        if matches!(window, EventWindow::Closing) && self.events.capacity() == 0 {
            let events = self.events.clone();
            // A tentativa final de aviso não pode transformar a espera de quem perguntou em
            // espera pela interface, mas continua viva pela janela reservada ao encerramento.
            tokio::spawn(async move {
                let send = events.send(event);
                tokio::pin!(send);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                    _ = &mut send => {}
                }
            });
            return;
        }

        let send = self.events.send(event);
        tokio::pin!(send);
        match window {
            EventWindow::Ordinary => {
                tokio::select! {
                    _ = self.cancel.cancelled() => {}
                    _ = tokio::time::sleep(window_duration) => {}
                    _ = &mut send => {}
                }
            }
            EventWindow::Closing => {
                tokio::select! {
                    _ = tokio::time::sleep(window_duration) => {}
                    _ = &mut send => {}
                }
            }
        }
    }

    /// Roda a pergunta até a resposta validada — ou até o primeiro teto que fechar.
    pub(crate) async fn run(&self, round: Round<'_>) -> RunOutcome {
        let deadline = Instant::now() + self.limits.max_duration;
        // O consentimento é a primeira pergunta da rodada, antes do transcript e antes do adapter:
        // uma verificação feita mais tarde já teria montado o pedido com os dados de quem não
        // autorizou. Falha de leitura recusa igual — a garantia é fechada, não otimista.
        if consent::authorize(self.pool, self.pin).await.is_err() {
            let error = run_error(RunErrorCode::ConsentMissing);
            let stop = StopReason::ConsentMissing;
            let trace = vec![TraceEntry {
                turn: 0,
                attempt: 0,
                kind: TraceKind::Stopped,
                detail: format!("A rodada terminou com {stop:?}."),
            }];
            self.emit(RunEvent::Error(error), deadline, EventWindow::Closing)
                .await;
            self.emit(
                RunEvent::RunFinished { stop },
                deadline,
                EventWindow::Closing,
            )
            .await;
            return RunOutcome {
                answer: None,
                provenance: None,
                stop,
                turns: 0,
                tool_calls: 0,
                cost_micro_usd: 0,
                // A rodada recusada não falou com o provedor: não há custo, e não há lacuna.
                cost_declared: true,
                attempts: 0,
                transcript: vec![],
                trace,
            };
        }
        let mut transcript = round.history.to_vec();
        transcript.push(json!({"role": "user", "content": round.question}));
        let mut facts = grounding::Facts::new();
        facts.absorb_text(round.system);

        self.emit(
            RunEvent::RunStarted {
                model: self.pin.model,
                endpoint: self.pin.endpoint,
            },
            deadline,
            EventWindow::Ordinary,
        )
        .await;

        let mut answer = None;
        let mut answer_provenance = None;
        let mut trace = vec![];
        let mut turns = 0;
        let mut tool_calls = 0;
        let mut cost_micro_usd = 0;
        let mut cost_declared = true;
        let mut attempts = 0;
        let mut regenerations = 0;
        let mut terminal_error = None;
        let mut last_attempt_in_turn = 0;

        let stop = 'conversation: loop {
            if self.cancel.is_cancelled() {
                terminal_error = Some(run_error(RunErrorCode::Cancelled));
                break StopReason::Cancelled;
            }
            if Instant::now() >= deadline {
                self.cancel.cancel();
                terminal_error = Some(run_error(RunErrorCode::TimeCap));
                break StopReason::TimeCap;
            }
            if turns >= self.limits.max_turns {
                terminal_error = Some(run_error(RunErrorCode::TurnCap));
                break StopReason::TurnCap;
            }

            turns += 1;
            let mut attempt_in_turn = 0;
            let mut turn_usage = CollectedUsage::default();

            loop {
                // O consentimento é relido a cada tentativa, não só na abertura: uma rodada dura
                // até o teto de tempo, e quem revoga no meio dela espera que a conversa PARE de
                // falar com o provedor — não que termine o que já tinha começado.
                if consent::authorize(self.pool, self.pin).await.is_err() {
                    self.cancel.cancel();
                    terminal_error = Some(run_error(RunErrorCode::ConsentMissing));
                    break 'conversation StopReason::ConsentMissing;
                }
                if self.cancel.is_cancelled() {
                    terminal_error = Some(run_error(RunErrorCode::Cancelled));
                    break 'conversation StopReason::Cancelled;
                }
                if Instant::now() >= deadline {
                    self.cancel.cancel();
                    terminal_error = Some(run_error(RunErrorCode::TimeCap));
                    break 'conversation StopReason::TimeCap;
                }

                attempt_in_turn += 1;
                last_attempt_in_turn = attempt_in_turn;
                attempts += 1;
                trace.push(TraceEntry {
                    turn: turns,
                    attempt: attempt_in_turn,
                    kind: TraceKind::TurnStarted,
                    detail: "Turno aberto com o provedor pinado.".to_string(),
                });

                let opened = {
                    let tools = tool_declarations();
                    let spec = RunSpec {
                        pin: self.pin,
                        system: round.system,
                        messages: &transcript,
                        tools: &tools,
                        max_tokens: MAX_TOKENS_PER_TURN,
                    };
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let open = self.adapter.open(&spec, &self.cancel);
                    tokio::pin!(open);
                    tokio::select! {
                        // Interromper a abertura em voo deixa dinheiro em dúvida: o pedido pode
                        // ter chegado ao servidor e gerado, com o custo no stream que nunca
                        // abriu. Na dúvida, o total desta rodada deixa de valer como completo.
                        _ = self.cancel.cancelled() => {
                            cost_declared = false;
                            terminal_error = Some(run_error(RunErrorCode::Cancelled));
                            break 'conversation StopReason::Cancelled;
                        }
                        _ = tokio::time::sleep(remaining) => {
                            cost_declared = false;
                            self.cancel.cancel();
                            terminal_error = Some(run_error(RunErrorCode::TimeCap));
                            break 'conversation StopReason::TimeCap;
                        }
                        result = &mut open => result,
                    }
                };

                let mut receiver = match opened {
                    Ok(receiver) => receiver,
                    Err(error) => {
                        // Recusa RESPONDIDA não mexe no dinheiro: com status HTTP na mão, o
                        // corpo de erro não é stream e nada foi gerado nem cobrado. Falha SEM
                        // resposta é outra coisa — o estágio que o servidor alcançou é
                        // desconhecido, e o custo possível ficaria fora do total; a dúvida
                        // fecha, em qualquer tentativa.
                        if !error.responded {
                            cost_declared = false;
                        }
                        let detail = redaction::credentials(&error.message);
                        let decision = retry_decision(
                            &error.kind,
                            false,
                            attempt_in_turn,
                            &self.limits,
                            deadline.saturating_duration_since(Instant::now()),
                        );
                        trace.push(TraceEntry {
                            turn: turns,
                            attempt: attempt_in_turn,
                            kind: TraceKind::ProviderFailure,
                            detail,
                        });
                        match retry_or_stop(
                            decision,
                            &self.cancel,
                            deadline,
                            &mut trace,
                            turns,
                            attempt_in_turn,
                        )
                        .await
                        {
                            RetryWait::Retry => continue,
                            RetryWait::Cancelled => {
                                terminal_error = Some(run_error(RunErrorCode::Cancelled));
                                break 'conversation StopReason::Cancelled;
                            }
                            RetryWait::TimeCap => {
                                self.cancel.cancel();
                                terminal_error = Some(run_error(RunErrorCode::TimeCap));
                                break 'conversation StopReason::TimeCap;
                            }
                            RetryWait::GiveUp(code) => {
                                terminal_error = Some(run_error(code));
                                if code == RunErrorCode::TimeCap {
                                    self.cancel.cancel();
                                    break 'conversation StopReason::TimeCap;
                                }
                                break 'conversation StopReason::Failed;
                            }
                        }
                    }
                };

                let turn_read = consume_turn(
                    &mut receiver,
                    &self.cancel,
                    deadline,
                    &mut turn_usage,
                    &mut cost_micro_usd,
                )
                .await;
                // Lido a cada tentativa, não ao fim do turno: a tentativa que falhou some do
                // caminho de sucesso, e é ela quem pode ter consumido dinheiro sem declarar.
                cost_declared = cost_declared && !turn_usage.missing_cost;
                if cost_micro_usd > self.limits.max_cost_micro_usd {
                    self.cancel.cancel();
                    terminal_error = Some(run_error(RunErrorCode::CostCap));
                    break 'conversation StopReason::CostCap;
                }

                match turn_read {
                    TurnRead::Cancelled => {
                        terminal_error = Some(run_error(RunErrorCode::Cancelled));
                        break 'conversation StopReason::Cancelled;
                    }
                    TurnRead::TimeCap => {
                        self.cancel.cancel();
                        terminal_error = Some(run_error(RunErrorCode::TimeCap));
                        break 'conversation StopReason::TimeCap;
                    }
                    TurnRead::ProviderFailure {
                        error,
                        produced_events,
                    } => {
                        let detail = redaction::credentials(&error.message);
                        let decision = retry_decision(
                            &error.kind,
                            produced_events,
                            attempt_in_turn,
                            &self.limits,
                            deadline.saturating_duration_since(Instant::now()),
                        );
                        trace.push(TraceEntry {
                            turn: turns,
                            attempt: attempt_in_turn,
                            kind: TraceKind::ProviderFailure,
                            detail,
                        });
                        match retry_or_stop(
                            decision,
                            &self.cancel,
                            deadline,
                            &mut trace,
                            turns,
                            attempt_in_turn,
                        )
                        .await
                        {
                            RetryWait::Retry => continue,
                            RetryWait::Cancelled => {
                                terminal_error = Some(run_error(RunErrorCode::Cancelled));
                                break 'conversation StopReason::Cancelled;
                            }
                            RetryWait::TimeCap => {
                                self.cancel.cancel();
                                terminal_error = Some(run_error(RunErrorCode::TimeCap));
                                break 'conversation StopReason::TimeCap;
                            }
                            RetryWait::GiveUp(code) => {
                                terminal_error = Some(run_error(code));
                                if code == RunErrorCode::TimeCap {
                                    self.cancel.cancel();
                                    break 'conversation StopReason::TimeCap;
                                }
                                break 'conversation StopReason::Failed;
                            }
                        }
                    }
                    TurnRead::Completed(turn) => {
                        self.emit(
                            RunEvent::Usage(RunUsage {
                                model: self.pin.model,
                                endpoint: self.pin.endpoint,
                                prompt_tokens: turn_usage.prompt_tokens,
                                completion_tokens: turn_usage.completion_tokens,
                                cost_micro_usd: turn_usage.cost_micro_usd,
                                attempts: attempt_in_turn,
                            }),
                            deadline,
                            EventWindow::Ordinary,
                        )
                        .await;

                        if self.cancel.is_cancelled() {
                            terminal_error = Some(run_error(RunErrorCode::Cancelled));
                            break 'conversation StopReason::Cancelled;
                        }
                        if Instant::now() >= deadline {
                            self.cancel.cancel();
                            terminal_error = Some(run_error(RunErrorCode::TimeCap));
                            break 'conversation StopReason::TimeCap;
                        }

                        if turn.tool_calls.len() > 1 {
                            terminal_error = Some(run_error(RunErrorCode::ProtocolViolation));
                            break 'conversation StopReason::Failed;
                        }

                        if let Some(call) = turn.tool_calls.into_iter().next() {
                            if tool_calls + 1 > self.limits.max_tool_calls {
                                terminal_error = Some(run_error(RunErrorCode::ToolCallCap));
                                break 'conversation StopReason::ToolCallCap;
                            }

                            transcript.push(json!({
                                "role": "assistant",
                                "content": null,
                                "tool_calls": [{
                                    "id": call.id,
                                    "type": "function",
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.arguments,
                                    },
                                }],
                            }));

                            let assistant_call = transcript
                                .last()
                                .expect("a chamada do assistente acabou de entrar no transcript");
                            let function = &assistant_call["tool_calls"][0]["function"];
                            let name = function["name"]
                                .as_str()
                                .expect("o nome da ferramenta é texto do laço")
                                .to_string();
                            let arguments = function["arguments"]
                                .as_str()
                                .expect("os argumentos da ferramenta são texto do laço");
                            let parsed = serde_json::from_str::<Value>(arguments);

                            let envelope = match parsed {
                                Ok(arguments) => {
                                    self.emit(
                                        RunEvent::ToolStarted {
                                            id: call.id.clone(),
                                            tool: name.clone(),
                                        },
                                        deadline,
                                        EventWindow::Ordinary,
                                    )
                                    .await;
                                    let envelope = dispatch(
                                        self.pool,
                                        &ToolCall::new(name.clone(), arguments),
                                        self.ctx,
                                    )
                                    .await;
                                    self.emit(
                                        RunEvent::ToolFinished {
                                            id: call.id.clone(),
                                            tool: name.clone(),
                                            ok: envelope.ok,
                                        },
                                        deadline,
                                        EventWindow::Ordinary,
                                    )
                                    .await;
                                    trace.push(TraceEntry {
                                        turn: turns,
                                        attempt: attempt_in_turn,
                                        kind: TraceKind::ToolDispatched,
                                        detail: redaction::credentials(&format!(
                                            "A ferramenta \"{name}\" respondeu."
                                        )),
                                    });
                                    tool_calls += 1;
                                    envelope
                                }
                                Err(_) => {
                                    let error = ToolError::new(
                                        ErrorCode::InvalidArgument,
                                        "Os argumentos da ferramenta não formam JSON válido.",
                                        "Chame a ferramenta de novo com um objeto JSON válido.",
                                    );
                                    let envelope = refuse(self.pool, &name, self.ctx, error).await;
                                    trace.push(TraceEntry {
                                        turn: turns,
                                        attempt: attempt_in_turn,
                                        kind: TraceKind::ToolRefused,
                                        detail: redaction::credentials(&format!(
                                            "A ferramenta \"{name}\" recebeu argumentos inválidos."
                                        )),
                                    });
                                    envelope
                                }
                            };

                            let origin = if catalog::is_method_layer(&name) {
                                grounding::FactOrigin::Method
                            } else {
                                grounding::FactOrigin::Data
                            };
                            facts.absorb_envelope(&envelope, origin);
                            let payload = serde_json::to_string(&envelope)
                                .expect("o envelope da ferramenta é serializável");
                            transcript.push(json!({
                                "role": "tool",
                                "tool_call_id": call.id,
                                "content": untrusted(&payload),
                            }));
                            continue 'conversation;
                        }

                        match turn.reason {
                            FinishReason::Stop if !turn.text.is_empty() => {
                                let text = redaction::credentials(&turn.text);
                                let orphans = grounding::orphans(&text, &facts);
                                if orphans.is_empty() {
                                    let provenance = if grounding::cites_data(&text, &facts) {
                                        AnswerProvenance::Calculo
                                    } else {
                                        AnswerProvenance::Metodo
                                    };
                                    self.emit(
                                        RunEvent::AnswerReady {
                                            text: text.clone(),
                                            provenance,
                                        },
                                        deadline,
                                        EventWindow::Ordinary,
                                    )
                                    .await;
                                    if Instant::now() >= deadline {
                                        self.cancel.cancel();
                                        terminal_error = Some(run_error(RunErrorCode::TimeCap));
                                        break 'conversation StopReason::TimeCap;
                                    }
                                    transcript.push(json!({
                                        "role": "assistant",
                                        "content": text,
                                    }));
                                    answer = transcript
                                        .last()
                                        .and_then(|message| message["content"].as_str())
                                        .map(str::to_string);
                                    answer_provenance = answer.as_ref().map(|_| provenance);
                                    break 'conversation StopReason::Answered;
                                }

                                trace.push(TraceEntry {
                                    turn: turns,
                                    attempt: attempt_in_turn,
                                    kind: TraceKind::Regeneration,
                                    detail: redaction::credentials(&format!(
                                        "A resposta citou números sem origem: {}.",
                                        orphans.join(", ")
                                    )),
                                });
                                if regenerations >= self.limits.max_regenerations {
                                    terminal_error = Some(run_error(RunErrorCode::Ungrounded));
                                    break 'conversation StopReason::Ungrounded;
                                }
                                regenerations += 1;
                                transcript.push(json!({
                                    "role": "assistant",
                                    "content": text,
                                }));
                                transcript.push(json!({
                                    "role": "system",
                                    "content": format!(
                                        "Os números {} não têm origem nos fatos desta rodada. Refaça a resposta usando apenas números vindos de ferramenta ou chame a ferramenta que traz o número.",
                                        orphans.join(", ")
                                    ),
                                }));
                                continue 'conversation;
                            }
                            FinishReason::Stop if turn.text.is_empty() => {
                                terminal_error = Some(run_error(RunErrorCode::ProtocolViolation));
                                break 'conversation StopReason::Failed;
                            }
                            FinishReason::Stop => {
                                terminal_error = Some(run_error(RunErrorCode::ProtocolViolation));
                                break 'conversation StopReason::Failed;
                            }
                            FinishReason::ToolCalls => {
                                terminal_error = Some(run_error(RunErrorCode::ProtocolViolation));
                                break 'conversation StopReason::Failed;
                            }
                            FinishReason::Length
                            | FinishReason::ContentFilter
                            | FinishReason::Error
                            | FinishReason::Other => {
                                terminal_error = Some(run_error(RunErrorCode::ProviderRefused));
                                break 'conversation StopReason::Failed;
                            }
                        }
                    }
                }
            }
        };

        trace.push(TraceEntry {
            turn: turns,
            attempt: last_attempt_in_turn,
            kind: TraceKind::Stopped,
            detail: format!("A rodada terminou com {stop:?}."),
        });
        if let Some(error) = terminal_error {
            self.emit(RunEvent::Error(error), deadline, EventWindow::Closing)
                .await;
        }
        self.emit(
            RunEvent::RunFinished { stop },
            deadline,
            EventWindow::Closing,
        )
        .await;

        RunOutcome {
            answer,
            provenance: answer_provenance,
            stop,
            turns,
            tool_calls,
            cost_micro_usd,
            cost_declared,
            attempts,
            transcript,
            trace,
        }
    }
}

/// A moldura que marca resultado de ferramenta como dado NÃO confiável dentro do prompt.
///
/// O conteúdo de um lançamento é texto que alguém escreveu, e a defesa contra ele ser lido como
/// instrução é estrutural: o modelo vê a fronteira, as ferramentas são todas de leitura, e não há
/// canal de saída para onde uma instrução injetada mandaria dado.
pub(crate) fn untrusted(payload: &str) -> String {
    format!("<dados_de_ferramenta confiavel=\"nao\">\n{payload}\n</dados_de_ferramenta>")
}

#[cfg(test)]
mod tests;
