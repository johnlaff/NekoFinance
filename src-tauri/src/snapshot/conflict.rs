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

/// A base em comum entre os dois aparelhos não tem uma coluna própria em `snapshot_state` — é
/// aproximada pelo timestamp mais recente entre o último check-in e o último check-out DESTE
/// aparelho, os dois únicos gestos que avançam `base_sequence`. `adopt_own_sequence` (o check-in
/// que morreu entre o upload confirmado e a gravação local) é a única exceção: avança a base sem
/// tocar nenhum dos dois timestamps, então a âncora fica um pouco mais ANTIGA que a base real
/// nesse caso raro — a lista de gestos vem levemente mais generosa (inclui alguns gestos já
/// publicados por este mesmo aparelho), nunca mais estreita a ponto de esconder um gesto de fato
/// em risco. Comparação lexicográfica: os dois produtores em produção usam
/// `chrono::Utc::now().to_rfc3339()`, cuja ordem textual já é a ordem cronológica.
pub(crate) fn base_anchor(
    last_checkin_at: Option<&str>,
    last_checkout_at: Option<&str>,
) -> Option<String> {
    match (last_checkin_at, last_checkout_at) {
        (None, None) => None,
        (Some(a), None) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (Some(a), Some(b)) => Some(if a >= b { a.to_string() } else { b.to_string() }),
    }
}

/// Gestos do `sync_log` deste aparelho desde `since` (exclusive), em ordem cronológica —
/// `since: None` devolve TODOS (primeira base deste aparelho, nada a excluir ainda).
///
/// A comparação normaliza os dois lados com `datetime()` do SQLite em vez de comparar as strings
/// cruas: `sync_log.timestamp` vem de `datetime('now')` ("YYYY-MM-DD HH:MM:SS", espaço) enquanto
/// `since` costuma vir de `chrono::Utc::now().to_rfc3339()` ("...T...+00:00"). Comparar os
/// literais byte a byte falharia sempre (o espaço de um vem ANTES do "T" do outro na tabela
/// ASCII, então toda linha do mesmo dia pareceria mais antiga que qualquer âncora RFC3339,
/// mesmo quando não é) — `datetime()` entende os dois formatos e normaliza antes de comparar.
pub(crate) async fn gestures_since(
    pool: &SqlitePool,
    since: Option<&str>,
) -> Result<Vec<ConflictGesture>, String> {
    let rows: Vec<(String, String, String, Option<String>)> = match since {
        Some(ts) => sqlx::query_as(
            "SELECT timestamp, event_type, entity_type, source_sheet FROM sync_log \
             WHERE datetime(timestamp) > datetime(?1) ORDER BY datetime(timestamp) ASC",
        )
        .bind(ts)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("ler gestos do sync_log: {e}"))?,
        None => sqlx::query_as(
            "SELECT timestamp, event_type, entity_type, source_sheet FROM sync_log \
             ORDER BY datetime(timestamp) ASC",
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
    since: Option<&str>,
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

    #[test]
    fn base_anchor_picks_the_later_of_checkin_and_checkout_or_none_when_neither_exists() {
        for (label, checkin, checkout, expected) in [
            (
                "nenhum dos dois: aparelho nunca sincronizou",
                None,
                None,
                None,
            ),
            (
                "só check-in: aparelho que só publicou",
                Some("2026-08-11T10:00:00Z"),
                None,
                Some("2026-08-11T10:00:00Z"),
            ),
            (
                "só check-out: aparelho que só puxou",
                None,
                Some("2026-08-11T10:00:00Z"),
                Some("2026-08-11T10:00:00Z"),
            ),
            (
                "check-in mais recente que o check-out",
                Some("2026-08-12T09:00:00Z"),
                Some("2026-08-10T09:00:00Z"),
                Some("2026-08-12T09:00:00Z"),
            ),
            (
                "check-out mais recente que o check-in",
                Some("2026-08-10T09:00:00Z"),
                Some("2026-08-12T09:00:00Z"),
                Some("2026-08-12T09:00:00Z"),
            ),
        ] {
            assert_eq!(
                base_anchor(checkin, checkout),
                expected.map(str::to_string),
                "caso: {label}"
            );
        }
    }

    #[tokio::test]
    async fn gestures_since_none_returns_every_gesture_in_chronological_order() {
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
        // Ordem cronológica, não ordem de inserção: o import (11) vem antes do write-back (12).
        assert_eq!(gestures[0].event_type, "import");
        assert_eq!(gestures[0].source_sheet.as_deref(), Some("Diário"));
        assert_eq!(gestures[1].event_type, "write_back");
        assert_eq!(gestures[1].source_sheet.as_deref(), Some("Saídas"));
    }

    #[tokio::test]
    async fn gestures_since_some_excludes_gestures_at_or_before_the_anchor() {
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

        let gestures = gestures_since(&pool, Some("2026-08-11 09:00:00"))
            .await
            .expect("gestures_since");
        assert_eq!(gestures.len(), 1, "gesto NO instante da âncora não conta");
        assert_eq!(gestures[0].event_type, "write_back");
    }

    #[tokio::test]
    async fn gestures_since_normalizes_the_two_timestamp_formats_the_producers_actually_emit() {
        // `sync_log.timestamp` sai de `datetime('now')` (espaço); a âncora real (`base_anchor`)
        // sai de `chrono::Utc::now().to_rfc3339()` ("T" + offset). Comparar os literais crus
        // falharia SEMPRE para uma linha do mesmo dia (o espaço vem antes do "T" na tabela
        // ASCII) — esta é a regressão que `datetime()` na query evita.
        let pool = single_connection_pool().await;
        let profile_id = seed_profile(&pool).await;
        seed_gesture(
            &pool,
            &profile_id,
            "2026-08-12 10:00:00",
            "write_back",
            "transaction",
            None,
        )
        .await;

        let gestures = gestures_since(&pool, Some("2026-08-12T09:00:00.000000+00:00"))
            .await
            .expect("gestures_since");
        assert_eq!(
            gestures.len(),
            1,
            "a linha das 10h deve contar como POSTERIOR à âncora das 9h, apesar dos formatos \
             diferentes"
        );
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
