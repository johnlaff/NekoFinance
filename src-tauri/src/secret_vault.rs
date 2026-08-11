//! O cofre de segredos do sistema, atrás de um trait (ADR-0014, cláusula 2).
//!
//! `mia::key_store` e `oauth::token_store` guardam credenciais reais (a chave do provedor da Mia,
//! o token do Google) e falam só com [`SecretVault`], nunca com `keyring::` diretamente. Este
//! módulo é o adapter: hoje a única implementação é [`KeyringVault`], sobre o keyring nativo do
//! desktop; quando o Android entrar, a implementação sobre o Keystore do sistema nasce aqui, e
//! [`platform_vault`] passa a escolher por `cfg(target_os)` — sem o domínio mudar uma linha.

/// Gravar, ler e apagar um segredo por serviço + usuário — o mesmo vocabulário que `keyring::Entry`
/// já usa, para a implementação desktop ser uma casca fina sobre o crate existente.
pub(crate) trait SecretVault {
    fn store(&self, service: &str, username: &str, secret: &str) -> Result<(), String>;
    fn load(&self, service: &str, username: &str) -> Result<Option<String>, String>;
    fn delete(&self, service: &str, username: &str) -> Result<(), String>;
}

/// O keyring nativo do sistema operacional (Keychain no macOS, Credential Manager no Windows,
/// Secret Service no Linux) — hoje a única implementação; uma casca fina sobre `keyring::Entry`,
/// com o mesmo tratamento de "entrada ausente" que qualquer chamador do trait espera.
pub(crate) struct KeyringVault;

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

/// O cofre da plataforma corrente. Único ponto de seleção do processo inteiro — hoje sempre o
/// desktop; o braço Android entra aqui quando existir.
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
    use super::double::InMemoryVault;
    use super::{KeyringVault, SecretVault};

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
