pub mod pkce;
pub mod server;
pub mod token_store;

use std::sync::Mutex;

pub struct OAuthStateStore(pub Mutex<Option<pkce::OAuthState>>);
pub struct AppDataDir(pub std::path::PathBuf);

/// Roda dentro do runtime do Tauri (`tokio::spawn` em commands.rs) — por isso é `async`
/// de ponta a ponta. A versão anterior criava um runtime tokio aninhado e chamava
/// `block_on` dentro do contexto async, o que PANICA ("cannot start a runtime from within
/// a runtime") — o fluxo OAuth nunca completava (descoberto no 1º dogfooding, 2026-06-12).
pub async fn run_oauth_flow(
    config: pkce::OAuthConfig,
    state: pkce::OAuthState,
    app_dir: std::path::PathBuf,
) -> Result<token_store::StoredToken, String> {
    // Open browser
    let auth_url = state.build_auth_url(&config);
    open::that(&auth_url).map_err(|e| format!("browser: {e}"))?;

    // Wait for the redirect with the authorization code
    let server = server::OAuthServer::new(state.redirect_port);
    let (code, returned_state) = server.listen_for_code().await?;

    // Valida o state (CSRF, RFC 6749 §10.12): o `state` do callback tem que casar com o csrf_token
    // que geramos. Sem isso, um redirect forjado para o loopback injetaria o code de um atacante.
    let expected = state.csrf_token.secret();
    let ok = returned_state
        .as_deref()
        .is_some_and(|s| constant_time_eq(s.as_bytes(), expected.as_bytes()));
    if !ok {
        return Err("state OAuth inválido (possível CSRF) — fluxo abortado".to_string());
    }

    // Exchange code for token
    let token = exchange_token(&config, &state, &code).await?;

    // Store token
    token_store::store_token(&app_dir, &token)?;

    Ok(token)
}

/// Compara dois bytes em tempo constante (não vaza onde diferem). Usado na checagem do state CSRF.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn exchange_token(
    config: &pkce::OAuthConfig,
    state: &pkce::OAuthState,
    code: &str,
) -> Result<token_store::StoredToken, String> {
    let client = reqwest::Client::new();
    let mut params = vec![
        ("code", code.to_string()),
        ("client_id", config.client_id.clone()),
        ("code_verifier", state.verifier().secret().to_string()),
        (
            "redirect_uri",
            format!("http://127.0.0.1:{}", state.redirect_port),
        ),
        ("grant_type", "authorization_code".to_string()),
    ];
    // Opcional no fluxo de app instalado (doc oficial); enviado quando configurado —
    // o secret de client "Desktop" não é confidencial, vive no .env local gitignored.
    if let Some(secret) = &config.client_secret {
        params.push(("client_secret", secret.clone()));
    }

    let resp = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token request: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("token exchange failed: {body}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or("no access_token")?
        .to_string();
    let refresh_token = json["refresh_token"].as_str().unwrap_or("").to_string();
    let expires_in: u64 = json["expires_in"].as_u64().unwrap_or(3600);
    let scope = json["scope"].as_str().unwrap_or("").to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(token_store::StoredToken {
        access_token,
        refresh_token,
        expires_at: now + expires_in - 60, // 60s safety margin
        scope,
    })
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_only_identical() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124")); // 1 byte diferente
        assert!(!constant_time_eq(b"abc", b"abc123")); // tamanhos diferentes
        assert!(!constant_time_eq(b"", b"x"));
    }
}
