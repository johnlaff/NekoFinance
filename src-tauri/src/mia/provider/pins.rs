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

/// O esforço de raciocínio que a rodada pede, no vocabulário oficial do modelo.
///
/// O esforço é propriedade do PIN, não regra do módulo: ele muda o objeto medido — o mesmo modelo
/// no mesmo endpoint sob outro esforço é outro candidato —, e a matriz só é comparável quando os
/// pins declaram o mesmo nível. A conversa não deriva número (todo valor material chega pronto da
/// ferramenta), mas a SELEÇÃO de ferramenta é raciocínio, e esforço de menos compra falha de
/// consistência: o modelo acerta a conta e erra qual conta fazer.
///
/// Os seis níveis são os que o fabricante documenta para a família pinada (verificado 2026-07-31,
/// developers.openai.com/api/docs/guides/reasoning). Nome fora desse vocabulário pode passar no
/// endpoint por tolerância — aí o que se mede deixa de ser o que se declarou.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ReasoningEffort {
    /// Desligado — só para modelo que aceita não raciocinar.
    None,
    Low,
    /// O default recomendado pelo fabricante para trabalho agentic com ferramentas.
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningEffort {
    /// Como o corpo da requisição nomeia o nível.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            ReasoningEffort::None => "none",
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
            ReasoningEffort::Xhigh => "xhigh",
            ReasoningEffort::Max => "max",
        }
    }
}

/// O nome do teto de saída que o endpoint anuncia.
///
/// Toda rodada envia um teto de tokens, mas os endpoints não concordam no nome do campo: parte
/// anuncia `max_tokens`, parte só `max_completion_tokens`. Sob `require_parameters`, mandar o
/// nome que o endpoint não anuncia é rodada recusada pelo roteador — não teto ignorado —, então
/// o nome certo é propriedade do pin, como o esforço de raciocínio.
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
    /// A identidade do CANDIDATO: `modelo@esforço`, única na matriz. O mesmo modelo pode correr
    /// mais de uma vez sob esforços diferentes — cada corrida é outro candidato, e é o rótulo,
    /// nunca o nome do modelo, que a contabilidade do bakeoff (dedup, sonda, chave do julgamento
    /// cego, retomada) usa para dizer quem é quem.
    pub label: &'static str,
    /// O que a API recebe — e só isso: para identidade, ver [`ModelPin::label`].
    pub model: &'static str,
    pub endpoint: &'static str,
    pub operator: &'static str,
    pub role: PinRole,
    pub beta_headers: &'static [&'static str],
    /// O esforço de raciocínio deste candidato. Declarado por pin porque muda o objeto medido:
    /// outro esforço é outro candidato, e enviar "desligado" a um modelo que exige raciocínio é
    /// rodada recusada — o catálogo anuncia o parâmetro, nunca os níveis que ele aceita.
    pub reasoning_effort: ReasoningEffort,
    /// O nome do teto de saída deste endpoint. O canary confere que o catálogo o anuncia; a
    /// requisição envia o teto sob este nome e nunca sob o irmão.
    pub token_cap: TokenCap,
    /// O teto de tokens de cada turno deste candidato. Raciocínio pago sai do MESMO orçamento da
    /// resposta: esforço alto sob teto de conversa é recusa do provedor, não resposta pior — o
    /// candidato de esforço máximo abre o teto até a saída máxima do modelo, e os demais correm no
    /// orçamento de conversa, que é parte do objeto que a régua comparável mede.
    pub turn_max_tokens: u32,
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
        label: "openai/gpt-5.6-terra@medium",
        model: "openai/gpt-5.6-terra",
        endpoint: "openai",
        operator: "OpenAI",
        // Default por necessidade técnica — o runtime exige um pin neste papel —, não por
        // vitória: o bakeoff ainda não fechou uma decisão, e é a corrida quem promove ou rebaixa.
        role: PinRole::Default,
        beta_headers: &[],
        reasoning_effort: ReasoningEffort::Medium,
        // O endpoint do próprio fabricante anuncia `max_tokens`, não o irmão que o azure usa.
        token_cap: TokenCap::MaxTokens,
        turn_max_tokens: 1_024,
        // Opt-out deliberado: $1/$6 no endpoint do fabricante contra $2,5/$15 no azure — quase um
        // quarto do preço — pela política de retenção do OPERADOR (sem treino, log limitado) em
        // vez da prova do catálogo de retenção zero (verificado 2026-07-30). O ganho de custo é
        // por isso que o pin sai do azure; sol não segue porque lá o preço é igual nos dois
        // endpoints, sem ganho para pagar a troca da garantia.
        retention: Retention::ProviderPolicy,
        run_order: 1,
    },
    ModelPin {
        label: "openai/gpt-5.6-luna@medium",
        model: "openai/gpt-5.6-luna",
        endpoint: "openai",
        operator: "OpenAI",
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_effort: ReasoningEffort::Medium,
        token_cap: TokenCap::MaxTokens,
        turn_max_tokens: 1_024,
        // Mesmo racional do terra: $0,10/$0,60 no fabricante contra $1/$6 no azure (verificado
        // 2026-07-30).
        retention: Retention::ProviderPolicy,
        run_order: 2,
    },
    ModelPin {
        label: "openai/gpt-5.6-luna@max",
        model: "openai/gpt-5.6-luna",
        endpoint: "openai",
        operator: "OpenAI",
        // O mesmo luna sob o esforço máximo: fora da régua comparável de "medium", ele mede o que
        // o teto de esforço compra — em consistência e em custo — no tier cujo preço torna a
        // pergunta barata de responder. Concorre ao default como qualquer candidato.
        role: PinRole::Candidate,
        beta_headers: &[],
        reasoning_effort: ReasoningEffort::Max,
        token_cap: TokenCap::MaxTokens,
        // A saída máxima do modelo: o raciocínio em max consome o que precisar e ainda sobra
        // espaço para a resposta — teto menor devolve recusa do provedor em vez de medição.
        turn_max_tokens: 128_000,
        retention: Retention::ProviderPolicy,
        run_order: 3,
    },
    ModelPin {
        label: "openai/gpt-5.6-sol@medium",
        model: "openai/gpt-5.6-sol",
        endpoint: "azure",
        operator: "Microsoft Azure",
        role: PinRole::Ceiling,
        beta_headers: &[],
        reasoning_effort: ReasoningEffort::Medium,
        token_cap: TokenCap::MaxCompletionTokens,
        turn_max_tokens: 1_024,
        // Sem desconto para pagar a troca ($5/$30 nos dois endpoints, verificado 2026-07-30) e
        // uptime pior no fabricante — o azure segue provando a garantia pelo catálogo.
        retention: Retention::Zero,
        run_order: 4,
    },
];

/// O pin pelo rótulo do candidato. O nome do modelo sozinho não identifica: ver [`by_model`].
pub(crate) fn pin(label: &str) -> Option<&'static ModelPin> {
    PINS.iter().find(|pin| pin.label == label)
}

/// Os pins que correm um mesmo modelo. Serve a quem só tem o nome do modelo na mão — a seleção
/// da linha de comando, a leitura de relatório sem rótulo — e precisa saber se ele basta para
/// identificar um candidato ou se a ambiguidade exige o rótulo.
pub(crate) fn by_model(model: &str) -> Vec<&'static ModelPin> {
    PINS.iter().filter(|pin| pin.model == model).collect()
}

pub(crate) fn default_pin() -> &'static ModelPin {
    PINS.iter()
        .find(|pin| pin.role == PinRole::Default)
        .expect("a matriz de pins declara um papel Default")
}
