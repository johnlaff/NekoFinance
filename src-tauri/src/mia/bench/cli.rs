//! A linha de comando da bancada e as guardas de execução.
//!
//! O parse é à mão de propósito: seis flags não pagam uma dependência nova num binário que
//! carrega chave e gasta dinheiro — superfície de cadeia de suprimento também é superfície.
//!
//! As duas recusas moram aqui como decisão pura, exercitável em teste: a bancada não roda em CI
//! (custo real e segredo não pertencem a pipeline) e não roda sem a chave dedicada — a variável
//! de ambiente, nunca a chave do app no cofre, porque a chave da bancada é a que tem limite de
//! gasto próprio no painel do provedor.

use std::path::PathBuf;

pub(crate) const USAGE: &str = "Uso: mia-bench [--model <id do pin>] [--max-spend-usd <decimal>] \
     [--pack <caminho>] [--only <trecho do id>] [--cases-dir <caminho>] [--reports-dir <caminho>]";

#[derive(Debug)]
pub(crate) struct CliArgs {
    pub model: Option<String>,
    pub max_spend_micro_usd: i64,
    pub pack_root: Option<PathBuf>,
    pub only: Option<String>,
    pub cases_dir: PathBuf,
    pub reports_dir: PathBuf,
}

pub(crate) fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let mut parsed = CliArgs {
        model: None,
        // Um dólar cobre o catálogo com folga e não cobre um laço desgovernado.
        max_spend_micro_usd: 1_000_000,
        pack_root: None,
        only: None,
        cases_dir: PathBuf::from("evals/mia/cases"),
        reports_dir: PathBuf::from("evals/mia/reports"),
    };

    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let mut value = || {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("A flag {flag} precisa de um valor. {USAGE}"))
        };
        match flag.as_str() {
            "--model" => parsed.model = Some(value()?),
            "--max-spend-usd" => parsed.max_spend_micro_usd = parse_usd(&value()?)?,
            "--pack" => parsed.pack_root = Some(PathBuf::from(value()?)),
            "--only" => parsed.only = Some(value()?),
            "--cases-dir" => parsed.cases_dir = PathBuf::from(value()?),
            "--reports-dir" => parsed.reports_dir = PathBuf::from(value()?),
            other => return Err(format!("Flag desconhecida: {other}. {USAGE}")),
        }
    }
    Ok(parsed)
}

/// Dólares em texto → milionésimos de dólar, sem ponto flutuante: a trava é dinheiro, e um
/// decimal binário que "quase" representa o teto travaria um micro antes ou depois do combinado.
pub(crate) fn parse_usd(text: &str) -> Result<i64, String> {
    let refuse = || {
        format!(
            "O valor de --max-spend-usd precisa ser um decimal positivo em dólares, com ponto \
             (ex.: 1.50); veio \"{text}\"."
        )
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(refuse());
    }
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
    {
        return Err(refuse());
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| refuse())?
    };
    let micros: i64 = format!("{fraction:0<6}").parse().map_err(|_| refuse())?;
    let total = whole
        .checked_mul(1_000_000)
        .and_then(|dollars| dollars.checked_add(micros))
        .ok_or_else(refuse)?;
    if total <= 0 {
        return Err(refuse());
    }
    Ok(total)
}

/// Por que a bancada NÃO vai rodar — ou nada, quando pode. A presença de `CI` no ambiente basta
/// como recusa: toda esteira relevante a define, e o valor dela não muda o custo nem o segredo.
pub(crate) fn refuse_reason(ci: Option<&str>, key: Option<&str>) -> Option<String> {
    if ci.is_some() {
        return Some(
            "A bancada não roda em CI: ela gasta dinheiro de verdade e depende de chave dedicada."
                .to_string(),
        );
    }
    if key.is_none_or(|key| key.trim().is_empty()) {
        return Some(
            "Defina NEKO_MIA_BENCH_KEY com a chave dedicada da bancada — criada no painel do \
             provedor com limite de gasto próprio, nunca a chave do app."
                .to_string(),
        );
    }
    None
}
