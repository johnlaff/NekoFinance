# Plan 066: Sidebar shows the real last-sync recency (from sync_log)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 290d538..HEAD -- src/shell/AppShell.tsx src/lib/api.ts src-tauri/src/commands`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx / trust
- **Planned at**: commit `290d538`, 2026-07-03

## Why this matters

The sidebar used to show a hardcoded "Sincronizada há 2 min" regardless of
reality (prototype leftover); it was replaced by the honest but static
"Conta Google ativa". The app already records every import and write-back in
the `sync_log` table — surfacing the real recency ("Sincronizada há 18 min")
restores the useful signal without lying, and gives the user confidence the
polling sync is alive.

## Current state

- `src/shell/AppShell.tsx:181-194` — the connection chip:

```tsx
<span className="sh-conn__s">
  {authStatus === "connected"
    ? "Conta Google ativa"
    : authStatus === "expired"
      ? "Sessão expirada"
      : ...}
</span>
```

- `src-tauri/migrations/20240608000014_sync_log.sql` — the table:

```sql
CREATE TABLE IF NOT EXISTS sync_log (
    id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT
);
```

Writers: the polling sync task (`src-tauri/src/sync_task.rs:469`), imports
(`src-tauri/src/lib.rs:526`), transaction edits
(`src-tauri/src/commands/transactions.rs:1450`) and write-back
(`src-tauri/src/commands/write_back_cmds.rs:838,970`). Check the actual
`event_type` values with `grep -rn "event_type" src-tauri/src --include="*.rs" | grep -i "import\|write_back\|sync"` before writing the SQL filter.

- Command conventions: Rust commands live in `src-tauri/src/commands/*.rs`,
  are registered in `src-tauri/src/lib.rs` (`invoke_handler` list — grep for
  `update_tag_cmd` as a recent exemplar), return `Result<T, String>`, and are
  tested with `#[tokio::test]` + the in-memory `fixture_pool()`/`test_pool()`
  helpers in the same module's `tests`.
- Frontend: bindings in `src/lib/api.ts` (thin `invoke` wrappers);
  `useCommand("<key>", fetcher)` for fetching (see `AppShell.tsx` or
  `SettingsScreen.tsx` for exemplars).

## Commands you will need

| Purpose    | Command                                 | Expected on success |
| ---------- | --------------------------------------- | ------------------- |
| Rust tests | `cd src-tauri && cargo test --lib sync` | all pass            |
| Rust gate  | `npm run rust:check`                    | exit 0              |
| Typecheck  | `npm run typecheck`                     | exit 0              |
| Unit test  | `npx vitest run src/shell`              | all pass            |
| Full gate  | `npm run check`                         | exit 0              |

## Scope

**In scope**:

- `src-tauri/src/commands/` — one new query command (pick the module that
  already owns sync-related commands; check `sheets_import.rs`)
- `src-tauri/src/lib.rs` — register the command
- `src/lib/api.ts` — binding + type
- `src/shell/AppShell.tsx` — render the recency
- Tests alongside each

**Out of scope**:

- Any change to what gets WRITTEN to `sync_log`.
- A live ticking clock — compute the label at render/fetch time; the next
  `invalidateCommands()` refreshes it. Do not add `setInterval`.

## Git workflow

- Branch: `feat/066-real-last-sync`
- Conventional commits, e.g. `feat(shell): recência real de sync na sidebar`

## Steps

### Step 1: Rust command `last_sync_at`

```rust
/// Timestamp (RFC "YYYY-MM-DD HH:MM:SS", UTC — datetime('now') do SQLite) do
/// evento de sync mais recente vindo da planilha; None sem histórico.
#[tauri::command]
pub async fn last_sync_at(pool: State<'_, SqlitePool>) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT MAX(timestamp) FROM sync_log WHERE event_type IN (<real event types>)")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("last_sync_at: {e}"))
}
```

Use the REAL `event_type` values found in recon (import/write-back/sync ones —
exclude purely local edits if they use a distinct type). Register in
`lib.rs`'s `invoke_handler`. Unit test: insert two sync_log rows with distinct
timestamps into the fixture pool, assert MAX comes back; empty table → `None`.

**Verify**: `cd src-tauri && cargo test --lib last_sync` → all pass.

### Step 2: Frontend binding + relative formatting

In `api.ts`: `export function lastSyncAt(): Promise<string | null>`. In
`AppShell.tsx`, fetch with `useCommand("last_sync_at", lastSyncAt)` and format
relative in pt-BR (minutes < 60 → "há X min"; hours < 24 → "há X h"; else
"há X dias"). SQLite's `datetime('now')` is UTC without timezone suffix —
parse as UTC (`new Date(ts.replace(" ", "T") + "Z")`). When null or in error,
keep "Conta Google ativa".

**Verify**: `npm run typecheck` → exit 0.

### Step 3: Render + test

Connected state renders `Sincronizada há X min` when a timestamp exists.
Test in the shell's test file (create `src/shell/AppShell.test.tsx` only if
one doesn't exist — check first): mock `last_sync_at` returning a timestamp
10 minutes before the frozen test clock, assert `/Sincronizada há 10 min/`;
mock null → "Conta Google ativa". Freeze the clock (`vi.setSystemTime`).

**Verify**: `npx vitest run src/shell` → all pass. Then `npm run check` → exit 0.

## Test plan

- Rust: MAX over multiple rows; empty table → None.
- Frontend: 10-min-old timestamp → "há 10 min"; null → fallback text; UTC
  parsing (timestamp without `Z`) does not shift by the local offset (assert
  exact minutes with a frozen clock in a non-UTC timezone — vitest default TZ
  may be UTC; set `process.env.TZ` in the test file if needed).

## Done criteria

- [ ] `npm run check` exits 0 (includes rust gate)
- [ ] Sidebar shows real recency when history exists; honest fallback otherwise
- [ ] UTC parsing test passes with a non-UTC TZ
- [ ] `plans/README.md` status row updated

## STOP conditions

- `sync_log.timestamp` turns out to be written in a different format by some
  writers (e.g. RFC3339 with `T`/`Z` from Rust vs `datetime('now')` default) —
  report the mix; the parse strategy must then normalize both, and that
  decision goes back to the reviewer.
- The polling sync task doesn't actually write sync_log rows on no-change
  cycles AND the user expects "checked 2 min ago" semantics — that is a
  product decision (last CHANGE vs last CHECK); implement last CHANGE and note
  it in the PR description.

## Maintenance notes

- If a "last checked" (vs last change) signal is wanted later, add an
  `app_setting` heartbeat in the sync task rather than spamming sync_log.
- Reviewer: confirm no `setInterval` was added; recency updates ride the
  existing `invalidateCommands()` cycles.
