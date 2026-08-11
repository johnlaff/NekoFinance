//! A chave do provedor, em serviço próprio do cofre do sistema.
//!
//! Serviço separado do da conta de planilha de propósito: revogar a conversa apaga esta credencial
//! e não toca na outra, e uma leitura acidental de um serviço nunca alcança o outro.
//!
//! Todo texto de erro daqui passa pelo redator antes de sair. A mensagem do cofre ecoa o que ele
//! recebeu, e é por aí que a chave chegaria a um log sem ninguém ter decidido isso.

use super::run::redaction;
use crate::secret_file;
use crate::secret_vault::{self, SecretVault};
use std::fmt;
use std::path::{Path, PathBuf};

pub(crate) const KEYRING_SERVICE: &str = "neko-finance-mia";
pub(crate) const KEYRING_USERNAME: &str = "openrouter";
const ENCRYPTED_FILE: &str = "mia-key.enc";
const SALT_FILE: &str = "mia-salt";

pub(crate) struct ApiKey(String);

impl ApiKey {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

// A redação pertence ao tipo para que uma chamada de diagnóstico não dependa de cada chamador
// lembrar que a chave não pode atravessar artefatos observáveis.
impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey(<redigida>)")
    }
}

fn encrypted_key_path(app_dir: &Path) -> PathBuf {
    app_dir.join(ENCRYPTED_FILE)
}

fn redacted_error(message: impl fmt::Display) -> String {
    redaction::credentials(&message.to_string())
}

fn fallback_allowed() -> bool {
    std::env::var("NEKO_INSECURE_FILE_FALLBACK").as_deref() == Ok("1")
}

fn fallback_unavailable(error: &str) -> String {
    redacted_error(format!(
        "Keychain unavailable ({error}). Set NEKO_INSECURE_FILE_FALLBACK=1 to allow the insecure file-based fallback, or install a keychain (libsecret on Linux)."
    ))
}

pub(crate) fn store(app_dir: &Path, key: &ApiKey) -> Result<(), String> {
    store_with(secret_vault::platform_vault(), app_dir, key)
}

fn store_with(vault: &dyn SecretVault, app_dir: &Path, key: &ApiKey) -> Result<(), String> {
    match vault
        .store(KEYRING_SERVICE, KEYRING_USERNAME, key.expose())
        .map_err(redacted_error)
    {
        Ok(()) => return Ok(()),
        Err(error) if !fallback_allowed() => return Err(fallback_unavailable(&error)),
        Err(error) => eprintln!(
            "NEKO_INSECURE_FILE_FALLBACK=1: using weak file-based Mia key storage ({})",
            redaction::credentials(&error)
        ),
    }

    store_in_file(app_dir, key)
}

fn store_in_file(app_dir: &Path, key: &ApiKey) -> Result<(), String> {
    let cipher_key = secret_file::derive_key(app_dir, SALT_FILE).map_err(redacted_error)?;
    let encrypted =
        secret_file::seal(key.expose().as_bytes(), &cipher_key).map_err(redacted_error)?;
    std::fs::write(encrypted_key_path(app_dir), encrypted)
        .map_err(|error| redacted_error(format!("write encrypted: {error}")))
}

pub(crate) fn load(app_dir: &Path) -> Result<Option<ApiKey>, String> {
    load_with(secret_vault::platform_vault(), app_dir)
}

fn load_with(vault: &dyn SecretVault, app_dir: &Path) -> Result<Option<ApiKey>, String> {
    match vault
        .load(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(redacted_error)
    {
        Ok(Some(secret)) => return Ok(Some(ApiKey::new(secret))),
        Ok(None) => {}
        Err(error) if !fallback_allowed() => return Err(fallback_unavailable(&error)),
        Err(error) => eprintln!(
            "NEKO_INSECURE_FILE_FALLBACK=1: using weak file-based Mia key storage ({})",
            redaction::credentials(&error)
        ),
    }

    load_from_file(app_dir)
}

fn load_from_file(app_dir: &Path) -> Result<Option<ApiKey>, String> {
    let path = encrypted_key_path(app_dir);
    if !path.exists() {
        return Ok(None);
    }
    let cipher_key = secret_file::derive_key(app_dir, SALT_FILE).map_err(redacted_error)?;
    let data =
        std::fs::read(path).map_err(|error| redacted_error(format!("read encrypted: {error}")))?;
    let plaintext = secret_file::open(&data, &cipher_key).map_err(redacted_error)?;
    let key = String::from_utf8(plaintext)
        .map_err(|error| redacted_error(format!("decode encrypted key: {error}")))?;
    Ok(Some(ApiKey::new(key)))
}

/// O veredito da revogação: cada armazenamento responde por si.
///
/// Reler para verificar o efeito NÃO serve de prova aqui, e essa é a armadilha: [`load`] cai de um
/// armazenamento para o outro, então um cofre que errou pareceria vazio só porque o arquivo nunca
/// existiu — e a revogação devolveria sucesso com a credencial intacta lá dentro.
fn delete_verdict(keyring: Result<(), String>, file: Result<(), String>) -> Result<(), String> {
    keyring.and(file)
}

/// Apaga a chave dos dois lugares onde ela pode estar.
///
/// Falha em um dos lados não interrompe o outro — deixar a credencial de pé em um cofre porque o
/// outro recusou seria o pior dos dois mundos. E nenhuma falha é engolida: "não consegui falar com
/// o cofre" é exatamente o caso em que a chave PODE ter ficado, e é isso que quem revogou precisa
/// saber. Prometer um apagamento que talvez não tenha acontecido é a pior das respostas.
pub(crate) fn delete(app_dir: &Path) -> Result<(), String> {
    delete_with(secret_vault::platform_vault(), app_dir)
}

fn delete_with(vault: &dyn SecretVault, app_dir: &Path) -> Result<(), String> {
    let keyring = vault
        .delete(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(redacted_error);

    let path = encrypted_key_path(app_dir);
    let file = if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|error| redacted_error(format!("delete encrypted: {error}")))
    } else {
        Ok(())
    };

    delete_verdict(keyring, file)
}

pub(crate) fn has_key(app_dir: &Path) -> bool {
    load(app_dir).ok().flatten().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_vault::double::InMemoryVault;

    // `store`/`load`/`delete` escreveriam na credencial REAL de quem roda os testes se falassem
    // com o keyring do sistema — por isso os fluxos completos abaixo passam pelo dublê em memória
    // (`InMemoryVault`), nunca por `secret_vault::platform_vault()`. O cofre do sistema em si tem
    // o próprio teste de contrato em `secret_vault`.
    const FIXTURE: &str = "sk-or-v1-fixture1234567890";

    fn temp_app_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-mia-key-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("o diretório temporário deve existir");
        dir
    }

    #[test]
    fn a_chave_nao_aparece_no_debug_do_tipo_que_a_carrega() {
        let key = ApiKey::new(FIXTURE.to_string());

        assert_eq!(format!("{key:?}"), "ApiKey(<redigida>)");
        assert!(!format!("{key:?}").contains(FIXTURE));
    }

    #[test]
    fn o_arquivo_guardado_nao_contem_a_chave_em_claro() {
        let dir = temp_app_dir();
        store_in_file(&dir, &ApiKey::new(FIXTURE.to_string())).expect("a gravação deve funcionar");

        let bytes = std::fs::read(encrypted_key_path(&dir)).expect("o arquivo deve existir");

        assert!(
            !bytes
                .windows(FIXTURE.len())
                .any(|janela| janela == FIXTURE.as_bytes())
        );
        assert_eq!(
            load_from_file(&dir)
                .expect("a leitura deve funcionar")
                .expect("a chave deve estar guardada")
                .expose(),
            FIXTURE
        );
        std::fs::remove_dir_all(dir).ok();
    }

    /// O erro é o caminho por onde uma credencial chega a um log sem ninguém ter decidido isso.
    #[test]
    fn o_erro_de_leitura_sai_redigido() {
        let dir = temp_app_dir();
        std::fs::write(
            encrypted_key_path(&dir),
            format!("cifra corrompida com Authorization: Bearer {FIXTURE}"),
        )
        .expect("o fixture deve existir");

        let error = load_from_file(&dir).expect_err("uma cifra corrompida não abre");

        assert!(!error.contains(FIXTURE));
        std::fs::remove_dir_all(dir).ok();
    }

    /// Um cofre que recusou apagar não pode virar sucesso. O caminho que isso fecha: reler para
    /// verificar cairia no armazenamento de arquivo, que nunca teve a chave, e a revogação
    /// prometeria um apagamento que não aconteceu — com a credencial viva no cofre.
    #[test]
    fn a_falha_do_cofre_nunca_vira_revogacao_bem_sucedida() {
        let recusa = || Err("keyring delete: indisponível".to_string());

        assert!(delete_verdict(recusa(), Ok(())).is_err());
        assert!(delete_verdict(Ok(()), recusa()).is_err());
        assert!(delete_verdict(recusa(), recusa()).is_err());
        assert_eq!(delete_verdict(Ok(()), Ok(())), Ok(()));
    }

    #[test]
    fn apagar_o_arquivo_deixa_o_cofre_sem_a_chave() {
        let dir = temp_app_dir();
        store_in_file(&dir, &ApiKey::new(FIXTURE.to_string())).expect("a gravação deve funcionar");

        std::fs::remove_file(encrypted_key_path(&dir)).expect("o arquivo deve sair");

        assert!(
            load_from_file(&dir)
                .expect("a leitura deve funcionar")
                .is_none()
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quando_o_cofre_funciona_a_chave_nunca_toca_o_arquivo() {
        let dir = temp_app_dir();
        let vault = InMemoryVault::default();

        store_with(&vault, &dir, &ApiKey::new(FIXTURE.to_string()))
            .expect("o dublê deve aceitar a gravação");

        assert!(!encrypted_key_path(&dir).exists());
        assert_eq!(
            load_with(&vault, &dir)
                .expect("a leitura deve funcionar")
                .expect("a chave deve estar no cofre")
                .expose(),
            FIXTURE
        );

        delete_with(&vault, &dir).expect("a revogação deve funcionar");
        assert!(
            load_with(&vault, &dir)
                .expect("a leitura pós-revogação deve funcionar")
                .is_none()
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quando_o_cofre_falha_sem_opt_in_a_gravacao_fica_fechada() {
        let _guard = secret_vault::INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // SAFETY: serializado pelo guard acima; nenhum outro teste concorrente altera o env var.
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };

        let dir = temp_app_dir();
        let vault = InMemoryVault::unavailable();

        let error = store_with(&vault, &dir, &ApiKey::new(FIXTURE.to_string()))
            .expect_err("sem cofre e sem opt-in, a gravação deve falhar fechada");

        assert!(error.contains("NEKO_INSECURE_FILE_FALLBACK"));
        assert!(!encrypted_key_path(&dir).exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quando_o_cofre_falha_com_opt_in_a_chave_cai_no_arquivo() {
        let _guard = secret_vault::INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // SAFETY: serializado pelo guard acima.
        unsafe { std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1") };

        let dir = temp_app_dir();
        let vault = InMemoryVault::unavailable();

        store_with(&vault, &dir, &ApiKey::new(FIXTURE.to_string()))
            .expect("com o opt-in, o fallback de arquivo deve aceitar a gravação");

        assert!(encrypted_key_path(&dir).exists());
        assert_eq!(
            load_with(&vault, &dir)
                .expect("a leitura deve funcionar")
                .expect("a chave deve estar no arquivo")
                .expose(),
            FIXTURE
        );

        // SAFETY: serializado pelo guard acima.
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        std::fs::remove_dir_all(dir).ok();
    }
}
