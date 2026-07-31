//! Suíte do adaptador do provedor: exercita a requisição, o stream e os pins no fio local.

use super::drift::{Drift, verify, verify_all};
use super::pins::{PINS, PinRole, default_pin, pin};
use super::request::{PreparedRequest, RunSpec, ToolDeclaration, build};
use super::stream::{ErrorKind, ProviderEvent, StreamParser};
use serde_json::{Value, json};

fn candidate_pin() -> &'static super::pins::ModelPin {
    pin("openai/gpt-5.6-luna@medium").expect("pin candidato declarado")
}

fn request_for(
    pin: &'static super::pins::ModelPin,
    tools: Vec<ToolDeclaration>,
) -> PreparedRequest {
    let transcript = vec![json!({"role": "user", "content": "transcrito nativo"})];
    build(&RunSpec {
        pin,
        system: "Sistema local.",
        messages: &transcript,
        tools: &tools,
        max_tokens: 700,
    })
}

fn header<'a>(request: &'a PreparedRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn sse(chunk: Value) -> String {
    format!("data: {chunk}")
}

fn tool_fragment(index: usize, id: Option<&str>, name: Option<&str>, arguments: &str) -> Value {
    let mut function = serde_json::Map::new();
    if let Some(name) = name {
        function.insert("name".to_string(), json!(name));
    }
    function.insert("arguments".to_string(), json!(arguments));

    let mut call = serde_json::Map::new();
    call.insert("index".to_string(), json!(index));
    if let Some(id) = id {
        call.insert("id".to_string(), json!(id));
    }
    call.insert("function".to_string(), Value::Object(function));
    Value::Object(call)
}

fn tool_chunk(calls: Vec<Value>) -> String {
    sse(json!({"choices": [{"delta": {"tool_calls": calls}}]}))
}

fn finish_chunk(reason: &str) -> String {
    sse(json!({"choices": [{"finish_reason": reason}]}))
}

/// `zdr` só sai para o pin que prova a garantia pelo catálogo de retenção zero — o teto de
/// referência, na matriz vigente.
#[test]
fn request_requires_zero_data_retention_for_a_zero_retention_pin() {
    let zero_retention_pin =
        pin("openai/gpt-5.6-sol@medium").expect("pin de retenção zero declarado");
    let request = request_for(zero_retention_pin, vec![]);

    assert_eq!(
        request
            .body
            .pointer("/provider/zdr")
            .and_then(Value::as_bool),
        Some(true)
    );
}

/// Um pin em opt-out deliberado ([`super::pins::Retention::ProviderPolicy`]) troca a prova do
/// catálogo pela política do operador — mandar `zdr` mesmo assim excluiria do roteamento o próprio
/// endpoint que o pin escolheu, que não está nesse catálogo.
#[test]
fn request_omits_zdr_for_a_provider_policy_opt_out_pin() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(request.body.pointer("/provider/zdr"), None);
}

#[test]
fn request_denies_provider_data_collection() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(
        request
            .body
            .pointer("/provider/data_collection")
            .and_then(Value::as_str),
        Some("deny")
    );
}

#[test]
fn request_allows_only_the_pinned_endpoint() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(
        request.body.pointer("/provider/only"),
        Some(&json!([default_pin().endpoint]))
    );
}

#[test]
fn request_disallows_provider_fallbacks() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(
        request
            .body
            .pointer("/provider/allow_fallbacks")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn request_requires_declared_provider_parameters() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(
        request
            .body
            .pointer("/provider/require_parameters")
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn request_disables_edge_response_cache() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(header(&request, "x-openrouter-cache"), Some("false"));
}

#[test]
fn request_uses_the_pinned_model_for_default_and_candidate() {
    let default_request = request_for(default_pin(), vec![]);
    let candidate_request = request_for(candidate_pin(), vec![]);

    assert_eq!(
        default_request.body.get("model").and_then(Value::as_str),
        Some(default_pin().model)
    );
    assert_eq!(
        candidate_request.body.get("model").and_then(Value::as_str),
        Some(candidate_pin().model)
    );
}

#[test]
fn request_sends_the_pins_declared_reasoning_effort() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(
        request
            .body
            .pointer("/reasoning/effort")
            .and_then(Value::as_str),
        Some("medium")
    );
}

/// Nenhum pin da matriz declara beta header, e o cabeçalho só existe quando declarado: um beta
/// enviado sem dono seria parâmetro fantasma — o mecanismo fica, por pin, para o endpoint que
/// vier a exigir um.
#[test]
fn request_sends_beta_headers_only_when_the_pin_declares() {
    for pin in PINS {
        assert!(
            pin.beta_headers.is_empty(),
            "o pin {} declara beta",
            pin.model
        );
        let request = request_for(pin, vec![]);
        assert_eq!(
            header(&request, "x-anthropic-beta"),
            None,
            "o pin {} não declara beta e o cabeçalho não sai",
            pin.model
        );
    }
}

#[test]
fn request_never_serializes_credentials() {
    let request = request_for(default_pin(), vec![]);
    let serialized = serde_json::to_string(&json!({
        "headers": request.headers,
        "body": request.body,
    }))
    .unwrap();

    for forbidden in ["Authorization", "api_key", "Bearer", "sk-"] {
        assert!(
            !serialized.contains(forbidden),
            "a requisição serializada contém {forbidden}"
        );
    }
}

/// O corpo NÃO pede uma chamada por turno ao provedor: sob parâmetros exigidos, um campo que os
/// endpoints não anunciam derruba o roteamento e nenhuma rodada acontece. Quem mantém a invariante
/// é o laço, que fecha a rodada ao ver a segunda chamada.
#[test]
fn request_omits_the_fields_the_wire_does_not_need() {
    let request = request_for(default_pin(), vec![]);

    // `parallel_tool_calls` derruba o roteamento sob `require_parameters`; `usage` é sem efeito
    // — a linha de uso vem por padrão no stream (verificado 2026-07, por execução com e sem).
    assert!(request.body.get("parallel_tool_calls").is_none());
    assert!(request.body.get("usage").is_none());
}

/// O teto de saída sai com o nome que o endpoint anuncia — os dois nunca juntos: sob
/// `require_parameters`, o nome que o endpoint não anuncia é rodada recusada pelo roteador.
#[test]
fn request_sends_the_token_cap_field_each_pin_declares() {
    for pin in PINS {
        let request = request_for(pin, vec![]);
        let declared = pin.token_cap.field();
        let other = match declared {
            "max_tokens" => "max_completion_tokens",
            _ => "max_tokens",
        };
        assert_eq!(
            request.body.get(declared).and_then(Value::as_u64),
            Some(700),
            "o pin {} envia {declared}",
            pin.model
        );
        assert!(
            request.body.get(other).is_none(),
            "o pin {} não envia {other}",
            pin.model
        );
    }

    // A matriz não é uniforme — se fosse, o campo por pin seria decoração.
    let caps: std::collections::BTreeSet<&str> =
        PINS.iter().map(|pin| pin.token_cap.field()).collect();
    assert_eq!(caps.len(), 2);
}

#[test]
fn request_preserves_strict_tools_and_omits_an_empty_list() {
    let tool = ToolDeclaration {
        name: "get_forecast".to_string(),
        description: "Lê a projeção.".to_string(),
        parameters: json!({"type": "object", "properties": {}}),
        strict: true,
    };
    let with_tool = request_for(default_pin(), vec![tool]);
    let without_tools = request_for(default_pin(), vec![]);

    assert_eq!(
        with_tool
            .body
            .pointer("/tools/0/function/strict")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(without_tools.body.get("tools").is_none());
}

#[test]
fn request_puts_system_message_before_the_unchanged_transcript() {
    let request = request_for(default_pin(), vec![]);
    let messages = request
        .body
        .get("messages")
        .and_then(Value::as_array)
        .unwrap();

    assert_eq!(
        messages[0],
        json!({"role": "system", "content": "Sistema local."})
    );
    assert_eq!(
        messages[1],
        json!({"role": "user", "content": "transcrito nativo"})
    );
}

#[test]
fn stream_emits_text_deltas_in_order() {
    let mut parser = StreamParser::new();
    let first = parser.push(&sse(json!({"choices": [{"delta": {"content": "Olá"}}]})));
    let second = parser.push(&sse(json!({"choices": [{"delta": {"content": " mundo"}}]})));

    assert!(matches!(first.as_slice(), [ProviderEvent::TextDelta(text)] if text == "Olá"));
    assert!(matches!(second.as_slice(), [ProviderEvent::TextDelta(text)] if text == " mundo"));
}

#[test]
fn stream_refuses_a_chunk_with_more_than_one_choice() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [{"delta": {"content": "primeira"}}, {"delta": {"content": "segunda"}}]
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Malformed)
    ));
}

#[test]
fn stream_ignores_blank_lines_and_keep_alives() {
    let mut parser = StreamParser::new();

    assert!(parser.push("").is_empty());
    assert!(parser.push(": OPENROUTER PROCESSING").is_empty());
}

#[test]
fn stream_combines_three_tool_call_fragments_before_emitting() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{\"month\":",
    )]));
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        None,
        None,
        "\"2026-07\"",
    )]));
    parser.push(&tool_chunk(vec![tool_fragment(0, None, None, "}")]));
    let events = parser.push(&finish_chunk("tool_calls"));

    assert!(matches!(
        events.as_slice(),
        [
            ProviderEvent::ToolCallComplete { id, name, arguments },
            ProviderEvent::Finished { .. }
        ] if id == "call-1" && name == "get_forecast" && arguments == "{\"month\":\"2026-07\"}"
    ));
}

#[test]
fn stream_keeps_two_tool_call_indices_separate() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![
        tool_fragment(0, Some("call-a"), Some("get_forecast"), "{\"year\":2026}"),
        tool_fragment(1, Some("call-b"), Some("get_accounts"), "{\"all\":true}"),
    ]));
    let events = parser.push(&finish_chunk("tool_calls"));

    assert!(matches!(
        events.as_slice(),
        [
            ProviderEvent::ToolCallComplete { id: first_id, name: first_name, .. },
            ProviderEvent::ToolCallComplete { id: second_id, name: second_name, .. },
            ProviderEvent::Finished { .. }
        ] if first_id == "call-a" && first_name == "get_forecast" && second_id == "call-b" && second_name == "get_accounts"
    ));
}

#[test]
fn stream_cut_during_a_tool_call_never_emits_a_partial_call() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{\"month\":",
    )]));
    let events = parser.finish();

    // Uma chamada parcial não tem semântica segura: executar só a parte legível inventaria intenção.
    assert!(matches!(events.as_slice(), [ProviderEvent::Failed(_)]));
}

#[test]
fn stream_cut_after_a_whole_tool_call_still_discards_it() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{}",
    )]));
    let events = parser.finish();

    // Argumento íntegro não prova turno fechado: a retentativa repetiria a mesma chamada.
    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Transient)
    ));
}

#[test]
fn stream_reads_empty_tool_arguments_as_an_empty_object() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_financial_snapshot"),
        "",
    )]));
    let events = parser.push(&finish_chunk("tool_calls"));

    // Ferramenta sem parâmetro é o caso mais comum; recusá-la aqui calaria a conversa inteira.
    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::ToolCallComplete { arguments, .. }, ProviderEvent::Finished { .. }]
            if arguments == "{}"
    ));
}

#[test]
fn stream_rejects_unclosed_tool_arguments_as_malformed() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{\"month\":",
    )]));
    let events = parser.push(&finish_chunk("tool_calls"));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Malformed)
    ));
}

#[test]
fn stream_emits_completed_calls_before_tool_call_finish_reason() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{}",
    )]));
    let events = parser.push(&finish_chunk("tool_calls"));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::ToolCallComplete { .. }, ProviderEvent::Finished { reason, .. }]
            if matches!(reason, super::stream::FinishReason::ToolCalls)
    ));
}

#[test]
fn stream_discards_tool_calls_cut_by_the_token_ceiling() {
    let mut parser = StreamParser::new();
    parser.push(&tool_chunk(vec![tool_fragment(
        0,
        Some("call-1"),
        Some("get_forecast"),
        "{}",
    )]));
    let events = parser.push(&finish_chunk("length"));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Malformed)
    ));
}

#[test]
fn stream_converts_usage_cost_to_truncated_micro_dollars() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [],
        "usage": {"prompt_tokens": 120, "completion_tokens": 45, "cost": 0.123456789}
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Usage(usage)]
            if usage.prompt_tokens == 120 && usage.completion_tokens == 45 && usage.cost_micro_usd == Some(123_456)
    ));
}

#[test]
fn stream_keeps_the_usage_line_when_the_cost_arrives_in_exponent_notation() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [],
        "usage": {"prompt_tokens": 8, "completion_tokens": 2, "cost": 1.2e-6}
    })));

    // Custo ínfimo não pode derrubar a linha de transparência: rodada sem custo declarado
    // esconderia justamente o que a pessoa autorizou ao ligar a conversa.
    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Usage(usage)] if usage.cost_micro_usd == Some(1)
    ));
}

#[test]
fn stream_reports_usage_without_cost_as_a_missing_amount() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [],
        "usage": {"prompt_tokens": 120, "completion_tokens": 45}
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Usage(usage)]
            if usage.prompt_tokens == 120
                && usage.completion_tokens == 45
                && usage.cost_micro_usd.is_none()
    ));
}

#[test]
fn stream_rejects_a_negative_cost_as_missing() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [],
        "usage": {"prompt_tokens": 120, "completion_tokens": 45, "cost": -0.123456}
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Usage(usage)] if usage.cost_micro_usd.is_none()
    ));
}

#[test]
fn stream_rejects_an_out_of_range_cost_as_missing() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "choices": [],
        "usage": {"prompt_tokens": 120, "completion_tokens": 45, "cost": 1e300}
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Usage(usage)] if usage.cost_micro_usd.is_none()
    ));
}

#[test]
fn stream_classifies_429_with_retry_after_as_rate_limited() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "error": {"code": 429, "message": "Muitas requisições.", "metadata": {"retry_after": 9}},
        "choices": [{"finish_reason": "error"}]
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)]
            if matches!(error.kind, ErrorKind::RateLimited { retry_after_secs: Some(9) })
    ));
}

#[test]
fn stream_classifies_503_as_transient() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "error": {"code": 503, "message": "Indisponível."},
        "choices": [{"finish_reason": "error"}]
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Transient)
    ));
}

#[test]
fn stream_classifies_400_as_permanent() {
    let mut parser = StreamParser::new();
    let events = parser.push(&sse(json!({
        "error": {"code": 400, "message": "Requisição inválida."},
        "choices": [{"finish_reason": "error"}]
    })));

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Permanent)
    ));
}

#[test]
fn stream_done_marker_does_not_invent_an_event() {
    let mut parser = StreamParser::new();

    assert!(parser.push("data: [DONE]").is_empty());
}

#[test]
fn stream_accepts_the_terminal_marker_with_carriage_return() {
    let mut parser = StreamParser::new();

    assert!(parser.push("data: [DONE]\r").is_empty());
    assert!(parser.finish().is_empty());
}

#[test]
fn stream_rejects_invalid_json_data_as_malformed() {
    let mut parser = StreamParser::new();
    let events = parser.push("data: {not json}");

    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Failed(error)] if matches!(error.kind, ErrorKind::Malformed)
    ));
}

#[test]
fn pins_have_one_default_and_non_empty_endpoints() {
    assert_eq!(
        PINS.iter()
            .filter(|pin| matches!(pin.role, PinRole::Default))
            .count(),
        1
    );
    assert!(PINS.iter().all(|pin| !pin.endpoint.is_empty()));
}

#[test]
fn todos_os_pins_declararam_um_operador_legivel() {
    assert!(PINS.iter().all(|pin| !pin.operator.is_empty()));
}

/// A ordem de corrida decide quem corre primeiro no bakeoff e é o último desempate da final —
/// com empate na própria ordem, as duas decisões cairiam na posição do pin dentro do arquivo.
#[test]
fn pins_declare_a_total_run_order() {
    let mut ranks: Vec<u8> = PINS.iter().map(|pin| pin.run_order).collect();
    ranks.sort_unstable();

    assert_eq!(
        ranks,
        (1..=PINS.len() as u8).collect::<Vec<u8>>(),
        "a ordem de corrida é 1..n sem empate nem buraco"
    );
}

/// As fixtures espelham os dois pedidos que a produção faz: o catálogo de retenção zero e o
/// geral dos pins em opt-out. Elas divergem de propósito — o endpoint geral não aparece na
/// fixture de retenção zero, como no provedor —, então a matriz só fecha limpa se cada pin for
/// provado pelo catálogo do caminho DELE.
#[test]
fn fixture_verifies_all_declared_pins() {
    let zdr: Value = serde_json::from_str(include_str!("fixtures/zdr_endpoints.json")).unwrap();
    let general: Value =
        serde_json::from_str(include_str!("fixtures/general_endpoints.json")).unwrap();

    assert!(verify_all(&zdr, &general).is_empty());
    // O caminho trocado divergiria: a prova de um pin em opt-out não vive no catálogo de
    // retenção zero. É a assimetria que uma fixture única esconderia.
    assert!(!verify_all(&general, &zdr).is_empty());
}

#[test]
fn drift_reports_a_model_absent_from_the_catalog() {
    let catalog = json!({"data": []});
    let result = verify(&catalog, default_pin());

    assert!(matches!(result, Err(ref drift) if matches!(drift.drift, Drift::ModelAbsent)));
}

#[test]
fn drift_lists_available_endpoints_when_the_pin_is_absent() {
    let catalog = json!({"data": [{
        "name": "Azure | openai/gpt-5.6-terra-20260709",
        "model_id": default_pin().model,
        "tag": "azure/eu",
        "supported_parameters": ["tools", "structured_outputs"]
    }]});
    let result = verify(&catalog, default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(
            &drift.drift,
            Drift::EndpointAbsent { available } if available == &vec!["azure/eu".to_string()]
        )
    ));
}

/// O catálogo traz dois nomes por entrada: `tag`, o slug que roteia — o mesmo que a requisição
/// fixa em `provider.only` — e `name`, um rótulo de exibição. Casar o pin pelo rótulo aprovaria
/// ou reprovaria a matriz inteira por um texto que não roteia nada.
#[test]
fn drift_matches_the_routing_tag_not_the_display_name() {
    let pin = default_pin();
    let by_tag = json!({"data": [{
        "name": "OpenAI | openai/gpt-5.6-terra-20260709",
        "model_id": pin.model,
        "tag": pin.endpoint,
        "supported_parameters": ["tools", "structured_outputs", "max_tokens", "reasoning"]
    }]});
    assert!(verify(&by_tag, pin).is_ok());

    // O rótulo coincidindo com o endpoint não vale nada: o slug de roteamento é outro.
    let by_name_only = json!({"data": [{
        "name": pin.endpoint,
        "model_id": pin.model,
        "tag": "azure/eu",
        "supported_parameters": ["tools", "structured_outputs", "max_tokens", "reasoning"]
    }]});
    assert!(matches!(
        verify(&by_name_only, pin),
        Err(ref drift) if matches!(&drift.drift, Drift::EndpointAbsent { .. })
    ));
}

#[test]
fn drift_reports_missing_tools_capability() {
    let catalog = json!({"data": [{
        "model_id": default_pin().model,
        "tag": default_pin().endpoint,
        "supported_parameters": ["structured_outputs"]
    }]});
    let result = verify(&catalog, default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(drift.drift, Drift::CapabilityAbsent { parameter: "tools" })
    ));
}

#[test]
fn drift_reports_missing_structured_outputs_capability() {
    let catalog = json!({"data": [{
        "model_id": default_pin().model,
        "tag": default_pin().endpoint,
        "supported_parameters": ["tools"]
    }]});
    let result = verify(&catalog, default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(drift.drift, Drift::CapabilityAbsent { parameter: "structured_outputs" })
    ));
}

/// O teto de saída é exigido com o NOME que o pin envia: um endpoint que só anuncia o outro nome
/// recusaria toda rodada sob `require_parameters`, e anunciar o nome irmão não conserta isso.
#[test]
fn drift_reports_a_missing_token_limit_parameter() {
    let catalog = json!({"data": [{
        "model_id": default_pin().model,
        "tag": default_pin().endpoint,
        "supported_parameters": ["tools", "structured_outputs", "reasoning", "max_completion_tokens"]
    }]});
    let result = verify(&catalog, default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(drift.drift, Drift::CapabilityAbsent { parameter: "max_tokens" })
    ));
}

/// O espelho do teste acima: o pin que envia `max_tokens` verifica num endpoint que anuncia esse
/// nome — e diverge num endpoint que só anuncia o irmão.
#[test]
fn drift_requires_the_token_cap_field_the_pin_sends() {
    let pin = candidate_pin();
    let announces_the_pins_cap = json!({"data": [{
        "model_id": pin.model,
        "tag": pin.endpoint,
        "supported_parameters": ["tools", "structured_outputs", "reasoning", "max_tokens"]
    }]});
    assert!(verify(&announces_the_pins_cap, pin).is_ok());

    let announces_the_sibling_only = json!({"data": [{
        "model_id": pin.model,
        "tag": pin.endpoint,
        "supported_parameters": ["tools", "structured_outputs", "reasoning", "max_completion_tokens"]
    }]});
    assert!(matches!(
        verify(&announces_the_sibling_only, pin),
        Err(ref drift) if matches!(
            drift.drift,
            Drift::CapabilityAbsent { parameter: "max_tokens" }
        )
    ));
}

/// O corpo envia `reasoning` sob parâmetros exigidos: um endpoint que não o anuncia recusaria a
/// rodada no provedor, e a verificação existe para dizer isso antes de a rodada ser paga.
#[test]
fn drift_reports_a_missing_reasoning_capability() {
    let catalog = json!({"data": [{
        "model_id": default_pin().model,
        "tag": default_pin().endpoint,
        "supported_parameters": ["tools", "structured_outputs", "max_completion_tokens"]
    }]});
    let result = verify(&catalog, default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(drift.drift, Drift::CapabilityAbsent { parameter: "reasoning" })
    ));
}

/// Um pin em opt-out deliberado ([`super::pins::Retention::ProviderPolicy`]) verifica contra o
/// catálogo GERAL de endpoints do modelo — nunca contra o de retenção zero, que ele nunca esteve
/// para provar. `catalog_for` é quem roteia, e as três divergências continuam valendo neste
/// caminho, como no de retenção zero.
#[test]
fn drift_verifies_a_provider_policy_pin_against_the_general_endpoints_catalog() {
    let opt_out_pin = default_pin();
    assert_eq!(
        opt_out_pin.retention,
        super::pins::Retention::ProviderPolicy,
        "o default vigente é o pin em opt-out — ajuste este teste se isso mudar"
    );

    let general_catalog = json!({"data": [{
        "model_id": opt_out_pin.model,
        "tag": opt_out_pin.endpoint,
        "supported_parameters": ["tools", "structured_outputs", "reasoning", "max_tokens"]
    }]});
    assert!(verify(&general_catalog, opt_out_pin).is_ok());

    // Ausente do catálogo geral: ModelAbsent — o mesmo diagnóstico de quem verifica por retenção
    // zero, sobre o catálogo certo.
    let empty_general_catalog = json!({"data": []});
    assert!(matches!(
        verify(&empty_general_catalog, opt_out_pin),
        Err(ref drift) if matches!(drift.drift, Drift::ModelAbsent)
    ));

    // `catalog_for` escolhe o catálogo geral para este pin e o de retenção zero para quem não
    // optou por fora — nunca o inverso.
    let zdr_catalog = json!({"data": []});
    assert!(std::ptr::eq(
        super::drift::catalog_for(opt_out_pin, &zdr_catalog, &general_catalog),
        &general_catalog
    ));
    let zero_retention_pin =
        pin("openai/gpt-5.6-sol@medium").expect("pin de retenção zero declarado");
    assert!(std::ptr::eq(
        super::drift::catalog_for(zero_retention_pin, &zdr_catalog, &general_catalog),
        &zdr_catalog
    ));
}

#[test]
fn drift_treats_a_catalog_without_data_as_unreadable() {
    let result = verify(&json!({"items": []}), default_pin());

    assert!(matches!(
        result,
        Err(ref drift) if matches!(drift.drift, Drift::CatalogUnreadable { .. })
    ));
}

use super::egress::{EgressDenied, check, on_redirect};

#[test]
fn egress_accepts_the_pinned_request_endpoint() {
    let request = request_for(default_pin(), vec![]);

    assert_eq!(check(request.url), Ok(()));
}

#[test]
fn egress_refuses_an_insecure_scheme() {
    assert!(matches!(
        check("http://openrouter.ai/api/v1/chat/completions"),
        Err(EgressDenied::InsecureScheme)
    ));
}

#[test]
fn egress_refuses_a_host_with_an_allowed_prefix() {
    assert!(matches!(
        check("https://openrouter.ai.evil.example/api/v1/chat/completions"),
        Err(EgressDenied::HostNotAllowed { ref host }) if host == "openrouter.ai.evil.example"
    ));
}

#[test]
fn egress_refuses_an_unlisted_host() {
    assert!(matches!(
        check("https://evil.example/api/v1/chat/completions"),
        Err(EgressDenied::HostNotAllowed { ref host }) if host == "evil.example"
    ));
}

#[test]
fn egress_refuses_an_unlisted_subdomain() {
    assert!(matches!(
        check("https://sub.openrouter.ai/api/v1/chat/completions"),
        Err(EgressDenied::HostNotAllowed { ref host }) if host == "sub.openrouter.ai"
    ));
}

#[test]
fn egress_refuses_credentials_in_the_authority() {
    assert!(matches!(
        check("https://openrouter.ai@evil.example/api/v1/chat/completions"),
        Err(EgressDenied::CredentialsInUrl)
    ));
}

#[test]
fn egress_refuses_an_explicit_non_standard_port() {
    assert!(matches!(
        check("https://openrouter.ai:8443/api/v1/chat/completions"),
        Err(EgressDenied::PortNotAllowed { port: 8443 })
    ));
}

#[test]
fn egress_accepts_an_explicit_https_port() {
    assert_eq!(
        check("https://openrouter.ai:443/api/v1/chat/completions"),
        Ok(())
    );
}

#[test]
fn egress_refuses_redirects_to_an_allowed_host() {
    assert_eq!(
        on_redirect("https://openrouter.ai/another-path"),
        EgressDenied::RedirectRefused
    );
}
