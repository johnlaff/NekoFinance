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

    let mut body = json!({
        "model": spec.pin.model,
        "stream": true,
        "usage": {"include": true},
        "max_tokens": spec.max_tokens,
        "parallel_tool_calls": false,
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
