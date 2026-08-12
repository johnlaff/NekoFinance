use super::*;
use crate::snapshot::{lease, manifest::SnapshotManifest, state, transport::DriveSnapshotClient};
use sha2::{Digest, Sha256};

/// As duas frases da recusa do check-in começam pelo mesmo prefixo ("Check-in recusado: ") de
/// PROPÓSITO: o frontend reconhece a recusa por esse prefixo ESTRUTURAL
/// (`CHECKIN_REFUSED_PREFIX` em `src/screens/configView.ts`), nunca por regex sobre as palavras
/// da frase descritiva que segue — mudar a explicação depois do prefixo não quebra o
/// reconhecimento em produção. Mudar o PREFIXO em si é mudança de contrato: atualize os dois
/// lados juntos, no mesmo commit (o teste `checkin_refusal_messages_share_the_stable_contract_prefix`
/// trava essa invariante deste lado).
///
/// Veredito `Pull`: outro aparelho publicou depois do nosso último check-in. Esta fatia (issue
/// #423) ainda não tem check-out/pull/restore — chega em fatia futura da spec 043 — então a
/// copy nunca instrui um gesto ("baixe") que o app ainda não oferece.
pub const CHECKIN_REFUSED_PULL: &str = "Check-in recusado: outro aparelho publicou depois do seu último check-in, e a leitura \
     dessa versão ainda não chegou a este app — chega numa atualização futura.";

/// Veredito `Conflict`: os dois lados avançaram a partir da mesma base. Nunca dizer "baixe" —
/// aqui isso significaria descartar o trabalho local sem aviso.
pub const CHECKIN_REFUSED_CONFLICT: &str = "Check-in recusado: os dois lados mudaram desde o último ponto em comum entre os \
     aparelhos.";

/// O que a UI de Conexão mostra sobre o último check-in — quando e por qual aparelho.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriveCheckinInfo {
    pub last_checkin_at: Option<String>,
    pub last_checkin_device_id: Option<String>,
    pub this_device_id: String,
}

/// Resultado do gesto de check-in. "Em dia" (nada mudou desde a última publicação) é SUCESSO —
/// o mesmo veredito do ADR-0015 —, mas a tela precisa distinguir dos dois para não anunciar uma
/// publicação que não aconteceu: `published` diz se este clique de fato subiu algo novo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriveCheckinResult {
    #[serde(flatten)]
    pub info: DriveCheckinInfo,
    pub published: bool,
}

#[tauri::command]
pub async fn last_drive_checkin(pool: State<'_, SqlitePool>) -> Result<DriveCheckinInfo, String> {
    let st = state::load_or_init(pool.inner()).await?;
    Ok(DriveCheckinInfo {
        last_checkin_at: st.last_checkin_at,
        last_checkin_device_id: st.last_checkin_device_id,
        this_device_id: st.device_id,
    })
}

/// O gesto de check-in: exporta um snapshot íntegro (`db_export::vacuum_into_atomic`, o mesmo
/// caminho do backup) e o publica no `appDataFolder`, com o manifest de sequência ao
/// lado. Recusa publicar quando o remoto avançou desde a última base local — force-with-lease,
/// nunca sobrescreve o outro aparelho em silêncio.
pub(crate) async fn drive_checkin_core(
    pool: &SqlitePool,
    app_dir: &std::path::Path,
    drive: &DriveSnapshotClient,
) -> Result<DriveCheckinResult, String> {
    let local_state = state::load_or_init(pool).await?;
    let remote = drive.fetch_manifest().await?;

    // Exporta e hasheia ANTES de decidir: sem hooks em todo gesto que muda o banco (fora do
    // escopo deste corte), comparar o hash do export atual contra o último publicado é o jeito
    // honesto de saber se há algo de fato novo — nunca assumir que houve mudança só porque o
    // dono clicou em "Fazer check-in" de novo.
    let tmp_path = app_dir.join(format!("neko-checkin-{}.db", uuid::Uuid::new_v4()));
    db_export::vacuum_into_atomic(pool, &tmp_path)
        .await
        .map_err(|e| format!("exportar snapshot: {e}"))?;
    // A identidade/sequência DESTE aparelho nunca viaja no snapshot compartilhado — ver
    // `state::strip_from_export_copy`.
    if let Err(e) = state::strip_from_export_copy(&tmp_path).await {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("preparar snapshot para publicação: {e}"));
    }
    let db_bytes = std::fs::read(&tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    let db_bytes = db_bytes.map_err(|e| format!("ler snapshot exportado: {e}"))?;
    let export_hash = hex::encode(Sha256::digest(&db_bytes));

    let content_changed = local_state.last_export_sha256.as_deref() != Some(export_hash.as_str());
    // Cada publicação reivindica a PRÓXIMA sequência a partir da base local — só quando o
    // conteúdo de fato mudou; senão o candidato fica na própria base e o árbitro lê "em dia".
    let candidate_sequence = if content_changed {
        local_state.base_sequence + 1
    } else {
        local_state.base_sequence
    };

    // `decide` recusa subir quando o remoto já avançou além da base — a mesma semântica do
    // `git push --force-with-lease`.
    match lease::decide(
        candidate_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    ) {
        lease::LeaseVerdict::Push => {}
        lease::LeaseVerdict::UpToDate => {
            // Sucesso, não erro (ADR-0015): nada de novo para publicar. O estado local não
            // muda — devolve exatamente o que já estava registrado.
            return Ok(DriveCheckinResult {
                info: DriveCheckinInfo {
                    last_checkin_at: local_state.last_checkin_at,
                    last_checkin_device_id: local_state.last_checkin_device_id,
                    this_device_id: local_state.device_id,
                },
                published: false,
            });
        }
        // Pull e Conflict têm copy PRÓPRIA: Pull não instrui "baixe" (esta fatia não tem
        // check-out/pull/restore ainda) e Conflict não sugere um gesto que descartaria trabalho
        // local sem aviso.
        lease::LeaseVerdict::Pull => return Err(CHECKIN_REFUSED_PULL.into()),
        lease::LeaseVerdict::Conflict => return Err(CHECKIN_REFUSED_CONFLICT.into()),
    }

    let schema_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("ler versão do schema: {e}"))?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let manifest = SnapshotManifest {
        device_id: local_state.device_id.clone(),
        sequence: candidate_sequence,
        created_at: created_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
    };

    drive.upload_snapshot(&db_bytes, &manifest).await?;

    // Só avança o estado local DEPOIS do upload confirmado — uma falha de rede no meio deixa a
    // base local intocada, então o próximo check-in tenta a MESMA sequência de novo.
    state::record_checkin(
        pool,
        candidate_sequence,
        &created_at,
        &local_state.device_id,
        &export_hash,
    )
    .await?;

    Ok(DriveCheckinResult {
        info: DriveCheckinInfo {
            last_checkin_at: Some(created_at),
            last_checkin_device_id: Some(local_state.device_id.clone()),
            this_device_id: local_state.device_id,
        },
        published: true,
    })
}

// Lista de parâmetros plana por design (cada um vem de state/request); `guard` (SyncGuard) é
// estado gerenciado — mesmo padrão de `import_sheet_data`.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn drive_checkin(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<DriveCheckinResult, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_drive_scope(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let drive = DriveSnapshotClient::new(token, crate::snapshot::transport::production_base_url());

    // Serializa contra import/sync de fundo no pool de 1 conexão — mesmo SyncGuard do import.
    let _lock = guard.inner().lock().await;
    drive_checkin_core(pool.inner(), &app_dir.0, &drive).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::token_store::StoredToken;

    #[test]
    fn checkin_refusal_messages_share_the_stable_contract_prefix() {
        // Espelha `CHECKIN_REFUSED_PREFIX` de `src/screens/configView.ts`, onde o frontend
        // reconhece a recusa do lease por este prefixo ESTRUTURAL — nunca por regex sobre as
        // palavras da frase descritiva. Se um dos dois textos deixar de começar por ele, o
        // reconhecimento quebra em produção mesmo com a suíte inteira verde.
        const CHECKIN_REFUSED_PREFIX: &str = "Check-in recusado: ";
        assert!(CHECKIN_REFUSED_PULL.starts_with(CHECKIN_REFUSED_PREFIX));
        assert!(CHECKIN_REFUSED_CONFLICT.starts_with(CHECKIN_REFUSED_PREFIX));
    }

    // `VACUUM INTO` exige um banco de ORIGEM em arquivo — a partir de `:memory:` ele não
    // materializa o destino (mesma observação já documentada no teste do backup, `commands::mod`).
    async fn test_pool(app_dir: &std::path::Path) -> SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        let src = app_dir.join("neko-src.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{}", src.display()))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .expect("pool SQLite em arquivo");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações");
        pool
    }

    fn test_app_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-checkin-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
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
    async fn first_checkin_ever_publishes_and_records_sequence_one() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let mut server = mockito::Server::new_async().await;
        // Nenhum manifest/snapshot publicado ainda: toda busca por nome devolve lista vazia.
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect("primeiro check-in deve publicar (primeira subida)");
        assert!(result.published);
        assert!(result.info.last_checkin_at.is_some());
        assert_eq!(
            result.info.last_checkin_device_id,
            Some(result.info.this_device_id.clone())
        );

        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn second_checkin_pushes_again_when_remote_unchanged_since_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11 10:00:00",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Remoto na MESMA base (sequência 1, publicada por este mesmo aparelho) — subir de novo é
        // seguro (avanço unilateral local).
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local.device_id.clone(),
            sequence: 1,
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
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": []}"#)
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
            // O snapshot ainda não existe (mock acima devolve lista vazia) → cria via POST.
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "snap-created"}"#)
            .create_async()
            .await;
        server
            // O manifest JÁ existe (id `man-1`, achado acima) → atualiza pelo MESMO id via PATCH,
            // nunca cria um segundo arquivo.
            .mock("PATCH", "/upload/drive/v3/files/man-1")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "man-1"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect("subir de novo com o remoto na mesma base deve ser seguro");
        assert!(result.published);
        assert!(result.info.last_checkin_at.is_some());

        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 2);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_refuses_with_conflict_message_when_both_sides_advanced_from_same_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11 10:00:00",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Remoto avançou para 5 (outro aparelho publicou) enquanto nossa base ainda é 1, E o
        // conteúdo local mudou desde a base (o hash semeado acima não bate com o export real) —
        // os dois lados avançaram a partir da mesma base: Conflict, não Pull.
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 5,
            created_at: "2026-08-11T11:00:00Z".into(),
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

        let err = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect_err("deve recusar publicar por cima do avanço do outro aparelho");
        // Conflito nunca instrui "baixe" — aqui significaria descartar trabalho local sem aviso.
        assert_eq!(err, CHECKIN_REFUSED_CONFLICT);

        // Estado local intocado: a base continua 1, nenhuma sequência foi reivindicada em vão.
        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_refuses_with_pull_message_when_remote_advanced_and_local_unchanged() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        // Primeiro check-in: publica a sequência 1 (primeira subida).
        let mut server1 = mockito::Server::new_async().await;
        server1
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server1
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive1 = DriveSnapshotClient::new(token(), server1.url());
        let first = drive_checkin_core(&pool, &app_dir, &drive1)
            .await
            .expect("primeiro check-in deve publicar");
        assert!(first.published);

        // Nenhuma escrita no banco depois disso: o próximo export teria o MESMO conteúdo. Mas
        // outro aparelho publicou por cima (sequência 2) — o remoto avançou sem que este
        // aparelho tivesse mudança própria para reivindicar: Pull, nunca Conflict.
        let mut server2 = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 2,
            created_at: "2026-08-11T12:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server2
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server2
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive2 = DriveSnapshotClient::new(token(), server2.url());

        let err = drive_checkin_core(&pool, &app_dir, &drive2)
            .await
            .expect_err("deve recusar com o veredito Pull, sem instruir gesto inexistente");
        // Esta fatia (issue #423) não tem check-out/pull/restore — a copy nunca instrui "baixe".
        assert_eq!(err, CHECKIN_REFUSED_PULL);

        // Estado local intocado: a base continua 1.
        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_twice_with_unchanged_content_is_up_to_date_and_never_republishes() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        // Primeiro check-in: nenhum snapshot publicado ainda (primeira subida).
        let mut server1 = mockito::Server::new_async().await;
        server1
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server1
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive1 = DriveSnapshotClient::new(token(), server1.url());
        let first = drive_checkin_core(&pool, &app_dir, &drive1)
            .await
            .expect("primeiro check-in deve publicar");
        assert!(first.published);
        let state_after_first = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after_first.base_sequence, 1);

        // Segundo check-in, banco INALTERADO desde o primeiro (nenhuma escrita entre as duas
        // chamadas): o remoto reflete exatamente o que o primeiro check-in publicou. Nenhum mock
        // de upload aqui — se o código tentar subir de novo, a chamada não-mockada devolve 501 e
        // o teste acusa a diferença.
        let mut server2 = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: state_after_first.device_id.clone(),
            sequence: 1,
            created_at: first.info.last_checkin_at.clone().unwrap(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: 1,
        })
        .unwrap();
        server2
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server2
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive2 = DriveSnapshotClient::new(token(), server2.url());

        // "Em dia" é SUCESSO (ADR-0015), não erro: o segundo clique não publica de novo, mas
        // também não deve virar mensagem de falha para o dono.
        let second = drive_checkin_core(&pool, &app_dir, &drive2)
            .await
            .expect("nada mudou desde o último check-in — 'em dia' é sucesso, não erro");
        assert!(
            !second.published,
            "clique redundante não deve reivindicar ter publicado algo novo"
        );
        assert_eq!(second.info.last_checkin_at, first.info.last_checkin_at);

        // Sequência intocada: um clique redundante nunca avança a base sem mudança real.
        let state_after_second = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after_second.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }
}
