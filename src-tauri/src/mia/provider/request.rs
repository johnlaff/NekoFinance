//! A montagem da requisição, com os gates de privacidade no corpo.
//!
//! Cada gate vira campo no JSON que sai, e é sobre esse JSON que os testes falam. A diferença
//! importa: uma preferência marcada no painel do provedor vale enquanto ninguém a desmarca, e
//! ninguém percebe quando ela cai; um campo ausente do corpo é teste vermelho.

use super::pins::ModelPin;
use serde_json::{Value, json};

/// Uma ferramenta como o provedor a declara. `strict` só tem efeito com o beta do pin junto.
pub(crate) struct ToolDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub strict: bool,
}

pub(crate) struct RunSpec<'a> {
    pub pin: &'static ModelPin,
    pub system: &'a str,
    /// O transcript nativo é preservado como veio: reescrevê-lo aqui perderia o que o provedor
    /// precisa reler, e a tradução do domínio pertence à fronteira.
    pub messages: &'a [Value],
    pub tools: &'a [ToolDeclaration],
    pub max_tokens: u32,
}

pub(crate) struct PreparedRequest {
    pub url: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

pub(crate) fn build(spec: &RunSpec<'_>) -> PreparedRequest {
    let mut messages = Vec::with_capacity(spec.messages.len() + 1);
    messages.push(json!({"role": "system", "content": spec.system}));
    messages.extend(spec.messages.iter().cloned());

    // `parallel_tool_calls` NÃO entra no corpo. Sob `require_parameters`, um campo que os
    // endpoints não anunciam derruba o roteamento inteiro — quase nenhum anuncia este (verificado
    // 2026-07), e o pedido volta como "no endpoints found" antes de qualquer rodada existir. A
    // invariante de uma chamada por turno continua garantida onde ela sempre foi decidida: o laço
    // fecha a rodada ao ver a segunda chamada, e é isso que o teste dela prova. Pedir ao provedor
    // era defesa em profundidade; entre perdê-la e não ter bancada, a bancada vence.
    // `usage` também fica de fora: a linha de uso, com o custo, vem por padrão no último evento
    // do stream — conferido por execução nas duas formas, com e sem o pedido explícito
    // (verificado 2026-07). Campo sem efeito no corpo é só superfície.
    let mut body = json!({
        "model": spec.pin.model,
        "stream": true,
        // Raciocínio no piso que ESTE pin aceita. A conversa não deriva número — todo valor
        // material chega pronto da ferramenta —, então raciocínio pago compraria latência e custo
        // sem comprar fidelidade; e o piso vem do pin porque a matriz não é uniforme, e o piso
        // errado é rodada recusada em vez de resposta pior.
        "reasoning": {"effort": spec.pin.reasoning_floor.effort()},
        "messages": messages,
        "provider": {
            // Impede que a rodada use endpoint fora do catálogo de retenção zero.
            "zdr": true,
            // Impede a coleta do conteúdo da rodada pelo provedor.
            "data_collection": "deny",
            // Impede que o roteador escolha um endpoint diferente do pin.
            "only": [spec.pin.endpoint],
            // Impede que falha do pin seja mascarada por outro provedor.
            "allow_fallbacks": false,
            // Impede que parâmetro exigido seja omitido sem recusa do endpoint.
            "require_parameters": true,
        },
    });

    // O teto de saída entra com o nome que ESTE endpoint anuncia: sob `require_parameters`, o
    // nome que o endpoint não anuncia é rodada recusada pelo roteador, não teto ignorado.
    body.as_object_mut()
        .expect("a requisição é um objeto JSON")
        .insert(
            spec.pin.token_cap.field().to_string(),
            json!(spec.max_tokens),
        );

    if !spec.tools.is_empty() {
        let tools: Vec<Value> = spec
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                        "strict": tool.strict,
                    },
                })
            })
            .collect();
        body.as_object_mut()
            .expect("a requisição é um objeto JSON")
            .insert("tools".to_string(), Value::Array(tools));
    }

    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    // O cache de resposta da borda serviria conteúdo idêntico sem consultar o modelo, mas uma
    // rodada financeira precisa ler o mundo vigente na rodada; o cache de prompt, que é a
    // economia real do laço, fica intacto.
    headers.push(("x-openrouter-cache".to_string(), "false".to_string()));
    if !spec.pin.beta_headers.is_empty() {
        headers.push((
            "x-anthropic-beta".to_string(),
            spec.pin.beta_headers.join(","),
        ));
    }

    PreparedRequest {
        url: "https://openrouter.ai/api/v1/chat/completions",
        headers,
        body,
    }
}
