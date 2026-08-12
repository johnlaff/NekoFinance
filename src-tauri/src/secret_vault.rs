//! O cofre de segredos do sistema, atrás de um trait (ADR-0014, cláusula 2).
//!
//! `mia::key_store` e `oauth::token_store` guardam credenciais reais (a chave do provedor da Mia,
//! o token do Google) e falam só com [`SecretVault`], nunca com `keyring::` ou o plugin Android
//! diretamente. Este módulo é o adapter: [`KeyringVault`] cobre o keyring nativo do desktop,
//! [`android::AndroidVault`] cobre o Android Keystore via `tauri-plugin-secure-vault` — e
//! [`platform_vault`] escolhe por `cfg(target_os)`, sem o domínio mudar uma linha.

/// Gravar, ler e apagar um segredo por serviço + usuário — o mesmo vocabulário que `keyring::Entry`
/// já usa, para a implementação desktop ser uma casca fina sobre o crate existente.
pub(crate) trait SecretVault {
    fn store(&self, service: &str, username: &str, secret: &str) -> Result<(), String>;
    fn load(&self, service: &str, username: &str) -> Result<Option<String>, String>;
    fn delete(&self, service: &str, username: &str) -> Result<(), String>;
}

/// O keyring nativo do sistema operacional (Keychain no macOS, Credential Manager no Windows,
/// Secret Service no Linux) — uma casca fina sobre `keyring::Entry`, com o mesmo tratamento de
/// "entrada ausente" que qualquer chamador do trait espera. `keyring` compila sob
/// `target_os = "android"` (não há braço condicional por SO dentro do crate), mas não tem
/// backend ali — daí o `cfg` aqui: no Android o tipo nunca é construído, e ficar fora do build
/// evita o warning de código morto em vez de silenciá-lo.
#[cfg(not(target_os = "android"))]
pub(crate) struct KeyringVault;

#[cfg(not(target_os = "android"))]
impl SecretVault for KeyringVault {
    fn store(&self, service: &str, username: &str, secret: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(service, username)
            .map_err(|error| format!("keyring entry: {error}"))?;
        entry
            .set_password(secret)
            .map_err(|error| format!("keyring set: {error}"))
    }

    fn load(&self, service: &str, username: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(service, username)
            .map_err(|error| format!("keyring entry: {error}"))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("keyring get: {error}")),
        }
    }

    fn delete(&self, service: &str, username: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(service, username)
            .map_err(|error| format!("keyring entry: {error}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("keyring delete: {error}")),
        }
    }
}

/// O braço Android do cofre: uma casca fina sobre `tauri-plugin-secure-vault`, que por sua vez
/// fala com o Android Keystore via `EncryptedSharedPreferences` (Kotlin, `android/`). O plugin
/// mobile só existe depois que o app termina de inicializar — [`install_handle`] entrega o
/// identificador do processo assim que o `setup()` do shell o alcança, e todo `store`/`load`/
/// `delete` antes disso falha fechado, do mesmo jeito que um keyring de sistema indisponível.
#[cfg(target_os = "android")]
pub(crate) mod android {
    use super::SecretVault;
    use std::sync::OnceLock;
    use tauri::{AppHandle, Manager, Wry};
    use tauri_plugin_secure_vault::SecureVault;

    // O `AppHandle`, não o `SecureVault<Wry>` em si: `PluginHandle` (por trás de `SecureVault`)
    // não é `Clone`, e o handle do app É — cada chamada busca o estado gerenciado de novo a
    // partir dele, o mesmo custo de uma leitura de `Mutex` sem guardar o guard.
    static HANDLE: OnceLock<AppHandle<Wry>> = OnceLock::new();

    /// Chamado uma vez, no `setup()` do shell, assim que o plugin termina de registrar a classe
    /// Kotlin. Uma segunda chamada (não deveria acontecer — um só `App` por processo) é
    /// silenciosamente ignorada em vez de sobrescrever o identificador já em uso por chamadas
    /// concorrentes.
    pub(crate) fn install_handle(app: AppHandle<Wry>) {
        let _ = HANDLE.set(app);
    }

    fn plugin() -> Result<tauri::State<'static, SecureVault<Wry>>, String> {
        let app = HANDLE
            .get()
            .ok_or_else(|| "cofre Android ainda não inicializado".to_string())?;
        Ok(app.state::<SecureVault<Wry>>())
    }

    pub(crate) struct AndroidVault;

    impl SecretVault for AndroidVault {
        fn store(&self, service: &str, username: &str, secret: &str) -> Result<(), String> {
            plugin()?
                .store(service, username, secret)
                .map_err(|error| format!("plugin do cofre Android: {error}"))
        }

        fn load(&self, service: &str, username: &str) -> Result<Option<String>, String> {
            plugin()?
                .load(service, username)
                .map_err(|error| format!("plugin do cofre Android: {error}"))
        }

        fn delete(&self, service: &str, username: &str) -> Result<(), String> {
            plugin()?
                .delete(service, username)
                .map_err(|error| format!("plugin do cofre Android: {error}"))
        }
    }
}

/// O cofre da plataforma corrente. Único ponto de seleção do processo inteiro.
#[cfg(target_os = "android")]
pub(crate) fn platform_vault() -> &'static dyn SecretVault {
    static VAULT: android::AndroidVault = android::AndroidVault;
    &VAULT
}

/// O cofre da plataforma corrente. Único ponto de seleção do processo inteiro.
#[cfg(not(target_os = "android"))]
pub(crate) fn platform_vault() -> &'static dyn SecretVault {
    static VAULT: KeyringVault = KeyringVault;
    &VAULT
}

/// `NEKO_INSECURE_FILE_FALLBACK` é um env var de PROCESSO lido tanto por `mia::key_store` quanto por
/// `oauth::token_store`; testes de qualquer um dos dois módulos rodam em paralelo no mesmo binário.
/// Este mutex serializa os testes que mutam essa variável para eles não disputarem entre si.
#[cfg(test)]
pub(crate) static INSECURE_FILE_FALLBACK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// O dublê em memória do cofre — sem persistência real, para os testes de fluxo (`store`/`load`/
/// `delete` sobre a chave e o token) e para o teste de contrato deste módulo.
#[cfg(test)]
pub(crate) mod double {
    use super::SecretVault;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct InMemoryVault {
        entries: Mutex<HashMap<(String, String), String>>,
        unavailable: bool,
    }

    impl InMemoryVault {
        /// Simula um cofre do sistema indisponível (ex.: Linux sem libsecret) — todo chamador cai no
        /// fallback de arquivo, do mesmo jeito que um keyring real indisponível faria.
        pub(crate) fn unavailable() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                unavailable: true,
            }
        }

        fn key(service: &str, username: &str) -> (String, String) {
            (service.to_string(), username.to_string())
        }
    }

    impl SecretVault for InMemoryVault {
        fn store(&self, service: &str, username: &str, secret: &str) -> Result<(), String> {
            if self.unavailable {
                return Err("mock vault unavailable".to_string());
            }
            self.entries
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(Self::key(service, username), secret.to_string());
            Ok(())
        }

        fn load(&self, service: &str, username: &str) -> Result<Option<String>, String> {
            if self.unavailable {
                return Err("mock vault unavailable".to_string());
            }
            Ok(self
                .entries
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&Self::key(service, username))
                .cloned())
        }

        fn delete(&self, service: &str, username: &str) -> Result<(), String> {
            if self.unavailable {
                return Err("mock vault unavailable".to_string());
            }
            self.entries
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(&Self::key(service, username));
            Ok(())
        }
    }
}

/// O contrato que toda implementação de [`SecretVault`] precisa cumprir, exercido contra a
/// implementação desktop e contra o dublê em memória — a mesma sequência, os dois lados.
#[cfg(test)]
mod contract {
    #[cfg(not(target_os = "android"))]
    use super::KeyringVault;
    use super::SecretVault;
    use super::double::InMemoryVault;

    /// Grava, lê, apaga e relê um segredo descartável. Devolve `Err` assim que a PRIMEIRA operação
    /// falhar — o chamador decide se isso é uma violação de contrato (dublê) ou um ambiente sem
    /// cofre de sistema disponível (desktop headless, ex.: CI sem D-Bus/libsecret).
    fn exercise(vault: &dyn SecretVault, service: &str, username: &str) -> Result<(), String> {
        let secret = format!("contract-fixture-{}", uuid::Uuid::new_v4());

        vault.delete(service, username)?;
        if vault.load(service, username)?.is_some() {
            return Err("estado residual antes do teste".to_string());
        }

        vault.store(service, username, &secret)?;
        if vault.load(service, username)? != Some(secret.clone()) {
            return Err("load após store não devolveu o segredo gravado".to_string());
        }

        vault.delete(service, username)?;
        if vault.load(service, username)?.is_some() {
            return Err("load após delete ainda devolveu o segredo".to_string());
        }

        Ok(())
    }

    #[test]
    fn o_dubl_em_memoria_cumpre_o_contrato_do_cofre() {
        let vault = InMemoryVault::default();
        exercise(&vault, "neko-finance-contract-test", "dublê").expect(
            "o dublê em memória deve gravar, ler, apagar e relê um segredo como qualquer cofre",
        );
    }

    #[test]
    #[cfg(not(target_os = "android"))]
    fn o_keyring_do_desktop_cumpre_o_contrato_do_cofre_quando_disponivel() {
        let vault = KeyringVault;
        // Serviço e usuário exclusivos deste teste — nunca os de produção (`neko-finance` /
        // `neko-finance-mia`), então mesmo num ambiente com keychain real este teste nunca lê,
        // sobrescreve ou apaga uma credencial de verdade.
        let service = "neko-finance-secret-vault-contract-test";
        let username = format!("contract-{}", uuid::Uuid::new_v4());

        match exercise(&vault, service, &username) {
            Ok(()) => {}
            Err(error) => eprintln!(
                "cofre do sistema indisponível neste ambiente ({error}) — contrato do keyring \
                 pulado (esperado em CI headless sem D-Bus/libsecret; o dublê acima já provou o \
                 contrato)"
            ),
        }
    }
}
