//! Onde o Google devolve o `code` do consentimento — loopback HTTP no desktop, deep link do
//! esquema próprio do app no Android (ADR-0014, cláusula 2). O PKCE por baixo dos dois nunca
//! muda; só a porta de retorno do `code` varia por plataforma, e este módulo é a fonte única do
//! `redirect_uri` que `pkce::OAuthState::build_auth_url` anuncia no consentimento e que
//! `oauth::exchange_token` repete na troca — os dois precisam casar byte a byte (RFC 6749 §3.1.2.3).

/// Esquema do deep link de retorno do OAuth no Android — o identificador de bundle já registrado
/// (`app.neko.finance`, `tauri.conf.json > identifier`), não um App Link com domínio verificado:
/// a distribuição é sideload, sem site para hospedar a verificação. Precisa casar
/// literalmente com `tauri.conf.json > plugins > deep-link > mobile[0].scheme` — o JSON não
/// enxerga esta constante, então uma mudança aqui precisa do mesmo commit lá (é isso que faz o
/// `AndroidManifest.xml` gerado aceitar o intent e o Kotlin do plugin reconhecer a URL como
/// deep link).
pub const ANDROID_OAUTH_SCHEME: &str = "app.neko.finance";

/// Host do deep link — só dá um caminho estável ao `redirect_uri`; o esquema já é exclusivo do
/// app, então nenhum outro host precisa ser aceito. Mesma ressalva de sincronia manual com
/// `tauri.conf.json > plugins > deep-link > mobile[0].host`.
pub const ANDROID_OAUTH_HOST: &str = "oauth-callback";

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
                format!("{ANDROID_OAUTH_SCHEME}://{ANDROID_OAUTH_HOST}")
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
    fn deep_link_redirect_uri_usa_o_esquema_do_app() {
        assert_eq!(
            RedirectStrategy::DeepLink.redirect_uri(),
            "app.neko.finance://oauth-callback"
        );
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
