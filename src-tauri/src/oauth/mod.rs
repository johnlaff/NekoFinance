pub mod pkce;
pub mod server;
pub mod token_store;

use std::sync::Mutex;

pub struct OAuthStateStore(pub Mutex<Option<pkce::OAuthState>>);
pub struct AppDataDir(pub std::path::PathBuf);

pub fn run_oauth_flow(
    config: pkce::OAuthConfig,
    state: pkce::OAuthState,
    app_dir: std::path::PathBuf,
) -> Result<token_store::StoredToken, String> {
    // Open browser
    let auth_url = state.build_auth_url(&config);
    open::that(&auth_url).map_err(|e| format!("browser: {e}"))?;

    // Block until redirect arrives
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("runtime: {e}"))?;

    let code = rt.block_on(async {
        let server = server::OAuthServer::new(state.redirect_port);
        server.listen_for_code().await
    })?;

    // Exchange code for token
    let token = exchange_token_sync(&config, &state, &code)?;

    // Store token
    token_store::store_token(&app_dir, &token)?;

    Ok(token)
}

fn exchange_token_sync(
    config: &pkce::OAuthConfig,
    state: &pkce::OAuthState,
    code: &str,
) -> Result<token_store::StoredToken, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("runtime: {e}"))?;

    rt.block_on(async {
        let client = reqwest::Client::new();
        let params = [
            ("code", code.to_string()),
            ("client_id", config.client_id.clone()),
            ("code_verifier", state.verifier().secret().to_string()),
            (
                "redirect_uri",
                format!("http://127.0.0.1:{}", state.redirect_port),
            ),
            ("grant_type", "authorization_code".to_string()),
        ];

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
    })
}
