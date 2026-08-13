//! Check-in automático do snapshot no Drive (ADR-0015): depois de um "gesto
//! material" (o mesmo que `sync_log` já registra — import/write-back da planilha, ver
//! `crate::commands::last_sync_at_query`) e ao fechar o app. O check-out ao ABRIR já roda em
//! `checkout::checkout_on_open_best_effort` (chamado no `setup()` de `lib.rs`); a sonda leve de
//! FOCO mora em `checkout::probe_newer_snapshot_on_focus_best_effort` — este módulo cuida só do
//! lado de PUBLICAR.
//!
//! O padrão de probe+debounce é o mesmo de `crate::sync_task` (o sync de leitura da planilha):
//! um loop de fundo cochila entre tentativas e só AGE quando algo mudou — aqui, "algo mudou" é o
//! cursor de gesto material ter avançado desde a última tentativa bem-sucedida. Nenhum teste toca
//! rede: a borda HTTP (`DriveSnapshotClient`) é sempre mockada.
//!
//! Toda função que precisa de `tauri::AppHandle` (para emitir [`SNAPSHOT_SYNC_DONE_EVENT`]) é um
//! wrapper fino em cima de uma contraparte `_core` sem `AppHandle` — o mesmo split que
//! `crate::sync_task` já usa, porque não há como construir um `AppHandle` de verdade fora de um
//! app Tauri rodando (nenhuma feature de mock do Tauri está habilitada neste projeto). Os testes
//! cobrem as funções `_core`; os wrappers públicos são deliberadamente finos o bastante para não
//! precisarem de teste próprio.

use super::{checkout::resolve_drive_client_best_effort, state};
use sqlx::SqlitePool;
use std::path::Path;
use std::time::Duration;

/// Cadência do loop de gesto material — nunca configurável (ao contrário do intervalo de
/// `sync_task`, que lê um `app_setting` do dono): a checagem em si é barata (uma query local em
/// `sync_log`), então um único valor fixo basta.
const POLL_INTERVAL_SECS: u64 = 45;
/// Prazo máximo que o check-in ao FECHAR espera por rede antes de deixar o app fechar mesmo assim
/// — "offline pleno" significa que fechar o app nunca pode travar esperando conexão.
const CLOSE_CHECKIN_TIMEOUT_SECS: u64 = 5;
/// Cursor persistido do último gesto material (timestamp de `sync_log`) que já gerou uma
/// tentativa de check-in — o debounce natural do loop: gestos repetidos dentro do mesmo intervalo
/// de poll viram uma única tentativa.
const LAST_SEEN_GESTURE_KEY: &str = "snapshot_last_seen_material_gesture_at";

/// Evento emitido ao FRONTEND depois de qualquer tentativa de check-in automático (sucesso,
/// recusa ou falha) — o mesmo espírito de `neko://sync-done` (planilha), mas para o eixo do
/// snapshot: o payload é só o que a UI de Conexão precisa para reagir sem repetir o cálculo (abrir
/// a tela de conflito sozinha quando `conflict_pending` vira `true`).
pub const SNAPSHOT_SYNC_DONE_EVENT: &str = "neko://snapshot-sync-done";

/// O que despachou esta tentativa — só para o log ficar legível; nenhuma das duas tem debounce
/// PRÓPRIO (gesto material usa a cadência do loop; fechar é um tiro único com timeout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckinTrigger {
    MaterialGesture,
    WindowClose,
}

async fn emit_snapshot_sync_done(app_handle: &tauri::AppHandle, pool: &SqlitePool) {
    use tauri::Emitter;
    let conflict_pending = state::load_or_init(pool)
        .await
        .map(|s| s.conflict_pending_since.is_some())
        .unwrap_or(false);
    let _ = app_handle.emit(
        SNAPSHOT_SYNC_DONE_EVENT,
        serde_json::json!({ "conflict_pending": conflict_pending }),
    );
}

/// Núcleo testável de uma tentativa de check-in automático — melhor esforço do início ao fim:
/// qualquer motivo de NÃO tentar (conflito pendente, sync do Drive nunca configurado, token sem
/// escopo) devolve `Ok(false)` em silêncio, o mesmo padrão de
/// `checkout::checkout_on_open_best_effort`. Devolve `Ok(true)` só quando `drive_checkin_core` de
/// fato RESOLVEU a tentativa sem erro (publicou ou já estava em dia) — o sinal que o chamador usa
/// para saber se pode avançar o cursor de debounce. Uma recusa do árbitro (`Pull`/`Conflict`) ou
/// uma falha de rede voltam `Ok(false)`: o chamador não avança nada, e a PRÓXIMA tentativa (gesto
/// novo, ou o próximo tick do loop) tenta de novo.
pub(crate) async fn run_checkin_attempt_core(
    pool: &SqlitePool,
    app_dir: &Path,
    guard: &crate::sync_task::SyncGuard,
    trigger: CheckinTrigger,
) -> Result<bool, String> {
    let local_state = state::load_or_init(pool).await?;
    // Gate central (ADR-0015): nenhum gatilho automático compete com a escolha do
    // dono na tela de conflito.
    if local_state.conflict_pending_since.is_some() {
        return Ok(false);
    }

    let Some(drive) = resolve_drive_client_best_effort(pool, app_dir).await else {
        return Ok(false);
    };

    // Serializa contra import/sync de fundo e comandos manuais no pool de 1 conexão — o MESMO
    // guard que `drive_checkin`/`drive_conflict_details`/`resolve_drive_conflict` já usam.
    let _lock = guard.lock().await;
    match crate::commands::snapshot_cmds::drive_checkin_core(pool, app_dir, &drive).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("[snapshot/checkin:{trigger:?}] {e}");
            Ok(false)
        }
    }
}

/// Wrapper público que soma a emissão de [`SNAPSHOT_SYNC_DONE_EVENT`] — chamado pelos gatilhos de
/// verdade (`lib.rs`). Não tem teste próprio (ver o comentário do módulo): a lógica está toda em
/// [`run_checkin_attempt_core`], que os testes exercitam diretamente.
pub async fn run_checkin_attempt(
    pool: &SqlitePool,
    app_dir: &Path,
    app_handle: &tauri::AppHandle,
    guard: &crate::sync_task::SyncGuard,
    trigger: CheckinTrigger,
) -> Result<bool, String> {
    let result = run_checkin_attempt_core(pool, app_dir, guard, trigger).await;
    emit_snapshot_sync_done(app_handle, pool).await;
    result
}

/// Núcleo testável de um tick do loop de gesto material: barato quando não há nada novo (uma
/// única query local), só chama [`run_checkin_attempt_core`] quando o cursor de gesto material
/// avançou desde a última tentativa — o debounce natural desta cadência.
pub(crate) async fn run_material_gesture_tick_core(
    pool: &SqlitePool,
    app_dir: &Path,
    guard: &crate::sync_task::SyncGuard,
) -> Result<(), String> {
    let Some(latest) = crate::commands::last_sync_at_query(pool).await? else {
        return Ok(());
    };
    let seen = crate::commands::app_setting_get(pool, LAST_SEEN_GESTURE_KEY).await?;
    if seen.as_deref() == Some(latest.as_str()) {
        return Ok(());
    }

    let advanced =
        run_checkin_attempt_core(pool, app_dir, guard, CheckinTrigger::MaterialGesture).await?;
    if advanced {
        crate::commands::app_setting_set(pool, LAST_SEEN_GESTURE_KEY, &latest).await?;
    }
    Ok(())
}

/// Wrapper público que soma a emissão de [`SNAPSHOT_SYNC_DONE_EVENT`] — chamado pelo loop de
/// fundo (`spawn_material_gesture_checkin_loop`). Não tem teste próprio, mesmo motivo de
/// [`run_checkin_attempt`].
pub async fn run_material_gesture_tick(
    pool: &SqlitePool,
    app_dir: &Path,
    app_handle: &tauri::AppHandle,
    guard: &crate::sync_task::SyncGuard,
) -> Result<(), String> {
    let result = run_material_gesture_tick_core(pool, app_dir, guard).await;
    emit_snapshot_sync_done(app_handle, pool).await;
    result
}

/// Spawna o loop de fundo do check-in por gesto material. Nunca panica o processo — erros de uma
/// tentativa são logados e o loop segue cochilando.
pub fn spawn_material_gesture_checkin_loop(
    pool: SqlitePool,
    app_dir: std::path::PathBuf,
    app_handle: tauri::AppHandle,
    guard: std::sync::Arc<crate::sync_task::SyncGuard>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            if let Err(e) = run_material_gesture_tick(&pool, &app_dir, &app_handle, &guard).await {
                eprintln!("[snapshot/checkin] tick de gesto material: {e}");
            }
        }
    });
}

/// O check-in ao FECHAR o app (ADR-0015): melhor esforço, com um TETO de espera —
/// "offline pleno" significa que fechar o app nunca pode travar esperando rede voltar. Nunca
/// propaga erro: `lib.rs` chama isto e então fecha a janela de verdade, sucesso ou não.
pub async fn run_checkin_on_close_best_effort(
    pool: &SqlitePool,
    app_dir: &Path,
    app_handle: &tauri::AppHandle,
    guard: &crate::sync_task::SyncGuard,
) {
    let attempt = run_checkin_attempt(
        pool,
        app_dir,
        app_handle,
        guard,
        CheckinTrigger::WindowClose,
    );
    match tokio::time::timeout(Duration::from_secs(CLOSE_CHECKIN_TIMEOUT_SECS), attempt).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => eprintln!("[snapshot/checkin:close] {e}"),
        Err(_) => eprintln!(
            "[snapshot/checkin:close] sem rede em {CLOSE_CHECKIN_TIMEOUT_SECS}s — fechando mesmo assim"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-checkin-task-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Pool de UMA conexão — o mesmo perfil de produção; regressão de deadlock exige isto (um
    /// pool default nunca pegaria a classe de bug de uma leitura disputando a única conexão com
    /// uma transação de escrita aberta).
    async fn test_pool(app_dir: &Path) -> SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
        use std::str::FromStr;
        let db_path = app_dir.join("neko-src.db");
        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("pool SQLite em arquivo");
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn seed_material_gesture(pool: &SqlitePool, timestamp: &str) {
        let person_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Dono')")
            .bind(&person_id)
            .execute(pool)
            .await
            .unwrap();
        let profile_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
            .bind(&profile_id)
            .bind(&person_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp) \
             VALUES (?1, 'import', 'transaction', 'e1', ?2, ?3)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&profile_id)
        .bind(timestamp)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn run_checkin_attempt_core_is_gated_silently_while_a_conflict_is_pending() {
        let dir = test_dir();
        let pool = test_pool(&dir).await;
        state::load_or_init(&pool).await.unwrap();
        state::record_conflict_pending(&pool, Some("2026-08-13T09:00:00Z"))
            .await
            .unwrap();

        // Nenhum mock de rede: se o gate não interceptasse ANTES até de resolver client id/token,
        // uma tentativa de verdade bateria numa rota não-mockada e o teste acusaria a diferença.
        let guard = crate::sync_task::SyncGuard::new(());
        let advanced =
            run_checkin_attempt_core(&pool, &dir, &guard, CheckinTrigger::MaterialGesture)
                .await
                .expect("gate silencioso não deve propagar erro");
        assert!(!advanced, "conflito pendente: nenhuma tentativa deve rodar");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn run_checkin_attempt_core_is_a_silent_no_op_without_a_configured_client_id() {
        let dir = test_dir();
        let pool = test_pool(&dir).await;

        let guard = crate::sync_task::SyncGuard::new(());
        let advanced =
            run_checkin_attempt_core(&pool, &dir, &guard, CheckinTrigger::MaterialGesture)
                .await
                .expect("sem client id configurado não é erro, é 'sync ainda não configurado'");
        assert!(!advanced);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn material_gesture_tick_core_is_a_no_op_without_any_gesture_logged() {
        let dir = test_dir();
        let pool = test_pool(&dir).await;
        let guard = crate::sync_task::SyncGuard::new(());

        run_material_gesture_tick_core(&pool, &dir, &guard)
            .await
            .expect("sem gesto nenhum não é erro");

        assert!(
            crate::commands::app_setting_get(&pool, LAST_SEEN_GESTURE_KEY)
                .await
                .unwrap()
                .is_none(),
            "sem gesto nenhum, o cursor nunca é gravado"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn material_gesture_tick_core_skips_a_gesture_already_seen() {
        let dir = test_dir();
        let pool = test_pool(&dir).await;
        seed_material_gesture(&pool, "2026-08-13 09:00:00").await;
        crate::commands::app_setting_set(&pool, LAST_SEEN_GESTURE_KEY, "2026-08-13 09:00:00")
            .await
            .unwrap();

        // Sem client id configurado: se o tick tentasse mesmo assim, cairia no caminho de rede
        // (sem mock) — mas o curto-circuito "já visto" precisa interceptar ANTES disso.
        let guard = crate::sync_task::SyncGuard::new(());
        run_material_gesture_tick_core(&pool, &dir, &guard)
            .await
            .expect("gesto já visto: no-op");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn material_gesture_tick_core_publishes_and_advances_the_cursor_on_a_new_gesture() {
        let dir = test_dir();
        let pool = test_pool(&dir).await;
        seed_material_gesture(&pool, "2026-08-13 09:00:00").await;
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste")
            .await
            .unwrap();

        // Sem token persistido no keyring de teste, `ensure_drive_scope` falha ao resolver — o
        // mesmo caminho "sync ainda não configurado" do teste acima, só que chegado pelo tick.
        // Cobre o contrato do CURSOR (não avança sem uma tentativa RESOLVIDA); o caminho de
        // publicação de fato (`drive_checkin_core` com a borda HTTP mockada) já é coberto em
        // `commands::snapshot_cmds::tests`.
        let guard = crate::sync_task::SyncGuard::new(());
        run_material_gesture_tick_core(&pool, &dir, &guard)
            .await
            .expect("tentativa sem token configurável não deve propagar erro");
        assert!(
            crate::commands::app_setting_get(&pool, LAST_SEEN_GESTURE_KEY)
                .await
                .unwrap()
                .is_none(),
            "sem token válido, a tentativa não resolve — o cursor não avança"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn run_checkin_attempt_core_queues_behind_an_open_write_transaction_instead_of_deadlocking()
     {
        // Mesma classe de regressão já coberta em `state.rs`/`checkout.rs`/`snapshot_cmds.rs`: uma
        // leitura (`state::load_or_init`, dentro do núcleo) precisa ESPERAR uma transação de
        // escrita aberta no pool de UMA conexão, nunca travar para sempre.
        let dir = test_dir();
        let pool = test_pool(&dir).await;

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let pool_for_read = pool.clone();
        let dir_for_read = dir.clone();
        let read = tokio::spawn(async move {
            let guard = crate::sync_task::SyncGuard::new(());
            run_checkin_attempt_core(
                &pool_for_read,
                &dir_for_read,
                &guard,
                CheckinTrigger::MaterialGesture,
            )
            .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), read)
            .await
            .expect("NÃO pode travar para sempre esperando a única conexão")
            .expect("task não deve panicar");
        assert_eq!(
            result,
            Ok(false),
            "sem client id configurado: no-op, não erro"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
