//! A matriz de modelos pinados.
//!
//! O pin é por ENDPOINT, não por provedor: o mesmo modelo servido de dois lugares aceita
//! parâmetros diferentes, e é o endpoint que responde por retenção zero. A troca de pin é gesto
//! manual e deliberado — não existe alternativa automática, porque cair para outro endpoint sem
//! ninguém decidir trocaria a garantia de privacidade por disponibilidade.

/// Papel do pin na seleção manual do provedor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PinRole {
    Default,
    Candidate,
    Ceiling,
}

pub(crate) struct ModelPin {
    pub model: &'static str,
    pub endpoint: &'static str,
    pub operator: &'static str,
    pub role: PinRole,
    pub beta_headers: &'static [&'static str],
}

// Sem este beta declarado, `strict` é removido em silêncio das ferramentas e o modo estrito não
// vale; como a falha é silenciosa, o cabeçalho é gate, não conveniência.
const STRUCTURED_OUTPUTS_BETA: &[&str] = &["structured-outputs-2025-11-13"];

pub(crate) const PINS: &[ModelPin] = &[
    ModelPin {
        model: "anthropic/claude-sonnet-5",
        endpoint: "amazon-bedrock/global",
        operator: "Amazon Bedrock",
        role: PinRole::Default,
        beta_headers: STRUCTURED_OUTPUTS_BETA,
    },
    ModelPin {
        model: "openai/gpt-5.6-terra",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Candidate,
        beta_headers: &[],
    },
    ModelPin {
        model: "openai/gpt-5.6-luna",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Candidate,
        beta_headers: &[],
    },
    ModelPin {
        model: "google/gemini-3.6-flash",
        endpoint: "google-vertex/global",
        operator: "Google Cloud Vertex AI",
        role: PinRole::Candidate,
        beta_headers: &[],
    },
    ModelPin {
        model: "x-ai/grok-4.5",
        endpoint: "xai/zdr",
        operator: "xAI",
        role: PinRole::Candidate,
        beta_headers: &[],
    },
    ModelPin {
        model: "anthropic/claude-opus-5",
        endpoint: "amazon-bedrock",
        operator: "Amazon Bedrock",
        role: PinRole::Ceiling,
        beta_headers: STRUCTURED_OUTPUTS_BETA,
    },
];

pub(crate) fn pin(model: &str) -> Option<&'static ModelPin> {
    PINS.iter().find(|pin| pin.model == model)
}

pub(crate) fn default_pin() -> &'static ModelPin {
    PINS.iter()
        .find(|pin| pin.role == PinRole::Default)
        .expect("a matriz de pins declara um papel Default")
}
