use oauth2::{
    AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenUrl,
};
use rand::RngExt;

pub fn generate_code_verifier() -> PkceCodeVerifier {
    PkceCodeVerifier::new(generate_random_string(64))
}

pub fn compute_challenge(verifier: &PkceCodeVerifier) -> PkceCodeChallenge {
    PkceCodeChallenge::from_code_verifier_sha256(verifier)
}

fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub struct OAuthConfig {
    pub client_id: String,
    /// Client secret de app "Desktop" do Google — exigido no token exchange mesmo com PKCE.
    /// Não é confidencial nesse tipo de client (vive embutido em qualquer app desktop);
    /// fica no .env local gitignored, nunca no repo.
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
}

impl OAuthConfig {
    pub fn google(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret: client_secret.filter(|s| !s.trim().is_empty()),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct OAuthState {
    pub verifier_secret: String,
    pub csrf_token: CsrfToken,
    pub redirect_port: u16,
}

impl OAuthState {
    pub fn new(redirect_port: u16) -> Self {
        let verifier = generate_code_verifier();
        let csrf_token = CsrfToken::new(generate_random_string(32));
        Self {
            verifier_secret: verifier.secret().to_string(),
            csrf_token,
            redirect_port,
        }
    }

    pub fn verifier(&self) -> PkceCodeVerifier {
        PkceCodeVerifier::new(self.verifier_secret.clone())
    }

    pub fn build_auth_url(&self, config: &OAuthConfig) -> String {
        let client_id = ClientId::new(config.client_id.clone());
        let auth_url = AuthUrl::new(config.auth_url.clone()).expect("invalid auth URL");
        let token_url = TokenUrl::new(config.token_url.clone()).expect("invalid token URL");
        let redirect_uri = RedirectUrl::new(format!("http://127.0.0.1:{}", self.redirect_port))
            .expect("invalid redirect URL");

        let (auth_url, _csrf_token) = oauth2::basic::BasicClient::new(client_id)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_uri)
            .authorize_url(|| self.csrf_token.clone())
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/spreadsheets.readonly".to_string(),
            ))
            // Listagem de planilhas (list_user_spreadsheets) usa o Drive v3 — sem este
            // scope o picker devolve 403 (spec 010, slice 2).
            // Nota: access_type=offline/prompt=consent NÃO se aplicam ao fluxo de app
            // instalado — "refresh tokens are always returned for installed applications"
            // (doc oficial native-app). Se o dogfooding mostrar refresh_token vazio,
            // a contingência registrada na spec 010 é adicioná-los aqui.
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/drive.metadata.readonly".to_string(),
            ))
            .set_pkce_challenge(compute_challenge(&self.verifier()))
            .url();

        auth_url.to_string()
    }
}

#[allow(dead_code)]
pub fn exchange_code(
    _config: &OAuthConfig,
    _state: &OAuthState,
    code: String,
) -> Result<(String, String), String> {
    // Token exchange requires HTTP client — placeholder for now
    Err(format!(
        "Code received (exchange not yet implemented): {code}"
    ))
}

#[allow(dead_code)]
pub fn is_valid_code_verifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() >= 43
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verifier_length() {
        let verifier = generate_code_verifier();
        assert!(is_valid_code_verifier(verifier.secret()));
    }

    #[test]
    fn test_challenge_is_not_empty() {
        let verifier = generate_code_verifier();
        let challenge = compute_challenge(&verifier);
        let challenge_str = challenge.as_str();
        assert!(!challenge_str.is_empty());
    }

    #[test]
    fn test_verifier_uniqueness() {
        let a = generate_code_verifier();
        let b = generate_code_verifier();
        assert_ne!(a.secret(), b.secret());
    }

    #[test]
    fn test_build_auth_url_contains_params() {
        let config = OAuthConfig::google("test-client-id.apps.googleusercontent.com".into(), None);
        let state = OAuthState::new(48080);
        let url = state.build_auth_url(&config);
        eprintln!("AUTH URL: {url}");
        assert!(url.contains("test-client-id"));
        assert!(url.contains("127.0.0.1"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("spreadsheets.readonly"));
        // Spec 010 slice 2: scope do Drive para o picker de planilhas.
        assert!(url.contains("drive.metadata.readonly"));
    }

    #[test]
    fn test_config_normalizes_empty_secret_to_none() {
        let with = OAuthConfig::google("id".into(), Some("s3cret".into()));
        assert_eq!(with.client_secret.as_deref(), Some("s3cret"));
        let blank = OAuthConfig::google("id".into(), Some("  ".into()));
        assert!(blank.client_secret.is_none());
        let none = OAuthConfig::google("id".into(), None);
        assert!(none.client_secret.is_none());
    }
}
