//! A ponte JNI até o `SecureVaultPlugin` Kotlin (Android Keystore via
//! `EncryptedSharedPreferences`). Este crate só compila sob `target_os = "android"` — é uma
//! dependência gated por plataforma no `Cargo.toml` do shell, nunca um dependência incondicional
//! — então não há braço desktop nem iOS aqui: ambos ficam fora do escopo desta fatia (ADR-0014,
//! spec 044).

use crate::models::*;
use tauri::{
    AppHandle, Runtime,
    plugin::{PluginApi, PluginHandle},
};

pub fn init<R: Runtime, C: serde::de::DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<SecureVault<R>> {
    let handle =
        api.register_android_plugin("app.neko.finance.securevault", "SecureVaultPlugin")?;
    Ok(SecureVault(handle))
}

/// Acesso ao cofre nativo do Android. Cada chamada é uma viagem síncrona até o main thread do
/// Kotlin (o mesmo contrato de qualquer plugin mobile do Tauri) — aceitável aqui porque o
/// chamador (`secret_vault::AndroidVault`) só é exercido por gestos raros do usuário (cadastrar,
/// trocar ou revogar a chave), nunca em laço.
pub struct SecureVault<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> SecureVault<R> {
    pub fn store(&self, service: &str, username: &str, secret: &str) -> crate::Result<()> {
        let _: Ack = self.0.run_mobile_plugin(
            "store",
            StoreRequest {
                service: service.to_string(),
                username: username.to_string(),
                secret: secret.to_string(),
            },
        )?;
        Ok(())
    }

    pub fn load(&self, service: &str, username: &str) -> crate::Result<Option<String>> {
        let response: LoadResponse = self.0.run_mobile_plugin(
            "load",
            SecretKey {
                service: service.to_string(),
                username: username.to_string(),
            },
        )?;
        Ok(response.secret)
    }

    pub fn delete(&self, service: &str, username: &str) -> crate::Result<()> {
        let _: Ack = self.0.run_mobile_plugin(
            "delete",
            SecretKey {
                service: service.to_string(),
                username: username.to_string(),
            },
        )?;
        Ok(())
    }
}
