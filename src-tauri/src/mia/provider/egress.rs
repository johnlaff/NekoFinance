//! A porta de saída: para onde a rodada pode falar, e o que ela nunca segue.
//!
//! A conversa lê tudo o que a pessoa tem. Um destino inesperado não é indisponibilidade: é
//! exfiltração. Por isso a allowlist é de host EXATO e o redirecionamento é recusado sempre —
//! inclusive para um host permitido, porque quem redireciona é o outro lado, e aceitar o desvio
//! entrega ao provedor a decisão de para onde os dados vão.
//!
//! A política mora aqui, separada de quem a aplica: quem abre conexão é a borda de rede, e é ela
//! que consulta [`check`] antes de enviar e [`on_redirect`] diante de um 3xx. A separação é o que
//! torna a decisão exercitável sem subir servidor — o laço da conversa fala com o provedor por um
//! trait, e nenhum caminho dele chega à rede.

use reqwest::Url;

/// Os únicos hosts com que a rodada fala. Comparação exata: subdomínio não herda permissão.
pub(crate) const ALLOWED_HOSTS: &[&str] = &["openrouter.ai"];

/// Por que a saída foi recusada. O motivo é próprio de cada caso para que a recusa seja
/// diagnosticável sem repetir a URL inteira em log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EgressDenied {
    /// A URL não pôde ser lida como URL absoluta.
    Malformed,
    /// Esquema diferente de `https`: texto claro no fio nunca é destino válido.
    InsecureScheme,
    /// Host fora da allowlist.
    HostNotAllowed { host: String },
    /// Porta explícita fora do canal HTTPS padrão.
    PortNotAllowed { port: u16 },
    /// Credencial embutida na autoridade — a forma clássica de fazer um host permitido parecer
    /// o destino quando o destino é outro.
    CredentialsInUrl,
    /// Redirecionamento: recusado por princípio, qualquer que seja o destino. O cliente da rodada
    /// já nasce sem seguir redirecionamento, então esta recusa é a política escrita — o que a
    /// suíte exercita para que a decisão não dependa só da configuração do cliente.
    #[allow(dead_code)]
    RedirectRefused,
}

/// A URL pode ser chamada?
pub(crate) fn check(url: &str) -> Result<(), EgressDenied> {
    let parsed = Url::parse(url).map_err(|_| EgressDenied::Malformed)?;

    if parsed.scheme() != "https" {
        return Err(EgressDenied::InsecureScheme);
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(EgressDenied::CredentialsInUrl);
    }

    let host = parsed
        .host_str()
        .ok_or(EgressDenied::Malformed)?
        .to_lowercase();
    if !ALLOWED_HOSTS.contains(&host.as_str()) {
        return Err(EgressDenied::HostNotAllowed { host });
    }

    if let Some(port) = parsed.port()
        && port != 443
    {
        return Err(EgressDenied::PortNotAllowed { port });
    }

    Ok(())
}

/// A decisão diante de um redirecionamento. Existe como função — em vez de uma política opaca do
/// cliente HTTP — para que a recusa seja exercitável em teste sem subir rede.
#[allow(dead_code)]
pub(crate) fn on_redirect(_location: &str) -> EgressDenied {
    // O destino não atenua o desvio: seguir qualquer redirecionamento delegaria ao provedor a
    // escolha do receptor do conteúdo da rodada.
    EgressDenied::RedirectRefused
}
