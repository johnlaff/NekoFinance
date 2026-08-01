//! A borda de rede do provedor: a única peça do módulo que abre conexão.
//!
//! Tudo o que é decidível sem rede mora nos vizinhos — a montagem do pedido em [`request`], a
//! tradução do stream em [`stream`], a política de saída em [`egress`]. O que sobra aqui é o
//! irredutível: enviar, ler bytes e entregar a leitura ao parser. Por isso a bomba de bytes é
//! genérica sobre uma fonte, e a fonte de verdade é só um invólucro do transporte — a suíte
//! roteiriza a fonte e exercita o caminho inteiro sem subir servidor.
//!
//! A credencial entra pelo construtor, vira cabeçalho e não existe em mais lugar nenhum: o tipo
//! não deriva Debug nem Serialize de propósito, para que nenhum formato a carregue por acidente.

use super::drift::{EndpointsCatalog, ZdrCatalog};
use super::egress;
use super::request::RunSpec;
use super::stream::{ErrorKind, ProviderError, ProviderEvent, StreamParser};
use crate::mia::run::{CancelToken, ProviderAdapter};
use serde_json::Value;
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc;

/// O catálogo de endpoints de retenção zero do provedor. É o que o canary consulta antes de
/// confiar num pin: cada presença nesta lista é, por definição, um endpoint de retenção zero.
const ZDR_CATALOG_URL: &str = "https://openrouter.ai/api/v1/endpoints/zdr";

/// Quanto do catálogo entra na memória. Folgado para a lista inteira do provedor e finito de
/// propósito: um corpo sem fim viraria memória sem fim.
const CATALOG_BODY_LIMIT: usize = 8 * 1024 * 1024;

/// Espera máxima pela abertura da conexão. Curta de propósito: falha de conexão é pré-resposta,
/// o caminho mais barato de retentar.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Espera máxima entre um byte e o próximo do stream. Um stream vivo manda evento com folga
/// dentro disso; um stream pendurado sem este teto seguraria a rodada até o teto de tempo dela.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Quanto do corpo de um erro HTTP entra na mensagem. O corpo é dado não confiável e serve ao
/// rastro técnico redigido — um trecho diagnostica; o corpo inteiro seria transporte de lixo.
const ERROR_BODY_EXCERPT: usize = 200;

/// Eventos em trânsito entre a bomba e o laço. O laço drena contínuo; a folga só absorve rajada.
const EVENT_CHANNEL_CAPACITY: usize = 64;

pub(crate) struct HttpAdapter {
    client: reqwest::Client,
    api_key: String,
}

impl HttpAdapter {
    /// Monta o cliente endurecido. Falha de construção é recusa, nunca queda para um cliente
    /// default: o default segue redirecionamento, e seguir redirecionamento entrega ao outro
    /// lado a escolha de quem recebe a rodada.
    pub(crate) fn new(api_key: String) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            // Proxy de ambiente é outro destino no fio: `HTTPS_PROXY` reencaminharia corpo e
            // credencial por um host que a allowlist nunca examinou. A conversa fala direto ou
            // não fala — ao contrário do cliente compartilhado do app, que honra proxy de
            // propósito para as rotas do Google.
            .no_proxy()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|error| {
                format!("O cliente HTTP da conversa não pôde ser montado: {error}.")
            })?;
        Ok(Self { client, api_key })
    }
}

impl ProviderAdapter for HttpAdapter {
    fn open(
        &self,
        spec: &RunSpec<'_>,
        cancel: &CancelToken,
    ) -> impl Future<Output = Result<mpsc::Receiver<ProviderEvent>, ProviderError>> + Send {
        let prepared = super::request::build(spec);
        let cancel = cancel.clone();
        let mut request = self.client.post(prepared.url).json(&prepared.body);
        for (name, value) in &prepared.headers {
            request = request.header(name, value);
        }
        request = request.header("authorization", format!("Bearer {}", self.api_key));

        async move {
            // Defesa em profundidade: a URL é a constante pinada da montagem, mas quem afirma a
            // política de saída é a política — não o hábito de ninguém mudar a constante.
            egress::check(prepared.url).map_err(|denied| ProviderError {
                kind: ErrorKind::Permanent,
                message: format!("A saída para o provedor foi recusada: {denied:?}."),
                // O pedido nem saiu — mas este caminho só dispara com invariante do programa
                // violada, e aí o lado fechado é o único barato.
                responded: false,
            })?;

            let response = request.send().await.map_err(|error| ProviderError {
                // Falha de envio acontece antes de qualquer evento: retentável por definição.
                kind: ErrorKind::Transient,
                message: format!("O pedido ao provedor não pôde ser enviado: {error}."),
                // Sem resposta não se sabe o estágio que o servidor alcançou: o pedido pode ter
                // chegado e gerado, com o custo no stream que nunca abriu.
                responded: false,
            })?;

            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let retry_after = retry_after_secs(&response);
                let body = error_excerpt(ResponseSource(response)).await;
                return Err(refusal(status, retry_after, &body));
            }

            let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
            tokio::spawn(pump(ResponseSource(response), sender, cancel));
            Ok(receiver)
        }
    }
}

impl ZdrCatalog for HttpAdapter {
    /// Busca o catálogo pelo mesmo cliente endurecido e a mesma credencial da rodada. Falha aqui
    /// nunca é "pin verificado": quem chama trata a ausência de catálogo como recusa.
    fn fetch(&self) -> impl Future<Output = Result<Value, String>> + Send {
        let request = self
            .client
            .get(ZDR_CATALOG_URL)
            .header("authorization", format!("Bearer {}", self.api_key));

        async move {
            egress::check(ZDR_CATALOG_URL)
                .map_err(|denied| format!("A saída para o catálogo foi recusada: {denied:?}."))?;

            let response = request.send().await.map_err(|error| {
                format!("O catálogo do provedor não pôde ser buscado: {error}.")
            })?;

            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                // O corpo é do outro lado e esta mensagem termina no terminal de quem rodou: um
                // erro que ecoasse o cabeçalho de autorização publicaria a chave. O status
                // diagnostica a recusa; o corpo não acrescenta nada que valha esse risco.
                return Err(format!(
                    "O provedor recusou o catálogo de retenção zero (HTTP {status})."
                ));
            }

            let body = read_body(ResponseSource(response), CATALOG_BODY_LIMIT).await?;
            serde_json::from_slice(&body)
                .map_err(|error| format!("O catálogo do provedor não parseia como JSON: {error}."))
        }
    }
}

/// O catálogo geral de endpoints de UM modelo — a fonte que prova um pin em opt-out deliberado
/// ([`super::pins::Retention::ProviderPolicy`]), ao contrário do catálogo de retenção zero, que é
/// global. O provedor não publica isto por modelo escaneado de antemão; é um pedido por modelo.
const MODEL_ENDPOINTS_URL: &str = "https://openrouter.ai/api/v1/models";

impl EndpointsCatalog for HttpAdapter {
    /// Busca e ACHATA a resposta para a mesma forma que [`super::drift::verify`] já lê do
    /// catálogo de retenção zero — `{"data": [{tag, model_id, supported_parameters}, ...]}`. A
    /// resposta por modelo aninha os endpoints sob `data.endpoints` e não repete o `model_id` em
    /// cada um; achatar aqui, na borda, evita que a verificação de drift precise conhecer uma
    /// segunda forma de catálogo.
    fn fetch(&self, model: &str) -> impl Future<Output = Result<Value, String>> + Send {
        let url = format!("{MODEL_ENDPOINTS_URL}/{model}/endpoints");
        let request = self
            .client
            .get(&url)
            .header("authorization", format!("Bearer {}", self.api_key));
        let model = model.to_string();

        async move {
            egress::check(&url).map_err(|denied| {
                format!("A saída para o catálogo de endpoints foi recusada: {denied:?}.")
            })?;

            let response = request.send().await.map_err(|error| {
                format!("O catálogo de endpoints de {model} não pôde ser buscado: {error}.")
            })?;

            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                // O corpo é do outro lado, como na busca do catálogo de retenção zero: o status
                // diagnostica a recusa sem arriscar publicar o cabeçalho de autorização.
                return Err(format!(
                    "O provedor recusou o catálogo de endpoints de {model} (HTTP {status})."
                ));
            }

            let body = read_body(ResponseSource(response), CATALOG_BODY_LIMIT).await?;
            let raw: Value = serde_json::from_slice(&body).map_err(|error| {
                format!("O catálogo de endpoints de {model} não parseia como JSON: {error}.")
            })?;
            flatten_model_endpoints(&raw, &model)
        }
    }
}

/// Achata `{"data": {"endpoints": [...]}}` em `{"data": [{tag, model_id, supported_parameters,
/// ...}, ...]}`, injetando o `model_id` que a resposta por modelo omite em cada entrada.
fn flatten_model_endpoints(raw: &Value, model: &str) -> Result<Value, String> {
    let endpoints = raw
        .pointer("/data/endpoints")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!("O catálogo de endpoints de {model} não contém data.endpoints em lista.")
        })?;
    let flattened: Vec<Value> = endpoints
        .iter()
        .map(|endpoint| {
            let mut entry = endpoint.clone();
            if let Value::Object(map) = &mut entry {
                map.insert("model_id".to_string(), Value::String(model.to_string()));
            }
            entry
        })
        .collect();
    Ok(serde_json::json!({ "data": flattened }))
}

/// Materializa um corpo inteiro até o teto. Estourar o teto é erro, não truncamento: um catálogo
/// cortado no meio parseia como JSON inválido ou, pior, como catálogo menor do que é.
pub(crate) async fn read_body<S: ByteSource>(
    mut source: S,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut collected: Vec<u8> = Vec::new();
    while let Some(bytes) = source.next_chunk().await? {
        // A conferência vem ANTES da cópia: medir depois de anexar deixaria um pedaço único e
        // enorme entrar inteiro na memória antes de ser recusado, e o teto não seria teto.
        if bytes.len() > limit - collected.len() {
            return Err(format!(
                "A resposta do provedor passou de {limit} bytes e foi descartada."
            ));
        }
        collected.extend_from_slice(&bytes);
    }
    Ok(collected)
}

/// Lê no máximo o trecho diagnosticável do corpo de um erro e SOLTA o resto: materializar o
/// corpo inteiro entregaria a um provedor defeituoso o poder de encher a memória da rodada com
/// uma resposta de erro sem fim.
async fn error_excerpt<S: ByteSource>(mut source: S) -> String {
    // Quatro vezes o trecho em bytes cobre o pior caso de caractere multibyte antes do corte.
    let ceiling = ERROR_BODY_EXCERPT * 4;
    let mut collected: Vec<u8> = Vec::new();
    while collected.len() < ceiling {
        match source.next_chunk().await {
            // Só o que falta para o trecho é copiado: anexar o pedaço inteiro e conferir depois
            // deixaria um corpo de erro gigante entrar na memória antes de ser descartado.
            Ok(Some(bytes)) => {
                let room = ceiling - collected.len();
                collected.extend_from_slice(&bytes[..bytes.len().min(room)]);
            }
            Ok(None) | Err(_) => break,
        }
    }
    String::from_utf8_lossy(&collected)
        .chars()
        .take(ERROR_BODY_EXCERPT)
        .collect()
}

/// A recusa HTTP traduzida para a taxonomia de retentativa. Pura para a suíte exercitar cada
/// classe sem servidor.
fn refusal(status: u16, retry_after_secs: Option<u64>, body: &str) -> ProviderError {
    let excerpt: String = body.chars().take(ERROR_BODY_EXCERPT).collect();
    let kind = match status {
        429 => ErrorKind::RateLimited { retry_after_secs },
        408 | 500 | 502 | 503 | 504 => ErrorKind::Transient,
        // Redirecionamento é recusado por princípio, qualquer que seja o destino: quem
        // redireciona é o outro lado, e a política de saída não delega o destino da rodada.
        300..=399 => ErrorKind::Permanent,
        _ => ErrorKind::Permanent,
    };
    let message = if (300..=399).contains(&status) {
        format!(
            "O provedor respondeu com redirecionamento (HTTP {status}), que a rodada nunca segue."
        )
    } else {
        format!("O provedor recusou a rodada (HTTP {status}): {excerpt}")
    };
    ProviderError {
        kind,
        message,
        // O status chegou: recusa comprovada, corpo de erro em vez de stream, nada gerado.
        responded: true,
    }
}

fn retry_after_secs(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        // Só a forma em segundos: a forma em data exigiria relógio, e a espera default do laço
        // cobre a lacuna melhor do que uma data mal interpretada.
        .and_then(|value| value.trim().parse().ok())
}

/// Uma fonte de bytes do stream. É o corte que deixa a bomba exercitável sem rede: a implementação
/// de verdade envolve a resposta HTTP, e a da suíte devolve um roteiro.
///
/// Devolver `None` é o fim normal do transporte; erro é texto porque a bomba não decide
/// retentativa — quem classifica é o parser e o laço, sobre o que a bomba publicar.
pub(crate) trait ByteSource: Send + 'static {
    fn next_chunk(&mut self) -> impl Future<Output = Result<Option<Vec<u8>>, String>> + Send;
}

struct ResponseSource(reqwest::Response);

impl ByteSource for ResponseSource {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.0
            .chunk()
            .await
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
            .map_err(|error| format!("O transporte falhou no meio do stream: {error}."))
    }
}

/// Remonta linhas a partir de pedaços que o transporte corta onde quer — inclusive no meio de um
/// caractere multibyte, razão de o buffer ser de bytes e a conversão acontecer por linha fechada.
#[derive(Default)]
pub(crate) struct LineAssembler {
    buffer: Vec<u8>,
}

impl LineAssembler {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(position) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let raw: Vec<u8> = self.buffer.drain(..=position).collect();
            lines.push(Self::line(&raw));
        }
        lines
    }

    /// O que sobrou sem quebra de linha quando o transporte fechou — um stream pode terminar sem
    /// a última quebra, e o resto ainda é uma linha.
    pub(crate) fn rest(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }
        let raw = std::mem::take(&mut self.buffer);
        Some(Self::line(&raw))
    }

    fn line(raw: &[u8]) -> String {
        String::from_utf8_lossy(raw)
            .trim_end_matches(['\n', '\r'])
            .to_string()
    }
}

/// A bomba: bytes da fonte → linhas → eventos do domínio → canal do laço.
///
/// O cancelamento fecha a conexão de verdade: sair daqui solta a fonte, e soltar a fonte derruba
/// o transporte — parar de ler sem fechar deixaria o provedor gerando, e cobrando, uma resposta
/// que ninguém vai ler. Receptor solto encerra igual, pelo mesmo motivo.
pub(crate) async fn pump<S: ByteSource>(
    mut source: S,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancelToken,
) {
    let mut parser = StreamParser::new();
    let mut lines = LineAssembler::default();

    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => return,
            // Receptor solto encerra mesmo sem byte novo: um stream que só manda comentário
            // SSE nunca produziria um `send` para revelar o abandono, e a conexão ficaria
            // aberta — gerando e cobrando — até alguém desligar o app.
            _ = events.closed() => return,
            chunk = source.next_chunk() => chunk,
        };

        match chunk {
            Ok(Some(bytes)) => {
                for line in lines.push(&bytes) {
                    for event in parser.push(&line) {
                        if events.send(event).await.is_err() {
                            return;
                        }
                    }
                }
            }
            Ok(None) => {
                if let Some(line) = lines.rest() {
                    for event in parser.push(&line) {
                        if events.send(event).await.is_err() {
                            return;
                        }
                    }
                }
                for event in parser.finish() {
                    let _ = events.send(event).await;
                }
                return;
            }
            Err(message) => {
                let _ = events
                    .send(ProviderEvent::Failed(ProviderError {
                        kind: ErrorKind::Transient,
                        message,
                        // A bomba só corre sobre um stream aberto: a resposta já veio.
                        responded: true,
                    }))
                    .await;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mia::provider::stream::{FinishReason, Usage};
    use std::collections::VecDeque;

    struct Scripted(VecDeque<Result<Option<Vec<u8>>, String>>);

    impl Scripted {
        fn chunks(payload: &str, size: usize) -> Self {
            let mut chunks: VecDeque<_> = payload
                .as_bytes()
                .chunks(size)
                .map(|chunk| Ok(Some(chunk.to_vec())))
                .collect();
            chunks.push_back(Ok(None));
            Self(chunks)
        }
    }

    impl ByteSource for Scripted {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
            self.0.pop_front().unwrap_or(Ok(None))
        }
    }

    /// Uma fonte que nunca entrega byte — o mundo em que só o cancelamento encerra a bomba.
    struct Pending;

    impl ByteSource for Pending {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
            std::future::pending().await
        }
    }

    async fn pumped(source: impl ByteSource) -> Vec<ProviderEvent> {
        let (sender, mut receiver) = mpsc::channel(16);
        let handle = tokio::spawn(pump(source, sender, CancelToken::new()));
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        handle.await.unwrap();
        events
    }

    #[test]
    fn o_remontador_fecha_linhas_atravessando_pedacos_e_crlf() {
        let mut assembler = LineAssembler::default();

        // O corte cai no meio do "á" (dois bytes em UTF-8): a linha fechada sai íntegra.
        let payload = "data: Olá\r\ndata: fim\n".as_bytes();
        let cut = payload.iter().position(|b| *b == 0xC3).unwrap() + 1;

        assert!(assembler.push(&payload[..cut]).is_empty());
        let lines = assembler.push(&payload[cut..]);
        assert_eq!(lines, vec!["data: Olá", "data: fim"]);
        assert_eq!(assembler.rest(), None);

        assembler.push(b"resto sem quebra");
        assert_eq!(assembler.rest(), Some("resto sem quebra".to_string()));
    }

    #[tokio::test]
    async fn a_bomba_traduz_uma_sessao_sse_em_eventos_do_dominio() {
        let payload = concat!(
            ": comentario ignorado\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Olá\"},\"index\":0}]}\n",
            "\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"cost\":0.0001}}\n",
            "data: [DONE]\n",
        );

        // Pedaços de sete bytes: nenhum corte respeita fronteira de linha nem de caractere.
        let events = pumped(Scripted::chunks(payload, 7)).await;

        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], ProviderEvent::TextDelta(text) if text == "Olá"));
        assert!(matches!(
            &events[1],
            ProviderEvent::Finished {
                reason: FinishReason::Stop,
                ..
            }
        ));
        assert!(matches!(
            &events[2],
            ProviderEvent::Usage(Usage {
                prompt_tokens: 10,
                completion_tokens: 2,
                cost_micro_usd: Some(100),
            })
        ));
    }

    #[tokio::test]
    async fn o_cancelamento_encerra_a_bomba_e_fecha_o_canal() {
        let (sender, mut receiver) = mpsc::channel(16);
        let cancel = CancelToken::new();
        let handle = tokio::spawn(pump(Pending, sender, cancel.clone()));

        cancel.cancel();
        handle.await.unwrap();
        assert!(receiver.recv().await.is_none());
    }

    /// Receptor solto encerra a bomba mesmo com a fonte muda: sem esta saída, um stream que só
    /// manda comentário SSE manteria a conexão — e a cobrança — viva para sempre.
    #[tokio::test]
    async fn receptor_solto_encerra_a_bomba() {
        let (sender, receiver) = mpsc::channel(16);
        let handle = tokio::spawn(pump(Pending, sender, CancelToken::new()));

        drop(receiver);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn erro_do_transporte_no_meio_vira_falha_retentavel() {
        let mut chunks: VecDeque<Result<Option<Vec<u8>>, String>> = VecDeque::new();
        chunks.push_back(Ok(Some(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"parcial\"},\"index\":0}]}\n".to_vec(),
        )));
        chunks.push_back(Err("a conexão caiu.".to_string()));

        let events = pumped(Scripted(chunks)).await;

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[1],
            ProviderEvent::Failed(ProviderError {
                kind: ErrorKind::Transient,
                message,
                // A bomba corre pós-resposta: a falha dela nunca vira "não sei se cobrou" na
                // fronteira de abertura.
                responded: true,
            }) if message.contains("a conexão caiu")
        ));
    }

    /// Transporte que fecha sem `[DONE]` nem motivo final: a rodada fica incompleta e
    /// retentável — é o parser quem o diz, e a bomba só precisa dar a ele a chance de dizer.
    #[tokio::test]
    async fn fim_sem_motivo_final_vira_falha_retentavel() {
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"meia\"},\"index\":0}]}\n";

        let events = pumped(Scripted::chunks(payload, 64)).await;

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[1],
            ProviderEvent::Failed(ProviderError {
                kind: ErrorKind::Transient,
                ..
            })
        ));
    }

    #[test]
    fn a_recusa_http_cai_na_taxonomia_certa() {
        assert!(matches!(
            refusal(429, Some(7), "").kind,
            ErrorKind::RateLimited {
                retry_after_secs: Some(7)
            }
        ));
        assert!(matches!(refusal(500, None, "").kind, ErrorKind::Transient));
        assert!(matches!(refusal(401, None, "").kind, ErrorKind::Permanent));
        // Toda recusa com status é resposta do servidor: comprovadamente nada foi gerado, e é
        // isso que autoriza a bancada a seguir sem fechar a trava.
        assert!(refusal(500, None, "").responded);
        assert!(refusal(404, None, "").responded);

        let redirect = refusal(302, None, "");
        assert!(matches!(redirect.kind, ErrorKind::Permanent));
        assert!(redirect.message.contains("redirecionamento"));
    }

    /// O catálogo chega inteiro, remontado de pedaços que o transporte cortou onde quis.
    #[tokio::test]
    async fn o_corpo_do_catalogo_e_remontado_ate_o_teto() {
        let payload = "{\"data\": []}";

        let body = read_body(Scripted::chunks(payload, 3), 1_024)
            .await
            .unwrap();

        assert_eq!(String::from_utf8(body).unwrap(), payload);
    }

    /// Corpo acima do teto é descartado, nunca truncado: um catálogo cortado no meio parseia
    /// como catálogo menor do que é, e um pin sumido do catálogo tira um modelo da corrida.
    #[tokio::test]
    async fn corpo_acima_do_teto_e_recusado_em_vez_de_cortado() {
        let payload = "x".repeat(2_048);

        let error = read_body(Scripted::chunks(&payload, 512), 1_024)
            .await
            .unwrap_err();

        assert!(error.contains("1024"));
    }

    /// O corpo de um erro entra na mensagem só como trecho: diagnóstico sem transporte de lixo.
    #[test]
    fn o_corpo_do_erro_entra_truncado() {
        let body = "x".repeat(1_000);
        let error = refusal(400, None, &body);
        assert!(error.message.len() < 400);
    }
}
