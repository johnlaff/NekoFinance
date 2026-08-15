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
///
/// `pool` grava `app_setting.sheets_client_id` no MESMO ponto em que o token acaba de ir para o
/// cofre (issue #475): conexão bem-sucedida É "sync configurado", então o client id que o boot
/// (`sync_task::resolve_client_id`) e a renovação em segundo plano precisam já fica disponível
/// aqui — antes deste fix, só um IMPORT gravava essa chave (`GoogleSheetsPanel.tsx`), então um
/// aparelho recém-conectado sem nenhum import ainda não tinha de onde o check-out do boot ler o
/// client id, e o check-out silenciava mesmo com um snapshot remoto disponível para restaurar.
/// Cobre tanto a primeira conexão quanto a RECONEXÃO — as duas passam por `start_oauth_flow`.
pub async fn run_oauth_flow(
    config: pkce::OAuthConfig,
    state: pkce::OAuthState,
    app_dir: std::path::PathBuf,
    callback: OAuthCallback,
    pool: &sqlx::SqlitePool,
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

    // A credencial Android (`RedirectStrategy::DeepLink`) não tem client secret no Console — só
    // PKCE autentica a troca. `pkce::resolve_client_secret` já zera o secret para o target
    // Android (`cfg!(target_os = "android")`, cobrindo também a renovação em segundo plano); esta
    // é a segunda camada, na única borda por onde todo `code` passa antes da troca — não depende
    // de quem montou `config` ter passado pelo resolvedor certo.
    let config = pkce::OAuthConfig {
        client_secret: secret_for_exchange(config.client_secret, &state.redirect),
        ..config
    };

    // Exchange code for token
    let token = exchange_token(&config, &state, &code).await?;

    // Store token
    token_store::store_token(&app_dir, &token)?;

    // Best-effort: o token já está gravado e a conexão já é um sucesso do ponto de vista do dono —
    // uma falha aqui não pode desfazer isso. Sem o client id persistido, o pior caso é o mesmo
    // silêncio de antes do fix (a defesa em `resolve_drive_client_best_effort_at` cobre esse
    // residual com um aviso próprio na tela de Conexão).
    if let Err(e) =
        crate::commands::app_setting_set(pool, "sheets_client_id", &config.client_id).await
    {
        eprintln!("OAuth: falha ao persistir sheets_client_id: {e}");
    }

    Ok(token)
}

/// O client secret que a troca do `code` deve enviar, dada a estratégia de redirect. Só o
/// `Loopback` (Desktop) tem uma credencial com secret; o `DeepLink` (Android) nunca envia um,
/// mesmo que `config_secret` traga algum valor.
fn secret_for_exchange(
    config_secret: Option<String>,
    redirect: &redirect::RedirectStrategy,
) -> Option<String> {
    match redirect {
        redirect::RedirectStrategy::DeepLink => None,
        redirect::RedirectStrategy::Loopback { .. } => config_secret,
    }
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
    use super::{OAuthCallback, constant_time_eq, run_oauth_flow, secret_for_exchange};
    use crate::oauth::pkce::{OAuthConfig, OAuthState};
    use crate::oauth::redirect::RedirectStrategy;
    use std::path::PathBuf;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-oauth-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn constant_time_eq_matches_only_identical() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124")); // 1 byte diferente
        assert!(!constant_time_eq(b"abc", b"abc123")); // tamanhos diferentes
        assert!(!constant_time_eq(b"", b"x"));
    }

    // A credencial Android não tem client secret — a troca do `code` no caminho DeepLink nunca
    // envia um, mesmo que `config` tenha chegado com um (ex.: vazamento do secret Desktop via env
    // de build compartilhado). O Loopback (Desktop) segue passando o que recebeu, sem mudança.

    #[test]
    fn secret_for_exchange_zera_o_secret_no_caminho_deep_link() {
        let secret = secret_for_exchange(
            Some("secret-do-desktop-vazado".to_string()),
            &RedirectStrategy::DeepLink,
        );
        assert_eq!(secret, None);
    }

    #[test]
    fn secret_for_exchange_zera_mesmo_sem_secret_nenhum_no_caminho_deep_link() {
        assert_eq!(secret_for_exchange(None, &RedirectStrategy::DeepLink), None);
    }

    #[test]
    fn secret_for_exchange_preserva_o_secret_no_caminho_loopback() {
        let secret = secret_for_exchange(
            Some("secret-do-desktop".to_string()),
            &RedirectStrategy::Loopback { port: 48080 },
        );
        assert_eq!(secret.as_deref(), Some("secret-do-desktop"));
    }

    #[test]
    fn secret_for_exchange_preserva_ausencia_no_caminho_loopback() {
        assert_eq!(
            secret_for_exchange(None, &RedirectStrategy::Loopback { port: 48080 }),
            None
        );
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

    // --- Issue #475: a conexão persiste o client id no mesmo passo em que grava o token --------

    /// Roda `run_oauth_flow` de ponta a ponta contra um endpoint de troca mockado — mock server,
    /// exchange, gravação do token E as asserções de `app_setting`/`resolve_client_id` inteiros
    /// dentro do MESMO `block_on`, porque `SqlitePool` não sobrevive à troca de runtime que a
    /// criou. Devolve `app_dir` (um `PathBuf` simples, sem laço com o runtime) para o chamador
    /// conferir o cofre com `token_store::load_token` (síncrono) depois.
    ///
    /// `run_oauth_flow` grava o token via `token_store::store_token` (síncrono) DEPOIS de um
    /// `.await` de rede — não dá para isolar essa escrita num bloco síncrono só do jeito que os
    /// outros testes deste repo guardam `NEKO_INSECURE_FILE_FALLBACK` (o comentário do módulo
    /// promete "nunca atravessa um `.await`" com o `std::sync::Mutex` do guard). A saída é rodar o
    /// fluxo inteiro dentro de um `Runtime` PRÓPRIO via `block_on` — uma chamada síncrona do ponto
    /// de vista do chamador — a partir de um `#[test]` comum (sem runtime ambiente, então sem o
    /// panic de "runtime aninhado" que o doc de `run_oauth_flow` alerta): o guard cerca o
    /// `block_on` inteiro sem nunca atravessar um `.await` léxico.
    fn assert_connect_persists_client_id(
        client_id: &str,
        access_token: &str,
        refresh_token: &str,
    ) -> PathBuf {
        let _guard = crate::secret_vault::INSECURE_FILE_FALLBACK_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // SAFETY: serializado pelo guard acima.
        unsafe { std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1") };

        let rt = tokio::runtime::Runtime::new().expect("runtime de teste");
        let app_dir = rt.block_on(async {
            let mut server = mockito::Server::new_async().await;
            server
                .mock("POST", "/token")
                .with_status(200)
                .with_body(format!(
                    r#"{{"access_token":"{access_token}","refresh_token":"{refresh_token}","expires_in":3600,"scope":""}}"#
                ))
                .create_async()
                .await;

            let dir = test_dir();
            let app_dir = dir.join("app-dir");
            std::fs::create_dir_all(&app_dir).unwrap();
            let db_path = dir.join("neko-finance.db");
            let pool = crate::snapshot::checkout::open_migrated_pool(&db_path)
                .await
                .expect("pool de teste");

            let mut config = OAuthConfig::google(client_id.to_string(), None);
            config.token_url = format!("{}/token", server.url());
            let oauth_state = OAuthState::new(RedirectStrategy::DeepLink);
            let csrf = oauth_state.csrf_token.secret().to_string();

            let (tx, rx) = tokio::sync::oneshot::channel();
            tx.send(Ok(("auth-code-475".to_string(), Some(csrf))))
                .expect("o canal deve aceitar o envio");

            run_oauth_flow(
                config,
                oauth_state,
                app_dir.clone(),
                OAuthCallback::DeepLink(rx),
                &pool,
            )
            .await
            .expect("token exchange mockado deve suceder");

            // A LACUNA da issue #475: sem este fix, `sheets_client_id` só era gravado por um
            // IMPORT (`GoogleSheetsPanel.tsx`), nunca pela conexão em si — um aparelho
            // recém-conectado sem nenhum import ainda não tinha de onde o boot ler o client id.
            let persisted = crate::commands::app_setting_get(&pool, "sheets_client_id")
                .await
                .unwrap();
            assert_eq!(
                persisted.as_deref(),
                Some(client_id),
                "a conexão deve persistir o client id — sem isto o check-out do boot nunca \
                 resolve um client id antes do primeiro import"
            );

            // A costura completa: o resolver que o boot usa de fato enxerga o valor gravado.
            let resolved = crate::sync_task::resolve_client_id(&pool).await;
            assert_eq!(
                resolved.as_deref(),
                Some(client_id),
                "resolve_client_id deve resolver o client id logo após a conexão, sem depender \
                 de um import prévio"
            );

            app_dir
        });

        // SAFETY: serializado pelo guard acima.
        unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        app_dir
    }

    #[test]
    fn conectar_persiste_o_client_id_no_mesmo_passo_que_grava_o_token() {
        let app_dir = assert_connect_persists_client_id(
            "client-475.apps.googleusercontent.com",
            "ya29.novo",
            "1//novo",
        );

        // O token foi para o cofre — pré-condição que já existia antes deste fix.
        let stored = crate::oauth::token_store::load_token(&app_dir)
            .expect("ler o cofre não deve falhar")
            .expect("o token deve ter sido gravado");
        assert_eq!(stored.access_token, "ya29.novo");
    }

    #[test]
    fn reconectar_tambem_persiste_o_client_id() {
        // A reconexão (`SettingsScreen.tsx::handleReconnect`) passa pelo MESMO comando
        // (`start_oauth_flow` → `run_oauth_flow`) que a primeira conexão — cobrir separadamente
        // documenta que não há um segundo caminho que precise do mesmo fix.
        assert_connect_persists_client_id(
            "client-reconexao.apps.googleusercontent.com",
            "ya29.reconexao",
            "1//reconexao",
        );
    }
}
