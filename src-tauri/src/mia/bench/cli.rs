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

pub(crate) const USAGE: &str = "Uso: mia-bench [bakeoff] [--model <id do pin>] \
     [--max-spend-usd <decimal>] [--pack <caminho>] [--only <trecho do id>] \
     [--cases-dir <caminho>] [--reports-dir <caminho>]";

/// O que a execução vai fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Uma corrida num pin só, com as repetições que os casos declaram. É a medição do dia a dia:
    /// mudou fachada, prompt ou ferramenta, roda de novo no modelo em uso.
    Single,
    /// As duas fases sobre a matriz inteira, terminando na decisão do modelo default.
    Bakeoff,
}

/// Um dólar cobre o catálogo inteiro com folga num modelo só, e não cobre um laço desgovernado.
const SINGLE_CAP_MICRO_USD: i64 = 1_000_000;

/// O teto do bakeoff, decidido junto da spec: cinco candidatos em uma repetição, mais três
/// repetições nos finalistas, mais o teto de referência, cabem aqui. Quem paga é a chave dedicada,
/// que tem o próprio limite no painel do provedor.
const BAKEOFF_CAP_MICRO_USD: i64 = 5_000_000;

#[derive(Debug)]
pub(crate) struct CliArgs {
    pub mode: Mode,
    pub model: Option<String>,
    pub max_spend_micro_usd: i64,
    pub pack_root: Option<PathBuf>,
    pub only: Option<String>,
    pub cases_dir: PathBuf,
    pub reports_dir: PathBuf,
}

pub(crate) fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let (mode, flags) = match args.first() {
        Some(first) if !first.starts_with("--") => {
            let mode = match first.as_str() {
                "bakeoff" => Mode::Bakeoff,
                other => return Err(format!("Modo desconhecido: {other}. {USAGE}")),
            };
            (mode, &args[1..])
        }
        _ => (Mode::Single, args),
    };

    let mut parsed = CliArgs {
        mode,
        model: None,
        max_spend_micro_usd: 0,
        pack_root: None,
        only: None,
        cases_dir: PathBuf::from("evals/mia/cases"),
        reports_dir: PathBuf::from("evals/mia/reports"),
    };
    // O teto pedido fica separado do default até o fim do parse: cada modo tem o seu, e o valor
    // explícito vence os dois.
    let mut requested_cap: Option<i64> = None;

    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        let mut value = || {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("A flag {flag} precisa de um valor. {USAGE}"))
        };
        match flag.as_str() {
            "--model" => parsed.model = Some(value()?),
            "--max-spend-usd" => requested_cap = Some(parse_usd(&value()?)?),
            "--pack" => parsed.pack_root = Some(PathBuf::from(value()?)),
            "--only" => parsed.only = Some(value()?),
            "--cases-dir" => parsed.cases_dir = PathBuf::from(value()?),
            "--reports-dir" => parsed.reports_dir = PathBuf::from(value()?),
            other => return Err(format!("Flag desconhecida: {other}. {USAGE}")),
        }
    }

    // Quem corre no bakeoff é a matriz de pins, na ordem a priori: escolher o modelo à mão seria
    // pedir uma corrida solta com outro nome, e o relatório sairia decidindo o default por um
    // recorte que ninguém declarou.
    if mode == Mode::Bakeoff && parsed.model.is_some() {
        return Err(
            "O bakeoff corre a matriz inteira e não aceita --model. Rode sem o modo bakeoff para \
             medir um modelo só."
                .to_string(),
        );
    }

    parsed.max_spend_micro_usd = requested_cap.unwrap_or(match mode {
        Mode::Single => SINGLE_CAP_MICRO_USD,
        Mode::Bakeoff => BAKEOFF_CAP_MICRO_USD,
    });
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
