use crate::secret_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(test)]
// `NEKO_INSECURE_FILE_FALLBACK` é um env var de PROCESSO; testes rodam em paralelo. Este mutex
// serializa os testes que leem/escrevem essa variável para não disputarem entre si.
pub(crate) static INSECURE_FILE_FALLBACK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const KEYRING_SERVICE: &str = "neko-finance";
const KEYRING_USERNAME: &str = "google-oauth";
const ENCRYPTED_FILE: &str = "oauth-token.enc";
const SALT_FILE: &str = "oauth-salt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub scope: String,
}

fn encrypted_token_path(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join(ENCRYPTED_FILE)
}

fn derive_key(app_dir: &std::path::Path) -> Result<[u8; 32], String> {
    secret_file::derive_key(app_dir, SALT_FILE)
}

fn encrypt_token(token: &StoredToken, key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(token).map_err(|e| format!("serialize: {e}"))?;
    secret_file::seal(&json, key)
}

fn decrypt_token(data: &[u8], key: &[u8; 32]) -> Result<StoredToken, String> {
    let plaintext = secret_file::open(data, key)?;
    serde_json::from_slice(&plaintext).map_err(|e| format!("deserialize: {e}"))
}

fn try_keyring_store(token: &StoredToken) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("keyring entry: {e}"))?;
    let json = serde_json::to_string(token).map_err(|e| format!("serialize: {e}"))?;
    entry
        .set_password(&json)
        .map_err(|e| format!("keyring set: {e}"))
}

fn try_keyring_load() -> Result<Option<StoredToken>, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("keyring entry: {e}"))?;
    match entry.get_password() {
        Ok(json) => {
            let token: StoredToken =
                serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))?;
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keyring get: {e}")),
    }
}

fn try_keyring_delete() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("keyring entry: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete: {e}")),
    }
}

pub fn store_token(app_dir: &std::path::Path, token: &StoredToken) -> Result<(), String> {
    match try_keyring_store(token) {
        Ok(()) => return Ok(()),
        Err(e) => {
            // Keychain indisponível (ex.: Linux headless / sem libsecret). Falha FECHADA por padrão
            // para não expor a credencial silenciosamente em ambientes headless/CI. Defina
            // NEKO_INSECURE_FILE_FALLBACK=1 para permitir o fallback de arquivo (só ofuscação
            // best-effort — não é proteção forte; ver `derive_key`).
            if std::env::var("NEKO_INSECURE_FILE_FALLBACK").as_deref() != Ok("1") {
                return Err(format!(
                    "Keychain unavailable ({e}). Set NEKO_INSECURE_FILE_FALLBACK=1 to allow \
                     the insecure file-based fallback, or install a keychain (libsecret on Linux)."
                ));
            }
            eprintln!("NEKO_INSECURE_FILE_FALLBACK=1: using weak file-based token storage ({e})");
        }
    }

    let key = derive_key(app_dir)?;
    let encrypted = encrypt_token(token, &key)?;
    let path = encrypted_token_path(app_dir);
    std::fs::write(&path, &encrypted).map_err(|e| format!("write encrypted: {e}"))
}

pub fn load_token(app_dir: &std::path::Path) -> Result<Option<StoredToken>, String> {
    if let Ok(Some(token)) = try_keyring_load() {
        return Ok(Some(token));
    }

    let path = encrypted_token_path(app_dir);
    if !path.exists() {
        return Ok(None);
    }

    let key = derive_key(app_dir)?;
    let data = std::fs::read(&path).map_err(|e| format!("read encrypted: {e}"))?;
    let token = decrypt_token(&data, &key)?;
    Ok(Some(token))
}

pub fn delete_token(app_dir: &std::path::Path) -> Result<(), String> {
    let _ = try_keyring_delete();

    let path = encrypted_token_path(app_dir);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete encrypted: {e}"))?;
    }
    Ok(())
}

/// Revoga o token no Google (best-effort). Desconectar deve invalidar o acesso DO LADO do Google,
/// não só esquecer o token localmente. Falhas são ignoradas — o apagamento local sempre ocorre.
pub async fn revoke_token(app_dir: &std::path::Path) {
    if let Ok(Some(token)) = load_token(app_dir) {
        let tok = if !token.refresh_token.is_empty() {
            token.refresh_token
        } else {
            token.access_token
        };
        if !tok.is_empty() {
            // Cliente compartilhado COM timeout (`http::client`): com `reqwest::Client::new()` cru
            // (sem timeout), um Google lento penduraria o `disconnect_google` indefinidamente.
            let _ = crate::http::client()
                .post("https://oauth2.googleapis.com/revoke")
                .form(&[("token", tok)])
                .send()
                .await;
        }
    }
}

pub fn is_token_expired(token: &StoredToken) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    token.expires_at <= now
}

pub async fn refresh_access_token(
    app_dir: &std::path::Path,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<StoredToken, String> {
    let token = load_token(app_dir)?.ok_or("no token to refresh".to_string())?;

    if token.refresh_token.is_empty() {
        return Err("no refresh token available".to_string());
    }

    let mut params = vec![
        ("client_id", client_id.to_string()),
        ("refresh_token", token.refresh_token.clone()),
        ("grant_type", "refresh_token".to_string()),
    ];
    if let Some(secret) = client_secret.filter(|s| !s.trim().is_empty()) {
        params.push(("client_secret", secret.to_string()));
    }

    let resp = crate::http::send_with_retry(
        crate::http::client()
            .post("https://oauth2.googleapis.com/token")
            .form(&params),
    )
    .await
    .map_err(|e| format!("refresh request: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        // Não repassa o corpo bruto do upstream para o frontend — pode conter detalhe de diagnóstico
        // não destinado ao usuário final. Mantém o status HTTP para depuração.
        let _ = resp.text().await; // consome o corpo para liberar a conexão
        return Err(format!(
            "Token refresh failed (HTTP {status}). Reconnect your Google account."
        ));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or("no access_token in refresh response")?
        .to_string();
    let expires_in: u64 = json["expires_in"].as_u64().unwrap_or(3600);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Rotação de refresh token: se a resposta trouxer um novo refresh_token, ADOTA-O (Google pode
    // rotacionar; reusar o antigo quebraria o próximo refresh). No-op quando não há rotação.
    let refresh_token = json["refresh_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or(token.refresh_token);

    let new_token = StoredToken {
        access_token,
        refresh_token,
        expires_at: now + expires_in - 60,
        scope: token.scope,
    };

    store_token(app_dir, &new_token)?;
    Ok(new_token)
}

pub async fn ensure_valid_token(
    app_dir: &std::path::Path,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<StoredToken, String> {
    let token = load_token(app_dir)?.ok_or("not authenticated".to_string())?;

    if !is_token_expired(&token) {
        return Ok(token);
    }

    refresh_access_token(app_dir, client_id, client_secret).await
}

/// Mensagem acionável devolvida quando o token armazenado NÃO tem o escopo de escrita: o usuário
/// precisa re-autorizar (o frontend a transforma num prompt de re-consentimento). Exposta como
/// constante para o teste/UI casarem a string exata, em vez de espalhar o literal.
pub const NEEDS_WRITE_REAUTH: &str =
    "Re-autorize para habilitar a escrita: sua conexão atual é somente leitura.";

/// O escopo concedido (gravado no token na troca, ver `oauth::mod`) inclui a escrita na planilha?
/// O Google devolve os escopos concedidos separados por espaço. Um token sem o escopo
/// `spreadsheets` só permite leitura; esta checagem faz a rota de apply falhar cedo com um erro de
/// re-consentimento em vez de propagar o 403 cru.
pub fn scope_grants_write(granted_scope: &str) -> bool {
    granted_scope
        .split_whitespace()
        .any(|s| s == super::pkce::SHEETS_WRITE_SCOPE)
}

/// Garante que o token armazenado pode ESCREVER na planilha. Reusa `ensure_valid_token` (refresh se
/// expirado) e então valida o escopo concedido; em falta de escopo, devolve `NEEDS_WRITE_REAUTH`.
/// A rota de apply chama isto ANTES de tentar a escrita real.
pub async fn ensure_write_scope(
    app_dir: &std::path::Path,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<StoredToken, String> {
    let token = ensure_valid_token(app_dir, client_id, client_secret).await?;
    if !scope_grants_write(&token.scope) {
        return Err(NEEDS_WRITE_REAUTH.to_string());
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    fn temp_app_dir() -> PathBuf {
        let dir = temp_dir().join(format!("neko-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_token_store_roundtrip() {
        let _guard = INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = temp_app_dir();
        let token = StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 1717977600,
            scope: "spreadsheets.readonly".into(),
        };
        // Permite o fallback de arquivo para o roundtrip ser determinístico tanto com keychain
        // disponível (keyring vence primeiro) quanto sem (ex.: CI headless).
        // SAFETY: serializado por INSECURE_FILE_FALLBACK_LOCK; nenhum teste concorrente altera o env.
        unsafe { std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1") };
        store_token(&dir, &token).unwrap();
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        let loaded = load_token(&dir).unwrap().unwrap();
        assert_eq!(loaded.access_token, token.access_token);
        assert_eq!(loaded.refresh_token, token.refresh_token);
        delete_token(&dir).unwrap();
        assert!(load_token(&dir).unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_token_serialization_roundtrip() {
        let token = StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 1717977600,
            scope: "spreadsheets.readonly".into(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let restored: StoredToken = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.access_token, token.access_token);
    }

    #[test]
    fn scope_grants_write_detects_readonly_vs_write_token() {
        // Token antigo (somente leitura) → sem escrita; precisa re-autorizar.
        assert!(!scope_grants_write(
            "https://www.googleapis.com/auth/spreadsheets.readonly \
             https://www.googleapis.com/auth/drive.metadata.readonly"
        ));
        assert!(!scope_grants_write(""));
        // Token novo (escopo de escrita concedido) → pode escrever.
        assert!(scope_grants_write(
            "https://www.googleapis.com/auth/spreadsheets \
             https://www.googleapis.com/auth/drive.metadata.readonly"
        ));
        // O readonly NÃO deve casar por ser prefixo do de escrita (split por espaço, igualdade exata).
        assert!(!scope_grants_write(
            "https://www.googleapis.com/auth/spreadsheets.readonly"
        ));
    }

    #[test]
    fn test_is_token_expired() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired = StoredToken {
            access_token: "test".into(),
            refresh_token: "test".into(),
            expires_at: now - 100,
            scope: "".into(),
        };
        assert!(is_token_expired(&expired));

        let valid = StoredToken {
            access_token: "test".into(),
            refresh_token: "test".into(),
            expires_at: now + 3600,
            scope: "".into(),
        };
        assert!(!is_token_expired(&valid));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let dir = temp_app_dir();
        let key = derive_key(&dir).unwrap();
        let token = StoredToken {
            access_token: "ya29.secret".into(),
            refresh_token: "1//secret".into(),
            expires_at: 1717977600,
            scope: "spreadsheets.readonly".into(),
        };

        let encrypted = encrypt_token(&token, &key).unwrap();
        assert!(!encrypted.is_empty());

        let decrypted = decrypt_token(&encrypted, &key).unwrap();
        assert_eq!(decrypted.access_token, token.access_token);
        assert_eq!(decrypted.refresh_token, token.refresh_token);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let dir = temp_app_dir();
        let key = derive_key(&dir).unwrap();
        let token = StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 1717977600,
            scope: "".into(),
        };

        let enc1 = encrypt_token(&token, &key).unwrap();
        let enc2 = encrypt_token(&token, &key).unwrap();
        assert_ne!(enc1, enc2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_derive_key_consistent() {
        let dir = temp_app_dir();
        let key1 = derive_key(&dir).unwrap();
        let key2 = derive_key(&dir).unwrap();
        assert_eq!(key1, key2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_store_token_fails_closed_without_keychain_env() {
        // Sem NEKO_INSECURE_FILE_FALLBACK e sem keychain, store_token deve FALHAR FECHADO e NÃO
        // escrever o arquivo cifrado. Em uma máquina com keychain funcionando o keyring vence
        // primeiro e store_token retorna Ok — então só asseguramos o invariante: se houve Err,
        // nenhum arquivo foi escrito.
        let _guard = INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = temp_app_dir();
        // SAFETY: serializado por INSECURE_FILE_FALLBACK_LOCK.
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        let token = StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 9_999_999_999,
            scope: "spreadsheets.readonly".into(),
        };
        let result = store_token(&dir, &token);
        let enc_path = encrypted_token_path(&dir);
        if let Err(msg) = result {
            // Caminho fail-closed: a mensagem deve apontar o env var e o arquivo NÃO pode existir.
            assert!(
                msg.contains("NEKO_INSECURE_FILE_FALLBACK"),
                "fail-closed error must reference the opt-in env var"
            );
            assert!(
                !enc_path.exists(),
                "encrypted token file must NOT be written when failing closed"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_store_token_file_fallback_when_env_set() {
        // Com NEKO_INSECURE_FILE_FALLBACK=1 e sem keychain, o arquivo DEVE ser escrito. Numa
        // máquina com keychain funcionando o keyring vence primeiro (no-op pass).
        let _guard = INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = temp_app_dir();
        // SAFETY: serializado por INSECURE_FILE_FALLBACK_LOCK.
        unsafe { std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1") };
        let token = StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 9_999_999_999,
            scope: "spreadsheets.readonly".into(),
        };
        // Não deve dar Err independentemente de o keychain estar presente.
        assert!(store_token(&dir, &token).is_ok());
        // SAFETY: serializado por INSECURE_FILE_FALLBACK_LOCK.
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        std::fs::remove_dir_all(&dir).ok();
    }
}
