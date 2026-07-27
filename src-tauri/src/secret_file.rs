//! O arquivo cifrado que substitui o cofre onde ele não existe.
//!
//! Uma implementação só, usada por todo segredo que precisa desse recurso: duas cifras paralelas
//! divergiriam em formato e em força, e a mais fraca passaria despercebida. O sal é por segredo —
//! arquivos distintos, chaves derivadas distintas.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Chave do fallback de arquivo cifrado. AVISO de segurança: é OFUSCAÇÃO BEST-EFFORT, não proteção
/// forte — o sal fica em claro ao lado do ciphertext e a chave deriva de machine-id + sal, ambos
/// legíveis por qualquer processo do mesmo usuário. Só protege contra leitura casual do arquivo, não
/// contra um atacante local. O caminho preferido é o keychain do SO; este fallback existe para
/// ambientes sem keychain. O fallback só é usado com NEKO_INSECURE_FILE_FALLBACK=1.
pub(crate) fn derive_key(app_dir: &Path, salt_file: &str) -> Result<[u8; 32], String> {
    let salt_path = app_dir.join(salt_file);
    let salt = if salt_path.exists() {
        std::fs::read(&salt_path).map_err(|e| format!("read salt: {e}"))?
    } else {
        let mut salt = [0u8; 16];
        getrandom::fill(&mut salt).map_err(|e| format!("generate salt: {e}"))?;
        std::fs::write(&salt_path, salt).map_err(|e| format!("write salt: {e}"))?;
        salt.to_vec()
    };

    let mut hasher = Sha256::new();
    hasher.update(get_machine_id().as_bytes());
    hasher.update(&salt);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    Ok(key)
}

pub(crate) fn seal(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "cipher key length invalid".to_string())?;
    let mut nonce_bytes = [0u8; 12];
    getrandom::fill(&mut nonce_bytes).map_err(|e| format!("generate nonce: {e}"))?;
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("encrypt: {e}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub(crate) fn open(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if data.len() < 12 {
        return Err("encrypted data too short".to_string());
    }

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "cipher key length invalid".to_string())?;
    let nonce = Nonce::try_from(&data[..12]).map_err(|_| "nonce length invalid".to_string())?;
    cipher
        .decrypt(&nonce, &data[12..])
        .map_err(|e| format!("decrypt: {e}"))
}

fn get_machine_id() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            return id.trim().to_string();
        }
        if let Ok(id) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            return id.trim().to_string();
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["csproduct", "get", "UUID"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    "neko-finance-default-machine-id".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-secret-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("o diretório temporário deve existir");
        dir
    }

    #[test]
    fn cifra_abre_o_mesmo_texto_com_a_chave_derivada() {
        let dir = temp_app_dir();
        let key = derive_key(&dir, "fixture-salt").expect("a chave deve derivar");
        let sealed = seal(b"segredo de fixture", &key).expect("o texto deve cifrar");

        assert_eq!(
            open(&sealed, &key).expect("o ciphertext deve abrir"),
            b"segredo de fixture"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn dois_selos_do_mesmo_texto_usam_nonces_distintos() {
        let dir = temp_app_dir();
        let key = derive_key(&dir, "fixture-salt").expect("a chave deve derivar");

        assert_ne!(
            seal(b"segredo de fixture", &key).expect("o primeiro selo deve funcionar"),
            seal(b"segredo de fixture", &key).expect("o segundo selo deve funcionar")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_chave_derivada_permanece_estavel_no_mesmo_diretorio() {
        let dir = temp_app_dir();

        assert_eq!(
            derive_key(&dir, "fixture-salt").expect("a primeira chave deve derivar"),
            derive_key(&dir, "fixture-salt").expect("a segunda chave deve derivar")
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
