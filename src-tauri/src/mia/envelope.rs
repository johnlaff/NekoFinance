//! Contrato comum de toda resposta da fachada.
//!
//! O envelope existe para que o modelo não precise interpretar formatos diferentes por
//! ferramenta: um `meta` herdado por todas (moeda, timezone, período, `as_of`, revisão dos
//! dados), dinheiro sempre em centavos inteiros, estado epistêmico explícito no lugar de
//! sentinela numérica, e erro que diz o que fazer.

use chrono::{DateTime, FixedOffset, NaiveDate, SecondsFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Teto de linhas que uma única chamada devolve. Existe por duas razões que se somam: contexto
/// do modelo é caro, e conteúdo de lançamento é dado não confiável no prompt — quanto menor a
/// superfície, menor o alvo de injeção. Lista maior que isto sai truncada e o diz.
pub(crate) const MAX_ROWS: usize = 200;

/// Moeda única do app. Não há conversão nem multi-moeda no domínio.
pub(crate) const CURRENCY: &str = "BRL";

/// Relógio da rodada. Injetado para que toda resposta seja determinística em teste — `as_of`,
/// `today` e o offset saem da mesma leitura, nunca de três chamadas ao relógio ambiente.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Clock {
    now: DateTime<FixedOffset>,
}

impl Clock {
    /// Em produção, `Local::now().fixed_offset()` — uma leitura só do relógio por rodada.
    pub(crate) fn at(now: DateTime<FixedOffset>) -> Self {
        Self { now }
    }

    pub(crate) fn today(&self) -> NaiveDate {
        self.now.date_naive()
    }

    /// Instante da leitura em RFC 3339. É o que amarra a resposta a um mundo: um número lido
    /// agora não vale para o mundo de dez minutos atrás.
    pub(crate) fn as_of(&self) -> String {
        self.now.to_rfc3339_opts(SecondsFormat::Secs, false)
    }

    /// Offset local em ISO 8601 (`-03:00`). O domínio não guarda zona IANA; o que muda a
    /// leitura de "hoje" é o offset em que o dia foi cortado.
    pub(crate) fn timezone(&self) -> String {
        self.now.offset().to_string()
    }
}

/// Recorte temporal a que a resposta se refere, sempre com datas explícitas — nunca "este mês"
/// ou "últimos 30 dias", que o modelo teria de interpretar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Period {
    pub start: String,
    pub end: String,
}

impl Period {
    pub(crate) fn day(date: NaiveDate) -> Self {
        let d = date.format("%Y-%m-%d").to_string();
        Self {
            start: d.clone(),
            end: d,
        }
    }

    pub(crate) fn between(start: NaiveDate, end: NaiveDate) -> Self {
        Self {
            start: start.format("%Y-%m-%d").to_string(),
            end: end.format("%Y-%m-%d").to_string(),
        }
    }
}

/// Estado epistêmico de um número do método. É o vocabulário único da fachada: as réguas do
/// domínio que falam `chosen`/`none` são traduzidas aqui, para o modelo não aprender dois
/// dialetos. Um número sem registro sai como `no_record` com valor nulo — jamais como zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DataState {
    /// Dado registrado/escolhido: o veredito.
    Verdict,
    /// Número derivado — sempre acompanhado do selo na superfície.
    Estimate,
    /// Entrada presente e legitimamente zero (ex.: contas de reserva zeradas).
    Zero,
    /// Lacuna: não há o que ler.
    NoRecord,
}

/// Leitura de valor único com estado. `value` é nulo em `no_record` por construção.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Reading<T> {
    pub state: DataState,
    pub value: Option<T>,
}

impl<T> Reading<T> {
    pub(crate) fn new(state: DataState, value: T) -> Self {
        Self {
            state,
            value: Some(value),
        }
    }

    pub(crate) fn no_record() -> Self {
        Self {
            state: DataState::NoRecord,
            value: None,
        }
    }
}

/// Lista com teto aplicado. `total` cobre o filtro inteiro (não a página), para que o modelo
/// nunca conclua "são 200" de uma lista cortada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Listing<T> {
    pub items: Vec<T>,
    pub returned: usize,
    pub total: usize,
    pub truncated: bool,
}

impl<T> Listing<T> {
    /// Corta em [`MAX_ROWS`] preservando a contagem real.
    pub(crate) fn capped(mut items: Vec<T>) -> Self {
        let total = items.len();
        let truncated = total > MAX_ROWS;
        if truncated {
            items.truncate(MAX_ROWS);
        }
        Self {
            returned: items.len(),
            items,
            total,
            truncated,
        }
    }
}

/// Por que a chamada não produziu resposta. O código é para o loop decidir; `message` e `fix`
/// são para o modelo se corrigir sozinho na mesma rodada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    /// Nome de ferramenta fora do catálogo.
    UnknownTool,
    /// Argumento que a ferramenta não declara — fail-closed: nunca ignorado em silêncio.
    UnknownArgument,
    /// Argumento declarado, valor inválido (tipo errado, fora do vocabulário).
    InvalidArgument,
    /// Leitura do banco falhou.
    ReadFailed,
}

/// Erro acionável: diz o que falhou E o que fazer a seguir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    pub fix: String,
}

impl ToolError {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            fix: fix.into(),
        }
    }

    /// Falha de leitura do banco. A mensagem crua do driver fica no `message` (é diagnóstico,
    /// não dado da pessoa) e o `fix` diz o gesto possível.
    pub(crate) fn read_failed(detail: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::ReadFailed,
            detail,
            "Tente de novo; se persistir, o banco local pode precisar de nova importação.",
        )
    }
}

/// O que uma ferramenta produz: o recorte a que respondeu e os dados. O `meta` completo é
/// montado pela porta — ferramenta nenhuma carimba moeda, relógio ou revisão por conta própria.
pub(crate) struct ToolOutput {
    pub period: Period,
    pub data: serde_json::Value,
}

pub(crate) type ToolResult = Result<ToolOutput, ToolError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Meta {
    pub currency: &'static str,
    pub timezone: String,
    pub period: Period,
    pub as_of: String,
    /// Impressão digital dos dados lidos. Duas respostas com a mesma revisão leram o mesmo
    /// mundo; é o que amarra uma proposta ao estado que a originou. Nulo só quando o próprio
    /// banco não pôde ser lido — a mesma falha que o `error` da resposta descreve.
    pub data_revision: Option<String>,
    /// Teto de linhas aplicado nesta chamada — o modelo lê o corte em vez de supor.
    pub row_limit: usize,
}

/// A resposta que sai da porta. Sucesso e erro compartilham o mesmo `meta`: mesmo quando a
/// chamada falha, o modelo sabe que mundo foi consultado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Envelope {
    pub tool: String,
    pub ok: bool,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
}

/// Revisão dos dados: hash de contagens, somas e maiores timestamps das tabelas materiais.
///
/// Contagem sozinha não detecta edição de valor e timestamp sozinho tem resolução de segundo,
/// então os três entram juntos. A revisão prova IGUALDADE de leitura para amarrar proposta a
/// mundo — não é log de auditoria nem substitui o `as_of`.
pub(crate) async fn data_revision(pool: &SqlitePool) -> Result<String, String> {
    type RevisionRow = (i64, String, i64, i64, i64, i64, String, i64, i64, i64);
    let row: RevisionRow = sqlx::query_as(
        "SELECT \
           (SELECT COUNT(*) FROM \"transaction\"), \
           (SELECT COALESCE(MAX(updated_at), '') FROM \"transaction\"), \
           (SELECT COALESCE(SUM(amount), 0) FROM \"transaction\"), \
           (SELECT COUNT(*) FROM account), \
           (SELECT COALESCE(SUM(balance), 0) FROM account), \
           (SELECT COUNT(*) FROM daily_budget), \
           (SELECT COALESCE(MAX(calculated_at), '') FROM daily_budget), \
           (SELECT COALESCE(SUM(amount), 0) FROM daily_budget), \
           (SELECT COUNT(*) FROM invoice), \
           (SELECT COALESCE(SUM(COALESCE(stated_total_cents, 0)), 0) FROM invoice)",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("revisão dos dados: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9
        )
        .as_bytes(),
    );
    Ok(hex::encode(hasher.finalize())[..16].to_string())
}
