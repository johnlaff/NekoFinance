//! Onde o Google devolve o `code` do consentimento — loopback HTTP no desktop, deep link do
//! esquema reverso da credencial Android no Android (ADR-0014, cláusula 2). O PKCE por baixo dos
//! dois nunca muda; só a porta de retorno do `code` varia por plataforma, e este módulo é a fonte
//! única do `redirect_uri` que `pkce::OAuthState::build_auth_url` anuncia no consentimento e que
//! `oauth::exchange_token` repete na troca — os dois precisam casar byte a byte (RFC 6749 §3.1.2.3).

/// Esquema do deep link de retorno do OAuth no Android — o CLIENT ID REVERSO da credencial Google
/// de tipo Android (`50282483752-h53glgfl0laqe0t3rtqsj5a9sgc6b60g.apps.googleusercontent.com`),
/// não o identificador de bundle do app. A política do Google só aceita redirect de esquema
/// customizado para esse tipo de credencial — ela valida o app pelo par (pacote, SHA-1) já
/// registrado no Console (`docs/building-android.md`), e o esquema tem que ser literalmente o
/// client id de trás para frente (é assim que o Google associa o retorno à credencial, sem
/// precisar de um domínio verificado — a distribuição é sideload, sem site para hospedar a
/// verificação de um App Link). Precisa casar com `tauri.conf.json > plugins > deep-link >
/// mobile[0].scheme` — o JSON não enxerga esta constante, então uma mudança aqui precisa do mesmo
/// commit lá (é isso que faz o `AndroidManifest.xml` gerado aceitar o intent e o Kotlin do plugin
/// reconhecer a URL como deep link).
pub const ANDROID_OAUTH_SCHEME: &str =
    "com.googleusercontent.apps.50282483752-h53glgfl0laqe0t3rtqsj5a9sgc6b60g";

/// Path do deep link — o formato documentado pelo Google para o esquema customizado de um cliente
/// instalado (`REVERSED_CLIENT_ID:/oauth2redirect`, um único `/`: sem autoridade, só path). Mesma
/// ressalva de sincronia manual com `tauri.conf.json > plugins > deep-link > mobile[0].path`.
pub const ANDROID_OAUTH_PATH: &str = "/oauth2redirect";

/// Verdadeiro quando `scheme` é o esquema do retorno do OAuth Android — o mesmo predicado que
/// `commands::oauth_cmds::register_deep_link_listener` usa para filtrar o evento do plugin,
/// extraído aqui para testar sem depender do plugin Tauri real nem do target Android. Só chamada
/// em produção pelo shell Android (código morto por desenho nos demais alvos — os testes abaixo
/// exercem a função sem depender do target real).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn is_oauth_callback_scheme(scheme: &str) -> bool {
    scheme == ANDROID_OAUTH_SCHEME
}

/// Onde o `code` do consentimento chega até o processo. O mesmo PKCE (`OAuthState`) funciona com
/// qualquer uma das duas — só o `redirect_uri` anunciado ao Google e o mecanismo de captura mudam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedirectStrategy {
    /// Desktop: servidor HTTP efêmero no loopback, na mesma porta que o `redirect_uri` anuncia
    /// (sem rebind — `commands::oauth_cmds::bind_loopback_listener` liga o socket antes de montar
    /// esta variante, eliminando a janela TOCTOU entre descobrir a porta e voltar a escutá-la). Só
    /// construída no shell desktop; no Android é código morto por desenho (os testes abaixo
    /// exercem `redirect_uri` sem depender do target real).
    #[cfg_attr(target_os = "android", allow(dead_code))]
    Loopback { port: u16 },
    /// Android: o navegador (Custom Tab) devolve pelo esquema do app; o retorno chega por evento
    /// do plugin de deep link, não por socket — `oauth::OAuthCallback::DeepLink`. Só construída
    /// no shell Android; nos demais alvos é código morto por desenho (os testes abaixo exercem
    /// `redirect_uri` sem depender do target real).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    DeepLink,
}

impl RedirectStrategy {
    /// O `redirect_uri` exato anunciado no consentimento e repetido na troca do `code` — os dois
    /// lados (`pkce::OAuthState::build_auth_url`, `oauth::exchange_token`) chamam este método em
    /// vez de reconstruir a string cada um a seu jeito, para nunca divergir por um dígito.
    pub fn redirect_uri(&self) -> String {
        match self {
            RedirectStrategy::Loopback { port } => format!("http://127.0.0.1:{port}"),
            RedirectStrategy::DeepLink => {
                format!("{ANDROID_OAUTH_SCHEME}:{ANDROID_OAUTH_PATH}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_redirect_uri_usa_a_porta_ligada() {
        assert_eq!(
            RedirectStrategy::Loopback { port: 48080 }.redirect_uri(),
            "http://127.0.0.1:48080"
        );
    }

    #[test]
    fn deep_link_redirect_uri_usa_o_esquema_reverso_da_credencial_android() {
        assert_eq!(
            RedirectStrategy::DeepLink.redirect_uri(),
            "com.googleusercontent.apps.50282483752-h53glgfl0laqe0t3rtqsj5a9sgc6b60g:/oauth2redirect"
        );
    }

    #[test]
    fn is_oauth_callback_scheme_aceita_o_esquema_reverso_da_credencial_android() {
        assert!(is_oauth_callback_scheme(
            "com.googleusercontent.apps.50282483752-h53glgfl0laqe0t3rtqsj5a9sgc6b60g"
        ));
    }

    #[test]
    fn is_oauth_callback_scheme_rejeita_outros_esquemas() {
        // O esquema antigo (identificador de bundle) nunca funcionou — a política do Google exige
        // o client id reverso para credenciais de tipo Android.
        assert!(!is_oauth_callback_scheme("app.neko.finance"));
        assert!(!is_oauth_callback_scheme("https"));
        assert!(!is_oauth_callback_scheme(""));
    }

    #[test]
    fn as_duas_variantes_nunca_colidem() {
        assert_ne!(
            RedirectStrategy::Loopback { port: 48080 }.redirect_uri(),
            RedirectStrategy::DeepLink.redirect_uri()
        );
    }

    #[test]
    fn portas_distintas_produzem_redirect_uri_distintos() {
        assert_ne!(
            RedirectStrategy::Loopback { port: 1 }.redirect_uri(),
            RedirectStrategy::Loopback { port: 2 }.redirect_uri()
        );
    }
}
