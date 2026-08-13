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
    /// O manifest remoto carrega o NOSSO PRÓPRIO `device_id` E `sequence == base_local + 1` — a
    /// janela exata de um check-in que morreu entre o upload confirmado e a gravação do estado
    /// local (ADR-0015). O conteúdo já é nosso; restaurar de verdade descartaria qualquer
    /// trabalho feito depois daquele upload, então só a sequência-base local avança para alcançar
    /// o remoto, sem baixar nem trocar arquivo. Fora dessa janela — mesmo com o mesmo `device_id`
    /// — o manifest pode pertencer a outra instalação que compartilha identidade por um caminho
    /// lateral (cópia manual da pasta do app, backup restaurado à mão sem passar pelo strip do
    /// export), então o check-out segue o veredito normal do árbitro em vez de adotar às cegas.
    CaughtUpOwnSequence { sequence: i64 },
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

    // O remoto avançou com a NOSSA PRÓPRIA identidade E na sequência EXATA que um check-in morto
    // deixaria (`base + 1`): publicamos aquele conteúdo nós mesmos — o upload confirmou, mas a
    // gravação da base local morreu antes de terminar (queda de rede/processo entre as duas
    // etapas). Restaurar de verdade baixaria e trocaria o banco ativo pelo NOSSO PRÓPRIO snapshot
    // antigo, descartando qualquer gesto feito depois daquele upload — o conteúdo já é nosso, só
    // a base local está atrasada. Nunca baixa nem troca arquivo neste ramo: só alcança a
    // sequência remota.
    //
    // Qualquer OUTRA sequência com o mesmo `device_id` não entra aqui, mesmo que também seja
    // "nossa": duas instalações podem compartilhar identidade por um caminho lateral (cópia
    // manual da pasta do app; backup local restaurado à mão, que não passa pelo `strip` do
    // export) — nesse caso o manifest pertence de fato a OUTRO aparelho que só usa o mesmo
    // rótulo, e cai no fluxo normal abaixo (restauração de verdade, registrada na linha
    // "Última leitura do Drive" e com a salvaguarda local), preservando a convergência entre
    // os dois.
    if remote_manifest.device_id == local_state.device_id
        && remote_manifest.sequence == local_state.base_sequence + 1
    {
        return match state::adopt_own_sequence(&pool, remote_manifest.sequence).await {
            Ok(()) => Ok(CheckoutResult {
                pool,
                outcome: Ok(CheckoutOutcome::CaughtUpOwnSequence {
                    sequence: remote_manifest.sequence,
                }),
            }),
            Err(e) => Ok(CheckoutResult {
                pool,
                outcome: Err(e),
            }),
        };
    }

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
    if let Err(e) = restore::stage_downloaded_snapshot(&tmp_path, &db_bytes).await {
        return Ok(CheckoutResult {
            pool,
            outcome: Err(e),
        });
    }

    // Ponto de não-retorno: tudo que podia falhar por rede/integridade já rodou com `pool`
    // intacto. A identidade DESTE aparelho E o histórico de check-in que ele já tinha precisam
    // sobreviver à troca — capturados ANTES de fechar o pool antigo, porque o arquivo baixado
    // chega com `snapshot_state` vazio (ver `state::strip_from_export_copy`, que roda do lado de
    // quem publicou). Sem capturar o histórico de check-in aqui, um aparelho que já publicou
    // perderia a própria linha do tempo a cada check-out.
    let device_id = local_state.device_id.clone();
    let last_checkin_at = local_state.last_checkin_at.clone();
    let last_checkin_device_id = local_state.last_checkin_device_id.clone();
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
        last_checkin_at.as_deref(),
        last_checkin_device_id.as_deref(),
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
    let result = checkout_on_open(pool, db_path, &drive).await?;

    // Persiste o desfecho para a UI de Conexão (ADR-0015): a recusa por schema mais
    // novo e a falha de rede/integridade merecem um aviso na tela, não só uma linha de log que o
    // dono nunca vê. `NothingToDo`/`Restored`/`CaughtUpOwnSequence` são sucesso — limpam qualquer
    // aviso de uma tentativa ANTERIOR, para ele não sobreviver a um check-out que deu certo depois.
    // Melhor esforço: uma falha ao GRAVAR o desfecho não pode derrubar o check-out em si, que já
    // rodou até aqui — só loga e segue com o `pool` que `checkout_on_open` devolveu.
    let (outcome_tag, outcome_detail) = outcome_warning_fields(&result.outcome);
    if let Err(e) = state::record_checkout_outcome(
        &result.pool,
        outcome_tag.as_deref(),
        outcome_detail.as_deref(),
    )
    .await
    {
        eprintln!("[snapshot/checkout] falha ao registrar o desfecho para a UI: {e}");
    }

    Ok(result)
}

/// Rótulo fechado gravado em `snapshot_state.last_checkout_outcome` pela sonda de FOCO (nunca por
/// `checkout_on_open`): remoto avançou além da base local, mas a sonda não baixa nem troca o
/// arquivo — só avisa. Mesma família de `outcome_warning_fields`, uma constante própria porque
/// nasce de um caminho diferente (nunca dentro de `CheckoutOutcome`, que é sempre resultado de uma
/// tentativa real de restauração).
pub const NEWER_SNAPSHOT_AVAILABLE_OUTCOME: &str = "newer_available";

/// Núcleo testável da sonda de FOCO (ADR-0015): consulta o manifest remoto e AVISA
/// quando ele avançou além da base local — mas NUNCA baixa nem troca o banco ativo. Diferente do
/// check-out no boot (`checkout_on_open`), aqui o pool já está `app.manage()`-do e em uso pelo app
/// inteiro; trocar o arquivo debaixo dele exigiria o mesmo "reinicie o app" que
/// `resolve_conflict_use_remote_core` já usa para a escolha explícita de conflito — fazer isso
/// silenciosamente a cada foco seria pior que só avisar e deixar o próximo reinício convergir de
/// verdade (o check-out do boot já faz isso sozinho).
///
/// A mesma guarda estreita do próprio `device_id` de `checkout_on_open` se aplica aqui: quando o
/// manifest remoto é o NOSSO check-in que morreu entre o upload confirmado e a gravação local
/// (`remote.sequence == base + 1`), não é uma versão mais nova de OUTRO aparelho — é segura de
/// adotar sem baixar nada, porque só atualiza bookkeeping local, nunca troca arquivo.
pub(crate) async fn probe_newer_snapshot_on_focus(
    pool: &SqlitePool,
    drive: &DriveSnapshotClient,
) -> Result<(), String> {
    let local_state = state::load_or_init(pool).await?;
    let remote = drive.fetch_manifest().await?;
    let verdict = lease::decide(
        local_state.base_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    );
    if verdict != lease::LeaseVerdict::Pull {
        // Sem disputa nenhuma agora (a leitura é FRESCA) — qualquer aviso de uma sonda anterior
        // não se sustenta mais.
        return state::record_checkout_outcome(pool, None, None).await;
    }
    let remote_manifest =
        remote.expect("veredito Pull do árbitro implica manifest remoto presente");
    if remote_manifest.device_id == local_state.device_id
        && remote_manifest.sequence == local_state.base_sequence + 1
    {
        state::adopt_own_sequence(pool, remote_manifest.sequence).await?;
        return state::record_checkout_outcome(pool, None, None).await;
    }
    state::record_checkout_outcome(pool, Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME), None).await
}

/// O gancho de verdade que `lib.rs` chama quando a janela ganha foco: resolve client id/token pelo
/// MESMO caminho best-effort de `checkout_on_open_best_effort` — qualquer motivo de não tentar
/// (nunca conectou, sem escopo) é silencioso. Uma falha DEPOIS de decidir tentar (rede,
/// integridade) é logada pelo chamador, nunca engolida aqui.
pub async fn probe_newer_snapshot_on_focus_best_effort(
    pool: &SqlitePool,
    app_dir: &Path,
) -> Result<(), String> {
    let Some(client_id) = crate::sync_task::resolve_client_id(pool).await else {
        return Ok(());
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
        Err(_) => return Ok(()),
    };
    let drive = DriveSnapshotClient::new(token, super::transport::production_base_url());
    probe_newer_snapshot_on_focus(pool, &drive).await
}

/// Mapeia o desfecho do check-out para o rótulo fechado que `snapshot_state.last_checkout_outcome`
/// grava — só os dois casos que a UI de Conexão precisa avisar (`CHECKIN`/`RefusedNewerSchema` tem
/// copy própria já visível; `NothingToDo`/`Restored`/`CaughtUpOwnSequence` não precisam de aviso,
/// então limpam qualquer um pendente de uma tentativa anterior). Função pura, testável sem rede.
fn outcome_warning_fields(
    outcome: &Result<CheckoutOutcome, String>,
) -> (Option<String>, Option<String>) {
    match outcome {
        Ok(CheckoutOutcome::RefusedNewerSchema {
            local_schema,
            remote_schema,
        }) => (
            Some("refused_newer_schema".to_string()),
            Some(format!("{local_schema}:{remote_schema}")),
        ),
        Err(e) => (Some("error".to_string()), Some(e.clone())),
        Ok(_) => (None, None),
    }
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

    // --- Defeito 1: histórico de check-in sobrevive à troca (regressão de ponta a ponta) -------

    #[tokio::test]
    async fn restore_preserves_this_devices_own_checkin_history_across_the_swap() {
        // O aparelho publicou antes (check-in próprio registrado) e AGORA recebe o snapshot de
        // OUTRO aparelho — o histórico de check-in DESTE aparelho é bookkeeping local, não dado
        // do snapshot baixado (que chega com `snapshot_state` vazio), e precisa sobreviver.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            3,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto-2").await;

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
        assert!(matches!(
            result.outcome,
            Ok(CheckoutOutcome::Restored { .. })
        ));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(
            state_after.last_checkin_at.as_deref(),
            Some("2026-08-10T09:00:00Z"),
            "o check-out apagava o histórico de check-in deste aparelho — a tela voltava a dizer \
             'nenhum check-in ainda' para um aparelho que já publicou"
        );
        assert_eq!(
            state_after.last_checkin_device_id.as_deref(),
            Some(local_before.device_id.as_str())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Defeito 2: desfecho do check-out fica visível na tela, não só no log ------------------

    #[test]
    fn outcome_warning_fields_maps_only_the_two_outcomes_the_screen_needs_to_warn_about() {
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema: 5,
                remote_schema: 8,
            })),
            (
                Some("refused_newer_schema".to_string()),
                Some("5:8".to_string())
            )
        );
        assert_eq!(
            outcome_warning_fields(&Err("timeout de rede".to_string())),
            (
                Some("error".to_string()),
                Some("timeout de rede".to_string())
            )
        );
        // Sucesso silencioso: nada a avisar, limpa qualquer aviso pendente.
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::NothingToDo)),
            (None, None)
        );
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::Restored {
                safeguard_path: None
            })),
            (None, None)
        );
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::CaughtUpOwnSequence { sequence: 3 })),
            (None, None)
        );
    }

    #[tokio::test]
    async fn refused_newer_schema_outcome_is_persisted_and_visible_through_last_drive_checkin() {
        // Atravessa a costura backend↔tela de ponta a ponta: roda o check-out de verdade, grava
        // o desfecho pelo MESMO caminho que `checkout_on_open_best_effort` usa, e lê de volta
        // pelo comando REAL que a tela chama (`last_drive_checkin_core`) — não uma reconstrução
        // à mão do formato esperado.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-12T10:00:00Z".into(),
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
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("recusa por schema não é um erro fatal");
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .expect("gravar desfecho");

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .expect("ler pelo comando real que a tela chama");
        assert_eq!(
            info.last_checkout_outcome.as_deref(),
            Some("refused_newer_schema")
        );
        assert_eq!(
            info.last_checkout_outcome_detail.as_deref(),
            Some(format!("{local_schema}:{}", local_schema + 1000).as_str())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn network_failure_outcome_is_persisted_and_visible_through_last_drive_checkin() {
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
            .expect("falha de rede não é fatal");
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .expect("gravar desfecho");

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .expect("ler pelo comando real que a tela chama");
        assert_eq!(info.last_checkout_outcome.as_deref(), Some("error"));
        assert!(
            info.last_checkout_outcome_detail
                .as_deref()
                .unwrap()
                .contains("backend hiccup"),
            "detalhe: {:?}",
            info.last_checkout_outcome_detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_outcome_warning_is_cleared_by_a_later_successful_checkout() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        state::record_checkout_outcome(&pool, Some("error"), Some("tentativa anterior"))
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive).await.unwrap();
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .unwrap();

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .unwrap();
        assert!(
            info.last_checkout_outcome.is_none(),
            "um check-out bem-sucedido depois limpa o aviso da tentativa anterior"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Defeito 3: nunca restaura o próprio snapshot por cima de trabalho posterior -----------

    #[tokio::test]
    async fn adopts_the_remote_sequence_without_restoring_when_the_manifest_is_our_own_device() {
        // O remoto avançou (sequência acima da nossa base), mas com o NOSSO PRÓPRIO device_id —
        // um check-in cujo upload confirmou mas cuja gravação local morreu antes de terminar.
        // Restaurar de verdade baixaria e trocaria pelo NOSSO PRÓPRIO snapshot antigo, descartando
        // qualquer gesto feito depois daquele upload. Nenhum mock de download é registrado: se o
        // código tentasse baixar mesmo assim, a chamada não-mockada devolveria 501 e o teste
        // acusaria a diferença.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        crate::commands::app_setting_set(
            &pool,
            "local_only_marker",
            "trabalho-posterior-ao-upload",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 1,
            created_at: "2026-08-12T09:00:00Z".into(),
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
            .expect("adotar a própria sequência não é um erro");
        assert_eq!(
            result.outcome,
            Ok(CheckoutOutcome::CaughtUpOwnSequence { sequence: 1 })
        );

        // Conteúdo local INTOCADO: nada foi baixado nem trocado.
        let marker = crate::commands::app_setting_get(&result.pool, "local_only_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("trabalho-posterior-ao-upload"));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.device_id, local_before.device_id);
        assert_eq!(
            state_after.base_sequence, 1,
            "a base local alcança a sequência remota mesmo sem restaurar"
        );
        assert!(
            state_after.last_checkout_at.is_none(),
            "nada foi de fato lido de outro aparelho — o eixo de check-out não muda"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restores_normally_when_the_own_device_id_sequence_is_not_exactly_base_plus_one() {
        // Duas instalações podem compartilhar `device_id` por um caminho lateral (cópia manual
        // da pasta do app; backup local restaurado à mão, que não passa pelo `strip` do export) —
        // aí o manifest com o NOSSO id não é necessariamente o check-in morto entre upload e
        // gravação: pode ser o conteúdo de OUTRA instalação com a mesma identidade, várias
        // sequências à frente. `remote.sequence == base + 1` é a única janela estreita o
        // suficiente para presumir "sou eu mesmo, upload confirmado" — qualquer coisa além disso
        // precisa passar pela restauração normal (com barulho visível), nunca ser adotada às
        // cegas.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            5,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();
        crate::commands::app_setting_set(
            &pool,
            "local_only_marker",
            "trabalho-que-nao-pode-ser-descartado-as-cegas",
        )
        .await
        .unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto-mesmo-device-id").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 8, // base(5) + 3: fora da janela upload→gravação (base + 1 = 6).
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
                assert!(safeguard_path.is_some());
            }
            other => panic!("esperava Restored (fora da janela base+1), veio {other:?}"),
        }

        let marker = crate::commands::app_setting_get(&result.pool, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-remoto-mesmo-device-id"));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 8);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn own_device_id_with_regressed_remote_sequence_follows_the_arbiter_verdict() {
        // Sequência remota abaixo da base local, mesmo com o NOSSO device_id: o árbitro
        // (`lease::decide`) já resolve isso como `Push` bem antes da guarda do próprio id ser
        // consultada — nada aqui é mais novo que a base para disputar, então não há o que
        // restaurar nem o que adotar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            7,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 3, // < base (7): regredido.
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
        // Nenhum mock de download: veredito `Push` nunca chega perto de baixar nada.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 7, "base local não regride");
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- probe_newer_snapshot_on_focus -----------------------------------------------------

    #[tokio::test]
    async fn focus_probe_flags_newer_available_without_downloading_when_remote_advanced() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-13T09:00:00Z".into(),
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
        // Nenhum mock de download do snapshot: a sonda de foco NUNCA baixa/troca o arquivo —
        // só avisa. Uma tentativa de download bateria numa rota não-mockada (501).
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(
            after.last_checkout_outcome.as_deref(),
            Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME)
        );
        assert_eq!(
            after.base_sequence, 0,
            "a sonda de foco nunca adota/avança a base de outro aparelho — só o boot restaura"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_adopts_own_sequence_silently_without_flagging_newer_available() {
        // Mesma janela estreita de `checkout_on_open` (ADR-0015): o manifest remoto é o NOSSO
        // check-in que morreu entre o upload confirmado e a gravação local — seguro adotar sem
        // baixar nada, e não é "uma versão mais nova" de outro aparelho para avisar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 1,
            created_at: "2026-08-13T09:00:00Z".into(),
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

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(after.base_sequence, 1, "a base alcança a própria sequência");
        assert!(
            after.last_checkout_outcome.is_none(),
            "conteúdo já é nosso — não é aviso de versão mais nova de outro aparelho"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_clears_a_stale_warning_when_the_fresh_read_is_up_to_date() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        // Aviso de uma sonda ANTERIOR, que a leitura fresca de agora precisa derrubar.
        state::record_checkout_outcome(&pool, Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME), None)
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Nenhum snapshot publicado ainda: remoto ausente, base em 0 — `UpToDate`.
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert!(after.last_checkout_outcome.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
