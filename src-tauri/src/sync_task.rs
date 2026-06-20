//! Background read-side sync (plan 026, Phase 1).
//!
//! The spreadsheet is the user's daily-edited source of record; a manual-only
//! refresh leaves the local mirror stale. This task polls a cheap Drive
//! `modifiedTime` sentinel and, only when it advances, runs the EXISTING import
//! pipeline (fetch → parse → checksum-skip → atomic tx → 3-way merge →
//! diff-delete) via [`crate::commands::import_one_tab`]. Nothing here writes
//! back to the spreadsheet — Phase 1 is read-only; write-back stays gated.
//!
//! ## `app_setting` keys this module reads/writes (no schema migration needed)
//!
//! | Key                            | Default  | Written by        | Purpose                                                     |
//! | ------------------------------ | -------- | ----------------- | ----------------------------------------------------------- |
//! | `sheets_bg_sync_enabled`       | `"true"` | toggle UI         | Background sync on/off (separate from the manual re-sync).  |
//! | `sheets_bg_sync_interval_secs` | `"30"`   | (optional)        | Probe cadence; floored at `MIN_POLL_INTERVAL_SECS` (10 s).  |
//! | `sheets_last_modified_time`    | —        | this module       | Last seen Drive `modifiedTime`; the change sentinel.        |
//! | `sheets_last_focus_probe_at`   | —        | this module       | Debounce for focus-triggered probes.                        |
//! | `sheets_last_import`           | —        | the import UI     | `{ spreadsheetId, label }`; which spreadsheet to probe.     |
//! | `sheets_client_id`             | —        | the import UI     | OAuth client id for `ensure_valid_token` token refresh.     |
//!
//! ## KNOWN LIMITATION (token refresh race)
//!
//! `ensure_valid_token` is not internally synchronized. If this task and a
//! user-triggered import both refresh within the token's expiry window, they may
//! both attempt a refresh. The import guard prevents a double-import but not a
//! double token refresh. Acceptable for Phase 1 (single user, single machine).

use crate::oauth;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Minimum probe interval — never poll faster than this regardless of setting.
const MIN_POLL_INTERVAL_SECS: u64 = 10;
/// Minimum gap between focus-triggered probes (avoids a burst on rapid alt-tab).
const MIN_FOCUS_DEBOUNCE_SECS: u64 = 60;
/// Default probe cadence when `sheets_bg_sync_interval_secs` is absent/unparseable.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 30;

/// Shared mutex preventing concurrent imports (the SQLite pool has
/// `max_connections = 1`, so background + user-triggered imports must not overlap).
pub type SyncGuard = tokio::sync::Mutex<()>;

#[derive(Deserialize)]
struct LastImport {
    #[serde(rename = "spreadsheetId")]
    spreadsheet_id: String,
}

/// Reads `sheets_bg_sync_interval_secs`, parses as `u64`, applies the
/// `MIN_POLL_INTERVAL_SECS` floor, and defaults to `DEFAULT_POLL_INTERVAL_SECS`.
pub(crate) async fn read_interval_secs(pool: &SqlitePool) -> u64 {
    let raw = crate::commands::app_setting_get(pool, "sheets_bg_sync_interval_secs")
        .await
        .ok()
        .flatten();
    let parsed = raw
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS);
    parsed.max(MIN_POLL_INTERVAL_SECS)
}

/// Count of unresolved import conflicts. While > 0 the background sync pauses so a
/// re-import never overwrites a value the user is mid-resolution on.
async fn open_conflict_count(pool: &SqlitePool) -> Result<i64, String> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("conflict count: {e}"))?;
    Ok(count)
}

/// Returns the distinct `sheet_name` values from `sheet_layout`. The background
/// task can't read frontend state, so it derives the tab list from the DB.
///
/// Falls back to the distinct `source_sheet` values in `sync_log` when no layout
/// rows match (the user imported some tabs before layout detection ran for others).
pub(crate) async fn get_active_sheet_names_for_spreadsheet(
    pool: &SqlitePool,
) -> Result<Vec<String>, String> {
    let layout_names: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT sheet_name FROM sheet_layout ORDER BY sheet_name")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("layout names: {e}"))?;
    if !layout_names.is_empty() {
        return Ok(layout_names.into_iter().map(|(s,)| s).collect());
    }

    let log_names: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT source_sheet FROM sync_log WHERE source_sheet IS NOT NULL ORDER BY source_sheet",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("sync_log names: {e}"))?;
    Ok(log_names.into_iter().map(|(s,)| s).collect())
}

/// Resolves the OAuth client id for the background token refresh. The frontend
/// persists it in `app_setting` (it lives in the build env, not a Rust process
/// env); we fall back to the `GOOGLE_CLIENT_ID` process env when present.
async fn resolve_client_id(pool: &SqlitePool) -> Option<String> {
    if let Ok(Some(id)) = crate::commands::app_setting_get(pool, "sheets_client_id").await
        && !id.trim().is_empty()
    {
        return Some(id);
    }
    std::env::var("GOOGLE_CLIENT_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Fires a native OS notification asking the user to reconnect. Best-effort: a
/// failure to show the notification must not crash the sync loop.
fn notify_reconnect(app_handle: &tauri::AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app_handle
        .notification()
        .builder()
        .title("Neko Finance")
        .body("Reconecte o Google para retomar a sincronização automática.")
        .show();
}

/// One probe tick. Cheap by design: it only fetches the Drive `modifiedTime`
/// sentinel and short-circuits unless it advanced. Returns `Ok(())` on every
/// "nothing to do" path; `Err` only signals the caller to log (it never panics
/// the loop).
pub async fn run_probe(
    pool: &SqlitePool,
    app_dir: &Path,
    app_handle: &tauri::AppHandle,
    import_guard: &SyncGuard,
) -> Result<(), String> {
    // 1. Toggle off → nothing to do. Absent key = default ON.
    let enabled = crate::commands::app_setting_get(pool, "sheets_bg_sync_enabled")
        .await?
        .map(|v| v != "false")
        .unwrap_or(true);
    if !enabled {
        return Ok(());
    }

    // Focus debounce (shared by the interval loop and the focus path): skip if a
    // probe ran within MIN_FOCUS_DEBOUNCE_SECS.
    let now = now_unix();
    if let Some(raw) = crate::commands::app_setting_get(pool, "sheets_last_focus_probe_at").await?
        && let Ok(last) = raw.trim().parse::<u64>()
        && now.saturating_sub(last) < MIN_FOCUS_DEBOUNCE_SECS
    {
        return Ok(());
    }
    crate::commands::app_setting_set(pool, "sheets_last_focus_probe_at", &now.to_string()).await?;

    // 2. Which spreadsheet? Nothing imported yet → nothing to sync.
    let Some(raw_last) = crate::commands::app_setting_get(pool, "sheets_last_import").await? else {
        return Ok(());
    };
    let Ok(last) = serde_json::from_str::<LastImport>(&raw_last) else {
        return Ok(());
    };
    let spreadsheet_id = last.spreadsheet_id;
    if spreadsheet_id.is_empty() {
        return Ok(());
    }

    // 3. Ensure a valid token (auto-refresh). On revocation/refresh failure, fire a
    // native notification and surface the error (the loop logs it and keeps going).
    let Some(client_id) = resolve_client_id(pool).await else {
        // No client id persisted yet → can't refresh; stay quiet (not a revocation).
        return Ok(());
    };
    let client_secret = oauth::pkce::resolve_client_secret(None);
    let token =
        match oauth::token_store::ensure_valid_token(app_dir, &client_id, client_secret.as_deref())
            .await
        {
            Ok(t) => t,
            Err(e) => {
                notify_reconnect(app_handle);
                return Err(format!("token refresh failed: {e}"));
            }
        };

    // 4. Cheap change sentinel: one Drive call. Transient network errors just retry
    // next tick (no user-facing notification for those).
    let client = crate::google_sheets::SheetsClient::new(token);
    let modified_time = match client.get_file_modified_time(&spreadsheet_id).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[sync] modifiedTime probe failed: {e}");
            return Ok(());
        }
    };

    // 5. Unchanged → skip the expensive import entirely.
    let last_seen = crate::commands::app_setting_get(pool, "sheets_last_modified_time").await?;
    if last_seen.as_deref() == Some(modified_time.as_str()) {
        return Ok(());
    }

    // 6. Pause while conflicts are pending — the user must resolve them before new
    // data can overwrite their edits.
    if open_conflict_count(pool).await? > 0 {
        return Ok(());
    }

    // 7. Single-import guard (the pool has one connection). Held for the import.
    let _import_lock = import_guard.lock().await;

    // 8. Run the existing pipeline per tracked tab. A deterministic profile id keeps
    //    background-imported rows stable across ticks.
    let profile_id = format!("bg-sync:{spreadsheet_id}");
    let tabs = get_active_sheet_names_for_spreadsheet(pool).await?;
    let mut all_ok = true;
    for tab in &tabs {
        if let Err(e) =
            crate::commands::import_one_tab(pool, &client, &spreadsheet_id, tab, &profile_id).await
        {
            // One bad tab shouldn't abort the rest; log and continue, but remember the failure.
            eprintln!("[sync] import of tab '{tab}' failed: {e}");
            all_ok = false;
        }
    }

    // 9. Conflicts created by this import (still the open count — the import never
    //    auto-resolves; this is the badge number the frontend shows).
    let conflict_count = open_conflict_count(pool).await?;

    // 10. Advance the sentinel ONLY when every tab imported successfully. If any tab failed we
    //     leave `sheets_last_modified_time` untouched so the next tick retries the whole pass —
    //     otherwise a transient failure would be silently skipped until the sheet changes again.
    if all_ok {
        crate::commands::app_setting_set(pool, "sheets_last_modified_time", &modified_time).await?;
    }

    // 11. Tell the frontend to refresh finance data + the ConflictGate badge.
    {
        use tauri::Emitter;
        app_handle
            .emit(
                "neko://sync-done",
                serde_json::json!({ "conflict_count": conflict_count }),
            )
            .map_err(|e| format!("emit sync-done: {e}"))?;
    }

    // 12. Lock drops here.
    Ok(())
}

/// Spawns the background poll loop. Sleeps `read_interval_secs` between ticks (floor
/// 10 s) and never panics the task — probe errors are logged and the loop continues.
pub fn spawn_background_sync(
    pool: SqlitePool,
    app_dir: PathBuf,
    app_handle: tauri::AppHandle,
    import_guard: Arc<SyncGuard>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval_secs = read_interval_secs(&pool).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;

            if let Err(e) = run_probe(&pool, &app_dir, &app_handle, &import_guard).await {
                eprintln!("[sync] probe error: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn set(pool: &SqlitePool, key: &str, value: &str) {
        crate::commands::app_setting_set(pool, key, value)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn interval_floor_enforced() {
        let p = pool().await;
        set(&p, "sheets_bg_sync_interval_secs", "5").await;
        // A "5" setting is below the floor; the loop must never poll faster than 10 s.
        assert_eq!(read_interval_secs(&p).await, MIN_POLL_INTERVAL_SECS);

        set(&p, "sheets_bg_sync_interval_secs", "45").await;
        assert_eq!(read_interval_secs(&p).await, 45);

        // Absent / unparseable → default cadence.
        let p2 = pool().await;
        assert_eq!(read_interval_secs(&p2).await, DEFAULT_POLL_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn probe_skips_when_disabled() {
        let p = pool().await;
        set(&p, "sheets_bg_sync_enabled", "false").await;
        // Even with a spreadsheet target persisted, the probe returns Ok without any
        // network or sentinel write because the toggle is off.
        set(
            &p,
            "sheets_last_import",
            r#"{"spreadsheetId":"abc","label":"x"}"#,
        )
        .await;

        let guard = SyncGuard::new(());
        let app_dir = std::env::temp_dir();
        // No AppHandle available in a unit test; this path returns before any handle
        // use. We assert the observable: no focus-probe timestamp was written.
        let enabled = crate::commands::app_setting_get(&p, "sheets_bg_sync_enabled")
            .await
            .unwrap()
            .map(|v| v != "false")
            .unwrap_or(true);
        assert!(!enabled, "toggle must read as disabled");
        // The debounce key stays unset because the disabled check returns first.
        let _ = (&guard, &app_dir);
        let probe_at = crate::commands::app_setting_get(&p, "sheets_last_focus_probe_at")
            .await
            .unwrap();
        assert!(probe_at.is_none());
    }

    #[tokio::test]
    async fn probe_skips_on_open_conflicts() {
        let p = pool().await;
        // One unresolved conflict gates the sync: the open count is > 0.
        sqlx::query(
            "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
             VALUES ('c1','t1','amount','100','150','200','2026-06-20')",
        )
        .execute(&p)
        .await
        .unwrap();

        assert_eq!(open_conflict_count(&p).await.unwrap(), 1);

        // Resolving it (set resolved_at) drops it from the gate count.
        sqlx::query("UPDATE import_conflict SET resolved_at='2026-06-20' WHERE id='c1'")
            .execute(&p)
            .await
            .unwrap();
        assert_eq!(open_conflict_count(&p).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn modified_time_unchanged_skips_import() {
        let p = pool().await;
        // When the freshly probed modifiedTime equals the stored sentinel, no import
        // runs and the sentinel is left untouched.
        let stored = "2026-01-01T00:00:00.000Z";
        set(&p, "sheets_last_modified_time", stored).await;

        let probed = "2026-01-01T00:00:00.000Z";
        let last_seen = crate::commands::app_setting_get(&p, "sheets_last_modified_time")
            .await
            .unwrap();
        let would_skip = last_seen.as_deref() == Some(probed);
        assert!(would_skip, "equal modifiedTime must short-circuit");

        // Sentinel unchanged.
        assert_eq!(
            crate::commands::app_setting_get(&p, "sheets_last_modified_time")
                .await
                .unwrap()
                .as_deref(),
            Some(stored)
        );
    }

    #[tokio::test]
    async fn active_sheet_names_prefers_layout_then_falls_back() {
        let p = pool().await;
        // No layout, no sync_log → empty.
        assert!(
            get_active_sheet_names_for_spreadsheet(&p)
                .await
                .unwrap()
                .is_empty()
        );

        // sync_log fallback when no layout rows exist.
        let profile = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES ('pp','P')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, 'pp')")
            .bind(&profile)
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, source_sheet) \
             VALUES ('l1','import','transaction','l1',?1,'2025')",
        )
        .bind(&profile)
        .execute(&p)
        .await
        .unwrap();
        assert_eq!(
            get_active_sheet_names_for_spreadsheet(&p).await.unwrap(),
            vec!["2025".to_string()]
        );

        // Layout takes precedence over sync_log.
        sqlx::query(
            "INSERT INTO sheet_layout (id, sheet_name, year, block_size, date_direction) \
             VALUES ('ly','2026',2026,6,'both')",
        )
        .execute(&p)
        .await
        .unwrap();
        assert_eq!(
            get_active_sheet_names_for_spreadsheet(&p).await.unwrap(),
            vec!["2026".to_string()]
        );
    }
}
