//! Cliente HTTP compartilhado para as chamadas ao Google (OAuth + Sheets/Drive).
//!
//! Antes cada chamada criava um `reqwest::Client::new()` SEM timeout e SEM retentativa. Numa rede
//! doméstica (DNS lento, Wi-Fi acordando do sono, soluço momentâneo) o `send()` falhava com
//! `error sending request` — e, pior, isso derrubava o REFRESH do token: o usuário precisava
//! reconectar o Google toda hora. Aqui centralizamos timeouts sãos + retentativa em falhas
//! TRANSITÓRIAS de transporte, sem mudar a semântica das respostas (status 4xx/5xx seguem como
//! resposta normal, não como erro de transporte).

use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ATTEMPTS: u32 = 3;

/// Cliente compartilhado com timeouts. `reqwest` já honra `HTTP(S)_PROXY`/`NO_PROXY` do ambiente.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// `true` para erros de transporte que valem retentativa (conexão/timeout/envio). Status HTTP NÃO
/// chega aqui (reqwest só vira `Err` em falha de transporte/decodificação).
fn is_transient(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

/// Envia a requisição com até `MAX_ATTEMPTS` tentativas, com backoff curto entre falhas transitórias.
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
            Ok(resp) => return Ok(resp),
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
