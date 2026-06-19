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
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
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
