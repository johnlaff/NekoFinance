use serde::{Deserialize, Serialize};

/// Identifica uma credencial no cofre — o mesmo par serviço + usuário que `keyring::Entry`
/// usa no lado desktop, para o chamador (`secret_vault::AndroidVault`) não precisar saber
/// que os dois lados falam vocabulários diferentes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretKey {
    pub service: String,
    pub username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreRequest {
    pub service: String,
    pub username: String,
    pub secret: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadResponse {
    pub secret: Option<String>,
}

/// A confirmação vazia de `store`/`delete`. `run_mobile_plugin` desserializa a resposta do
/// Kotlin — um `()` falharia contra o `{}` que `invoke.resolve(JSObject())` devolve, então a
/// confirmação é um struct sem campos em vez do unit type.
#[derive(Deserialize)]
pub struct Ack {}
