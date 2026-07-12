use super::*;

#[tauri::command]
pub async fn start_oauth_flow(
    state: tauri::State<'_, OAuthStateStore>,
    app_dir: tauri::State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<String, String> {
    // Shell: o secret pode vir do env do processo (não do bundle do frontend) — ver resolve_*.
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let config = oauth::pkce::OAuthConfig::google(client_id, client_secret);
    // Liga o listener UMA vez e o mantém: a porta do redirect_uri e a que vamos escutar são a
    // mesma conexão — sem janela TOCTOU entre descobrir a porta e voltar a ligá-la.
    let (listener, port) = bind_loopback_listener()?;
    let oauth_state = oauth::pkce::OAuthState::new(port);

    let app_dir_path = app_dir.0.clone();
    let config_for_bg =
        oauth::pkce::OAuthConfig::google(config.client_id.clone(), config.client_secret.clone());

    // Store state and spawn flow
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        let flow_state = oauth_state.clone();
        *guard = Some(oauth_state);

        tokio::spawn(async move {
            match oauth::run_oauth_flow(config_for_bg, flow_state, app_dir_path, listener).await {
                Ok(_token) => {}
                Err(e) => eprintln!("OAuth flow error: {e}"),
            }
        });
    }

    // Clear state after spawn
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        *guard = None;
    }

    Ok("oauth_started".to_string())
}

#[tauri::command]
pub async fn check_auth_status(app_dir: tauri::State<'_, AppDataDir>) -> Result<String, String> {
    match crate::oauth::token_store::load_token(&app_dir.0) {
        Ok(Some(token)) => {
            // Access token expirado mas com refresh_token disponível segue "connected":
            // `ensure_valid_token` renova sob demanda no próximo uso.
            if crate::oauth::token_store::is_token_expired(&token) && token.refresh_token.is_empty()
            {
                Ok("expired".to_string())
            } else {
                Ok("connected".to_string())
            }
        }
        Ok(None) => Ok("disconnected".to_string()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn disconnect_google(app_dir: tauri::State<'_, AppDataDir>) -> Result<(), String> {
    // Revoga no Google (best-effort) ANTES de apagar localmente — desconectar de verdade.
    crate::oauth::token_store::revoke_token(&app_dir.0).await;
    crate::oauth::token_store::delete_token(&app_dir.0)
}

/// Liga um socket de loopback numa porta efêmera e devolve `(listener, porta)`. O listener NÃO é
/// dropado: quem chama o usa para escutar o callback — eliminando o rebind (TOCTOU) do fluxo OAuth.
pub(crate) fn bind_loopback_listener() -> Result<(std::net::TcpListener, u16), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| format!("port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    Ok((listener, port))
}
