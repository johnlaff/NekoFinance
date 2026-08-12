//! O braço Android do cofre de segredos (ADR-0014, cláusula 2): `secret_vault::AndroidVault`
//! fala com este plugin para chegar ao Android Keystore, do mesmo jeito que a implementação
//! desktop fala com `keyring::Entry` — o domínio (`mia::key_store`, `oauth::token_store`) nunca
//! sabe qual dos dois está do outro lado.
//!
//! Sem API JavaScript de propósito: nenhum comando aqui entra em `tauri::generate_handler!` do
//! shell, e é essa ausência (não uma allowlist de capabilities) que garante que o segredo nunca
//! atravessa a ponte para o webview.

mod mobile;
mod models;

mod error;
pub use error::{Error, Result};

use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

pub use mobile::SecureVault;

/// Registra a classe Kotlin e guarda o handle resultante em estado gerenciado —
/// `secret_vault::android::install_handle` lê o `AppHandle` de volta depois e alcança o handle
/// via `Manager::state::<SecureVault<R>>()`, o mesmo caminho de qualquer outro estado do Tauri.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("secure-vault")
        .setup(|app, api| {
            let secure_vault = mobile::init(app, api)?;
            app.manage(secure_vault);
            Ok(())
        })
        .build()
}
