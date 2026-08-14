//! Extração dos gestos de CADA lado para a tela de conflito (ADR-0015): quando o árbitro
//! (`lease::decide`) devolve `Conflict`, a escolha do dono é simétrica (manter este aparelho ou
//! usar o outro) — então o dono vê o que se perde nos dois sentidos, cada lista lida do
//! `sync_log` do lado correspondente, nunca reconstruída.
//!
//! O `sync_log` hoje só é escrito pelo import/write-back da planilha (`event_type` "import" ou
//! "write_back") — a extração é agnóstica a isso e lê o que houver, sem assumir domínio: se um
//! gesto de domínio (split, tag, teto) passar a gravar ali no futuro, aparece aqui sem mudança
//! nenhuma nesta função.

use sqlx::SqlitePool;
use std::path::Path;

/// Um gesto do `sync_log`, já na forma que a tela de conflito precisa: quando aconteceu, que tipo
/// de gesto foi, sobre qual entidade, e (quando veio da planilha) de qual aba.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConflictGesture {
    pub at: String,
    pub event_type: String,
    pub entity_type: String,
    pub source_sheet: Option<String>,
}

/// Gestos do `sync_log` deste aparelho desde `since` (exclusive), em ordem de inserção — `since:
/// None` devolve TODOS (primeira base deste aparelho, nada a excluir ainda).
///
/// Corte por SEQUÊNCIA (`sync_log.seq`), nunca por timestamp (ADR-0015, issue #446 D3 do PR #447):
/// a versão anterior comparava `sync_log.timestamp` do OUTRO aparelho contra uma âncora derivada
/// do relógio DESTE — um relógio remoto atrasado escondia gestos recentes dele, e a lista ficava
/// mais estreita que a verdade. `seq` é um contador monotônico gravado NA LINHA (nunca o rowid
/// implícito do SQLite, que `VACUUM INTO` pode renumerar para tabelas sem `INTEGER PRIMARY KEY` —
/// `sync_log.id` é `TEXT`, o rowid não é estável através do export) — sobrevive intacto ao
/// `VACUUM INTO`/download porque é um valor de COLUNA. `since` vem de
/// `SnapshotState::base_sync_log_seq`, capturado como `MAX(seq)` no momento em que os dois
/// aparelhos eram bytes idênticos (o último sync) — o MESMO valor, com o MESMO significado, nos
/// dois lados, sem depender de qual relógio está certo.
pub(crate) async fn gestures_since(
    pool: &SqlitePool,
    since: Option<i64>,
) -> Result<Vec<ConflictGesture>, String> {
    let rows: Vec<(String, String, String, Option<String>)> = match since {
        Some(seq) => sqlx::query_as(
            "SELECT timestamp, event_type, entity_type, source_sheet FROM sync_log \
             WHERE seq > ?1 ORDER BY seq ASC",
        )
        .bind(seq)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("ler gestos do sync_log: {e}"))?,
        None => sqlx::query_as(
            "SELECT timestamp, event_type, entity_type, source_sheet FROM sync_log \
             ORDER BY seq ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("ler gestos do sync_log: {e}"))?,
    };

    Ok(rows
        .into_iter()
        .map(
            |(at, event_type, entity_type, source_sheet)| ConflictGesture {
                at,
                event_type,
                entity_type,
                source_sheet,
            },
        )
        .collect())
}

/// Espelho de `gestures_since`, mas contra um arquivo JÁ BAIXADO em vez do pool ativo — o jeito de
/// ler o `sync_log` do OUTRO aparelho antes do dono escolher, sem nunca migrar nem escrever nele
/// (mesmo perfil `read_only` de conexão que `restore::validate_downloaded_db` usa para o mesmo
/// arquivo). Nunca substitui o banco ativo — essa troca só acontece se o dono escolher usar o
/// outro aparelho, em `resolve_conflict_use_remote_core`.
pub(crate) async fn gestures_since_in_file(
    path: &Path,
    since: Option<i64>,
) -> Result<Vec<ConflictGesture>, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
        .map_err(|e| format!("abrir snapshot do outro aparelho: {e}"))?
        .read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| format!("abrir snapshot do outro aparelho: {e}"))?;
    let result = gestures_since(&pool, since).await;
    pool.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::time::Duration;

    async fn single_connection_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool SQLite em memória");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações");
        pool
    }

    /// `sync_log.profile_id` referencia `profile(id)`, que por sua vez referencia `person(id)` —
    /// semeia os dois antes de semear gestos, senão a FK rejeita o insert.
    async fn seed_profile(pool: &SqlitePool) -> String {
        let person_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Dono')")
            .bind(&person_id)
            .execute(pool)
            .await
            .expect("semear pessoa");

        let profile_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
            .bind(&profile_id)
            .bind(&person_id)
            .execute(pool)
            .await
            .expect("semear perfil");
        profile_id
    }

    #[allow(clippy::too_many_arguments)]
    async fn seed_gesture(
        pool: &SqlitePool,
        profile_id: &str,
        timestamp: &str,
        event_type: &str,
        entity_type: &str,
        source_sheet: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, \
             timestamp, source_sheet) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(event_type)
        .bind(entity_type)
        .bind("entidade-qualquer")
        .bind(profile_id)
        .bind(timestamp)
        .bind(source_sheet)
        .execute(pool)
        .await
        .expect("semear gesto");
    }

    #[tokio::test]
    async fn gestures_since_none_returns_every_gesture_in_insertion_order() {
        let pool = single_connection_pool().await;
        let profile_id = seed_profile(&pool).await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-12 10:00:00",
            "write_back",
            "transaction",
            Some("Saídas"),
        )
        .await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-11 09:00:00",
            "import",
            "transaction",
            Some("Diário"),
        )
        .await;

        let gestures = gestures_since(&pool, None).await.expect("gestures_since");
        assert_eq!(gestures.len(), 2);
        // Ordem de INSERÇÃO (`seq`, o gatilho `sync_log_assign_seq`), não ordem cronológica do
        // `timestamp`: o write-back foi inserido primeiro (mesmo com timestamp mais recente).
        assert_eq!(gestures[0].event_type, "write_back");
        assert_eq!(gestures[0].source_sheet.as_deref(), Some("Saídas"));
        assert_eq!(gestures[1].event_type, "import");
        assert_eq!(gestures[1].source_sheet.as_deref(), Some("Diário"));
    }

    #[tokio::test]
    async fn gestures_since_some_excludes_gestures_at_or_before_the_anchor_sequence() {
        let pool = single_connection_pool().await;
        let profile_id = seed_profile(&pool).await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-11 09:00:00",
            "import",
            "transaction",
            None,
        )
        .await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-12 10:00:00",
            "write_back",
            "transaction",
            None,
        )
        .await;

        // O import acima foi o PRIMEIRO gesto inserido — o gatilho `sync_log_assign_seq` deu a
        // ele `seq = 1`. A âncora `Some(1)` é "a base ficou exatamente no ponto do import".
        let gestures = gestures_since(&pool, Some(1))
            .await
            .expect("gestures_since");
        assert_eq!(gestures.len(), 1, "gesto NA sequência da âncora não conta");
        assert_eq!(gestures[0].event_type, "write_back");
    }

    #[tokio::test]
    async fn gestures_since_never_hides_a_gesture_because_the_remote_clock_lags() {
        // A regressão que a âncora por sequência existe para fechar (ADR-0015, issue #446 D3 do
        // PR #447): um gesto com `timestamp` ANTERIOR à âncora (relógio do OUTRO aparelho
        // atrasado) precisa continuar visível se foi inserido DEPOIS do ponto de base — o corte
        // por `seq` nunca lê o valor de `timestamp` para decidir inclusão, só a ordem real de
        // inserção.
        let pool = single_connection_pool().await;
        let profile_id = seed_profile(&pool).await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-12 10:00:00",
            "import",
            "transaction",
            None,
        )
        .await; // seq = 1: este é o ponto de base.
        seed_gesture(
            &pool,
            &profile_id,
            // Timestamp ANTERIOR ao da base acima — um relógio remoto atrasado geraria isto na
            // prática. Um corte por `datetime(timestamp)` esconderia esta linha; o corte por
            // `seq` não.
            "2026-08-10 08:00:00",
            "write_back",
            "transaction",
            None,
        )
        .await; // seq = 2: inserido DEPOIS da base, mesmo com timestamp mais antigo.

        let gestures = gestures_since(&pool, Some(1))
            .await
            .expect("gestures_since");
        assert_eq!(
            gestures.len(),
            1,
            "o gesto pós-base aparece mesmo com timestamp mais antigo que a âncora"
        );
        assert_eq!(gestures[0].event_type, "write_back");
    }

    #[tokio::test]
    async fn gestures_since_empty_log_returns_an_empty_list_never_an_error() {
        let pool = single_connection_pool().await;
        let gestures = gestures_since(&pool, None).await.expect("gestures_since");
        assert!(gestures.is_empty());
    }

    #[tokio::test]
    async fn gestures_since_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        // Mesma classe de regressão já documentada em `state.rs`/`checkout.rs`: com pool de 1
        // conexão, ler o `sync_log` enquanto uma escrita mantém uma transação aberta precisa
        // ENFILEIRAR e completar assim que a tx solta a conexão, nunca travar para sempre.
        let pool = single_connection_pool().await;
        let profile_id = seed_profile(&pool).await;

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id) \
             VALUES (?1, 'import', 'transaction', 'e1', ?2)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&profile_id)
        .execute(&mut *tx)
        .await
        .expect("escrita dentro da transação");

        let pool_for_read = pool.clone();
        let read = tokio::spawn(async move { gestures_since(&pool_for_read, None).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(Duration::from_secs(5), read)
            .await
            .expect("a leitura NÃO pode travar para sempre esperando a única conexão")
            .expect("a task de leitura não deve entrar em panic");
        assert_eq!(result.expect("gestures_since").len(), 1);
    }

    #[tokio::test]
    async fn gestures_since_in_file_reads_the_sync_log_of_a_downloaded_file_read_only() {
        use std::str::FromStr;

        let dir = std::env::temp_dir().join(format!("neko-conflict-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("remote.db");

        let file_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations")
            .run(&file_pool)
            .await
            .unwrap();
        let profile_id = seed_profile(&file_pool).await;
        seed_gesture(
            &file_pool,
            &profile_id,
            "2026-08-11 09:00:00",
            "import",
            "transaction",
            Some("Diário"),
        )
        .await;
        file_pool.close().await;

        let gestures = gestures_since_in_file(&path, None)
            .await
            .expect("gestures_since_in_file");
        assert_eq!(gestures.len(), 1);
        assert_eq!(gestures[0].event_type, "import");

        std::fs::remove_dir_all(&dir).ok();
    }
}
