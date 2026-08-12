//! Orquestração do check-out ao abrir o app (ADR-0015): consulta o manifest remoto e, quando o
//! árbitro (`lease::decide`) devolve `Pull`, baixa, valida e troca o banco ativo atomicamente
//! ANTES de qualquer gesto do dono. `checkout_on_open` é o núcleo testável (recebe um
//! `DriveSnapshotClient` já pronto); `checkout_on_open_best_effort` é o gancho que `lib.rs` chama
//! de verdade, resolvendo token/escopo e silenciando qualquer motivo de NÃO tentar.

use super::{lease, restore, state, transport::DriveSnapshotClient};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// O que `checkout_on_open` fez, quando termina sem erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutOutcome {
    /// Remoto não avançou além da base local (ou nenhum snapshot foi publicado ainda) — nada
    /// para baixar. Cobre também o veredito `Push`/`Conflict` do árbitro: check-out só AGE no
    /// veredito `Pull`, os outros dois são assunto do check-in.
    NothingToDo,
    /// O snapshot remoto foi baixado, validado e trocou o banco ativo.
    Restored { safeguard_path: Option<PathBuf> },
    /// O manifest remoto tem schema mais nova que este app — restauração recusada, nada mudou.
    RefusedNewerSchema {
        local_schema: i64,
        remote_schema: i64,
    },
}

/// Pool sempre utilizável + o que aconteceu. `outcome: Err(_)` é um problema NÃO-FATAL (rede,
/// integridade do download) — `pool` continua sendo o MESMO recebido por `checkout_on_open`,
/// intocado; o chamador só loga e segue com ele.
pub struct CheckoutResult {
    pub pool: SqlitePool,
    pub outcome: Result<CheckoutOutcome, String>,
}

async fn local_schema_version(pool: &SqlitePool) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("ler versão do schema: {e}"))
}

/// Mesmas opções de conexão do `setup()` do app (`lib.rs`): WAL, `foreign_keys` explícito, pool de
/// UMA conexão (escritor único), migrado. Reusado tanto pela abertura inicial quanto pela
/// reabertura depois de uma restauração — a única fonte de como este app abre seu banco.
pub(crate) async fn open_migrated_pool(db_path: &Path) -> Result<SqlitePool, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
        .map_err(|e| format!("URL do banco: {e}"))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| format!("abrir o banco: {e}"))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("migrações do banco: {e}"))?;
    Ok(pool)
}

/// Núcleo testável: `pool` já migrado no arquivo `db_path`, `drive` já autenticado. Devolve
/// `Err` SÓ quando não sobra pool utilizável nenhum — a troca de arquivo teve sucesso, mas
/// reabrir uma conexão nela falhou. Este é o mesmo tipo de falha fatal que a abertura inicial do
/// banco já trata em `lib.rs` (diálogo nativo + abort); fora desse caso extremo, o retorno é
/// sempre `Ok(CheckoutResult)` com um pool pronto para uso.
pub async fn checkout_on_open(
    pool: SqlitePool,
    db_path: &Path,
    drive: &DriveSnapshotClient,
) -> Result<CheckoutResult, String> {
    let local_state = match state::load_or_init(&pool).await {
        Ok(s) => s,
        Err(e) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    let remote = match drive.fetch_manifest().await {
        Ok(m) => m,
        Err(e) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    // Check-out nunca publica: não há candidato local novo, só a pergunta "o remoto avançou além
    // da nossa base?" — o mesmo árbitro do check-in, com `local == base`. `Conflict` nunca surge
    // desta chamada (exigiria `local > base`, e aqui `local` é sempre igual a `base`); `UpToDate`
    // e `Push` surgem normalmente (`Push` cobre o remoto ausente/regredido — nada ali é mais novo
    // que a nossa base) e caem no mesmo `NothingToDo`: check-out só AGE no veredito `Pull`.
    let verdict = lease::decide(
        local_state.base_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    );
    if verdict != lease::LeaseVerdict::Pull {
        return Ok(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::NothingToDo),
        });
    }
    // Pull exige `remote_seq > base`, o que só é possível com um manifest presente.
    let remote_manifest =
        remote.expect("veredito Pull do árbitro implica manifest remoto presente");

    let local_schema = match local_schema_version(&pool).await {
        Ok(v) => v,
        Err(e) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };
    if remote_manifest.schema_version > local_schema {
        return Ok(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema,
                remote_schema: remote_manifest.schema_version,
            }),
        });
    }

    // `fetch_manifest` (acima) e `download_snapshot` (aqui) são duas chamadas HTTP separadas: se
    // outro aparelho publicar exatamente nesse intervalo, os bytes baixados podem já ser mais
    // novos que `remote_manifest.sequence`/`schema_version` — a sequência gravada no fim ficaria
    // um passo atrás do conteúdo de fato restaurado. Risco aceito: o próximo check-out re-detecta
    // o remoto como avançado (o manifest mais novo não bate com a base recém-gravada) e converge
    // sozinho — nunca perde dado nem trava, só repete o ciclo uma vez a mais.
    let db_bytes = match drive.download_snapshot().await {
        Ok(Some(bytes)) => bytes,
        // Veredito Pull mas o binário sumiu (só o manifest sobrou) — nada para restaurar.
        Ok(None) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Ok(CheckoutOutcome::NothingToDo),
            });
        }
        Err(e) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    let tmp_path = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("neko-checkout-{}.db", uuid::Uuid::new_v4()));
    if let Err(e) = tokio::fs::write(&tmp_path, &db_bytes).await {
        return Ok(CheckoutResult {
            pool,
            outcome: Err(format!("gravar snapshot baixado: {e}")),
        });
    }
    if let Err(e) = restore::validate_downloaded_db(&tmp_path).await {
        let _ = std::fs::remove_file(&tmp_path);
        return Ok(CheckoutResult {
            pool,
            outcome: Err(e),
        });
    }

    // Ponto de não-retorno: tudo que podia falhar por rede/integridade já rodou com `pool`
    // intacto. A identidade DESTE aparelho precisa sobreviver à troca — capturada ANTES de
    // fechar o pool antigo, porque o arquivo baixado chega com `snapshot_state` vazio (ver
    // `state::strip_from_export_copy`, que roda do lado de quem publicou).
    let device_id = local_state.device_id.clone();
    pool.close().await;

    let safeguard_path = match restore::swap_active_db_atomically(&tmp_path, db_path) {
        Ok(p) => p,
        Err(swap_err) => {
            let _ = std::fs::remove_file(&tmp_path);
            // A troca salvaguarda por CÓPIA, nunca renomeio — `active_db` nunca chega a ser
            // tocado quando o rename final falha (ver `swap_active_db_atomically`). Só falta uma
            // conexão nova para o mesmo arquivo de sempre.
            let reopened = open_migrated_pool(db_path).await.map_err(|open_err| {
                format!("{swap_err}; adicionalmente falhou reabrir o banco: {open_err}")
            })?;
            return Ok(CheckoutResult {
                pool: reopened,
                outcome: Err(swap_err),
            });
        }
    };

    // A troca já está confirmada neste ponto — uma falha aqui é rara (o conteúdo baixado já
    // passou por `validate_downloaded_db`) mas não impossível (I/O transitório, disco cheio na
    // migração). A mensagem cita o caminho da salvaguarda: o conteúdo de ANTES da troca está
    // intacto lá (cópia, nunca movida), disponível para restauração manual se for preciso.
    let new_pool = open_migrated_pool(db_path).await.map_err(|e| {
        let recovery = safeguard_path
            .as_ref()
            .map(|p| format!("o conteúdo anterior está preservado em {}", p.display()))
            .unwrap_or_else(|| {
                "não havia banco anterior a preservar (primeira restauração)".into()
            });
        format!("reabrir banco depois da restauração: {e}; {recovery}")
    })?;

    let checked_out_at = chrono::Utc::now().to_rfc3339();
    if let Err(e) = state::adopt_after_restore(
        &new_pool,
        &device_id,
        remote_manifest.sequence,
        &checked_out_at,
        &remote_manifest.device_id,
    )
    .await
    {
        return Ok(CheckoutResult {
            pool: new_pool,
            outcome: Err(e),
        });
    }

    Ok(CheckoutResult {
        pool: new_pool,
        outcome: Ok(CheckoutOutcome::Restored { safeguard_path }),
    })
}

/// O gancho de verdade que `lib.rs` chama na abertura do app: resolve client id/secret/token pelo
/// MESMO caminho do sync de fundo (`sync_task::resolve_client_id`) e SILENCIA qualquer motivo de
/// não tentar — nunca conectou, sem client id configurado, token sem o escopo `drive.appdata`.
/// Nenhum desses é uma falha do check-out em si, é "sync ainda não configurado" (offline pleno).
/// Uma falha DEPOIS de decidir tentar (rede, integridade) continua reportada em `outcome`, nunca
/// engolida — só a decisão de TENTAR é best-effort, não o resultado da tentativa.
pub async fn checkout_on_open_best_effort(
    pool: SqlitePool,
    db_path: &Path,
    app_dir: &Path,
) -> Result<CheckoutResult, String> {
    let Some(client_id) = crate::sync_task::resolve_client_id(&pool).await else {
        return Ok(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::NothingToDo),
        });
    };
    let client_secret = crate::oauth::pkce::resolve_client_secret(None);
    let token = match crate::oauth::token_store::ensure_drive_scope(
        app_dir,
        &client_id,
        client_secret.as_deref(),
    )
    .await
    {
        Ok(t) => t,
        Err(_) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Ok(CheckoutOutcome::NothingToDo),
            });
        }
    };
    let drive = DriveSnapshotClient::new(token, super::transport::production_base_url());
    checkout_on_open(pool, db_path, &drive).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::token_store::StoredToken;
    use crate::snapshot::manifest::SnapshotManifest;
    use std::time::Duration;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-checkout-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn test_pool(db_path: &Path) -> SqlitePool {
        open_migrated_pool(db_path).await.expect("pool de teste")
    }

    fn token() -> StoredToken {
        StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 9_999_999_999,
            scope: "".into(),
        }
    }

    #[tokio::test]
    async fn nothing_to_do_when_remote_has_no_manifest_published_yet() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));

        // O pool devolvido continua o MESMO banco, utilizável.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn nothing_to_do_when_remote_sequence_matches_local_base() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(&pool, 3, "2026-08-11T10:00:00Z", &local.device_id, "hash")
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local.device_id.clone(),
            sequence: 3,
            created_at: "2026-08-11T10:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn manifest_fetch_failure_leaves_the_original_pool_untouched_and_reports_the_error() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(500)
            .with_body(r#"{"error": {"message": "backend hiccup"}}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("falha de rede não é fatal — pool original continua utilizável");
        let err = result.outcome.unwrap_err();
        assert!(err.contains("backend hiccup"), "erro: {err}");

        // Pool intacto: a leitura funciona normalmente.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn refuses_restore_when_remote_schema_is_newer_than_local_and_changes_nothing() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-11T10:00:00Z".into(),
            app_version: "9.9.9".into(),
            schema_version: local_schema + 1000,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        // Nenhum mock para o download do snapshot: se o código tentasse baixar mesmo com o
        // schema recusado, a chamada não-mockada devolveria 501 e o teste acusaria a diferença.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("recusa por schema não é um erro fatal");
        assert_eq!(
            result.outcome,
            Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema,
                remote_schema: local_schema + 1000,
            })
        );

        // Nada mudou: mesma base, mesmo device_id de antes.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Cria um segundo banco (o "remoto" baixado), migrado e com um marcador em `app_setting`
    /// que não existe no banco local — o jeito de provar, depois da restauração, que o conteúdo
    /// ativo é mesmo o do remoto e não uma cópia do que já estava aqui.
    async fn build_remote_db_bytes(dir: &Path, marker: &str) -> Vec<u8> {
        let remote_path = dir.join(format!("remote-source-{}.db", uuid::Uuid::new_v4()));
        let remote_pool = open_migrated_pool(&remote_path).await.unwrap();
        crate::commands::app_setting_set(&remote_pool, "restore_marker", marker)
            .await
            .unwrap();
        // Espelha `strip_from_export_copy`: o snapshot publicado nunca carrega a identidade de
        // quem publicou.
        sqlx::query("DELETE FROM snapshot_state")
            .execute(&remote_pool)
            .await
            .unwrap();
        remote_pool.close().await;
        std::fs::read(&remote_path).unwrap()
    }

    #[tokio::test]
    async fn restores_the_active_db_when_remote_advanced_and_schema_is_compatible() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();
        crate::commands::app_setting_set(&pool, "local_only_marker", "presente-antes")
            .await
            .unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 9,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: local_schema,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "snap-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/snap-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(remote_bytes)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("restauração deve suceder");
        let outcome = result.outcome.expect("nenhum erro esperado");
        match outcome {
            CheckoutOutcome::Restored { safeguard_path } => {
                assert!(
                    safeguard_path.is_some(),
                    "havia banco ativo antes — deve gerar salvaguarda"
                );
                assert!(safeguard_path.unwrap().exists());
            }
            other => panic!("esperava Restored, veio {other:?}"),
        }

        // O conteúdo ativo agora é o do remoto...
        let marker = crate::commands::app_setting_get(&result.pool, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-remoto"));
        // ...e o que só existia no local ANTES da troca sumiu (o remoto o substituiu de verdade).
        let local_only = crate::commands::app_setting_get(&result.pool, "local_only_marker")
            .await
            .unwrap();
        assert!(local_only.is_none());

        // A identidade DESTE aparelho sobrevive à troca — nunca é regerada.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.device_id, local_before.device_id);
        assert_eq!(state_after.base_sequence, 9);
        assert_eq!(
            state_after.last_checkout_device_id.as_deref(),
            Some("outro-aparelho")
        );
        assert!(state_after.last_checkout_at.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_on_open_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        // Mesma classe de regressão já documentada em `state.rs`/`db_export.rs`: com pool de 1
        // conexão, o check-out lê `snapshot_state` (via `state::load_or_init`) como primeiro
        // passo — enquanto outra transação de escrita segura a única conexão, essa leitura
        // precisa ENFILEIRAR e completar assim que a tx solta a conexão, nunca travar para
        // sempre. `download_snapshot`/`restore` também competiriam pela mesma conexão única se
        // chegassem a rodar, mas aqui a ausência de manifest remoto interrompe o fluxo antes
        // disso — o que importa é a leitura inicial não travar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let checkout = tokio::spawn(async move { checkout_on_open(pool, &db_path, &drive).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(Duration::from_secs(5), checkout)
            .await
            .expect("checkout_on_open NÃO pode travar para sempre esperando a única conexão")
            .expect("task não deve entrar em panic")
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn best_effort_is_a_silent_no_op_when_nothing_was_ever_configured() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        // `app_dir` sem token nenhum e sem `sheets_client_id`/`GOOGLE_CLIENT_ID`: nunca conectou.
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();

        let result = checkout_on_open_best_effort(pool, &db_path, &app_dir)
            .await
            .expect("best-effort nunca falha quando não há como tentar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }
}
