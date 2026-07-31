//! A conversa que dura — e o que sobrevive a apagá-la.
//!
//! Três durabilidades diferentes, de propósito. O que a pessoa lê e o rastro técnico da rodada
//! pertencem à conversa e morrem com ela; a proveniência de um lançamento aprovado é histórico
//! financeiro e fica. Quem garante isso é a cascata declarada no esquema, não um caminho de
//! aplicação que alguém pode esquecer de chamar: apagar é um DELETE só.
//!
//! O histórico reenviado ao provedor sai do transcript NATIVO, guardado inteiro. Reconstruí-lo das
//! mensagens visíveis perderia as chamadas de ferramenta e os envelopes, e a próxima rodada
//! receberia uma conversa que não aconteceu.
//!
//! Toda escrita respeita o pool de uma conexão do app: leitura necessária a uma gravação acontece
//! ANTES da transação abrir, porque ler em outra conexão com uma escrita em curso é esperar por
//! uma conexão que não existe.

use super::provider::pins::ModelPin;
use super::run::{RunOutcome, TraceEntry, TraceKind};
use super::screen_events::stop_reason;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// A retenção do rastro técnico. Passado esse prazo ele some sozinho: o que ele serve — depurar
/// uma rodada recente — não sobrevive ao mês, e guardar além disso é acumular sem uso.
const TRACE_RETENTION_DAYS: i64 = 30;

/// Quantos caracteres valem um token, por estimativa. Grosseira de propósito: o teto existe para
/// avisar antes de o provedor recusar, e uma contagem exata exigiria o tokenizador do modelo.
pub(crate) const CHARS_PER_TOKEN: usize = 4;

/// O teto da janela da conversa, em tokens estimados. Conservador em relação à janela real dos
/// pins porque o histórico é só uma parte do que a rodada envia — prompt de sistema, envelopes de
/// ferramenta e resposta disputam o mesmo espaço.
pub(crate) const MAX_HISTORY_TOKENS: usize = 150_000;

/// Uma linha da conversa como a tela a desenha. `answer` é opaco para o backend: quem reduz os
/// eventos da rodada à resposta visível é a interface, e uma segunda definição desse formato aqui
/// existiria só para divergir daquela.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct StoredMessage {
    pub author: String,
    pub question: Option<String>,
    pub answer: Option<Value>,
    pub at_iso: String,
}

/// A conversa única desta instalação, criada na primeira vez que alguém a pede.
pub(crate) async fn active_conversation(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM mia_conversation ORDER BY id")
        .fetch_optional(pool)
        .await?;
    if let Some((id,)) = existing {
        return Ok(id);
    }

    let (id,): (i64,) =
        sqlx::query_as("INSERT INTO mia_conversation (created_at) VALUES (?1) RETURNING id")
            .bind(now())
            .fetch_one(pool)
            .await?;
    Ok(id)
}

/// O transcript nativo guardado, pronto para reidratar a próxima rodada. Um transcript corrompido
/// vale como conversa vazia: a rodada seguinte perde o contexto, mas acontece — recusá-la deixaria
/// a conversa travada até alguém apagar o banco na mão.
pub(crate) async fn load_history(
    pool: &SqlitePool,
    conversation_id: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    let stored: Option<(String,)> =
        sqlx::query_as("SELECT runtime_transcript_json FROM mia_conversation WHERE id = ?1")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await?;
    Ok(stored
        .and_then(|(json,)| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

/// Grava o que a rodada produziu: o transcript que substitui o histórico e o rastro técnico dela.
///
/// A mensagem visível NÃO nasce aqui. Ela chega pelo gesto da tela, com o texto já reduzido ao que
/// a pessoa lê — gravá-la nos dois caminhos duplicaria a mesma linha na conversa.
pub(crate) async fn save_round(
    pool: &SqlitePool,
    conversation_id: i64,
    round_id: &str,
    pin: &ModelPin,
    outcome: &RunOutcome,
) -> Result<(), sqlx::Error> {
    let transcript = serde_json::to_string(&outcome.transcript)
        .expect("o transcript do provedor é serializável");
    let payload =
        serde_json::to_string(&trace_payload(pin, outcome)).expect("o rastro é serializável");
    let now = now();

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE mia_conversation SET runtime_transcript_json = ?1 WHERE id = ?2")
        .bind(transcript)
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO mia_round_trace (conversation_id, round_id, payload_json, created_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(conversation_id)
    .bind(round_id)
    .bind(payload)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// A conversa visível, na ordem em que foi dita.
pub(crate) async fn visible_messages(
    pool: &SqlitePool,
    conversation_id: i64,
) -> Result<Vec<StoredMessage>, sqlx::Error> {
    let rows: Vec<(String, Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT author, question, answer_json, at_iso FROM mia_message WHERE conversation_id = ?1 ORDER BY seq, id",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(author, question, answer_json, at_iso)| StoredMessage {
            author,
            question,
            answer: answer_json.and_then(|json| serde_json::from_str(&json).ok()),
            at_iso,
        })
        .collect())
}

/// Registra o par pergunta/resposta da tela. As duas linhas entram juntas: uma pergunta sem a
/// resposta que a acompanha reabriria a conversa num estado que a tela não sabe desenhar.
pub(crate) async fn append_exchange(
    pool: &SqlitePool,
    conversation_id: i64,
    question: &str,
    answer_json: &str,
) -> Result<(), sqlx::Error> {
    // A próxima posição é lida ANTES da transação: no pool de uma conexão, consultá-la com a
    // escrita já aberta esperaria por uma segunda conexão que nunca vem.
    let (next_seq,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq), -1) + 1 FROM mia_message WHERE conversation_id = ?1",
    )
    .bind(conversation_id)
    .fetch_one(pool)
    .await?;
    let now = now();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO mia_message (conversation_id, seq, author, question, at_iso) VALUES (?1, ?2, 'voce', ?3, ?4)",
    )
    .bind(conversation_id)
    .bind(next_seq)
    .bind(question)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO mia_message (conversation_id, seq, author, answer_json, at_iso) VALUES (?1, ?2, 'mia', ?3, ?4)",
    )
    .bind(conversation_id)
    .bind(next_seq + 1)
    .bind(answer_json)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Apaga a conversa de verdade. Um DELETE só: mensagens e rastro caem pela cascata, e o ledger de
/// propostas fica — órfão da conversa, nunca apagado com ela.
pub(crate) async fn delete_conversation(
    pool: &SqlitePool,
    conversation_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM mia_conversation WHERE id = ?1")
        .bind(conversation_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Descarta o rastro que passou da retenção.
pub(crate) async fn purge_stale_traces(
    pool: &SqlitePool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    let cutoff = (now - chrono::Duration::days(TRACE_RETENTION_DAYS)).to_rfc3339();
    sqlx::query("DELETE FROM mia_round_trace WHERE created_at < ?1")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(())
}

/// O tamanho do histórico em caracteres, medido sobre o JSON que de fato viaja.
pub(crate) fn history_chars(history: &[Value]) -> usize {
    history
        .iter()
        .map(|entry| entry.to_string().chars().count())
        .sum()
}

/// O histórico já não cabe na janela? Nada é resumido: a conversa avisa, e o gesto de apagar é a
/// saída — resumir por conta própria trocaria o que a pessoa disse por uma paráfrase nossa.
pub(crate) fn window_exceeded(history: &[Value]) -> bool {
    history_chars(history) > MAX_HISTORY_TOKENS * CHARS_PER_TOKEN
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// A conta da rodada num objeto só: para onde foi, quanto custou, quantas tentativas, o que
/// falhou. O detalhe de cada entrada já chega redigido do laço.
fn trace_payload(pin: &ModelPin, outcome: &RunOutcome) -> Value {
    json!({
        "model": pin.model,
        "endpoint": pin.endpoint,
        "operator": pin.operator,
        "prompt_tokens": outcome.prompt_tokens,
        "completion_tokens": outcome.completion_tokens,
        "cost_micro_usd": outcome.cost_micro_usd,
        "cost_declared": outcome.cost_declared,
        "attempts": outcome.attempts,
        "turns": outcome.turns,
        "tool_calls": outcome.tool_calls,
        "stop": stop_reason(outcome.stop),
        "entries": outcome.trace.iter().map(trace_entry).collect::<Vec<_>>(),
    })
}

fn trace_entry(entry: &TraceEntry) -> Value {
    json!({
        "turn": entry.turn,
        "attempt": entry.attempt,
        "kind": trace_kind(entry.kind),
        "detail": entry.detail,
    })
}

fn trace_kind(kind: TraceKind) -> &'static str {
    match kind {
        TraceKind::TurnStarted => "turn_started",
        TraceKind::ToolDispatched => "tool_dispatched",
        TraceKind::ToolRefused => "tool_refused",
        TraceKind::ProviderFailure => "provider_failure",
        TraceKind::Retry => "retry",
        TraceKind::Regeneration => "regeneration",
        TraceKind::Stopped => "stopped",
    }
}

#[cfg(test)]
mod tests;
