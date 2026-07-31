//! O vocabulário que atravessa o fio até a tela.
//!
//! O laço fala em tipos do domínio; a interface recebe JSON. A tradução é uma função pura, e é
//! por isso que ela mora aqui: o que sai da máquina para o webview é exercitável sem Tauri, sem
//! rede e sem chave — e o contrato que o frontend espelha fica sob teste de igualdade, não sob
//! confiança.
//!
//! Nada do provedor atravessa esta fronteira. Texto de erro cru é dado não confiável e fica no
//! rastro técnico, redigido; o que chega à tela é sempre mensagem NOSSA, com a saída concreta
//! junto — dizer o que falhou sem dizer o que fazer devolve a pessoa ao mesmo lugar.

use super::run::{AnswerProvenance, RunError, RunErrorCode, RunEvent, RunUsage, StopReason};
use serde::Serialize;

/// O evento como a tela o lê. `kind` é a etiqueta que o frontend usa para escolher a linha da
/// conversa; os demais campos são o que aquela linha precisa e nada além.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MiaScreenEvent {
    RunStarted {
        run_id: String,
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
    /// A proposta de lançamento, assinada pela ferramenta e ainda não aprovada. O gesto de
    /// aprovar acontece fora do laço: o que trafega aqui é leitura.
    ProposalReady {
        id: String,
        proposal: serde_json::Value,
    },
    AnswerReady {
        text: String,
        provenance: &'static str,
    },
    Usage {
        model: &'static str,
        endpoint: &'static str,
        prompt_tokens: u32,
        completion_tokens: u32,
        /// Nulo é lacuna declarada. Zero afirmaria uma rodada gratuita, e a linha de
        /// transparência passaria a mentir justamente quando o custo é desconhecido.
        cost_micro_usd: Option<i64>,
        attempts: u32,
    },
    Error {
        /// A classe da falha, para a tela escolher o gesto que oferece.
        code: &'static str,
        message: String,
        fix: String,
    },
    RunFinished {
        stop: &'static str,
    },
}

/// A classe de erro no vocabulário do fio.
fn error_code(code: RunErrorCode) -> &'static str {
    match code {
        RunErrorCode::ConsentMissing => "consent_missing",
        RunErrorCode::ProviderUnavailable => "provider_unavailable",
        RunErrorCode::RateLimited => "rate_limited",
        RunErrorCode::ProviderRefused => "provider_refused",
        RunErrorCode::ProtocolViolation => "protocol_violation",
        RunErrorCode::TurnCap => "turn_cap",
        RunErrorCode::ToolCallCap => "tool_call_cap",
        RunErrorCode::CostCap => "cost_cap",
        RunErrorCode::TimeCap => "time_cap",
        RunErrorCode::Cancelled => "cancelled",
        RunErrorCode::Ungrounded => "ungrounded",
    }
}

fn provenance(provenance: AnswerProvenance) -> &'static str {
    match provenance {
        AnswerProvenance::Metodo => "metodo",
        AnswerProvenance::Calculo => "calculo",
    }
}

fn stop_reason(stop: StopReason) -> &'static str {
    match stop {
        StopReason::ConsentMissing => "consent_missing",
        StopReason::Answered => "answered",
        StopReason::TurnCap => "turn_cap",
        StopReason::ToolCallCap => "tool_call_cap",
        StopReason::CostCap => "cost_cap",
        StopReason::TimeCap => "time_cap",
        StopReason::Cancelled => "cancelled",
        StopReason::Ungrounded => "ungrounded",
        StopReason::Failed => "failed",
    }
}

/// A tradução. O `run_id` entra só no evento de abertura: o canal é de uma rodada, e repetir a
/// identidade em cada linha seria ruído que a tela já tem.
pub(crate) fn screen_event(run_id: &str, event: RunEvent) -> MiaScreenEvent {
    match event {
        RunEvent::RunStarted { model, endpoint } => MiaScreenEvent::RunStarted {
            run_id: run_id.to_string(),
            model,
            endpoint,
        },
        RunEvent::ToolStarted { id, tool } => MiaScreenEvent::ToolStarted { id, tool },
        RunEvent::ToolFinished { id, tool, ok } => MiaScreenEvent::ToolFinished { id, tool, ok },
        RunEvent::ProposalReady { id, proposal } => MiaScreenEvent::ProposalReady { id, proposal },
        RunEvent::AnswerReady {
            text,
            provenance: origin,
        } => MiaScreenEvent::AnswerReady {
            text,
            provenance: provenance(origin),
        },
        RunEvent::Usage(RunUsage {
            model,
            endpoint,
            prompt_tokens,
            completion_tokens,
            cost_micro_usd,
            attempts,
        }) => MiaScreenEvent::Usage {
            model,
            endpoint,
            prompt_tokens,
            completion_tokens,
            cost_micro_usd,
            attempts,
        },
        RunEvent::Error(RunError { code, message, fix }) => MiaScreenEvent::Error {
            code: error_code(code),
            message,
            fix,
        },
        RunEvent::RunFinished { stop } => MiaScreenEvent::RunFinished {
            stop: stop_reason(stop),
        },
    }
}

#[cfg(test)]
mod tests;
