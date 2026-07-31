//! A proposta de lançamento: montar o gesto sem executá-lo.
//!
//! Esta é a única ferramenta da fachada que não responde uma leitura — e mesmo assim ela continua
//! read-only. O que sai daqui é um lançamento canônico validado, normalizado, com validade e
//! assinatura; quem grava é o gesto da pessoa sobre o cartão, fora do laço. É essa separação que
//! torna a defesa contra injeção estrutural: texto convincente no meio de um dado não alcança o
//! histórico financeiro, porque nenhum caminho do laço escreve.
//!
//! O escopo é estreito de propósito. Transferência, Economia, recorrência, parcelamento e divisão
//! entre pessoas continuam nos formulários que os tratam direito: cada um deles tem campos e
//! consequências que uma frase não carrega, e supô-los seria inventar o que a pessoa não disse.
//!
//! A aprovação revalida tudo antes de gravar — decisão ainda pendente, validade, mundo inalterado,
//! assinatura correspondente à proposta emitida e payload (editável no cartão) aprovado pela mesma
//! régua da emissão — e cria o lançamento pelo MESMO helper que o formulário chama. A régua do lançamento vive numa função só; copiá-la para cá seria criar uma
//! segunda para divergir. Nada disso toca a planilha: a convergência segue pelo write-back, que
//! exige diff e aprovação próprios.

use super::envelope::{Clock, ErrorCode, Period, ToolError, ToolOutput, ToolResult, data_revision};
use super::{Args, Context};
use crate::commands::transactions::create_transaction_inner;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, SecondsFormat};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// A versão do formato da proposta. Entra na assinatura: uma proposta emitida sob outro formato
/// não pode ser aprovada por um leitor que entende outro conjunto de campos.
pub(crate) const SCHEMA_VERSION: i64 = 1;

/// Quanto tempo uma proposta vale. Curto porque ela é uma foto do mundo: aprovar meia hora depois
/// aprovaria sobre números que já mudaram.
const TTL_MINUTES: i64 = 10;

/// As duas naturezas que a conversa propõe.
const KINDS: [&str; 2] = ["income", "expense"];

/// Como a linha do ledger registra o destino de uma proposta.
const PROPOSED: &str = "proposed";
const APPROVED: &str = "approved";
const REJECTED: &str = "rejected";
const EXPIRED: &str = "expired";

/// O lançamento como a proposta o descreve, já normalizado. É esta forma — e não o texto que a
/// originou — que a assinatura cobre e que a aprovação recria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Payload {
    pub kind: String,
    pub amount_cents: i64,
    pub date: String,
    pub description: Option<String>,
    pub payment_method: Option<String>,
    pub is_fixed: bool,
    pub tag_ids: Vec<String>,
}

impl Payload {
    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "amount_cents": self.amount_cents,
            "date": self.date,
            "description": self.description,
            "payment_method": self.payment_method,
            "is_fixed": self.is_fixed,
            "tag_ids": self.tag_ids,
        })
    }
}

/// A recusa que devolve a pergunta a quem tem como respondê-la. Campo material ausente nunca vira
/// suposição: um valor, uma data ou uma natureza inventados chegariam ao cartão com a aparência de
/// algo que a pessoa disse.
fn ask_for(field: &str, question: &str) -> ToolError {
    ToolError::new(
        ErrorCode::InvalidArgument,
        format!("A proposta não sai sem \"{field}\", e ele não pode ser suposto."),
        format!("Pergunte antes de propor: {question}"),
    )
}

/// A recusa do escopo estreito. Ela nomeia o caminho que resolve, porque a pessoa continua tendo
/// como registrar o que a conversa não propõe.
fn out_of_scope(kind: &str) -> ToolError {
    ToolError::new(
        ErrorCode::InvalidArgument,
        format!("A conversa não propõe \"{kind}\": ela monta só entrada ou despesa avulsa."),
        "Transferência, Economia, recorrência, parcelamento e divisão entre pessoas se registram \
         pelo formulário de Lançar, que trata cada um deles. Diga isso a quem perguntou, ou chame \
         de novo com kind em: income, expense."
            .to_string(),
    )
}

/// Lê os argumentos crus na forma canônica do lançamento.
fn parse_payload(args: &Args, today: NaiveDate) -> Result<Payload, ToolError> {
    let kind = match args.text("kind")? {
        None => return Err(ask_for("kind", "é uma entrada ou uma despesa?")),
        Some(kind) if KINDS.contains(&kind) => kind.to_string(),
        Some(other) => return Err(out_of_scope(other)),
    };

    let amount_cents = args
        .cents("amount_cents")?
        .ok_or_else(|| ask_for("amount_cents", "de quanto foi, exatamente?"))?;
    if amount_cents <= 0 {
        return Err(ToolError::new(
            ErrorCode::InvalidArgument,
            "O valor de um lançamento é sempre positivo — o sinal vem de kind.",
            "Chame de novo com amount_cents em centavos inteiros acima de zero, por exemplo \
             amount_cents: 8000 (R$ 80,00)."
                .to_string(),
        ));
    }

    let date = match args.text("date")? {
        None => {
            return Err(ask_for(
                "date",
                &format!("em que dia foi? Hoje é {}.", today.format("%Y-%m-%d")),
            ));
        }
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map_err(|_| {
                ToolError::new(
                    ErrorCode::InvalidArgument,
                    format!("\"{raw}\" não é uma data."),
                    format!(
                        "Chame de novo com date em YYYY-MM-DD, por exemplo date: \"{}\".",
                        today.format("%Y-%m-%d")
                    ),
                )
            })?
            .format("%Y-%m-%d")
            .to_string(),
    };

    // Texto em branco é ausência escrita por extenso: gravá-lo criaria uma descrição vazia que a
    // tela desenharia como se algo tivesse sido dito.
    let trimmed = |key| -> Result<Option<String>, ToolError> {
        Ok(args
            .text(key)?
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string))
    };

    Ok(Payload {
        kind,
        amount_cents,
        date,
        description: trimmed("description")?,
        payment_method: trimmed("payment_method")?,
        is_fixed: args.flag("is_fixed")?,
        tag_ids: args.strings("tag_ids")?,
    })
}

/// Confere que cada tag citada existe. Uma tag inventada mudaria as réguas em que o lançamento
/// conta — ela é interruptor de contabilidade, não rótulo — e o erro cita a que faltou.
async fn check_tags(pool: &SqlitePool, tag_ids: &[String]) -> Result<(), ToolError> {
    for id in tag_ids {
        let found: Option<(String,)> = sqlx::query_as("SELECT id FROM tag WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ToolError::read_failed(format!("conferir a tag: {e}")))?;
        if found.is_none() {
            return Err(ToolError::new(
                ErrorCode::InvalidArgument,
                format!("Não existe a tag \"{id}\"."),
                "Chame get_tags para ver as tags que existem, e proponha só com os identificadores \
                 que ela devolver — ou sem tag_ids."
                    .to_string(),
            ));
        }
    }
    Ok(())
}

/// A assinatura da proposta: SHA-256 sobre uma serialização de ORDEM FIXA de `{schema_version,
/// payload, data_revision}`.
///
/// A ordem é fixada por construção (pares em vetor, nunca objeto) porque a assinatura precisa ser
/// a mesma nos dois momentos que a computam — a emissão e a aprovação — e um formato cuja ordem
/// dependesse da biblioteca faria a proposta caducar por motivo nenhum.
fn sign(payload: &Payload, data_revision: &str) -> String {
    let canonical = json!([
        ["schema_version", SCHEMA_VERSION],
        [
            "payload",
            [
                ["kind", Value::String(payload.kind.clone())],
                ["amount_cents", Value::from(payload.amount_cents)],
                ["date", Value::String(payload.date.clone())],
                ["description", Value::from(payload.description.clone())],
                [
                    "payment_method",
                    Value::from(payload.payment_method.clone())
                ],
                ["is_fixed", Value::Bool(payload.is_fixed)],
                ["tag_ids", Value::from(payload.tag_ids.clone())],
            ]
        ],
        ["data_revision", Value::String(data_revision.to_string())],
    ])
    .to_string();

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// O corpo guardado no ledger. O identificador fica de fora: ele é a chave da própria linha, e
/// duplicá-lo aqui abriria a chance de as duas cópias discordarem.
fn body(payload: &Payload, data_revision: &str, issued_at: &str, expires_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "payload": payload.to_json(),
        "data_revision": data_revision,
        "issued_at": issued_at,
        "expires_at": expires_at,
        "hash": sign(payload, data_revision),
    })
}

fn expires_at(clock: &Clock) -> String {
    (clock.now() + Duration::minutes(TTL_MINUTES)).to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub(crate) async fn propose_transaction(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
    ctx: &Context,
    revision: Option<&str>,
) -> ToolResult {
    // Sem revisão dos dados não há proposta: é ela que amarra o lançamento ao mundo que o
    // originou, e uma assinatura sem esse laço aprovaria sobre um mundo qualquer.
    let revision = revision.ok_or_else(|| {
        ToolError::read_failed("a revisão dos dados não pôde ser lida para assinar a proposta")
    })?;

    let payload = parse_payload(args, today)?;
    check_tags(pool, &payload.tag_ids).await?;

    let issued_at = ctx.clock.as_of();
    let body = body(&payload, revision, &issued_at, &expires_at(&ctx.clock));

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO mia_proposal_ledger (conversation_id, proposal_json, proposal_hash, decision, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
    )
    .bind(ctx.conversation_id)
    .bind(body.to_string())
    .bind(body["hash"].as_str().unwrap_or_default())
    .bind(PROPOSED)
    .bind(&issued_at)
    .fetch_one(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("registrar a proposta: {e}")))?;

    let mut proposal = body;
    super::insert(&mut proposal, "id", id);

    let date = NaiveDate::parse_from_str(&payload.date, "%Y-%m-%d").unwrap_or(today);
    Ok(ToolOutput {
        period: Period::day(date),
        data: json!({ "proposal": proposal }),
    })
}

async fn decide(pool: &SqlitePool, id: i64, decision: &str) -> Result<(), String> {
    sqlx::query("UPDATE mia_proposal_ledger SET decision = ?1 WHERE id = ?2")
        .bind(decision)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("registrar a decisão da proposta: {e}"))?;
    Ok(())
}

/// Cria o lançamento de uma proposta, depois de revalidar tudo o que a emissão prometeu.
///
/// A ordem das conferências é a ordem em que elas ficam mais baratas de explicar: decisão já
/// tomada, validade vencida, mundo mudado, assinatura que não corresponde ao payload. Só depois
/// de todas o helper de lançamento é chamado — e é ele, não uma cópia da régua, que decide se o
/// lançamento é válido.
pub(crate) async fn approve(
    pool: &SqlitePool,
    id: i64,
    payload_json: &str,
    hash: &str,
    now: DateTime<FixedOffset>,
) -> Result<String, String> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT proposal_json, proposal_hash, decision FROM mia_proposal_ledger WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("ler a proposta: {e}"))?;

    let Some((stored, stored_hash, decision)) = row else {
        return Err("Esta proposta não existe mais. Peça a proposta de novo na conversa.".into());
    };
    if decision.as_deref() != Some(PROPOSED) {
        return Err("Esta proposta já foi decidida. Peça a proposta de novo na conversa.".into());
    }

    let stored: Value =
        serde_json::from_str(&stored).map_err(|e| format!("ler a proposta guardada: {e}"))?;
    let stored_revision = stored["data_revision"].as_str().unwrap_or_default();

    let expires = stored["expires_at"]
        .as_str()
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .ok_or_else(|| format!("a proposta {id} não guarda validade legível"))?;
    if now > expires {
        decide(pool, id, EXPIRED).await?;
        return Err(
            "Esta proposta venceu. Peça a proposta de novo na conversa para aprovar sobre \
                    os números de agora."
                .into(),
        );
    }

    // O mundo precisa ser o mesmo que assinou a proposta. Mudou embaixo dela — uma importação, um
    // lançamento pelo formulário —, e o que a pessoa leu no cartão já não descreve o que aprovaria.
    let current = data_revision(pool)
        .await
        .map_err(|e| format!("conferir a revisão dos dados: {e}"))?;
    if current != stored_revision {
        decide(pool, id, EXPIRED).await?;
        return Err(
            "Seus dados mudaram depois desta proposta. Peça a proposta de novo na \
                    conversa para aprovar sobre os números de agora."
                .into(),
        );
    }

    // A assinatura recebida identifica QUAL proposta o gesto aprova — a que a pessoa leu no
    // cartão, intacta desde a emissão. O payload pode chegar editado: quem o autoriza não é a
    // assinatura, é a revalidação pela mesma régua logo abaixo, mais o próprio gesto.
    if hash != stored_hash {
        return Err(
            "Esta assinatura não corresponde à proposta. Peça a proposta de novo na conversa."
                .into(),
        );
    }
    let payload = payload_from_json(payload_json)?;
    check_tags(pool, &payload.tag_ids)
        .await
        .map_err(|error| format!("{} {}", error.message, error.fix))?;

    // A régua do lançamento é a do formulário, chamada — nunca copiada. Nada aqui escreve na
    // planilha: a convergência segue pelo write-back, com diff e aprovação próprios.
    let transaction_id = create_transaction_inner(
        pool,
        &payload.kind,
        payload.amount_cents,
        payload.description.clone(),
        &payload.date,
        payload.payment_method.clone(),
        payload.is_fixed,
        &payload.tag_ids,
        None,
        None,
        None,
    )
    .await?;

    let issued_at = stored["issued_at"].as_str().unwrap_or_default();
    let expires_at = stored["expires_at"].as_str().unwrap_or_default();
    let final_body = body(&payload, stored_revision, issued_at, expires_at);
    sqlx::query(
        "UPDATE mia_proposal_ledger \
         SET decision = ?1, transaction_id = ?2, proposal_json = ?3, proposal_hash = ?4 \
         WHERE id = ?5",
    )
    .bind(APPROVED)
    .bind(&transaction_id)
    .bind(final_body.to_string())
    .bind(final_body["hash"].as_str().unwrap_or_default())
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("registrar a aprovação da proposta: {e}"))?;

    Ok(transaction_id)
}

/// Registra a recusa. A proposta permanece no ledger: o que a pessoa NÃO quis é parte da
/// proveniência tanto quanto o que ela quis.
pub(crate) async fn reject(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let decision: Option<(Option<String>,)> =
        sqlx::query_as("SELECT decision FROM mia_proposal_ledger WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("ler a proposta: {e}"))?;
    match decision {
        None => Err("Esta proposta não existe mais.".into()),
        Some((decision,)) if decision.as_deref() != Some(PROPOSED) => {
            Err("Esta proposta já foi decidida. Peça a proposta de novo na conversa.".into())
        }
        Some(_) => decide(pool, id, REJECTED).await,
    }
}

/// O payload que volta da tela, lido pela MESMA régua da emissão — inclusive o escopo estreito.
/// A tela é dado não confiável como qualquer outro: um campo editado nela passa pela validação
/// inteira, não por uma versão relaxada dela.
fn payload_from_json(raw: &str) -> Result<Payload, String> {
    let value: Value = serde_json::from_str(raw).map_err(|e| format!("ler a proposta: {e}"))?;
    let spec = super::catalog::spec(super::catalog::PROPOSAL_TOOL)
        .expect("a ferramenta de proposta está no catálogo");
    let args =
        Args::parse(spec, &value).map_err(|error| format!("{} {}", error.message, error.fix))?;
    parse_payload(&args, chrono::Local::now().date_naive())
        .map_err(|error| format!("{} {}", error.message, error.fix))
}
