use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

pub struct OAuthServer {
    port: u16,
}

impl OAuthServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Devolve `(code, state)` do callback de loopback. O `state` (CSRF) é validado pelo chamador
    /// contra o csrf_token gerado — sem isso, um redirect forjado injetaria um code de terceiro.
    pub async fn listen_for_code(self) -> Result<(String, Option<String>), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).map_err(|e| format!("bind error: {e}"))?;
        listener
            .set_nonblocking(false)
            .map_err(|e| format!("nonblocking error: {e}"))?;

        let stream = listener
            .incoming()
            .next()
            .ok_or("no incoming connection")?
            .map_err(|e| format!("accept error: {e}"))?;

        let mut reader = BufReader::new(stream.try_clone().map_err(|e| format!("clone: {e}"))?);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| format!("read error: {e}"))?;

        let (code, state) = extract_code_and_state(&request_line)?;

        let mut stream = stream;
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
            <!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Neko Finance</title></head>\
            <body style=\"font-family:sans-serif;text-align:center;padding:40px;\">\
            <h1>Conectado!</h1><p>Google vinculado com sucesso. Pode fechar esta janela.</p></body></html>";
        stream
            .write_all(response.as_bytes())
            .map_err(|e| format!("write error: {e}"))?;

        Ok((code, state))
    }
}

fn extract_code_and_state(request_line: &str) -> Result<(String, Option<String>), String> {
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or("invalid request line")?;

    let query_start = path.find('?').ok_or("no query string")?;
    let query = &path[query_start + 1..];

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            let decoded = url_decode(value);
            if !decoded.is_empty() {
                code = Some(decoded);
            }
        } else if let Some(value) = pair.strip_prefix("state=") {
            state = Some(url_decode(value));
        }
    }

    let code = code.ok_or("no code parameter")?;
    Ok((code, state))
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                result.push(byte as char);
                continue;
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_from_path() {
        let line = "GET /?code=abc123&state=xyz HTTP/1.1";
        let (code, state) = extract_code_and_state(line).unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state.as_deref(), Some("xyz")); // o state volta para validação CSRF
    }

    #[test]
    fn test_extract_code_urlencoded() {
        let line = "GET /?code=abc%2Fdef%3Dghi&state=xyz HTTP/1.1";
        assert_eq!(extract_code_and_state(line).unwrap().0, "abc/def=ghi");
    }

    #[test]
    fn test_extract_code_no_code() {
        let line = "GET /?state=xyz HTTP/1.1";
        assert!(extract_code_and_state(line).is_err());
    }

    #[test]
    fn test_extract_missing_state_is_none() {
        let line = "GET /?code=abc123 HTTP/1.1";
        let (code, state) = extract_code_and_state(line).unwrap();
        assert_eq!(code, "abc123");
        assert!(state.is_none()); // sem state → o chamador rejeita (CSRF)
    }
}
