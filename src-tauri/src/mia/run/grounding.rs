//! O aterramento dos números da resposta.
//!
//! O modelo nunca calcula. Todo número material vem pronto da ferramenta, e a garantia disso não
//! é a instrução do prompt — é esta verificação: um número na resposta que não tenha origem em
//! fato retornado na mesma rodada faz a resposta ser descartada antes de existir para quem
//! perguntou. É por isso que a interface não recebe texto token a token: publicar um número para
//! depois retirá-lo seria pior do que demorar a publicá-lo.
//!
//! Os fatos citáveis vêm somente dos dados de ferramenta bem-sucedida, do `meta` que a casa
//! fornece para cada ferramenta e do prefixo do método. A pergunta fica de fora porque ela pede
//! uma leitura, mas não comprova o número que menciona.
//!
//! A verificação é deliberadamente conservadora. Falso positivo custa uma regeneração; falso
//! negativo publica número inventado sobre o dinheiro de alguém.

use crate::mia::envelope::Envelope;
use serde_json::{Number, Value};
use std::collections::BTreeSet;

/// De onde um fato citável veio. A camada de método sustenta régua, faixa e limiar; os dados
/// sustentam o que é da pessoa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactOrigin {
    Method,
    Data,
}

/// Os números que a rodada pode citar, em forma canônica.
///
/// Três origens entram, e só elas: dados de ferramenta bem-sucedida, `meta` da ferramenta e o
/// prefixo do método. A pergunta não entra porque repetir uma hipótese da pessoa não a transforma
/// em fato.
#[derive(Debug, Default)]
pub(crate) struct Facts {
    numbers: BTreeSet<String>,
    data_numbers: BTreeSet<String>,
    method_numbers: BTreeSet<String>,
}

impl Facts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Absorve o `meta` de todo envelope e o `data` de ferramenta bem-sucedida. O `meta` sustenta
    /// somente a citação: período, instante de leitura e revisão não tornam uma explicação em
    /// cálculo sobre a pessoa.
    ///
    /// Cada valor inteiro entra com as leituras que a superfície pode imprimir a partir dele:
    /// o inteiro cru, os centavos lidos como reais com duas casas, o mesmo truncado a reais
    /// inteiros, e os décimos — porque o produto TRUNCA percentual na exibição, e um número
    /// impresso assim continua tendo origem no fato que o gerou.
    pub(crate) fn absorb_envelope(&mut self, envelope: &Envelope, origin: FactOrigin) {
        let meta = serde_json::to_value(&envelope.meta).expect("o metadado é serializável");
        Self::absorb_value(&meta, &mut self.numbers);
        if envelope.ok
            && let Some(data) = &envelope.data
        {
            Self::absorb_value(data, &mut self.numbers);
            match origin {
                FactOrigin::Method => Self::absorb_value(data, &mut self.method_numbers),
                FactOrigin::Data => Self::absorb_value(data, &mut self.data_numbers),
            }
        }
    }

    /// Absorve os números escritos no prefixo do método.
    pub(crate) fn absorb_text(&mut self, text: &str) {
        Self::absorb_text_into(text, &mut self.numbers);
        Self::absorb_text_into(text, &mut self.method_numbers);
    }

    fn absorb_text_into(text: &str, numbers: &mut BTreeSet<String>) {
        for token in numeric_tokens(text) {
            if let NumericToken::Supported(token) = token {
                numbers.insert(canonical_token(token));
            }
        }
    }

    fn absorb_value(value: &Value, numbers: &mut BTreeSet<String>) {
        match value {
            Value::Number(number) => Self::absorb_number(number, numbers),
            Value::String(text) => Self::absorb_text_into(text, numbers),
            Value::Array(items) => {
                for item in items {
                    Self::absorb_value(item, numbers);
                }
            }
            Value::Object(fields) => {
                for value in fields.values() {
                    Self::absorb_value(value, numbers);
                }
            }
            Value::Null | Value::Bool(_) => {}
        }
    }

    fn absorb_number(number: &Number, numbers: &mut BTreeSet<String>) {
        if let Some(integer) = number.as_i64() {
            Self::absorb_integer(integer.unsigned_abs(), numbers);
        } else if let Some(integer) = number.as_u64() {
            Self::absorb_integer(integer, numbers);
        } else {
            let canonical = canonical_json_number(&number.to_string());
            let truncated = canonical
                .split_once('.')
                .map_or(canonical.as_str(), |(integer, _)| integer)
                .to_string();
            numbers.insert(canonical);
            numbers.insert(truncated);
        }
    }

    fn absorb_integer(integer: u64, numbers: &mut BTreeSet<String>) {
        numbers.insert(integer.to_string());
        numbers.insert(scaled_decimal(integer, 100, 2));
        numbers.insert((integer / 100).to_string());
        numbers.insert(scaled_decimal(integer, 10, 1));
        numbers.insert((integer / 10).to_string());
    }
}

/// Os números da resposta que não têm origem nos fatos da rodada. Lista vazia = resposta
/// aterrada. A lista existe para o rastro técnico dizer QUAL número derrubou a resposta.
pub(crate) fn orphans(answer: &str, facts: &Facts) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();

    for token in numeric_tokens(answer) {
        match token {
            NumericToken::Supported(token) => {
                let canonical = canonical_token(token);
                if !facts.numbers.contains(&canonical) && seen.insert(canonical) {
                    missing.push(token.to_string());
                }
            }
            NumericToken::Unsupported(token) => {
                let canonical = format!("notacao_nao_suportada:{token}");
                if seen.insert(canonical) {
                    missing.push(token.to_string());
                }
            }
        }
    }

    missing
}

/// A resposta se apoia em número que só os dados da pessoa sustentam?
///
/// Número que o método também sustenta — uma faixa, um limiar — não transforma explicação em
/// conta: o que transforma é um número que só existe porque uma leitura o trouxe.
pub(crate) fn cites_data(answer: &str, facts: &Facts) -> bool {
    numeric_tokens(answer).into_iter().any(|token| match token {
        NumericToken::Supported(token) => {
            let canonical = canonical_token(token);
            facts.data_numbers.contains(&canonical) && !facts.method_numbers.contains(&canonical)
        }
        NumericToken::Unsupported(_) => false,
    })
}

enum NumericToken<'a> {
    Supported(&'a str),
    Unsupported(&'a str),
}

fn numeric_tokens(text: &str) -> Vec<NumericToken<'_>> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut last_digit_end = 0;
    let mut unsupported = false;
    let mut characters = text.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        if let Some(token_start) = start {
            if character.is_ascii_digit() {
                last_digit_end = index + character.len_utf8();
                continue;
            }
            if matches!(character, '.' | ',') {
                continue;
            }
            if matches!(character, 'e' | 'E')
                && characters
                    .peek()
                    .is_some_and(|(_, next)| next.is_ascii_digit())
            {
                unsupported = true;
                continue;
            }
            let token = &text[token_start..last_digit_end];
            tokens.push(if unsupported {
                NumericToken::Unsupported(token)
            } else {
                NumericToken::Supported(token)
            });
            start = None;
            unsupported = false;
        } else if character.is_ascii_digit() {
            start = Some(index);
            last_digit_end = index + character.len_utf8();
        }
    }

    if let Some(token_start) = start {
        let token = &text[token_start..last_digit_end];
        tokens.push(if unsupported {
            NumericToken::Unsupported(token)
        } else {
            NumericToken::Supported(token)
        });
    }

    tokens
}

fn canonical_token(token: &str) -> String {
    if let Some(decimal_separator) = token.rfind(',') {
        let integer = digits_only(&token[..decimal_separator]);
        let fraction = digits_only(&token[decimal_separator + 1..]);
        return canonical_parts(&integer, &fraction);
    }

    let dot_count = token.bytes().filter(|byte| *byte == b'.').count();
    if dot_count == 1 {
        let decimal_separator = token.find('.').expect("o token contém ponto decimal");
        let fraction = &token[decimal_separator + 1..];
        if (1..=2).contains(&fraction.len()) {
            return canonical_parts(&token[..decimal_separator], fraction);
        }
    }

    canonical_parts(&digits_only(token), "")
}

fn canonical_json_number(raw: &str) -> String {
    let unsigned = raw
        .strip_prefix('-')
        .or_else(|| raw.strip_prefix('+'))
        .unwrap_or(raw);
    let (significand, exponent) = match unsigned.find('e').or_else(|| unsigned.find('E')) {
        Some(separator) => {
            let exponent = unsigned[separator + 1..]
                .parse::<i32>()
                .expect("o número JSON contém expoente inteiro");
            (&unsigned[..separator], exponent)
        }
        None => (unsigned, 0),
    };
    let decimal_separator = significand.find('.');
    let digits = digits_only(significand);
    let decimal_position =
        decimal_separator.unwrap_or(significand.len()) as i64 + i64::from(exponent);

    if decimal_position <= 0 {
        let zeros = "0".repeat(decimal_position.unsigned_abs() as usize);
        return canonical_parts("0", &format!("{zeros}{digits}"));
    }

    let decimal_position = decimal_position as usize;
    if decimal_position >= digits.len() {
        return canonical_parts(
            &format!("{digits}{}", "0".repeat(decimal_position - digits.len())),
            "",
        );
    }

    canonical_parts(&digits[..decimal_position], &digits[decimal_position..])
}

fn scaled_decimal(integer: u64, scale: u64, fractional_digits: usize) -> String {
    let whole = integer / scale;
    let mut fraction = (integer % scale).to_string();
    while fraction.len() < fractional_digits {
        fraction.insert(0, '0');
    }
    canonical_parts(&whole.to_string(), &fraction)
}

fn digits_only(text: &str) -> String {
    text.bytes()
        .filter(u8::is_ascii_digit)
        .map(char::from)
        .collect()
}

fn canonical_parts(integer: &str, fraction: &str) -> String {
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.trim_end_matches('0');

    if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    }
}
