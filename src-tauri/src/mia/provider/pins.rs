//! A matriz de modelos pinados.
//!
//! O pin é por ENDPOINT, não por provedor: o mesmo modelo servido de dois lugares aceita
//! parâmetros diferentes, e é o endpoint que responde por retenção zero. A troca de pin é gesto
//! manual e deliberado — não existe alternativa automática, porque cair para outro endpoint sem
//! ninguém decidir trocaria a garantia de privacidade por disponibilidade.
//!
//! Para modelo de peso aberto, o endpoint carrega também a PRECISÃO servida — a tag nomeia a
//! quantização, e ela é parte da identidade do pin: outro endpoint com outra precisão é outro
//! candidato, ainda que o nome do modelo seja o mesmo. Só entra na matriz precisão declarada
//! pelo catálogo; "unknown" não identifica o que se está medindo.
//!
//! Retenção zero é o padrão, provado por presença no catálogo do provedor — mas [`Retention`]
//! deixa o opt-out ser propriedade do PIN, não regra fixa do módulo: o dono pode trocar a prova do
//! catálogo pela política declarada do operador quando o desconto justifica, e essa troca precisa
//! ficar tão visível quanto o endpoint em si, nunca implícita num "todo pin é ZDR".

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
/// Os pisos dos veteranos da matriz foram conferidos por execução real contra cada endpoint
/// pinado — prova mais forte que o catálogo, que anuncia o parâmetro mas não os esforços que ele
/// aceita (verificado 2026-07). Estreante entra com o piso mais provável declarado, e a sonda do
/// bakeoff o confirma antes de qualquer fase paga.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningFloor {
    /// Desligado. O piso de quem pode não raciocinar — nenhum endpoint da matriz vigente aceita
    /// desligar, e o variante fica porque o piso é fato do endpoint, não da matriz.
    #[allow(dead_code)]
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

/// De onde vem a prova de que a rodada não fica retida no provedor.
///
/// O padrão é `Zero`: o catálogo de retenção zero prova a garantia por presença, sem depender de
/// ninguém ler política nenhuma. `ProviderPolicy` é opt-out DELIBERADO — o dono decidiu que o
/// desconto do endpoint compensa trocar a prova do catálogo pela política declarada do operador
/// (treino desligado, retenção de log limitada). A troca nunca é automática nem silenciosa: ela é
/// visível no pin, como o endpoint e o token_cap, e o canary verifica cada caminho contra o
/// catálogo que prova aquele caminho — o geral para quem optou por fora, o de retenção zero para
/// quem não optou.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retention {
    Zero,
    ProviderPolicy,
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
    /// De onde vem a prova de retenção zero deste pin — ver [`Retention`].
    pub retention: Retention,
    /// A ordem em que os candidatos correm no bakeoff, de 1 em diante e sem empate. Ela existe
    /// para que o dinheiro alcance a matriz inteira antes de a trava fechar — e diz quem corre
    /// primeiro se ela fechar antes. Sem alegação de mérito: mérito é o que a corrida mede.
    ///
    /// No desempate da final ela é o ÚLTIMO critério, atrás do custo real publicado — entra só
    /// como garantia de determinismo quando tudo o mais empata.
    pub run_order: u8,
}

pub(crate) const PINS: &[ModelPin] = &[
    ModelPin {
        model: "openai/gpt-5.6-terra",
        endpoint: "openai",
        operator: "OpenAI",
        // Default por necessidade técnica — o runtime exige um pin neste papel —, não por
        // vitória: o bakeoff ainda não fechou uma decisão, e é a corrida quem promove ou rebaixa.
        role: PinRole::Default,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        // O endpoint do próprio fabricante anuncia `max_tokens`, não o irmão que o azure usa.
        token_cap: TokenCap::MaxTokens,
        // Opt-out deliberado: $1/$6 no endpoint do fabricante contra $2,5/$15 no azure — quase um
        // quarto do preço — pela política de retenção do OPERADOR (sem treino, log limitado) em
        // vez da prova do catálogo de retenção zero (verificado 2026-07-30). O ganho de custo é
        // por isso que o pin sai do azure; sol não segue porque lá o preço é igual nos dois
        // endpoints, sem ganho para pagar a troca da garantia.
        retention: Retention::ProviderPolicy,
        run_order: 1,
    },
    ModelPin {
        model: "openai/gpt-5.6-luna",
        endpoint: "openai",
        operator: "OpenAI",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxTokens,
        // Mesmo racional do terra: $0,10/$0,60 no fabricante contra $1/$6 no azure (verificado
        // 2026-07-30).
        retention: Retention::ProviderPolicy,
        run_order: 2,
    },
    ModelPin {
        model: "openai/gpt-5.6-sol",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Ceiling,
        beta_headers: &[],
        reasoning_floor: ReasoningFloor::Minimal,
        token_cap: TokenCap::MaxCompletionTokens,
        // Sem desconto para pagar a troca ($5/$30 nos dois endpoints, verificado 2026-07-30) e
        // uptime pior no fabricante — o azure segue provando a garantia pelo catálogo.
        retention: Retention::Zero,
        run_order: 3,
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
