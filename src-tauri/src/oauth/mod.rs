pub mod pkce;
pub mod redirect;
pub mod server;
pub mod token_store;

use std::sync::Mutex;

pub struct OAuthStateStore(pub Mutex<Option<pkce::OAuthState>>);
pub struct AppDataDir(pub std::path::PathBuf);

/// Onde o `code` do consentimento chega até este fluxo — o loopback HTTP do desktop (o listener já
/// ligado na porta do `redirect_uri`) ou o canal alimentado pelo evento de deep link do Android
/// (`commands::oauth_cmds::register_deep_link_listener`, shell puro — este módulo não sabe que o
/// canal do Android existe por trás de um plugin Tauri, só que é "mais um jeito de receber o code").
pub enum OAuthCallback {
    // Só construída no shell desktop (`commands::oauth_cmds::start_desktop_oauth`) — no Android é
    // código morto por desenho, não uma lacuna; o loopback já é exercido por `server::tests`.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    Loopback(std::net::TcpListener),
    // Só construída no shell Android (`commands::oauth_cmds::start_android_oauth`) — nos demais
    // alvos é código morto por desenho, não uma lacuna; os testes de `wait_for_code` abaixo
    // exercem a variante sem depender do target real.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    DeepLink(tokio::sync::oneshot::Receiver<Result<(String, Option<String>), String>>),
}

impl OAuthCallback {
    async fn wait_for_code(self) -> Result<(String, Option<String>), String> {
        match self {
            OAuthCallback::Loopback(listener) => {
                server::OAuthServer::new(listener).listen_for_code().await
            }
            OAuthCallback::DeepLink(rx) => rx.await.map_err(|_| {
                "deep link: canal de retorno fechado antes da resposta do Google".to_string()
            })?,
        }
    }
}

/// Roda dentro do runtime do Tauri (`tokio::spawn` em commands.rs), portanto precisa permanecer
/// `async` de ponta a ponta. Criar um runtime Tokio aninhado e chamar `block_on` nesse contexto
/// causa panic ("cannot start a runtime from within a runtime") e impede a conclusão do fluxo OAuth.
/// Abrir o navegador/Custom Tab é responsabilidade de quem chama (`commands::oauth_cmds`, que tem o
/// `AppHandle` que o Android precisa para abrir a Custom Tab via `tauri-plugin-opener`) — este fluxo
/// só espera o `code` chegar por `callback`, valida e troca.
pub async fn run_oauth_flow(
    config: pkce::OAuthConfig,
    state: pkce::OAuthState,
    app_dir: std::path::PathBuf,
    callback: OAuthCallback,
) -> Result<token_store::StoredToken, String> {
    let (code, returned_state) = callback.wait_for_code().await?;

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
    let mut params = vec![
        ("code", code.to_string()),
        ("client_id", config.client_id.clone()),
        ("code_verifier", state.verifier().secret().to_string()),
        ("redirect_uri", state.redirect.redirect_uri()),
        ("grant_type", "authorization_code".to_string()),
    ];
    // Opcional no fluxo de app instalado (doc oficial); enviado quando configurado —
    // o secret de client "Desktop" não é confidencial, vive no .env local gitignored.
    if let Some(secret) = &config.client_secret {
        params.push(("client_secret", secret.clone()));
    }

    let resp =
        crate::http::send_with_retry(crate::http::client().post(&config.token_url).form(&params))
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
    use super::{OAuthCallback, constant_time_eq};

    #[test]
    fn constant_time_eq_matches_only_identical() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124")); // 1 byte diferente
        assert!(!constant_time_eq(b"abc", b"abc123")); // tamanhos diferentes
        assert!(!constant_time_eq(b"", b"x"));
    }

    // As duas variantes de `OAuthCallback` sobre o mesmo PKCE — o loopback já é exercido por
    // `server::tests`; aqui cobrimos a variante nova, o canal do deep link do Android.

    #[tokio::test]
    async fn deep_link_wait_for_code_devolve_o_que_o_listener_enviou() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tx.send(Ok((
            "auth-code-123".to_string(),
            Some("csrf-state".to_string()),
        )))
        .expect("o canal deve aceitar o envio");

        let (code, state) = OAuthCallback::DeepLink(rx)
            .wait_for_code()
            .await
            .expect("o code enviado deve chegar sem erro");
        assert_eq!(code, "auth-code-123");
        assert_eq!(state.as_deref(), Some("csrf-state"));
    }

    #[tokio::test]
    async fn deep_link_wait_for_code_repassa_o_erro_do_listener() {
        // O listener do plugin manda `Err` quando a URL não tem `code` (ex.: usuário cancelou o
        // consentimento e o Google devolveu só `error=access_denied`).
        let (tx, rx) = tokio::sync::oneshot::channel();
        tx.send(Err("deep link de retorno sem o parâmetro code".to_string()))
            .expect("o canal deve aceitar o envio");

        let error = OAuthCallback::DeepLink(rx)
            .wait_for_code()
            .await
            .expect_err("o erro do listener deve propagar");
        assert_eq!(error, "deep link de retorno sem o parâmetro code");
    }

    #[tokio::test]
    async fn deep_link_wait_for_code_falha_quando_o_canal_fecha_sem_resposta() {
        // A limitação conhecida do upstream (tauri-apps/plugins-workspace#2397): um segundo deep
        // link no mesmo ciclo de vida pode nunca disparar o listener. Sem resposta, o emissor
        // droppado fecha o canal e este fluxo falha de forma legível, em vez de travar para
        // sempre — é o que dá tempo da UI de Conexão orientar reiniciar o app.
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(String, Option<String>), String>>();
        drop(tx);

        let error = OAuthCallback::DeepLink(rx)
            .wait_for_code()
            .await
            .expect_err("canal fechado sem resposta deve virar erro, nunca travar");
        assert!(error.contains("canal de retorno fechado"));
    }
}
