//! O stream do provedor traduzido para o domínio interno.
//!
//! A tradução é função pura de linha em eventos: o laço da conversa nunca lê o formato de fio, e
//! o formato de fio nunca decide o que a rodada faz. Duas regras mandam no parser — chamada de
//! ferramenta só existe quando fecha por inteiro, e falha carrega a taxonomia que decide a
//! retentativa. As duas existem porque o erro do provedor chega com HTTP 200, no meio do stream,
//! depois de já ter entregado texto.

use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) enum ProviderEvent {
    TextDelta(String),
    ToolCallComplete {
        id: String,
        name: String,
        arguments: String,
    },
    Usage(Usage),
    Finished {
        reason: FinishReason,
        native: Option<String>,
    },
    Failed(ProviderError),
}

pub(crate) struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Custo ausente é lacuna declarada, nunca zero, que afirmaria uma rodada gratuita.
    pub cost_micro_usd: Option<i64>,
}

pub(crate) enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Other,
}

pub(crate) struct ProviderError {
    pub kind: ErrorKind,
    /// Texto vindo do provedor, que pode ecoar trecho enviado e é dado não confiável: serve ao
    /// rastro técnico, nunca é publicado como resposta ou colado no texto que o modelo relê.
    pub message: String,
    /// O servidor chegou a responder — qualquer status HTTP. É o fato que separa, na fronteira
    /// de ABERTURA, recusa comprovada de falha incerta: com resposta, o corpo de erro não é
    /// stream e nada foi gerado nem cobrado; sem resposta, o pedido pode ter alcançado o
    /// servidor e gerado custo que só viria no stream que nunca abriu — e dinheiro em dúvida
    /// fecha. Depois de o stream abrir, quem contabiliza é a linha de uso, e este campo deixa
    /// de decidir.
    pub responded: bool,
}

/// A taxonomia escolhe a próxima ação do laço: cada variante pede uma resposta diferente.
pub(crate) enum ErrorKind {
    RateLimited { retry_after_secs: Option<u64> },
    Transient,
    Permanent,
    Malformed,
}

#[derive(Default)]
struct ToolCallState {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

pub(crate) struct StreamParser {
    tool_calls: BTreeMap<usize, ToolCallState>,
    saw_finish: bool,
    done: bool,
    failed: bool,
}

impl StreamParser {
    pub(crate) fn new() -> Self {
        Self {
            tool_calls: BTreeMap::new(),
            saw_finish: false,
            done: false,
            failed: false,
        }
    }

    /// Consome uma linha do stream e devolve os eventos que ela fecha.
    pub(crate) fn push(&mut self, line: &str) -> Vec<ProviderEvent> {
        if self.failed || self.done || line.trim().is_empty() || line.starts_with(':') {
            return vec![];
        }

        let Some(data) = line.strip_prefix("data:") else {
            return vec![];
        };
        // O fim de linha do transporte não é conteúdo do evento SSE.
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return vec![];
        }

        let chunk: Value = match serde_json::from_str(data) {
            Ok(chunk) => chunk,
            Err(_) => return self.fail(ErrorKind::Malformed, "O stream trouxe JSON inválido."),
        };

        if let Some(error) = chunk.get("error") {
            let (kind, message) = stream_error(error);
            return self.fail(kind, message);
        }

        if chunk
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|choices| choices.len() > 1)
        {
            return self.fail(
                ErrorKind::Malformed,
                "O stream trouxe mais de uma escolha na mesma rodada.",
            );
        }

        let mut events = Vec::new();
        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());

        if let Some(content) = choice
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            .filter(|content| !content.is_empty())
        {
            events.push(ProviderEvent::TextDelta(content.to_string()));
        }

        if let Some(calls) = choice
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array)
        {
            for call in calls {
                let Some(index) = call.get("index").and_then(Value::as_u64) else {
                    return self.fail(
                        ErrorKind::Malformed,
                        "Uma chamada de ferramenta não tem índice.",
                    );
                };
                let Ok(index) = usize::try_from(index) else {
                    return self.fail(
                        ErrorKind::Malformed,
                        "O índice da ferramenta não cabe nesta plataforma.",
                    );
                };
                let state = self.tool_calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    state.id = Some(id.to_string());
                }
                if let Some(name) = call
                    .get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                {
                    state.name = Some(name.to_string());
                }
                if let Some(arguments) = call
                    .get("function")
                    .and_then(|function| function.get("arguments"))
                    .and_then(Value::as_str)
                {
                    state.arguments.push_str(arguments);
                }
            }
        }

        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
        {
            // Só `length` prova que a geração foi interrompida no teto; os demais motivos
            // fecham uma resposta íntegra, mesmo quando a chamada de ferramenta veio no fim.
            if reason == "length" && !self.tool_calls.is_empty() {
                self.tool_calls.clear();
                return self.fail(
                    ErrorKind::Malformed,
                    "A rodada foi cortada no teto de tokens.",
                );
            }
            let completed = match self.complete_tool_calls() {
                Ok(completed) => completed,
                Err((kind, message)) => return self.fail(kind, message),
            };
            events.extend(completed);
            self.saw_finish = true;
            events.push(ProviderEvent::Finished {
                reason: finish_reason(reason),
                native: choice
                    .and_then(|choice| choice.get("native_finish_reason"))
                    .or_else(|| chunk.get("native_finish_reason"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }

        if let Some(usage) = chunk.get("usage").and_then(usage) {
            events.push(ProviderEvent::Usage(usage));
        }

        events
    }

    /// Fecha o stream: evento final ausente deixa a rodada incompleta e retentável.
    pub(crate) fn finish(&mut self) -> Vec<ProviderEvent> {
        if self.failed || self.saw_finish || self.done {
            return vec![];
        }
        // A chamada pendente morre com a rodada, mesmo íntegra: sem motivo final o turno não
        // fechou, e despachar ferramenta dele executaria duas vezes o que a retentativa repete.
        self.tool_calls.clear();
        self.fail(
            ErrorKind::Transient,
            "O stream fechou no meio da rodada, sem motivo final.",
        )
    }

    fn complete_tool_calls(&mut self) -> Result<Vec<ProviderEvent>, (ErrorKind, String)> {
        let calls = std::mem::take(&mut self.tool_calls);
        calls
            .into_values()
            .map(|call| {
                let id = call.id.ok_or_else(|| {
                    (
                        ErrorKind::Malformed,
                        "Uma chamada de ferramenta terminou sem identificador.".to_string(),
                    )
                })?;
                let name = call.name.ok_or_else(|| {
                    (
                        ErrorKind::Malformed,
                        "Uma chamada de ferramenta terminou sem nome.".to_string(),
                    )
                })?;
                // Ferramenta que não declara parâmetro chega com os argumentos vazios; o objeto
                // vazio é a leitura fiel disso. Recusar aqui deixaria de fora justamente as
                // perguntas que não pedem recorte, que são as mais comuns.
                let arguments = if call.arguments.trim().is_empty() {
                    "{}".to_string()
                } else {
                    call.arguments
                };
                if serde_json::from_str::<Value>(&arguments).is_err() {
                    return Err((
                        ErrorKind::Malformed,
                        "Os argumentos da ferramenta terminaram como JSON inválido.".to_string(),
                    ));
                }
                Ok(ProviderEvent::ToolCallComplete {
                    id,
                    name,
                    arguments,
                })
            })
            .collect()
    }

    fn fail(&mut self, kind: ErrorKind, message: impl Into<String>) -> Vec<ProviderEvent> {
        self.failed = true;
        vec![ProviderEvent::Failed(ProviderError {
            kind,
            message: message.into(),
            // O parser só existe depois de o stream abrir: houve resposta por definição.
            responded: true,
        })]
    }
}

fn finish_reason(reason: &str) -> FinishReason {
    match reason {
        "stop" => FinishReason::Stop,
        "tool_calls" => FinishReason::ToolCalls,
        "length" => FinishReason::Length,
        "content_filter" => FinishReason::ContentFilter,
        "error" => FinishReason::Error,
        _ => FinishReason::Other,
    }
}

fn usage(raw: &Value) -> Option<Usage> {
    Some(Usage {
        prompt_tokens: u32::try_from(raw.get("prompt_tokens")?.as_u64()?).ok()?,
        completion_tokens: u32::try_from(raw.get("completion_tokens")?.as_u64()?).ok()?,
        cost_micro_usd: raw.get("cost").and_then(cost_micro_usd),
    })
}

/// Custo não negativo em milionésimos de dólar, truncado para baixo.
///
/// A leitura é textual porque o decimal do provedor não é representável em binário: multiplicar
/// `0.07` por um milhão em ponto flutuante cai abaixo de setenta mil, e o truncamento comeria um
/// micro a cada rodada. Custo ínfimo chega em notação exponencial, e só ele passa pelo caminho
/// flutuante — ali o erro de representação é menor que o micro que o próprio truncamento
/// descarta. Valor negativo ou fora da faixa é uma lacuna, pois convertê-lo inventaria gasto.
fn cost_micro_usd(cost: &Value) -> Option<i64> {
    let raw = cost.as_number()?.to_string();
    if raw.starts_with('-') {
        return None;
    }
    if raw.contains(['e', 'E']) {
        let micros = cost.as_f64()? * 1_000_000.0;
        if !micros.is_finite() || micros < 0.0 || micros >= i64::MAX as f64 {
            return None;
        }
        return Some(micros.floor() as i64);
    }
    let (whole, fraction) = match raw.split_once('.') {
        Some(parts) => parts,
        None => (raw.as_str(), ""),
    };
    let whole: i64 = whole.parse().ok()?;
    let micros = fraction
        .chars()
        .take(6)
        .collect::<String>()
        .parse::<i64>()
        .ok()
        .unwrap_or(0)
        * 10_i64.pow(6_u32.saturating_sub(fraction.len().min(6) as u32));
    whole.checked_mul(1_000_000)?.checked_add(micros)
}

fn stream_error(error: &Value) -> (ErrorKind, String) {
    let code = error
        .get("code")
        .and_then(|code| code.as_u64().or_else(|| code.as_str()?.parse().ok()));
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("O provedor encerrou o stream sem mensagem.")
        .to_string();
    let kind = match code {
        Some(429) => ErrorKind::RateLimited {
            retry_after_secs: error
                .get("metadata")
                .and_then(|metadata| metadata.get("retry_after"))
                .and_then(|retry_after| {
                    retry_after
                        .as_u64()
                        .or_else(|| retry_after.as_str()?.parse().ok())
                }),
        },
        Some(408 | 500 | 502 | 503 | 504) => ErrorKind::Transient,
        _ => ErrorKind::Permanent,
    };
    (kind, message)
}
