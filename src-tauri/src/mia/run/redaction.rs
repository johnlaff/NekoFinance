//! O redator de credenciais.
//!
//! A chave vive no cofre do sistema e nunca é passada ao laço. Isso basta para o caminho feliz e
//! não basta para o resto: mensagem de erro de provedor ecoa cabeçalho enviado, e é por aí que
//! uma credencial chegaria a evento, log ou banco sem ninguém ter decidido isso. Todo texto vindo
//! do outro lado passa por aqui antes de existir em qualquer artefato.

/// A marca que substitui o segredo. É deliberadamente legível: quem depura precisa saber que
/// houve redação, não descobrir um texto truncado sem explicação.
pub(crate) const REDACTED: &str = "[credencial removida]";

/// Substitui toda sequência com forma de credencial pelo marcador.
///
/// Reconhece: prefixos de chave de API (`sk-`, `sk_`), o esquema `Bearer <token>` e valores de
/// cabeçalho de chave (`authorization: …`, `x-api-key: …`, `api-key: …`), em qualquer caixa.
pub(crate) fn credentials(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut redacted = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if let Some(value_start) = header_value_start(bytes, index) {
            let value_end = line_end(bytes, value_start);
            redacted.extend_from_slice(&bytes[index..value_start]);
            redacted.extend_from_slice(REDACTED.as_bytes());
            index = value_end;
        } else if let Some(token_end) = api_key_end(bytes, index) {
            redacted.extend_from_slice(REDACTED.as_bytes());
            index = token_end;
        } else if let Some((token_start, token_end)) = bearer_token_range(text, index) {
            redacted.extend_from_slice(&bytes[index..token_start]);
            redacted.extend_from_slice(REDACTED.as_bytes());
            index = token_end;
        } else {
            redacted.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(redacted).expect("a redação preserva sequências UTF-8 válidas")
}

fn header_value_start(bytes: &[u8], index: usize) -> Option<usize> {
    const HEADER_NAMES: &[&[u8]] = &[b"authorization", b"x-api-key", b"api-key"];

    for name in HEADER_NAMES {
        let colon = index + name.len();
        if bytes
            .get(index..colon)
            .is_some_and(|candidate| ascii_case_eq(candidate, name))
            && bytes.get(colon) == Some(&b':')
        {
            let value_start = skip_horizontal_whitespace(bytes, colon + 1);
            if value_start < bytes.len() && !is_line_break(bytes[value_start]) {
                return Some(value_start);
            }
        }
    }

    None
}

fn api_key_end(bytes: &[u8], index: usize) -> Option<usize> {
    let prefix_end = index + 3;
    let prefix = bytes.get(index..prefix_end)?;
    if !ascii_case_eq(prefix, b"sk-") && !ascii_case_eq(prefix, b"sk_") {
        return None;
    }

    let mut token_end = prefix_end;
    while bytes
        .get(token_end)
        .is_some_and(|byte| is_api_key_character(*byte))
    {
        token_end += 1;
    }

    (token_end - prefix_end >= 16).then_some(token_end)
}

fn bearer_token_range(text: &str, index: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let word_end = index + b"bearer".len();
    if !bytes
        .get(index..word_end)
        .is_some_and(|candidate| ascii_case_eq(candidate, b"bearer"))
        || index > 0 && is_word_character(bytes[index - 1])
    {
        return None;
    }

    let token_start = skip_horizontal_whitespace(bytes, word_end);
    if token_start == word_end {
        return None;
    }

    let mut token_end = token_start;
    let mut token_length = 0;
    for (offset, character) in text[token_start..].char_indices() {
        if character.is_whitespace() {
            break;
        }
        token_end = token_start + offset + character.len_utf8();
        token_length += 1;
    }

    (token_length >= 8).then_some((token_start, token_end))
}

fn ascii_case_eq(candidate: &[u8], expected: &[u8]) -> bool {
    candidate.len() == expected.len()
        && candidate
            .iter()
            .zip(expected)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn skip_horizontal_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
    {
        index += 1;
    }
    index
}

fn line_end(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(|byte| !is_line_break(*byte)) {
        index += 1;
    }
    index
}

fn is_line_break(byte: u8) -> bool {
    matches!(byte, b'\r' | b'\n')
}

fn is_api_key_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn is_word_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}
