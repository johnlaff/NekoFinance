# Plan 026: Implement Phase 1 read-side real-time spreadsheet sync

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 8bb090b..HEAD -- src-tauri/src/lib.rs src-tauri/src/google_sheets/mod.rs src-tauri/src/google_sheets/import.rs src-tauri/src/google_sheets/write_back.rs src-tauri/src/commands/write_back_cmds.rs src-tauri/src/oauth/token_store.rs src/features/sheets/GoogleSheetsPanel.tsx src/features/reconcile/ConflictGate.tsx src/lib/useCommand.ts`
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: plans/002-atomic-sheet-import.md (atomic import transaction must be landed)
- **Category**: tech-debt / feature
- **Planned at**: commit `8bb090b`, 2026-06-20

## Why this matters

Today every data refresh requires a manual user action. The spreadsheet is
the user's daily-edited source of record; any stale SQLite mirror means the
dashboard shows yesterday's numbers. This plan promotes the spike in
`plans/021-spike-realtime-sheets-sync.md` to working code: a poll-based
background sync (the provider's push notifications require a public HTTPS
endpoint a local-first desktop cannot host) with a cheap Drive `modifiedTime`
sentinel that avoids burning the per-minute Sheets read quota on unchanged
files, plus an immediate probe on window focus. Phase 1 is READ-SIDE ONLY —
write-back remains locked behind `WRITE_BACK_ENABLED = false` (Phase 2, not
in scope here).

## Current state

### Files and roles

- `src-tauri/src/lib.rs` — Tauri entry point. `setup()` (line 67) opens the
  SQLite pool with WAL, runs migrations, calls `app.manage(pool)` at line 123.
  No background tasks today. The new background sync task spawns here, after
  `app.manage(pool)` and before `Ok(())`.
- `src-tauri/src/google_sheets/mod.rs` — `SheetsClient` struct (line 36).
  Existing methods: `get_sheet_values` (line 45), `get_sheet_notes` (line 88),
  `get_sheet_metadata` (line 136), `batch_update_values` (line 166 — write only,
  never called by this plan). Add a new `get_file_modified_time` method here
  using the Drive API's `files.get?fields=modifiedTime` endpoint.
- `src-tauri/src/google_sheets/import.rs` — import domain core.
  - `compute_checksum` (line 112): SHA-256 over row fields.
  - `check_duplicate_import` (line 162): `SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1 AND checksum = ?2` — early-exit when data is unchanged.
  - `import_rows_with_options` / `import_rows_with_options_in_tx` — the full UPSERT + three-way merge + diff-delete pipeline.
  - `row_id` (line 149): deterministic `sha256("txn-v1|{sheet}|{date}|{kind}|{slot}")`.
- `src-tauri/src/google_sheets/write_back.rs` — line 14: `pub const WRITE_BACK_ENABLED: bool = false;`. **Do NOT touch this file.**
- `src-tauri/src/commands/write_back_cmds.rs` — `app_setting_get` (line 187) and `app_setting_set` (line 209): internal helpers (non-command) for reading/writing `app_setting`. The background task reuses these.
- `src-tauri/src/commands/sheets_import.rs` — `import_sheet_data` command (line 78). This is the full import pipeline (fetch → parse → checksum-check → atomic tx → upsert → diff-delete). The background task calls the same domain-layer functions, not this command.
- `src-tauri/src/oauth/token_store.rs` — `ensure_valid_token` (line 316): reads the stored token, refreshes via `refresh_access_token` if expired, returns `Err` on revocation. The background task calls this before each probe.
- `src-tauri/src/http.rs` — `send_with_retry` (line 58): 3 attempts, 400 ms×attempt backoff, caps `Retry-After` at 10 s. All network calls from the background task go through this.
- `src/features/sheets/GoogleSheetsPanel.tsx` — `LAST_IMPORT_KEY = "sheets_last_import"` (line 64); `handleResync` (line 404) calls `importAllTabs(state.lastImport.spreadsheetId)`. The manual re-sync button is separate from background sync — do not remove it.
- `src/features/reconcile/ConflictGate.tsx` — self-hides when the conflict list is empty (line 64). Currently polls `getImportConflicts()` once on mount (line 41). This plan wires it to re-poll when the `neko://sync-done` Tauri event arrives.
- `src/lib/useCommand.ts` — `invalidateCommands()` (line 25) clears the entire SWR-lite cache. Called after every write/import so finance numbers refresh on next render.

### Existing `app_setting` keys (do not collide)

- `sheets_last_import` — JSON `{ spreadsheetId, label }`. Written by `persistLastImport` in `GoogleSheetsPanel.tsx:300`. The background task reads this to know which spreadsheet to probe.
- `onboarding_done` — unrelated; present as example of the KV pattern.

### New `app_setting` keys this plan introduces

| Key                            | Default  | Type                  | Purpose                                                                     |
| ------------------------------ | -------- | --------------------- | --------------------------------------------------------------------------- |
| `sheets_bg_sync_enabled`       | `"true"` | `"true"` \| `"false"` | Background sync toggle (user-configurable, separate from manual re-sync)    |
| `sheets_bg_sync_interval_secs` | `"30"`   | integer string        | Probe cadence (floor 10 s; never poll faster than `MIN_POLL_INTERVAL = 10`) |
| `sheets_last_modified_time`    | —        | RFC-3339 string       | Last seen `modifiedTime` from Drive; per-spreadsheet sentinel               |

### ADR-0003 constraint (non-negotiable)

From `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md`:

> "Every material write requires a structured before→after diff, validation,
> and explicit human approval."

The sync loop only reads. Auto-write to the spreadsheet is Phase 2, gated
separately.

### Drive API endpoint for the change sentinel

`GET https://www.googleapis.com/drive/v3/files/{fileId}?fields=modifiedTime`

Returns `{ "modifiedTime": "2026-06-20T15:32:00.000Z" }`. Requires the
`https://www.googleapis.com/auth/drive.readonly` or
`https://www.googleapis.com/auth/drive.metadata.readonly` scope. Confirm the
existing OAuth scope in `src-tauri/src/oauth/mod.rs` includes one of these;
if not, add `drive.metadata.readonly` to the scope list (it is significantly
narrower than `drive.readonly`). One Drive API call per probe tick replaces
the multiple Sheets `values.get` calls that would otherwise probe each tab.

### Quota budget (verified arithmetic)

- Background probe (Drive `files.get`): 1 call/probe tick. At 30 s interval: 2 calls/min. Drive quota: 1000 req/100 s/user — probe is negligible.
- Full import (on change): 1 `spreadsheets.values.batchGet` per tracked tab (≤13 tabs). At most 1 full import / probe that detects change. Sheets quota: 60 req/min/user. Even a continuous stream of changes at 30 s cadence → ≤2 full imports/min × 13 calls = 26 req/min — well within budget.
- Window-focus trigger: debounced to `MIN_FOCUS_DEBOUNCE = 60` s; immediate probe only.

## Commands you will need

| Purpose            | Command                | Expected on success                |
| ------------------ | ---------------------- | ---------------------------------- |
| Typecheck (TS)     | `npm run typecheck`    | exit 0, no errors                  |
| Lint               | `npm run lint`         | exit 0, no errors                  |
| Unit tests (front) | `npm run test:run`     | all pass                           |
| Rust check         | `npm run rust:check`   | exit 0 (fmt + clippy + cargo test) |
| Full gate          | `npm run check`        | exit 0                             |
| Privacy scan       | `npm run privacy:scan` | exit 0                             |
| E2E smoke          | `npm run e2e`          | exit 0                             |

(All verified at `8bb090b`; use exactly as written.)

## Suggested executor toolkit

- Read `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md` in full
  before touching any sync path. The ADR is the final authority on write-back
  gating.
- Read `CONTEXT.md` at repo root for canonical vocabulary (Transaction,
  EventKind, sync_log, ConflictGate, MergeDecision).
- Use the `neko-finance-design` skill if adding any new UI component (tokens
  are in `src/design-system/`).
- React Compiler is ON: do NOT add manual `useMemo`/`useCallback`. Write
  stable module-level arrow functions for `useCommand` fetchers.

## Scope

**In scope** (the only files you should create or modify):

- `src-tauri/src/google_sheets/mod.rs` — add `get_file_modified_time` method to `SheetsClient`
- `src-tauri/src/lib.rs` — spawn background sync task; add `WindowEvent::Focused` handler
- `src-tauri/src/sync_task.rs` — CREATE: the background sync task module
- `src-tauri/Cargo.toml` — add `tauri-plugin-notification` dependency
- `src-tauri/capabilities/default.json` — add `notification:default` permission
- `src/features/sheets/GoogleSheetsPanel.tsx` — add bg-sync toggle UI; wire `neko://sync-done` listener
- `src/features/reconcile/ConflictGate.tsx` — subscribe to `neko://sync-done` to re-poll
- `src/lib/api.ts` — add `listen` wrapper (if not already present)
- `src-tauri/src/sync_task.rs` tests (inline `#[cfg(test)]`) — unit tests
- `src/features/sheets/GoogleSheetsPanel.test.tsx` (or new file) — frontend tests

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/write_back.rs` — **NEVER**. Phase 2 only. If you find yourself editing this file, STOP.
- `WRITE_BACK_ENABLED` flag — stays `false`. Do not read it, gate on it, or reference it in new sync code.
- `src-tauri/src/commands/sheets_import.rs` — call the underlying domain functions; do not modify the command surface.
- Any migration file — no schema changes needed (new `app_setting` keys require no migration; the table already exists).
- Concurrent write-back paths (`preview_write_back`, `apply_write_back`) — Phase 2.
- Multi-device sync, push relay, Mia/copilot tool integrations.

## Git workflow

- Branch: `feat/026-realtime-sync-phase1`
- Commit per logical step. Match the repo's conventional-commit style observed in `git log`: `feat:`, `fix:`, `chore:` prefixes; present tense imperative body.
  Example: `feat: add Drive modifiedTime probe to SheetsClient`
- Do NOT push or open a PR unless the operator explicitly instructs it.

## Steps

### Step 1: Confirm baseline is green and write_back.rs is untouched

Before writing a line of code, verify the baseline.

1. Run the drift check from the executor instructions header.
2. Confirm `WRITE_BACK_ENABLED` is still `false`:
   `grep "WRITE_BACK_ENABLED" src-tauri/src/google_sheets/write_back.rs`
   → must print `pub const WRITE_BACK_ENABLED: bool = false;`
3. Confirm the existing OAuth scope list in `src-tauri/src/oauth/mod.rs`:
   `grep -n "scope\|drive" src-tauri/src/oauth/mod.rs`
   Note whether `drive.metadata.readonly` or `drive.readonly` is present.
   If neither is present, you must add `drive.metadata.readonly` in Step 2.

**Verify**: `npm run check` → exit 0 (full gate clean; no changes yet).

### Step 2: Add `get_file_modified_time` to `SheetsClient` in `src-tauri/src/google_sheets/mod.rs`

Add the following method to `SheetsClient` (after `get_sheet_metadata`, around line 160 in the current file):

```rust
/// Probes the Drive metadata for a spreadsheet file to cheaply detect whether
/// it has changed since we last imported. Returns the RFC-3339 `modifiedTime`
/// string from the Drive API's `files.get` endpoint.
///
/// One Drive call replaces N Sheets reads as a change sentinel — the full
/// `spreadsheets.values.batchGet` only runs when `modifiedTime` advanced.
pub async fn get_file_modified_time(
    &self,
    file_id: &str,
) -> Result<String, String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{file_id}?fields=modifiedTime",
    );
    let resp = crate::http::send_with_retry(
        crate::http::client()
            .get(&url)
            .bearer_auth(&self.token.access_token),
    )
    .await
    .map_err(|e| format!("drive files.get error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Drive API error {status}: {body}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("drive modifiedTime parse: {e}"))?;

    json["modifiedTime"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "modifiedTime field absent from Drive response".into())
}
```

If Step 1 found that `drive.metadata.readonly` is absent from the OAuth scope
list in `src-tauri/src/oauth/mod.rs`, add it now to the scope string. Do not
add broader scopes.

**Verify**: `npm run rust:check` → exit 0.

### Step 3: Add `tauri-plugin-notification` to Cargo.toml and capabilities

3a. In `src-tauri/Cargo.toml`, under `[dependencies]`, add:

```toml
tauri-plugin-notification = "=2"
```

Pin to the `2.x` series that matches `tauri = "=2.11.2"`. Check
`https://crates.io/crates/tauri-plugin-notification` for the latest `2.x`
patch; use the exact version (e.g. `"=2.2.1"`). Document the pinned version
in `docs/version-matrix.md` under the Tauri plugins section (add the section
if absent).

3b. In `src-tauri/capabilities/default.json`, add `"notification:default"` to
the `"permissions"` array:

```json
"permissions": [
  "core:default",
  "opener:default",
  "dialog:allow-open",
  "dialog:allow-save",
  "notification:default"
]
```

3c. In `src-tauri/src/lib.rs`, add the plugin init before existing plugins:

```rust
.plugin(tauri_plugin_notification::init())
```

(Place it before `.plugin(tauri_plugin_opener::init())`.)

**Verify**: `npm run rust:check` → exit 0 (plugin resolves and compiles).

### Step 4: Create `src-tauri/src/sync_task.rs` — the background sync module

Create this new file. It must contain:

#### 4a. Constants

```rust
/// Minimum probe interval — never poll faster than this regardless of setting.
const MIN_POLL_INTERVAL_SECS: u64 = 10;
/// Minimum gap between focus-triggered probes (avoids burst on rapid alt-tab).
const MIN_FOCUS_DEBOUNCE_SECS: u64 = 60;
```

#### 4b. `SyncGuard` type alias

```rust
/// Shared mutex preventing concurrent imports (the SQLite pool has max_connections=1).
pub type SyncGuard = tokio::sync::Mutex<()>;
```

#### 4c. `run_probe` async fn (the inner loop body)

Signature:

```rust
pub async fn run_probe(
    pool: &sqlx::SqlitePool,
    app_dir: &std::path::Path,
    app_handle: &tauri::AppHandle,
    import_guard: &SyncGuard,
) -> Result<(), String>
```

Logic:

1. Read `sheets_bg_sync_enabled` from `app_setting`. If `"false"`, return `Ok(())` immediately.
2. Read `sheets_last_import` from `app_setting`. If absent or unparseable as
   `{ spreadsheetId: String, label: String }` JSON, return `Ok(())` (nothing
   to sync yet).
3. Call `oauth::token_store::ensure_valid_token(app_dir, &client_id, client_secret)`.
   - `client_id` must come from the same compile-time env var as commands
     (`GOOGLE_CLIENT_ID` — see `commands/oauth_cmds.rs` for the existing
     pattern; the background task reads the same env var).
   - On `Err` (token revoked / network failure): send a native OS notification
     via `tauri_plugin_notification::NotificationExt` with title "Neko Finance"
     and body "Reconecte o Google para retomar a sincronização automática.";
     return `Err(msg)` (the caller logs it and continues the loop without
     crashing the task).
4. Call `SheetsClient::new(token).get_file_modified_time(&spreadsheet_id).await`.
   On `Err`: log (do not notify the user for transient network errors); return
   `Ok(())` to let the loop retry next tick.
5. Read `sheets_last_modified_time` from `app_setting`.
   - If the new `modifiedTime` string equals the stored value → data unchanged,
     return `Ok(())`.
   - Otherwise proceed to step 6.
6. **Conflict check** — skip re-import when conflicts are pending:
   ```sql
   SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL
   ```
   If count > 0, return `Ok(())` without importing (the user must resolve
   open conflicts before new data overwrites them).
7. Acquire `import_guard.lock().await` (blocks until any concurrent import
   finishes, then holds the lock for the duration of the import).
8. Run the full import pipeline for all tracked tabs. The tracked tab list is
   derived from the existing `sheet_mapping` or `sheet_layout` tables for the
   `spreadsheetId`. Use `get_active_sheet_names_for_spreadsheet` (a new
   internal helper, see below) to get the tab list without touching React state.
9. Sum `conflict_count` across all imported tabs (count of new rows inserted
   into `import_conflict WHERE resolved_at IS NULL` after the import).
10. Update `sheets_last_modified_time` in `app_setting` with the new value.
11. Emit Tauri event `neko://sync-done` with payload `{ "conflict_count": N }`:
    ```rust
    use tauri::Emitter;
    app_handle.emit("neko://sync-done", serde_json::json!({ "conflict_count": conflict_count }))?;
    ```
12. Release the lock (drop guard).

#### 4d. `get_active_sheet_names_for_spreadsheet` helper

```rust
/// Returns the distinct `sheet_name` values from `sheet_layout` that were
/// imported from `spreadsheet_id`. The background task can't read React state,
/// so it derives the tab list from the DB instead.
///
/// Falls back to all distinct `source_sheet` values in `sync_log` for the
/// profile when no layout rows match (handles the case where the user imported
/// only some tabs and layout detection hasn't run for others).
pub(crate) async fn get_active_sheet_names_for_spreadsheet(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<String>, String>
```

Query `SELECT DISTINCT sheet_name FROM sheet_layout ORDER BY sheet_name` as
the primary source. If empty, fall back to
`SELECT DISTINCT source_sheet FROM sync_log WHERE source_sheet IS NOT NULL ORDER BY source_sheet`.

#### 4e. `spawn_background_sync` pub fn

```rust
pub fn spawn_background_sync(
    pool: sqlx::SqlitePool,
    app_dir: std::path::PathBuf,
    app_handle: tauri::AppHandle,
    import_guard: std::sync::Arc<SyncGuard>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            // Read interval from app_setting; default 30 s; floor MIN_POLL_INTERVAL_SECS.
            let interval_secs = read_interval_secs(&pool).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;

            if let Err(e) = run_probe(&pool, &app_dir, &app_handle, &import_guard).await {
                // Log only — never panic the background task.
                eprintln!("[sync] probe error: {e}");
            }
        }
    });
}
```

`read_interval_secs` reads `sheets_bg_sync_interval_secs` from `app_setting`,
parses as `u64`, applies the `MIN_POLL_INTERVAL_SECS` floor, defaults to 30.

#### 4f. Unit tests (inline `#[cfg(test)]` module)

Model after `src-tauri/src/google_sheets/reconcile.rs` lines 73–163.

Tests to include:

- `interval_floor_enforced`: verify `read_interval_secs` returns at least
  `MIN_POLL_INTERVAL_SECS` when the setting holds `"5"`.
- `probe_skips_when_disabled`: given `sheets_bg_sync_enabled = "false"`,
  `run_probe` returns `Ok(())` without touching the network (mock the
  `SheetsClient` — use a test double or integration-style in-memory pool where
  checking the setting is the observable behaviour).
- `probe_skips_on_open_conflicts`: insert one row into `import_conflict WHERE
resolved_at IS NULL`; assert `run_probe` returns `Ok(())` without
  incrementing any sync_log row.
- `modified_time_unchanged_skips_import`: set `sheets_last_modified_time` to
  `"2026-01-01T00:00:00.000Z"` and arrange `get_file_modified_time` to return
  the same string; assert no import occurs and `sheets_last_modified_time` in
  `app_setting` is unchanged.

**Verify**: `npm run rust:check` → exit 0; `cargo test --manifest-path src-tauri/Cargo.toml --locked` → all pass including the four new tests.

### Step 5: Wire the background task and window-focus handler in `src-tauri/src/lib.rs`

5a. Add `mod sync_task;` near the top of `lib.rs` (alongside the existing module declarations).

5b. Add `use std::sync::Arc;` if not already present.

5c. After `app.manage(pool)` (currently at line 123) and before `Ok(())`,
add:

```rust
// Background sync task (Phase 1: read-only). Spawned after the pool is managed
// so it can clone the Arc. Never touches WRITE_BACK_ENABLED.
let import_guard = Arc::new(sync_task::SyncGuard::new(()));
app.manage(import_guard.clone());
sync_task::spawn_background_sync(
    pool.clone(),
    app_dir.clone(),
    app.handle().clone(),
    import_guard,
);
```

5d. Add a window-event listener for focus. In Tauri 2.x, use
`app.on_window_event`:

```rust
// Focus-triggered probe: fires when the user switches back to the app.
// Debounced (MIN_FOCUS_DEBOUNCE_SECS) inside run_probe to avoid burst.
{
    let pool_focus = pool.clone();
    let app_dir_focus = app_dir.clone();
    let guard_focus = app.state::<Arc<sync_task::SyncGuard>>().inner().clone();
    let handle_focus = app.handle().clone();
    app.on_window_event(move |_window, event| {
        if let tauri::WindowEvent::Focused(true) = event {
            let pool = pool_focus.clone();
            let app_dir = app_dir_focus.clone();
            let guard = guard_focus.clone();
            let handle = handle_focus.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sync_task::run_probe(&pool, &app_dir, &handle, &guard).await {
                    eprintln!("[sync/focus] probe error: {e}");
                }
            });
        }
    });
}
```

Note: `pool` is an `Arc<Pool>` internally; `.clone()` is cheap. `app_dir` is
a `PathBuf` — clone is also cheap here.

Focus debounce must be enforced inside `run_probe` so both the background
loop and the focus path share the same guard. Add it at the top of
`run_probe` after the enabled check: read `sheets_last_focus_probe_at` from
`app_setting`; if less than `MIN_FOCUS_DEBOUNCE_SECS` ago, return `Ok(())`.
Update `sheets_last_focus_probe_at` to `now` before acquiring the import lock.

Add `sheets_last_focus_probe_at` (ISO-8601 timestamp, default absent) to the
new `app_setting` keys table in the Current state section — this is written
by `run_probe` to implement the debounce; no schema migration needed.

**Verify**: `npm run rust:check` → exit 0.

### Step 6: Frontend — wire `neko://sync-done` in `ConflictGate.tsx`

`ConflictGate` currently polls `getImportConflicts()` once on mount
(line 41). Add a Tauri event listener so it re-polls when the background sync
emits `neko://sync-done`:

```tsx
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../../lib/api";

// Inside the ConflictGate component, add alongside the existing useEffect:
useEffect(() => {
  if (!isTauri) return;
  let unlisten: (() => void) | undefined;
  listen<{ conflict_count: number }>("neko://sync-done", () => {
    let alive = true;
    getImportConflicts()
      .then((c) => alive && setConflicts(c))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }).then((fn) => {
    unlisten = fn;
  });
  return () => {
    unlisten?.();
  };
}, []);
```

Also call `invalidateCommands()` inside the `neko://sync-done` handler before
the `getImportConflicts()` call so that all other finance data (dashboard
numbers, month grid, etc.) also refreshes:

```tsx
listen<{ conflict_count: number }>("neko://sync-done", () => {
  invalidateCommands();
  getImportConflicts()
    .then((c) => setConflicts(c))
    .catch(() => undefined);
});
```

The `listen` call must happen inside `useEffect` with cleanup (`unlisten()`
on unmount) to avoid leaking the listener on hot module replacement.

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0.

### Step 7: Frontend — add bg-sync toggle UI in `GoogleSheetsPanel.tsx`

7a. Add a new `app_setting` key constant near `LAST_IMPORT_KEY` (line 64):

```tsx
const BG_SYNC_KEY = "sheets_bg_sync_enabled";
```

7b. Add `bgSyncEnabled: boolean` to `SheetState` (defaults to `true`).

7c. On mount (alongside the existing `getAppSetting(LAST_IMPORT_KEY)` effect),
load `BG_SYNC_KEY`:

```tsx
getAppSetting(BG_SYNC_KEY).then((raw) => {
  if (!alive) return;
  // Absent key = default ON.
  dispatch({ type: "set", patch: { bgSyncEnabled: raw !== "false" } });
});
```

7d. Add a toggle control in the panel's settings/footer area. Use a native
`<input type="checkbox">` with a `<label>` (accessible, no custom component
required). When toggled, call `setAppSetting(BG_SYNC_KEY, checked ? "true" : "false")`.

Design guidance: place the toggle after the "Re-sincronizar" button in the
panel footer. Label text: "Atualização automática". Keep it small and
unobtrusive — this is a secondary control. Use `var(--fs-label)` for the
label font size, consistent with the rest of the panel.

7e. Do NOT remove or disable the manual "Re-sincronizar" button. The toggle
controls only the background task; manual sync is always available.

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0.

### Step 8: Frontend tests

Add tests to `src/features/sheets/GoogleSheetsPanel.test.tsx` (or create it
if absent) and `src/features/reconcile/ConflictGate.test.tsx`.

Model new tests after existing patterns in `src/features/reconcile/ConflictGate.test.tsx`.

Tests to write:

1. **`toggle persists BG_SYNC_KEY`**: render `GoogleSheetsPanel` with a mock
   `getAppSetting` that returns `"true"`; click the "Atualização automática"
   checkbox; assert `setAppSetting` was called with `(BG_SYNC_KEY, "false")`.

2. **`toggle default is ON when key absent`**: mock `getAppSetting` to return
   `null`; assert checkbox is checked on mount.

3. **`ConflictGate re-polls on sync-done event`**: mock `listen` to capture
   the callback; call the callback manually; assert `getImportConflicts` is
   called a second time (once on mount + once on event).

4. **`ConflictGate unlisten called on unmount`**: assert the `unlisten` stub
   returned by `listen` is invoked when the component unmounts.

**Verify**: `npm run test:run` → all pass, including the 4 new tests.

### Step 9: Full gate, privacy scan, E2E smoke

**Verify**:

1. `npm run check` → exit 0
2. `npm run privacy:scan` → exit 0 (no spreadsheet URLs, tokens, or private names in new files)
3. `npm run e2e` → exit 0 (visual smoke passes; inspect screenshots if any layout change is visible)
4. `grep "WRITE_BACK_ENABLED" src-tauri/src/google_sheets/write_back.rs` → `pub const WRITE_BACK_ENABLED: bool = false;`
5. `git diff --name-only HEAD` shows only files listed in the Scope section

### Step 10: Update `plans/README.md`

Mark plan 026 status as `IN PROGRESS` while working, then `DONE` when all
Done criteria are met. Update the row — do not change other rows.

**Verify**: `grep "026" plans/README.md` → shows `| 026 | ... | DONE |`.

## Test plan

### Rust unit tests (in `src-tauri/src/sync_task.rs`)

All four tests listed in Step 4f, using in-memory SQLite pools (pattern from
`src-tauri/src/lib.rs` lines 135–155: `SqlitePoolOptions::new().connect("sqlite::memory:")`):

- `interval_floor_enforced`
- `probe_skips_when_disabled`
- `probe_skips_on_open_conflicts`
- `modified_time_unchanged_skips_import`

### Frontend tests (in `src/features/sheets/` and `src/features/reconcile/`)

All four tests listed in Step 8.

### Integration / manual smoke (post-implementation)

After the implementation compiles and all automated tests pass, the executor
should do a minimal manual smoke:

1. Run `npm run tauri dev`.
2. Connect a real OAuth account and import one tab.
3. Make a cell edit in the spreadsheet in the browser.
4. Wait ≤60 s; confirm the app emits `neko://sync-done` (visible in the
   Tauri dev console as an event) and the dashboard numbers change without
   user action.
5. Open the ConflictGate panel; if conflicts were created, verify the badge
   appears.
6. Toggle "Atualização automática" off; wait 60 s; make another edit; confirm
   no sync fires.

## Done criteria

ALL must hold:

- [ ] `npm run check` exits 0
- [ ] `npm run privacy:scan` exits 0
- [ ] `npm run e2e` exits 0
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` passes; the four new Rust tests in `sync_task.rs` exist and pass
- [ ] `npm run test:run` passes; the four new frontend tests exist and pass
- [ ] `grep "WRITE_BACK_ENABLED" src-tauri/src/google_sheets/write_back.rs` returns `pub const WRITE_BACK_ENABLED: bool = false;`
- [ ] `grep -n "write_back" src-tauri/src/sync_task.rs` returns no matches (the sync module never references write-back)
- [ ] `app_setting` keys `sheets_bg_sync_enabled`, `sheets_bg_sync_interval_secs`, `sheets_last_modified_time`, `sheets_last_focus_probe_at` are documented in comments in `sync_task.rs`
- [ ] `tauri-plugin-notification` version is pinned in `Cargo.toml` and documented in `docs/version-matrix.md`
- [ ] `neko://sync-done` event payload type is `{ conflict_count: number }` (TypeScript) — assert in the frontend with a runtime type guard or JSDoc
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD`)
- [ ] `plans/README.md` row for plan 026 shows `DONE`

## STOP conditions

Stop immediately and report back (do not improvise) if:

- `WRITE_BACK_ENABLED` in `src-tauri/src/google_sheets/write_back.rs:14` is
  anything other than `false` — the codebase has diverged; the scope of this
  plan changes.
- You find yourself editing `src-tauri/src/google_sheets/write_back.rs` for
  any reason. **STOP. Phase 1 is read-side only.**
- The `ensure_valid_token` function in `src-tauri/src/oauth/token_store.rs`
  has been replaced with one that panics on token expiry (instead of returning
  `Err`) — the background task depends on the error-returning contract.
- The `tauri-plugin-notification` `2.x` API is incompatible with the exact
  `tauri = "=2.11.2"` version locked in Cargo.toml (peer version conflict).
  Report; do not downgrade Tauri to satisfy the plugin.
- The Drive `files.get?fields=modifiedTime` endpoint returns a permission
  error for the existing OAuth scopes AND adding `drive.metadata.readonly` to
  the scope list requires re-consent from the user (i.e., the existing
  `drive.metadata.readonly` scope is absent and cannot be added silently).
  Report the scope situation; do not add `drive` (full access) as a workaround.
- A step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching a file not in the Scope section.
- `npm run privacy:scan` fails because new code includes a spreadsheet URL,
  OAuth token, or personal name. Sanitize before committing; do not bypass the scan.

## Maintenance notes

- **Phase 2 (write-back)** is the explicit next step after this plan lands
  and is proven stable under real daily use. Phase 2 tasks: per-cell
  re-verify before write (`verify_then_write` function in `write_back.rs`),
  formula-column blocklist (`Saldo`, `Data`) enforced in `plan_write_back`,
  conflict-queue guard (write disabled while `import_conflict` has open rows),
  integration test coverage for the round-trip, then flip `WRITE_BACK_ENABLED
= true`. None of those belong to this plan.
- **Quota headroom**: if the spreadsheet shape grows beyond ~30 tabs, the
  30 s background probe + focus trigger combination could approach 60 req/min.
  Revisit `MIN_POLL_INTERVAL_SECS` upward (e.g., `30`) and consider increasing
  the default `sheets_bg_sync_interval_secs` before that point.
- **Token refresh race**: `ensure_valid_token` is not internally synchronized.
  If the background task and a user-triggered import both call it within the
  token's expiry window simultaneously, they may both attempt to refresh. The
  import guard (`SyncGuard`) prevents double-import but does not cover the
  token refresh itself. This is a known limitation acceptable for Phase 1
  (single-user, single-machine). Document it in a `// KNOWN LIMITATION` comment
  in `sync_task.rs`.
- **`ConflictGate` listener model**: this plan uses the Tauri event
  (`neko://sync-done` → re-poll) approach rather than wiring `ConflictGate`
  to `invalidateCommands()`. Both are called in the handler, so finance cache
  AND conflict list refresh on every sync. If `ConflictGate` later moves to
  use `useCommand`, the `listen` call can be removed in favour of the cache
  invalidation alone.
- **PR reviewer checklist**: confirm (a) `write_back.rs` is absent from the
  diff, (b) the `SyncGuard` Arc is correctly managed (no deadlock path), (c)
  the focus debounce is tested, (d) the notification fires only on token
  failure (not on every transient network error).
