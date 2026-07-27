//! A chave do provedor, em serviço próprio do cofre do sistema.
//!
//! Serviço separado do da conta de planilha de propósito: revogar a conversa apaga esta credencial
//! e não toca na outra, e uma leitura acidental de um serviço nunca alcança o outro.
//!
//! Todo texto de erro daqui passa pelo redator antes de sair. A mensagem do cofre ecoa o que ele
//! recebeu, e é por aí que a chave chegaria a um log sem ninguém ter decidido isso.

use super::run::redaction;
use crate::secret_file;
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

fn try_keyring_store(key: &ApiKey) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|error| redacted_error(format!("keyring entry: {error}")))?;
    entry
        .set_password(key.expose())
        .map_err(|error| redacted_error(format!("keyring set: {error}")))
}

fn try_keyring_load() -> Result<Option<ApiKey>, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|error| redacted_error(format!("keyring entry: {error}")))?;
    match entry.get_password() {
        Ok(key) => Ok(Some(ApiKey::new(key))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(redacted_error(format!("keyring get: {error}"))),
    }
}

fn try_keyring_delete() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|error| redacted_error(format!("keyring entry: {error}")))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(redacted_error(format!("keyring delete: {error}"))),
    }
}

pub(crate) fn store(app_dir: &Path, key: &ApiKey) -> Result<(), String> {
    match try_keyring_store(key) {
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
    match try_keyring_load() {
        Ok(Some(key)) => return Ok(Some(key)),
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
    let keyring = try_keyring_delete();

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

    // A suíte nunca toca o cofre do sistema: `store`/`load`/`delete` escreveriam na credencial
    // REAL de quem roda os testes, sobrescrevendo e apagando a chave dessa pessoa. O que é nosso
    // para exercitar é o caminho de arquivo e a redação — o cofre é do sistema operacional, e ele
    // tem os testes dele.
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
}
