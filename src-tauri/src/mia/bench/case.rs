//! O caso de eval como o arquivo o declara.
//!
//! Um caso é um arquivo JSON em `evals/mia/cases/`: identificador, família, pergunta, fixture,
//! repetições e o que se espera da rodada. O parse é fail-closed — campo desconhecido, família
//! fora das seis, ferramenta que o catálogo da fachada não declara ou fixture inexistente
//! recusam o caso na carga, porque um catálogo que aceita caso malformado transforma erro de
//! autoria em falha (ou aprovação) atribuída ao modelo.
//!
//! A validação conhece o resto da casa de propósito: os nomes de ferramenta vêm do catálogo da
//! fachada e os nomes de fixture das fixtures da bancada. É esse acoplamento que faz uma tela
//! nova, uma ferramenta renomeada ou uma fixture apagada quebrarem o catálogo em teste, antes de
//! queimarem uma rodada paga.

use super::fixtures;
use crate::mia::catalog;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// As seis famílias do catálogo. A lista é fechada: família nova é decisão de spec, não de
/// autoria de caso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Family {
    SelecaoDeFerramenta,
    MultiHop,
    FidelidadeNumerica,
    Didatica,
    Injecao,
    RecusaHonesta,
}

impl Family {
    pub(crate) const ALL: [Family; 6] = [
        Family::SelecaoDeFerramenta,
        Family::MultiHop,
        Family::FidelidadeNumerica,
        Family::Didatica,
        Family::Injecao,
        Family::RecusaHonesta,
    ];

    /// O nome como os arquivos o escrevem — o mesmo que o serde aceita na carga.
    pub(crate) fn slug(&self) -> &'static str {
        match self {
            Family::SelecaoDeFerramenta => "selecao_de_ferramenta",
            Family::MultiHop => "multi_hop",
            Family::FidelidadeNumerica => "fidelidade_numerica",
            Family::Didatica => "didatica",
            Family::Injecao => "injecao",
            Family::RecusaHonesta => "recusa_honesta",
        }
    }
}

/// Quem dá o veredito do caso: a máquina, ou um julgamento cego humano depois da rodada.
/// Julgamento cego não desliga os checks mecânicos — ele diz apenas que passar neles ainda não
/// é passar no caso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Judgment {
    Mecanico,
    Cego,
}

/// Como a resposta precisa se apresentar: conta sobre os números da pessoa ou explicação do
/// método.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedProvenance {
    Calculo,
    Metodo,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpectedTools {
    #[serde(default)]
    pub must_call: Vec<String>,
    #[serde(default)]
    pub must_not_call: Vec<String>,
    #[serde(default)]
    pub min_calls: Option<u32>,
    #[serde(default)]
    pub max_calls: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpectedAnswer {
    /// Cada texto precisa aparecer na resposta (comparação sem caixa).
    #[serde(default)]
    pub must_contain: Vec<String>,
    /// Cada grupo exige pelo menos UM dos textos — o jeito de aceitar sinônimos ("importar" ou
    /// "lançar") sem afrouxar o resto.
    #[serde(default)]
    pub must_contain_any: Vec<Vec<String>>,
    /// Nenhum destes pode aparecer — é onde vivem as iscas de injeção.
    #[serde(default)]
    pub must_not_contain: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Expected {
    pub judgment: Judgment,
    #[serde(default)]
    pub provenance: Option<ExpectedProvenance>,
    #[serde(default)]
    pub tools: ExpectedTools,
    #[serde(default)]
    pub answer: ExpectedAnswer,
}

/// Uma chamada de fachada autorada junto do caso, com argumentos fixos. É o que amarra o número
/// esperado ao motor: um teste roda esta chamada contra a MESMA fixture e confere que os números
/// citados em `must_contain` existem no envelope — o catálogo não pode mentir sobre a fachada.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Verification {
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub id: String,
    pub family: Family,
    pub question: String,
    pub fixture: String,
    pub repetitions: u32,
    pub expected: Expected,
    #[serde(default)]
    pub verification: Option<Verification>,
}

/// Recusa de carga do catálogo. Fala com quem autora casos, no formato da casa: o que travou e
/// o que fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogError {
    pub message: String,
    pub fix: String,
}

impl CatalogError {
    fn new(message: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fix: fix.into(),
        }
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.message, self.fix)
    }
}

/// Parseia e valida um caso a partir do texto do arquivo. Puro: o IO fica em [`load_catalog`].
pub(crate) fn parse_case(file_name: &str, text: &str) -> Result<Case, CatalogError> {
    let case: Case = serde_json::from_str(text).map_err(|error| {
        CatalogError::new(
            format!("O caso {file_name} não parseia: {error}."),
            "Corrija o arquivo para o schema do catálogo (veja evals/mia/README.md).".to_string(),
        )
    })?;

    let stem = file_name.strip_suffix(".json").unwrap_or(file_name);
    if case.id != stem {
        return Err(CatalogError::new(
            format!(
                "O caso {file_name} declara o identificador \"{}\".",
                case.id
            ),
            format!(
                "Renomeie o arquivo para {}.json ou o identificador para {stem}.",
                case.id
            ),
        ));
    }

    if !fixtures::exists(&case.fixture) {
        return Err(CatalogError::new(
            format!(
                "O caso {file_name} pede a fixture \"{}\", que não existe.",
                case.fixture
            ),
            format!("Use uma destas: {}.", fixtures::NAMES.join(", ")),
        ));
    }

    if case.repetitions == 0 {
        return Err(CatalogError::new(
            format!("O caso {file_name} declara zero repetições."),
            "Declare pelo menos 1 — caso que nunca roda é caso desligado em silêncio.".to_string(),
        ));
    }

    let known_tools = catalog::tool_names();
    let declared = case
        .expected
        .tools
        .must_call
        .iter()
        .chain(&case.expected.tools.must_not_call)
        .chain(case.verification.iter().map(|v| &v.tool));
    for tool in declared {
        if !known_tools.contains(&tool.as_str()) {
            return Err(CatalogError::new(
                format!(
                    "O caso {file_name} espera a ferramenta \"{tool}\", que a fachada não declara."
                ),
                format!("Use uma destas: {}.", known_tools.join(", ")),
            ));
        }
    }

    if case
        .expected
        .answer
        .must_contain_any
        .iter()
        .any(|group| group.is_empty())
    {
        return Err(CatalogError::new(
            format!("O caso {file_name} tem um grupo vazio em must_contain_any."),
            "Preencha o grupo ou remova-o — grupo vazio é insatisfazível e falharia sempre."
                .to_string(),
        ));
    }

    // Dinheiro esperado sem verificação seria promessa sem prova: é a chamada de verificação
    // que amarra o número do caso ao envelope do motor, e um valor re-semeado na fixture
    // reprovaria o modelo por erro de autoria sem nada avisar.
    let expects_money = case
        .expected
        .answer
        .must_contain
        .iter()
        .chain(case.expected.answer.must_contain_any.iter().flatten())
        .any(|text| money_cents(text).is_some());
    if expects_money && case.verification.is_none() {
        return Err(CatalogError::new(
            format!("O caso {file_name} espera dinheiro na resposta e não declara verification."),
            "Declare a chamada de fachada (tool + arguments) que devolve esse número na mesma \
             fixture."
                .to_string(),
        ));
    }

    Ok(case)
}

/// "8.412,37" (ou "R$ 8.412,37") → 841237. Só a forma monetária brasileira com duas casas: é
/// ela que os casos usam para citar dinheiro, e é em centavos que o envelope o carrega.
pub(crate) fn money_cents(text: &str) -> Option<i64> {
    let trimmed = text
        .trim_start_matches(|character: char| !character.is_ascii_digit())
        .trim_end_matches(|character: char| !character.is_ascii_digit());
    let (integer, fraction) = trimmed.rsplit_once(',')?;
    if fraction.len() != 2 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if integer.is_empty()
        || !integer
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return None;
    }
    let digits: String = integer
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(char::from)
        .collect();
    format!("{digits}{fraction}").parse().ok()
}

/// As seis famílias precisam estar representadas: o catálogo mede a conversa inteira, e uma
/// família ausente é uma dimensão que ninguém está medindo.
pub(crate) fn ensure_families(cases: &[Case]) -> Result<(), CatalogError> {
    let present: BTreeSet<Family> = cases.iter().map(|case| case.family).collect();
    let missing: Vec<&str> = Family::ALL
        .iter()
        .filter(|family| !present.contains(family))
        .map(Family::slug)
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(CatalogError::new(
        format!("O catálogo não cobre as famílias: {}.", missing.join(", ")),
        "Escreva pelo menos um caso para cada família em evals/mia/cases/.".to_string(),
    ))
}

pub(crate) fn ensure_unique_ids(cases: &[Case]) -> Result<(), CatalogError> {
    let mut seen = BTreeSet::new();
    for case in cases {
        if !seen.insert(case.id.as_str()) {
            return Err(CatalogError::new(
                format!(
                    "O identificador \"{}\" aparece em mais de um caso.",
                    case.id
                ),
                "Dê a cada caso um identificador próprio.".to_string(),
            ));
        }
    }
    Ok(())
}

/// Carrega o catálogo inteiro de um diretório: todo `.json` é um caso, e a carga só devolve o
/// catálogo que passou inteiro — caso inválido não é pulado, porque pular seria desligar um eval
/// em silêncio.
pub(crate) fn load_catalog(dir: &Path) -> Result<Vec<Case>, CatalogError> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        CatalogError::new(
            format!("O catálogo em {} não abre: {error}.", dir.display()),
            "Rode a bancada da raiz do repositório, onde evals/mia/cases/ existe.".to_string(),
        )
    })?;

    let mut files: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();

    let mut cases = Vec::with_capacity(files.len());
    for path in files {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let text = std::fs::read_to_string(&path).map_err(|error| {
            CatalogError::new(
                format!("O caso {file_name} não pôde ser lido: {error}."),
                "Confira as permissões do arquivo.".to_string(),
            )
        })?;
        cases.push(parse_case(&file_name, &text)?);
    }

    if cases.is_empty() {
        return Err(CatalogError::new(
            format!("O catálogo em {} está vazio.", dir.display()),
            "Escreva os casos em evals/mia/cases/ — um arquivo por caso.".to_string(),
        ));
    }

    ensure_unique_ids(&cases)?;
    ensure_families(&cases)?;
    Ok(cases)
}
