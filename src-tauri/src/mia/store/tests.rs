use super::*;
use crate::mia::provider::pins::default_pin;
use crate::mia::run::{RunOutcome, StopReason, TraceEntry, TraceKind, redaction};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;

/// O pool de teste tem UMA conexão, como o de produção: com duas, uma leitura feita durante uma
/// transação de escrita passa por outra conexão e o deadlock que o app veria some do teste.
async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("o pool SQLite em memória deve abrir");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("as migrações devem preparar o pool de teste");
    pool
}

fn outcome(transcript: Vec<Value>, trace: Vec<TraceEntry>) -> RunOutcome {
    RunOutcome {
        answer: Some("A economia do ano está em 22%.".to_string()),
        provenance: None,
        stop: StopReason::Answered,
        turns: 2,
        tool_calls: 1,
        cost_micro_usd: 4_200,
        cost_declared: true,
        attempts: 3,
        prompt_tokens: 1_200,
        completion_tokens: 340,
        transcript,
        trace,
    }
}

/// A conversa é uma só: pedir de novo devolve a mesma, senão cada abertura do app começaria uma
/// conversa nova e o histórico ficaria espalhado por linhas que ninguém lê.
#[tokio::test]
async fn a_conversa_ativa_e_criada_uma_vez_e_reencontrada() {
    let pool = pool().await;

    let first = active_conversation(&pool).await.expect("deve criar");
    let second = active_conversation(&pool).await.expect("deve reencontrar");

    assert_eq!(first, second);
}

/// O transcript nativo sobrevive à rodada: é ele que reidrata o histórico da próxima.
#[tokio::test]
async fn o_transcript_da_rodada_reidrata_o_historico() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");
    let transcript = vec![
        json!({"role": "user", "content": "Quanto economizei?"}),
        json!({"role": "assistant", "content": "22%."}),
    ];

    save_round(
        &pool,
        conversation,
        "run-1",
        default_pin(),
        &outcome(transcript.clone(), vec![]),
    )
    .await
    .expect("deve gravar");

    assert_eq!(
        load_history(&pool, conversation).await.expect("deve reler"),
        transcript
    );
}

/// A mensagem visível chega pelo gesto da tela e volta na ordem em que foi dita.
#[tokio::test]
async fn as_mensagens_visiveis_voltam_em_ordem() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");

    append_exchange(&pool, conversation, "Primeira?", r#"{"text":"Uma."}"#)
        .await
        .expect("deve gravar");
    append_exchange(&pool, conversation, "Segunda?", r#"{"text":"Duas."}"#)
        .await
        .expect("deve gravar");

    let messages = visible_messages(&pool, conversation)
        .await
        .expect("deve reler");

    let ditos: Vec<_> = messages.iter().map(|m| m.author.as_str()).collect();
    assert_eq!(ditos, ["voce", "mia", "voce", "mia"]);
    assert_eq!(messages[0].question.as_deref(), Some("Primeira?"));
    assert_eq!(messages[1].answer, Some(json!({"text": "Uma."})));
    assert_eq!(messages[2].question.as_deref(), Some("Segunda?"));
    assert_eq!(messages[3].answer, Some(json!({"text": "Duas."})));
}

/// O rastro registra a conta inteira da rodada: para onde foi, quanto custou, quantas tentativas
/// e o que falhou no caminho.
#[tokio::test]
async fn o_rastro_registra_tokens_custo_provedor_tentativas_e_erros() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");
    let trace = vec![TraceEntry {
        turn: 1,
        attempt: 2,
        kind: TraceKind::ProviderFailure,
        detail: "HTTP 503".to_string(),
    }];

    save_round(
        &pool,
        conversation,
        "run-7",
        default_pin(),
        &outcome(vec![], trace),
    )
    .await
    .expect("deve gravar");

    let (payload,): (String,) =
        sqlx::query_as("SELECT payload_json FROM mia_round_trace WHERE round_id = ?1")
            .bind("run-7")
            .fetch_one(&pool)
            .await
            .expect("o rastro deve existir");
    let payload: Value = serde_json::from_str(&payload).expect("o rastro é JSON");

    assert_eq!(payload["model"], json!(default_pin().model));
    assert_eq!(payload["endpoint"], json!(default_pin().endpoint));
    assert_eq!(payload["operator"], json!(default_pin().operator));
    assert_eq!(payload["prompt_tokens"], json!(1_200));
    assert_eq!(payload["completion_tokens"], json!(340));
    assert_eq!(payload["cost_micro_usd"], json!(4_200));
    assert_eq!(payload["cost_declared"], json!(true));
    assert_eq!(payload["attempts"], json!(3));
    assert_eq!(payload["turns"], json!(2));
    assert_eq!(payload["tool_calls"], json!(1));
    assert_eq!(payload["stop"], json!("answered"));
    assert_eq!(payload["entries"][0]["kind"], json!("provider_failure"));
    assert_eq!(payload["entries"][0]["detail"], json!("HTTP 503"));
    assert_eq!(payload["entries"][0]["turn"], json!(1));
    assert_eq!(payload["entries"][0]["attempt"], json!(2));
}

/// A chave nunca chega ao banco. Quem a apaga é o redator, na borda do laço; o que este teste
/// prova é que o rastro gravado é aquele texto, e não o cru que o provedor devolveu.
#[tokio::test]
async fn a_chave_nunca_entra_no_rastro() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");
    let key = "sk-or-v1-fixture1234567890";
    let trace = vec![TraceEntry {
        turn: 1,
        attempt: 1,
        kind: TraceKind::ProviderFailure,
        detail: redaction::credentials(&format!("HTTP 401 authorization: Bearer {key}")),
    }];

    save_round(
        &pool,
        conversation,
        "run-9",
        default_pin(),
        &outcome(vec![], trace),
    )
    .await
    .expect("deve gravar");

    let (payload,): (String,) =
        sqlx::query_as("SELECT payload_json FROM mia_round_trace WHERE round_id = ?1")
            .bind("run-9")
            .fetch_one(&pool)
            .await
            .expect("o rastro deve existir");

    assert!(!payload.contains(key));
    assert!(!payload.contains("sk-or-v1"));
}

/// Trinta dias é a retenção do rastro. O que passou some sozinho; o que não passou fica.
#[tokio::test]
async fn a_purga_remove_o_rastro_vencido_e_preserva_o_vigente() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");
    let now = Utc::now();
    for (round_id, age) in [("velho", 31), ("novo", 29)] {
        sqlx::query(
            "INSERT INTO mia_round_trace (conversation_id, round_id, payload_json, created_at) VALUES (?1, ?2, '{}', ?3)",
        )
        .bind(conversation)
        .bind(round_id)
        .bind((now - Duration::days(age)).to_rfc3339())
        .execute(&pool)
        .await
        .expect("o fixture deve gravar");
    }

    purge_stale_traces(&pool, now).await.expect("deve purgar");

    let restantes: Vec<(String,)> = sqlx::query_as("SELECT round_id FROM mia_round_trace")
        .fetch_all(&pool)
        .await
        .expect("deve reler");
    assert_eq!(restantes.len(), 1);
    assert_eq!(restantes[0].0, "novo");
}

/// Apagar a conversa apaga o que é da conversa — e só isso. A proveniência de um lançamento
/// aprovado é histórico financeiro: ela fica, órfã da conversa que a originou.
#[tokio::test]
async fn apagar_a_conversa_leva_mensagens_e_rastro_e_preserva_o_ledger() {
    let pool = pool().await;
    let conversation = active_conversation(&pool).await.expect("deve criar");
    append_exchange(&pool, conversation, "Pergunta?", r#"{"text":"Resposta."}"#)
        .await
        .expect("deve gravar");
    save_round(
        &pool,
        conversation,
        "run-3",
        default_pin(),
        &outcome(
            vec![json!({"role": "user", "content": "Pergunta?"})],
            vec![],
        ),
    )
    .await
    .expect("deve gravar");
    sqlx::query(
        "INSERT INTO mia_proposal_ledger (conversation_id, proposal_json, proposal_hash, decision, transaction_id, created_at) VALUES (?1, '{}', 'abc', 'aprovada', 'txn-1', ?2)",
    )
    .bind(conversation)
    .bind(Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .expect("o fixture deve gravar");

    delete_conversation(&pool, conversation)
        .await
        .expect("deve apagar");

    for (table, sql) in [
        ("mia_message", "SELECT COUNT(*) FROM mia_message"),
        ("mia_round_trace", "SELECT COUNT(*) FROM mia_round_trace"),
        ("mia_conversation", "SELECT COUNT(*) FROM mia_conversation"),
    ] {
        let (count,): (i64,) = sqlx::query_as(sql)
            .fetch_one(&pool)
            .await
            .expect("deve contar");
        assert_eq!(count, 0, "{table} deve ficar vazia");
    }

    let (ledger, linked): (i64, Option<i64>) =
        sqlx::query_as("SELECT COUNT(*), MAX(conversation_id) FROM mia_proposal_ledger")
            .fetch_one(&pool)
            .await
            .expect("deve reler");
    assert_eq!(ledger, 1);
    assert_eq!(linked, None);
}

/// O teto da janela é medido no histórico, não adivinhado: abaixo dele a rodada corre, acima ela
/// é recusada — a v1 avisa em vez de resumir a conversa por conta própria.
#[test]
fn o_teto_da_janela_recusa_so_o_historico_que_passa_do_limite() {
    let curto = vec![json!({"role": "user", "content": "Quanto economizei?"})];
    let longo = vec![json!({
        "role": "user",
        "content": "x".repeat(MAX_HISTORY_TOKENS * CHARS_PER_TOKEN + 1),
    })];

    assert!(history_chars(&curto) < MAX_HISTORY_TOKENS * CHARS_PER_TOKEN);
    assert!(!window_exceeded(&curto));
    assert!(window_exceeded(&longo));
}
