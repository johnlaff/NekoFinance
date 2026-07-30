//! Verificação dos pins contra o catálogo que prova a retenção deles.
//!
//! `verify` é agnóstico à origem do catálogo: casa `model_id` e `tag` — o slug de roteamento, o
//! mesmo que a requisição fixa em `provider.only`; o `name` é rótulo de exibição e não roteia
//! nada — e confere as capacidades exigidas, qualquer que seja a lista de entradas recebida. Quem
//! decide QUAL catálogo prova qual pin é o chamador: um pin de retenção zero
//! ([`super::pins::Retention::Zero`]) só é provado pelo catálogo de retenção zero do provedor;
//! um pin em opt-out deliberado ([`super::pins::Retention::ProviderPolicy`]) troca essa prova pela
//! do catálogo geral de endpoints do próprio modelo — a garantia dele é a política do operador, e
//! o catálogo geral é onde ela aparece por presença do endpoint, não por uma bandeira de retenção
//! zero que ele nunca teve. A verificação separa modelo, endpoint e capacidade ausentes para que a
//! troca manual do pin seja possível sem investigação adicional; não há fallback automático porque
//! a troca é deliberada. Forma inválida vira erro diagnosticável, nunca a falsa conclusão de que
//! sumiu.

use super::pins::{ModelPin, PINS, Retention};
use serde_json::Value;
use std::future::Future;

/// De onde o catálogo de endpoints de retenção zero chega. É o corte que deixa a verificação ao
/// vivo exercitável sem rede: quem abre conexão implementa isto, e a suíte entrega um catálogo
/// gravado pelo mesmo caminho que o canary usa em produção.
pub(crate) trait ZdrCatalog {
    fn fetch(&self) -> impl Future<Output = Result<Value, String>> + Send;
}

/// De onde o catálogo GERAL de endpoints de um modelo chega — a fonte que prova um pin em opt-out
/// deliberado. Ao contrário do catálogo de retenção zero, que é global, este é por modelo: o
/// provedor não publica um catálogo geral único, e pedir o de um modelo que ninguém pinou seria
/// rede gasta sem pin para verificar.
pub(crate) trait EndpointsCatalog {
    fn fetch(&self, model: &str) -> impl Future<Output = Result<Value, String>> + Send;
}

/// Divergência entre um pin e o catálogo de endpoints de retenção zero.
pub(crate) enum Drift {
    ModelAbsent,
    EndpointAbsent { available: Vec<String> },
    CapabilityAbsent { parameter: &'static str },
    CatalogUnreadable { detail: String },
}

pub(crate) struct PinDrift {
    pub model: &'static str,
    pub endpoint: &'static str,
    pub drift: Drift,
}

impl PinDrift {
    /// A divergência em uma frase: o que travou e o que fazer. Quem lê é quem troca o pin à mão —
    /// e a troca precisa ser possível sem abrir o catálogo do provedor para investigar.
    pub(crate) fn explain(&self) -> String {
        let Self {
            model,
            endpoint,
            drift,
        } = self;
        match drift {
            Drift::ModelAbsent => format!(
                "O modelo {model} não aparece no catálogo de retenção zero do provedor. Troque o \
                 pin para um modelo servido com retenção zero."
            ),
            Drift::EndpointAbsent { available } => format!(
                "O endpoint {endpoint} não serve {model} com retenção zero; o catálogo lista: {}. \
                 Troque o pin para um endpoint da lista.",
                available.join(", ")
            ),
            Drift::CapabilityAbsent { parameter } => format!(
                "O endpoint {endpoint} de {model} não anuncia o parâmetro {parameter}, que toda \
                 rodada envia. Troque o pin para um endpoint que o anuncie."
            ),
            Drift::CatalogUnreadable { detail } => format!(
                "O catálogo de retenção zero não pôde ser lido: {detail} O formato do provedor \
                 mudou, e a verificação de pin precisa acompanhar."
            ),
        }
    }
}

struct CatalogEntry<'a> {
    tag: &'a str,
    model_id: &'a str,
    supported_parameters: Vec<&'a str>,
}

// A lista espelha os parâmetros que a montagem da requisição envia: o roteador filtra endpoints
// por eles sob `require_parameters`, e acrescentar um campo ao corpo sem acrescentá-lo aqui é a
// falha silenciosa que esta verificação existe para pegar — um parâmetro fora da lista dos
// endpoints derruba TODA rodada com "no endpoints found". O teto de saída entra por pin, porque
// o nome do campo varia por endpoint.
const REQUIRED_PARAMETERS: &[&str] = &["tools", "structured_outputs", "reasoning"];

pub(crate) fn verify(catalog: &Value, pin: &ModelPin) -> Result<(), PinDrift> {
    let entries = parse_catalog(catalog).map_err(|detail| PinDrift {
        model: pin.model,
        endpoint: pin.endpoint,
        drift: Drift::CatalogUnreadable { detail },
    })?;
    let model_entries: Vec<&CatalogEntry<'_>> = entries
        .iter()
        .filter(|entry| entry.model_id == pin.model)
        .collect();
    if model_entries.is_empty() {
        return Err(PinDrift {
            model: pin.model,
            endpoint: pin.endpoint,
            drift: Drift::ModelAbsent,
        });
    }

    let Some(endpoint) = model_entries
        .iter()
        .copied()
        .find(|entry| entry.tag == pin.endpoint)
    else {
        return Err(PinDrift {
            model: pin.model,
            endpoint: pin.endpoint,
            drift: Drift::EndpointAbsent {
                available: model_entries
                    .iter()
                    .map(|entry| entry.tag.to_string())
                    .collect(),
            },
        });
    };

    let required = REQUIRED_PARAMETERS
        .iter()
        .copied()
        .chain(std::iter::once(pin.token_cap.field()));
    for parameter in required {
        if !endpoint.supported_parameters.contains(&parameter) {
            return Err(PinDrift {
                model: pin.model,
                endpoint: pin.endpoint,
                drift: Drift::CapabilityAbsent { parameter },
            });
        }
    }
    Ok(())
}

/// Verifica cada pin contra o catálogo que prova o caminho de retenção DELE — zero para quem não
/// optou por fora, geral para quem optou.
pub(crate) fn verify_all(zdr_catalog: &Value, general_catalog: &Value) -> Vec<PinDrift> {
    PINS.iter()
        .filter_map(|pin| verify(catalog_for(pin, zdr_catalog, general_catalog), pin).err())
        .collect()
}

/// Qual catálogo prova este pin — ver o porquê no doc do módulo.
pub(crate) fn catalog_for<'a>(
    pin: &ModelPin,
    zdr_catalog: &'a Value,
    general_catalog: &'a Value,
) -> &'a Value {
    match pin.retention {
        Retention::Zero => zdr_catalog,
        Retention::ProviderPolicy => general_catalog,
    }
}

fn parse_catalog(catalog: &Value) -> Result<Vec<CatalogEntry<'_>>, String> {
    let data = catalog
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "O catálogo não contém uma lista data.".to_string())?;
    data.iter()
        .enumerate()
        .map(|(index, item)| {
            let object = item
                .as_object()
                .ok_or_else(|| format!("O item {index} do catálogo não é um objeto."))?;
            let tag = object
                .get("tag")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("O item {index} não tem tag textual."))?;
            let model_id = object
                .get("model_id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("O item {index} não tem model_id textual."))?;
            let supported_parameters = object
                .get("supported_parameters")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("O item {index} não tem supported_parameters em lista."))?
                .iter()
                .map(|parameter| {
                    parameter.as_str().ok_or_else(|| {
                        format!("O item {index} tem supported_parameters não textual.")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CatalogEntry {
                tag,
                model_id,
                supported_parameters,
            })
        })
        .collect()
}
