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

/// O piso de raciocínio que o endpoint aceita.
///
/// A conversa quer o mínimo em todos: ela não deriva número — todo valor material chega pronto da
/// ferramenta —, e raciocínio pago compraria latência e custo sem comprar fidelidade. Só que o
/// mínimo não é o mesmo em toda a matriz, e mandar o piso errado é rodada recusada, não resposta
/// pior: modelo com raciocínio obrigatório rejeita "desligado", e quem aceita desligar não pode
/// receber um orçamento mínimo que o teto de tokens do turno não comporta.
///
/// Os pisos declarados na matriz foram conferidos por execução real contra cada endpoint pinado —
/// prova mais forte que o catálogo, que anuncia o parâmetro mas não os esforços que ele aceita
/// (verificado 2026-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningFloor {
    /// Desligado. O piso de quem pode não raciocinar.
    Off,
    /// O menor esforço que o modelo aceita, para quem não pode desligar.
    Minimal,
}

impl ReasoningFloor {
    /// Como o provedor nomeia o esforço.
    pub(crate) fn effort(self) -> &'static str {
        match self {
            ReasoningFloor::Off => "none",
            ReasoningFloor::Minimal => "minimal",
        }
    }
}

/// O nome do teto de saída que o endpoint anuncia.
///
/// Toda rodada envia um teto de tokens, mas os endpoints não concordam no nome do campo: parte
/// anuncia `max_tokens`, parte só `max_completion_tokens`. Sob `require_parameters`, mandar o
/// nome que o endpoint não anuncia é rodada recusada pelo roteador — não teto ignorado —, então
/// o nome certo é propriedade do pin, como o piso de raciocínio.
///
/// Os nomes declarados na matriz batem com o catálogo do provedor e foram conferidos por
/// execução real contra os endpoints pinados (verificado 2026-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenCap {
    MaxTokens,
    MaxCompletionTokens,
}

impl TokenCap {
    /// Como o corpo da requisição nomeia o campo — e como o catálogo o anuncia.
    pub(crate) fn field(self) -> &'static str {
        match self {
            TokenCap::MaxTokens => "max_tokens",
            TokenCap::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ModelPin {
    pub model: &'static str,
    pub endpoint: &'static str,
    pub operator: &'static str,
    pub role: PinRole,
    pub beta_headers: &'static [&'static str],
    /// O piso de raciocínio deste endpoint. Declarado por pin porque a matriz não é uniforme:
    /// enviar "desligado" a um modelo que exige raciocínio é rodada recusada, e o canary não
    /// alcança isso — o catálogo anuncia o parâmetro, nunca os esforços que ele aceita.
    pub reasoning_floor: ReasoningFloor,
    /// O nome do teto de saída deste endpoint. O canary confere que o catálogo o anuncia; a
    /// requisição envia o teto sob este nome e nunca sob o irmão.
    pub token_cap: TokenCap,
    /// A ordem a priori, de 1 em diante e sem empate. Ela lê o benchmark de agente bancário —
    /// ferramentas, várias etapas, dado financeiro — ACIMA do índice geral de inteligência, que
    /// só desempata: a conversa é um agente que consulta, não um ensaísta.
    ///
    /// Ela decide duas coisas pequenas e nenhuma grande: quem corre primeiro no bakeoff, para que
    /// o dinheiro chegue aos mais promissores antes de a trava fechar, e quem ganha empate na
    /// final. O gate é a suíte própria. (verificado 2026-07)
    pub prior_rank: u8,
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
        reasoning_floor: ReasoningFloor::Off,
        token_cap: TokenCap::MaxTokens,
        prior_rank: 2,
    },
    ModelPin {
        model: "openai/gpt-5.6-terra",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxCompletionTokens,
        prior_rank: 3,
    },
    ModelPin {
        model: "openai/gpt-5.6-luna",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxCompletionTokens,
        prior_rank: 4,
    },
    ModelPin {
        model: "google/gemini-3.6-flash",
        endpoint: "google-vertex/global",
        operator: "Google Cloud Vertex AI",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxTokens,
        prior_rank: 5,
    },
    ModelPin {
        model: "x-ai/grok-4.5",
        endpoint: "xai/zdr",
        operator: "xAI",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxTokens,
        prior_rank: 6,
    },
    ModelPin {
        model: "anthropic/claude-opus-5",
        endpoint: "amazon-bedrock",
        operator: "Amazon Bedrock",
        role: PinRole::Ceiling,
        beta_headers: STRUCTURED_OUTPUTS_BETA,
        reasoning_floor: ReasoningFloor::Off,
        token_cap: TokenCap::MaxTokens,
        prior_rank: 1,
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
