//! Cliente HTTP compartilhado para as chamadas ao Google (OAuth + Sheets/Drive).
//!
//! Antes cada chamada criava um `reqwest::Client::new()` SEM timeout e SEM retentativa. Numa rede
//! doméstica (DNS lento, Wi-Fi acordando do sono, soluço momentâneo) o `send()` falhava com
//! `error sending request` — e, pior, isso derrubava o REFRESH do token: o usuário precisava
//! reconectar o Google toda hora. Aqui centralizamos timeouts sãos + retentativa em falhas
//! TRANSITÓRIAS de transporte, sem mudar a semântica das respostas (status 4xx/5xx seguem como
//! resposta normal, não como erro de transporte).

use std::sync::OnceLock;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ATTEMPTS: u32 = 3;
/// Teto da espera honrando `Retry-After`: um servidor pode pedir minutos; não penduramos o usuário.
const MAX_RETRY_WAIT: Duration = Duration::from_secs(10);

/// Cliente compartilhado com timeouts. `reqwest` já honra `HTTP(S)_PROXY`/`NO_PROXY` do ambiente.
/// Reusado via `OnceLock` (um único pool de conexões/keep-alive para toda a sessão) — antes cada
/// chamada reconstruía o cliente e refazia o handshake TLS (até ~24 por import anual).
pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(build_client).clone()
}

fn build_client() -> reqwest::Client {
    android_client_builder(
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT),
    )
    .build()
    .unwrap_or_else(on_build_failure)
}

/// Fora do Android, degradar para o cliente default do `reqwest` é uma perda aceitável — mesmo
/// verificador de plataforma nativo do SO, só sem os timeouts desta sessão.
#[cfg(not(target_os = "android"))]
fn on_build_failure(_error: reqwest::Error) -> reqwest::Client {
    reqwest::Client::new()
}

/// No Android, essa degradação é exatamente o bug desta issue: `reqwest::Client::new()` volta a
/// usar a config default, cujo verificador de plataforma pânica sem a inicialização JNI que o
/// app não faz (`rustls-platform-verifier`, ver [`android_client_builder`]) — e a `OnceLock` de
/// [`client`] congelaria essa escolha ruim pela sessão inteira, disfarçando o pânico real (na
/// primeira request) atrás de um cliente que parece funcional. Falha de `.build()` aqui só pode
/// vir de um bug de empacotamento (ex.: [`embedded_root_certs`] vazio) — pânica alto e cedo, na
/// construção, em vez de mascarar.
#[cfg(target_os = "android")]
fn on_build_failure(error: reqwest::Error) -> reqwest::Client {
    panic!("cliente HTTP do Android sem raízes embutidas — bug de build: {error}")
}

/// Raízes de CA da Mozilla embutidas no binário (`webpki-root-certs`), convertidas para o tipo
/// que o `reqwest` aceita em [`reqwest::ClientBuilder::tls_certs_only`]. Parsing puro, sem
/// nenhuma dependência de plataforma — testável em qualquer host, mesmo que só
/// [`android_client_builder`] o use em produção.
#[cfg(any(target_os = "android", test))]
fn embedded_root_certs() -> &'static [reqwest::Certificate] {
    static ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der.as_ref()).ok())
            .collect()
    })
}

/// No Android, a verificação de certificado do `rustls` (dependência transitiva do `reqwest`)
/// delega ao verificador de plataforma via JNI (`rustls-platform-verifier`): sem uma
/// inicialização que o app não faz, a primeira chamada pânica o worker tokio e todo caminho de
/// rede do núcleo (OAuth, Sheets, snapshot no Drive, Mia em nuvem) fica inoperante. Em vez de
/// acoplar o núcleo à ponte JNI e ao componente Gradle/Kotlin que o verificador de plataforma
/// exige, trocamos a confiança no sistema pela lista de raízes da Mozilla embutida no binário —
/// sem JNI, cobre os hosts do Google e do OpenRouter que o núcleo fala. Fora do Android é
/// passagem direta: o verificador de plataforma nativo (Keychain/Credential Manager/libsecret)
/// segue como está, com a vantagem de honrar CA corporativa/de usuário que o desktop já tem.
///
/// Três trocas conscientes dessa rota, válidas **só no Android** (verificado 2026-08 — revalidar
/// a cada bump do `webpki-root-certs`, ou antes de publicar o app numa loja):
/// 1. As raízes ficam presas na versão embutida em tempo de compilação — não seguem a lista da
///    Mozilla até o próximo rebuild/release, ao contrário do verificador de plataforma (que lê o
///    trust store do SO, atualizado pelo sistema).
/// 2. Sem dado de revogação: `tls_certs_only` faz verificação de cadeia pura (webpki), sem CRL/
///    OCSP — o verificador de plataforma consultaria o SO para isso.
/// 3. CA corporativa ou instalada manualmente pelo usuário no aparelho deixa de valer para o
///    núcleo (o app não passa mais pelo trust store do Android). Sideload-only (ADR-0014) torna
///    isso aceitável hoje; reconsiderar a rota (a) — inicialização JNI do verificador de
///    plataforma — se o app algum dia precisar honrar esse tipo de CA no Android.
#[cfg(target_os = "android")]
pub(crate) fn android_client_builder(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.tls_certs_only(embedded_root_certs().iter().cloned())
}

#[cfg(not(target_os = "android"))]
pub(crate) fn android_client_builder(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder
}

/// `true` para erros de transporte que valem retentativa (conexão/timeout/envio). Status HTTP NÃO
/// chega aqui (reqwest só vira `Err` em falha de transporte/decodificação).
fn is_transient(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

/// Espera pedida pelo servidor no header `Retry-After` (em segundos), limitada a [`MAX_RETRY_WAIT`].
fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let secs = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_secs(secs).min(MAX_RETRY_WAIT))
}

/// Envia a requisição com até `MAX_ATTEMPTS` tentativas, com backoff curto. Re-tenta tanto falhas
/// transitórias de transporte (conexão/timeout) quanto throttling do servidor (`429`/`503`) — o
/// import multi-ano do método dispara dezenas de chamadas e estoura a cota de 60/min do Sheets.
/// O corpo das nossas chamadas (form/json/GET) é clonável, então `try_clone` devolve `Some`.
pub async fn send_with_retry(
    req: reqwest::RequestBuilder,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt: u32 = 1;
    loop {
        let Some(this) = req.try_clone() else {
            // Corpo não-clonável (stream): tentativa única.
            return req.send().await;
        };
        match this.send().await {
            Ok(resp) => {
                // Throttling (429) ou indisponibilidade transitória (503): respeita `Retry-After`
                // quando presente, senão backoff. 4xx/5xx restantes seguem como resposta normal.
                let status = resp.status().as_u16();
                if (status == 429 || status == 503) && attempt < MAX_ATTEMPTS {
                    let wait =
                        retry_after(&resp).unwrap_or(Duration::from_millis(400 * attempt as u64));
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                if !is_transient(&e) || attempt >= MAX_ATTEMPTS {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod android_tls_tests {
    use super::embedded_root_certs;

    /// A decisão de fiação (`android_client_builder`) só se prova no aparelho — mas o parsing das
    /// raízes embutidas é puro e testável aqui. Igualdade EXATA com o tamanho da lista fonte, não
    /// só "não vazia": `embedded_root_certs` descarta em silêncio (`filter_map(...ok())`) qualquer
    /// DER que o `reqwest` recuse — sem essa contagem exata, um bump do `webpki-root-certs` que
    /// passasse a exportar uma raiz num formato que o `reqwest` não decodifica reduziria a lista
    /// de confiança sem quebrar teste nenhum.
    #[test]
    fn as_raizes_embutidas_da_mozilla_analisam_sem_perder_nenhuma() {
        let certs = embedded_root_certs();
        assert_eq!(
            certs.len(),
            webpki_root_certs::TLS_SERVER_ROOT_CERTS.len(),
            "uma ou mais raízes da lista fonte não decodificaram como reqwest::Certificate"
        );
    }
}
