//! O contrato do fio, exercitado como o frontend o lê: JSON, campo a campo.

use super::*;
use crate::mia::run::{RunError, RunErrorCode, RunEvent, RunUsage, StopReason, run_error};
use serde_json::json;

const RUN_ID: &str = "run-1";

fn wire(event: RunEvent) -> serde_json::Value {
    serde_json::to_value(screen_event(RUN_ID, event)).expect("o evento de tela é serializável")
}

#[test]
fn a_abertura_declara_a_rodada_o_modelo_e_o_endpoint() {
    assert_eq!(
        wire(RunEvent::RunStarted {
            model: "openai/gpt-5.6-luna",
            endpoint: "openai",
        }),
        json!({
            "kind": "run_started",
            "run_id": "run-1",
            "model": "openai/gpt-5.6-luna",
            "endpoint": "openai",
        })
    );
}

#[test]
fn a_leitura_publica_o_comeco_e_o_fim_com_o_mesmo_identificador() {
    assert_eq!(
        wire(RunEvent::ToolStarted {
            id: "call-1".to_string(),
            tool: "get_financial_snapshot".to_string(),
        }),
        json!({
            "kind": "tool_started",
            "id": "call-1",
            "tool": "get_financial_snapshot",
        })
    );
    assert_eq!(
        wire(RunEvent::ToolFinished {
            id: "call-1".to_string(),
            tool: "get_financial_snapshot".to_string(),
            ok: false,
        }),
        json!({
            "kind": "tool_finished",
            "id": "call-1",
            "tool": "get_financial_snapshot",
            "ok": false,
        })
    );
}

#[test]
fn a_proposta_atravessa_inteira_para_a_tela() {
    assert_eq!(
        wire(RunEvent::ProposalReady {
            id: "call-2".to_string(),
            proposal: json!({"tool": "propose_transaction", "ok": true}),
        }),
        json!({
            "kind": "proposal_ready",
            "id": "call-2",
            "proposal": {"tool": "propose_transaction", "ok": true},
        })
    );
}

#[test]
fn a_resposta_carrega_a_proveniencia_no_vocabulario_da_tela() {
    assert_eq!(
        wire(RunEvent::AnswerReady {
            text: "Você gastou R$ 81,01.".to_string(),
            provenance: AnswerProvenance::Calculo,
        }),
        json!({
            "kind": "answer_ready",
            "text": "Você gastou R$ 81,01.",
            "provenance": "calculo",
        })
    );
    assert_eq!(
        wire(RunEvent::AnswerReady {
            text: "O método separa economia de custo de vida.".to_string(),
            provenance: AnswerProvenance::Metodo,
        })["provenance"],
        json!("metodo")
    );
}

#[test]
fn a_linha_de_transparencia_traz_custo_provedor_e_tentativas() {
    assert_eq!(
        wire(RunEvent::Usage(RunUsage {
            model: "openai/gpt-5.6-luna",
            endpoint: "openai",
            prompt_tokens: 1_200,
            completion_tokens: 80,
            cost_micro_usd: Some(2_600),
            attempts: 2,
        })),
        json!({
            "kind": "usage",
            "model": "openai/gpt-5.6-luna",
            "endpoint": "openai",
            "prompt_tokens": 1200,
            "completion_tokens": 80,
            "cost_micro_usd": 2600,
            "attempts": 2,
        })
    );
}

/// Custo ausente é lacuna, nunca gratuidade: um zero aqui faria a transparência afirmar que a
/// rodada não custou nada.
#[test]
fn custo_ausente_chega_nulo_e_nunca_zero() {
    let usage = wire(RunEvent::Usage(RunUsage {
        model: "openai/gpt-5.6-luna",
        endpoint: "openai",
        prompt_tokens: 10,
        completion_tokens: 2,
        cost_micro_usd: None,
        attempts: 1,
    }));

    assert_eq!(usage["cost_micro_usd"], json!(null));
    assert!(usage.get("cost_micro_usd").is_some());
}

#[test]
fn o_encerramento_nomeia_o_motivo() {
    assert_eq!(
        wire(RunEvent::RunFinished {
            stop: StopReason::Answered,
        }),
        json!({"kind": "run_finished", "stop": "answered"})
    );
    assert_eq!(
        wire(RunEvent::RunFinished {
            stop: StopReason::Cancelled,
        })["stop"],
        json!("cancelled")
    );
}

#[test]
fn o_limite_de_taxa_chega_como_recusa_honesta_com_saida() {
    assert_eq!(
        wire(RunEvent::Error(run_error(RunErrorCode::RateLimited))),
        json!({
            "kind": "error",
            "code": "rate_limited",
            "message": "O provedor limitou temporariamente esta conversa.",
            "fix": "Aguarde alguns instantes antes de tentar de novo.",
        })
    );
}

/// Toda classe da taxonomia chega nomeada, com texto nosso e com um gesto possível. Uma classe
/// sem saída termina a conversa numa parede.
#[test]
fn toda_classe_de_erro_chega_nomeada_e_com_saida() {
    let codes = [
        (RunErrorCode::ConsentMissing, "consent_missing"),
        (RunErrorCode::ProviderUnavailable, "provider_unavailable"),
        (RunErrorCode::RateLimited, "rate_limited"),
        (RunErrorCode::ProviderRefused, "provider_refused"),
        (RunErrorCode::ProtocolViolation, "protocol_violation"),
        (RunErrorCode::TurnCap, "turn_cap"),
        (RunErrorCode::ToolCallCap, "tool_call_cap"),
        (RunErrorCode::CostCap, "cost_cap"),
        (RunErrorCode::TimeCap, "time_cap"),
        (RunErrorCode::Cancelled, "cancelled"),
        (RunErrorCode::Ungrounded, "ungrounded"),
        (RunErrorCode::ContextCap, "context_cap"),
    ];

    for (code, name) in codes {
        let event = wire(RunEvent::Error(run_error(code)));
        assert_eq!(event["code"], json!(name));
        assert!(
            !event["message"]
                .as_str()
                .expect("a mensagem é texto")
                .is_empty()
        );
        assert!(!event["fix"].as_str().expect("a saída é texto").is_empty());
    }
}

/// A chave nunca atravessa o fio, e o texto cru do provedor tampouco: o erro publicável carrega
/// mensagem nossa, e é ela — não o corpo de erro do outro lado — que a tela lê.
#[test]
fn nenhum_evento_carrega_a_chave_nem_o_texto_cru_do_provedor() {
    let key = "sk-or-v1-fixture1234567890";
    let cru = format!("HTTP 401 authorization: Bearer {key}");

    let publicado = wire(RunEvent::Error(RunError {
        code: RunErrorCode::ProviderRefused,
        message: run_error(RunErrorCode::ProviderRefused).message,
        fix: run_error(RunErrorCode::ProviderRefused).fix,
    }))
    .to_string();

    assert!(!publicado.contains(key));
    assert!(!publicado.contains(&cru));
    assert!(publicado.contains("O provedor recusou concluir esta resposta."));
}
