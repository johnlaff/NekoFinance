use super::*;
#[cfg(target_os = "android")]
use tauri::Manager;

#[tauri::command]
pub async fn start_oauth_flow(
    app: tauri::AppHandle,
    state: tauri::State<'_, OAuthStateStore>,
    app_dir: tauri::State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<String, String> {
    // Shell: o secret pode vir do env do processo (não do bundle do frontend) — ver resolve_*.
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let config = oauth::pkce::OAuthConfig::google(client_id, client_secret);

    #[cfg(not(target_os = "android"))]
    let (oauth_state, callback) = start_desktop_oauth()?;
    #[cfg(target_os = "android")]
    let (oauth_state, callback) = start_android_oauth(&app)?;

    let auth_url = oauth_state.build_auth_url(&config);
    open_auth_url(&app, &auth_url)?;

    let app_dir_path = app_dir.0.clone();
    let config_for_bg =
        oauth::pkce::OAuthConfig::google(config.client_id.clone(), config.client_secret.clone());

    // Store state and spawn flow
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        let flow_state = oauth_state.clone();
        *guard = Some(oauth_state);

        tokio::spawn(async move {
            match oauth::run_oauth_flow(config_for_bg, flow_state, app_dir_path, callback).await {
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
/// Só chamada pelo shell desktop (`start_desktop_oauth`); no Android é código morto por desenho.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub(crate) fn bind_loopback_listener() -> Result<(std::net::TcpListener, u16), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| format!("port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    Ok((listener, port))
}

/// Desktop: liga o listener de loopback UMA vez e o mantém — a porta do `redirect_uri` e a que
/// vamos escutar são a mesma conexão, sem janela TOCTOU entre descobrir a porta e voltar a ligá-la.
#[cfg(not(target_os = "android"))]
fn start_desktop_oauth() -> Result<(oauth::pkce::OAuthState, oauth::OAuthCallback), String> {
    let (listener, port) = bind_loopback_listener()?;
    let oauth_state =
        oauth::pkce::OAuthState::new(oauth::redirect::RedirectStrategy::Loopback { port });
    Ok((oauth_state, oauth::OAuthCallback::Loopback(listener)))
}

/// Android: nenhum socket — o retorno chega pelo deep link do esquema do app. Guarda o emissor do
/// canal em [`PendingAndroidOAuthCallback`] para [`register_deep_link_listener`] entregar o `code`
/// assim que o evento chegar; só UM fluxo pendente por vez (um novo `start_oauth_flow` substitui o
/// emissor anterior, que aí falha ao enviar — o `rx` órfão do fluxo velho já teria estourado no
/// timeout de qualquer forma).
#[cfg(target_os = "android")]
fn start_android_oauth(
    app: &tauri::AppHandle,
) -> Result<(oauth::pkce::OAuthState, oauth::OAuthCallback), String> {
    let oauth_state = oauth::pkce::OAuthState::new(oauth::redirect::RedirectStrategy::DeepLink);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let pending = app.state::<PendingAndroidOAuthCallback>();
    *pending.0.lock().map_err(|e| format!("lock: {e}"))? = Some(tx);
    Ok((oauth_state, oauth::OAuthCallback::DeepLink(rx)))
}

/// Desktop: abre o navegador padrão do sistema no `auth_url` — inalterado desde antes do porte
/// Android.
#[cfg(not(target_os = "android"))]
fn open_auth_url(_app: &tauri::AppHandle, auth_url: &str) -> Result<(), String> {
    open::that(auth_url).map_err(|e| format!("browser: {e}"))
}

/// Android: abre o `auth_url` numa Custom Tab (`androidx.browser.customtabs`, via
/// `tauri-plugin-opener` com `with: "inAppBrowser"`) — o consentimento acontece sobre o app, sem
/// trocar de aplicativo.
#[cfg(target_os = "android")]
fn open_auth_url(app: &tauri::AppHandle, auth_url: &str) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(auth_url, Some("inAppBrowser"))
        .map_err(|e| format!("custom tab: {e}"))
}

/// Emissor do canal que entrega o `(code, state)` do deep link pendente ao fluxo OAuth em curso —
/// gerenciado por `lib.rs` (`.manage()`) e alimentado por [`register_deep_link_listener`].
#[cfg(target_os = "android")]
pub(crate) struct PendingAndroidOAuthCallback(
    pub(crate)  std::sync::Mutex<
        Option<tokio::sync::oneshot::Sender<Result<(String, Option<String>), String>>>,
    >,
);

/// Registra, UMA vez no `.setup()`, o listener persistente do plugin de deep link. Fica de pé pelo
/// processo inteiro — cada `start_oauth_flow` só troca QUEM está esperando no outro lado do canal
/// (`PendingAndroidOAuthCallback`), nunca re-registra o listener em si.
///
/// Limitação conhecida do upstream (tauri-apps/plugins-workspace#2397): um SEGUNDO deep link no
/// mesmo ciclo de vida do processo pode não disparar este listener — o retorno da primeira conexão
/// funciona, o de uma reautenticação subsequente às vezes não chega. Não há correção no lado deste
/// código: o listener está correto e persistente: a falha é o plugin nativo não repassar o evento.
/// A UI de Conexão (`SettingsScreen.tsx`) orienta reiniciar o app quando a reautenticação não
/// completa, enquanto o upstream não corrige.
#[cfg(target_os = "android")]
pub(crate) fn register_deep_link_listener(app: &tauri::AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        let matched = event
            .urls()
            .into_iter()
            .find(|url| url.scheme() == oauth::redirect::ANDROID_OAUTH_SCHEME);
        let Some(url) = matched else { return };

        let code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned());
        let returned_state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned());

        let result = match code {
            Some(code) => Ok((code, returned_state)),
            None => Err("deep link de retorno sem o parâmetro code".to_string()),
        };

        if let Some(pending) = handle.try_state::<PendingAndroidOAuthCallback>()
            && let Ok(mut guard) = pending.0.lock()
            && let Some(tx) = guard.take()
        {
            let _ = tx.send(result);
        }
    });
}
