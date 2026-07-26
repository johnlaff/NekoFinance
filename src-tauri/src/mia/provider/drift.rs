//! Verificação dos pins contra o catálogo de endpoints de retenção zero.
//!
//! O catálogo é exclusivamente de retenção zero, portanto cada presença prova essa condição.
//! A verificação separa modelo, endpoint e capacidade ausentes para que a troca manual do pin
//! seja possível sem investigação adicional; não há fallback automático porque a troca é
//! deliberada. Forma inválida vira erro diagnosticável, nunca a falsa conclusão de que sumiu.

use super::pins::{ModelPin, PINS};
use serde_json::Value;

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

struct CatalogEntry<'a> {
    name: &'a str,
    model_id: &'a str,
    supported_parameters: Vec<&'a str>,
}

// A lista espelha o que a montagem da requisição envia sob parâmetros exigidos; acrescentar um
// campo ao corpo sem acrescentá-lo aqui é a falha silenciosa que esta verificação existe para pegar.
const REQUIRED_PARAMETERS: &[&str] = &["tools", "structured_outputs", "max_tokens"];

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
        .find(|entry| entry.name == pin.endpoint)
    else {
        return Err(PinDrift {
            model: pin.model,
            endpoint: pin.endpoint,
            drift: Drift::EndpointAbsent {
                available: model_entries
                    .iter()
                    .map(|entry| entry.name.to_string())
                    .collect(),
            },
        });
    };

    for &parameter in REQUIRED_PARAMETERS {
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

pub(crate) fn verify_all(catalog: &Value) -> Vec<PinDrift> {
    PINS.iter()
        .filter_map(|pin| verify(catalog, pin).err())
        .collect()
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
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("O item {index} não tem name textual."))?;
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
                name,
                model_id,
                supported_parameters,
            })
        })
        .collect()
}
