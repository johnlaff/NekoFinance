# Plan 021: SPIKE: real-time two-way Google Sheets sync

> **Executor instructions**: This is a SPIKE — its deliverable is a design
> document (a new spec file under `specs/021-realtime-sync/`), NOT working
> code. Follow each step to survey the existing infrastructure, then author
> the spec that a future implementation plan will be built from. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src/features/sheets/GoogleSheetsPanel.tsx src-tauri/src/google_sheets/import.rs src-tauri/src/google_sheets/reconcile.rs src-tauri/src/google_sheets/write_back.rs src/features/reconcile/ConflictGate.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: · (direction spike, not ranked against P1–P3 implementation work)
- **Effort**: spike
- **Risk**: HIGH
- **Depends on**: plans/001-economia-side-by-side-layout.md, plans/002-atomic-sheet-import.md
- **Category**: direction
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Today the user must manually trigger every import. The spreadsheet is their
daily-edited source of record, and any stale SQLite mirror means the dashboard
shows yesterday's numbers. A "real-time-ish" sync strategy — even just
auto-refresh on window focus or a configurable background poll — would make the
app feel live rather than a periodic snapshot tool.

At the same time, the write-back path (plan_write_back, apply_write_back) is
fully implemented but gated OFF at `WRITE_BACK_ENABLED = false`
(src-tauri/src/google_sheets/write_back.rs:14). Enabling it in a sync loop
without rigorous safety gates would silently overwrite hand-edited spreadsheet
data. This spike must define exactly what the gate criteria are, what cadence
is safe given Sheets API quota, and how conflicts (three-way merge + ConflictGate)
fit into a recurring sync — before anyone touches the flag.

## Current state

### Files and roles

- `src/features/sheets/GoogleSheetsPanel.tsx` — the import UI; all import
  triggering today is manual (user clicks Import/Re-sync). The "Re-sincronizar"
  button (line 404) calls `importAllTabs(lastImport.spreadsheetId)` — a full
  re-read of every tab. `LAST_IMPORT_KEY` (line 64) persists the last
  spreadsheet in `app_setting` so cold-start can restore the re-sync target.
- `src-tauri/src/google_sheets/import.rs` — import domain core. Key functions:
  - `compute_checksum` (line 102): SHA-256 over (date, amount, description, is_projection, kind) for the whole tab's row set.
  - `check_duplicate_import` (line 152): queries `sync_log WHERE source_sheet = ?1 AND checksum = ?2`; returns `Ok(true)` if the dataset is unchanged → early-exit with 0 upserts.
  - `import_rows_with_options` (line 199): opens a SQLite transaction, UPSERTs each row via deterministic `row_id`, then diff-deletes rows absent from the new set, then commits.
  - `row_id` (line 139): `sha256("txn-v1|{sheet}|{date}|{kind}|{slot}")` — stable across value/description edits, changes only on date/column moves.
- `src-tauri/src/google_sheets/reconcile.rs` — three-way merge kernel. PURE,
  no IO. `reconcile(base, local, sheet) -> MergeDecision` (line 19):
  - `KeepLocal` when only local changed or nothing changed.
  - `ApplySheet` when only the sheet changed (or no base exists — first import).
  - `Conflict` when both diverged → human gate required; never auto-resolves.
    `apply(base, local, sheet) -> FieldOutcome` (line 53) wraps `reconcile` and
    returns the value to write, the new base, and a `conflict: bool`.
- `src-tauri/src/google_sheets/write_back.rs` — write-back planner + safety
  gate.
  - `WRITE_BACK_ENABLED: bool = false` (line 14): master kill switch. All write
    paths call `ensure_write_back_enabled()` (line 17) first; if false, returns
    `Err("Write-back desligado …")`.
  - `plan_write_back` (line 118): PURE, read-only. Computes `Vec<CellWrite>`
    — the before→after diff — without touching the network or the DB. Safe to
    call even with the flag off.
  - `CellWrite.changed: bool` (line 49): marks only cells whose proposed value
    differs from what is currently in the sheet (numeric comparison, not string).
- `src/features/reconcile/ConflictGate.tsx` — the human conflict-resolution
  UI. Polls `getImportConflicts()` on mount (line 41), renders one
  `ApprovalDiffCard` per open conflict, routes resolution to
  `resolveImportConflict(id, "sheet" | "local")` (line 53). Returns `null`
  when the conflict list is empty (line 64) — UI is self-hiding.
- `src/features/sheets/WriteBackPreview.tsx` — the write-back approval UI.
  Reads the enabled flag from `writeBackEnabled()` (line 13 import), renders a
  locked state when the flag is off. Calls `previewWriteBack` (line 128) then
  `applyWriteBack` (line 151); the apply only fires after explicit human click.

### Relevant schema (migrations verified at d183bbf)

`sync_log` (migration 14, `20240608000014_sync_log.sql`):

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

`sync_log` gains `source_sheet TEXT` and `checksum TEXT` columns (migration 18,
`20240608000018_sync_log_checksum.sql`):

```sql
ALTER TABLE sync_log ADD COLUMN source_sheet TEXT;
ALTER TABLE sync_log ADD COLUMN checksum TEXT;
CREATE INDEX IF NOT EXISTS idx_sync_log_source ON sync_log(source_sheet, checksum);
```

`import_conflict` (migration `20240612000007_advanced_reconciliation.sql`):

```sql
CREATE TABLE IF NOT EXISTS import_conflict (
    id              TEXT PRIMARY KEY,
    transaction_id  TEXT NOT NULL,
    field           TEXT NOT NULL,
    base_value      TEXT,
    local_value     TEXT NOT NULL,
    sheet_value     TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at     TEXT,
    resolution      TEXT,
    UNIQUE (transaction_id, field)
);
```

`transaction` carries `source_amount INTEGER` and `source_description TEXT`
columns (same migration) — these are the "base" snapshot for the three-way merge.

`app_setting` (migration `20240612000006_app_setting.sql`) is a general KV
store (key TEXT PK, value TEXT). Existing keys: `sheets_last_import`
(line 64, GoogleSheetsPanel.tsx). The sync cadence setting should live here
under a new key (`sheets_sync_cadence_secs`).

### HTTP layer (verified in `src-tauri/src/http.rs`)

`send_with_retry` (line 58) retries up to 3 attempts with 400ms×attempt
backoff, honoring `Retry-After` from the server (capped at 10 s). On 429,
it backs off before the next attempt. This is the HTTP client all Sheets calls
go through. Any background polling loop must not drive request rates faster
than this client can absorb, and must not issue concurrent imports for the same
tab.

### ADR-0003 (non-negotiable constraint, `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md`)

> "Every material write requires a structured before→after diff, validation,
> and explicit human approval (`sync_log` checksums detect concurrent sheet
> edits and force re-review)."

The ADR is explicit: write-back is NEVER silent. Auto-sync may read (import)
on any cadence. It may NEVER auto-write to the spreadsheet. The transition from
`WRITE_BACK_ENABLED = false` to `true` is a one-time gated milestone (not a
per-sync-cycle decision) and requires the criteria in the Done Criteria section
of this spike's spec to be satisfied first.

### Sheets API rate limits (open question — must be quantified in the spec)

The Sheets API Read quota is documented at 300 requests/minute/project and
60 requests/minute/user by default. A full annual re-import reads one API call
per tab (12 year-tabs + 1 Economia tab = up to 13 calls). A focus-trigger sync
at a typical desktop use pattern (open/minimize ~20×/hour) would issue up to
260 calls/hour (4.3/min) — well within quota for moderate use. A 15-minute
background poll would add 4 calls/hour (≤5 tabs). The spec must quantify the
worst-case usage for the method's typical spreadsheet shape (≤13 tabs) and
confirm it fits within the 60 req/min/user budget across trigger combinations.

## Commands you will need

| Purpose            | Command                                                    | Expected on success          |
| ------------------ | ---------------------------------------------------------- | ---------------------------- |
| Typecheck          | `npm run typecheck`                                        | exit 0, no errors            |
| Lint               | `npm run lint`                                             | exit 0, no errors            |
| Rust check         | `npm run rust:check`                                       | exit 0 (fmt + clippy + test) |
| Unit tests (Rust)  | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass                     |
| Unit tests (front) | `npm run test:run`                                         | all pass                     |
| Privacy scan       | `npm run privacy:scan`                                     | exit 0                       |
| Full gate          | `npm run check`                                            | exit 0                       |

(All verified during recon; use exactly as written.)

## Suggested executor toolkit

- Read `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md` in full
  before writing the spec — the ADR's constraints are non-negotiable.
- Read `CONTEXT.md` at repo root for canonical domain vocabulary (use those
  exact terms in the spec: Transaction, EventKind, sync_log, ConflictGate,
  MergeDecision, etc.).
- Use the `neko-finance-design` skill if any UI mock or component sketch is
  included in the spec (design tokens live in `src/design-system/`).

## Scope

**In scope** (the only files this spike should create or edit):

- `specs/021-realtime-sync/spec.md` — CREATE: the full sync architecture spec.
- `specs/021-realtime-sync/open-questions.md` — CREATE: enumerated open
  questions with owners and resolution criteria (rate limits, offline, multi-
  device, conflict backlog management).
- `plans/README.md` — UPDATE: mark plan 021 status as DONE.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/write_back.rs` — specifically, do NOT flip
  `WRITE_BACK_ENABLED` to `true`. That flip belongs to the implementation plan
  that follows this spike, after the gate criteria are met.
- `src/features/sheets/GoogleSheetsPanel.tsx` — no UI changes during the spike.
- Any migration file — schema changes belong to the implementation plan.
- `src-tauri/src/commands.rs` — no new Tauri commands during the spike.
- `src-tauri/src/google_sheets/import.rs` — no changes during the spike.

## Git workflow

- Branch: `advisor/021-spike-realtime-sync`
- Commit per logical step; follow the repo's conventional-commit style observed
  in `git log`: `feat:`, `fix:`, `chore:`, `docs:`, `spec:` prefixes; present
  tense imperative body.
  Example from log: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`
  For a spike: `spec: add 021 spike — realtime two-way Sheets sync architecture`
- Do NOT push or open a PR unless the operator explicitly instructs it.

## Steps

### Step 1: Survey the existing infrastructure and confirm line numbers

Before writing a word of the spec, open and read these files in their
current state to confirm the excerpts in "Current state" still match:

1. `src/features/sheets/GoogleSheetsPanel.tsx` — confirm `handleResync` at line
   ~404 and `LAST_IMPORT_KEY` at line ~64.
2. `src-tauri/src/google_sheets/import.rs` — confirm `check_duplicate_import`
   at line ~152 and `compute_checksum` at line ~102.
3. `src-tauri/src/google_sheets/reconcile.rs` — confirm `reconcile` function
   at line ~19 and `MergeDecision` enum at line ~8.
4. `src-tauri/src/google_sheets/write_back.rs` — confirm `WRITE_BACK_ENABLED`
   at line 14 is still `false`.
5. `src/features/reconcile/ConflictGate.tsx` — confirm `getImportConflicts`
   at line ~41.

**Verify**: `git diff --stat d183bbf..HEAD -- src/features/sheets/GoogleSheetsPanel.tsx src-tauri/src/google_sheets/import.rs src-tauri/src/google_sheets/reconcile.rs src-tauri/src/google_sheets/write_back.rs src/features/reconcile/ConflictGate.tsx` → should list no changed files (all clean, confirming no drift).

### Step 2: Define the sync trigger and cadence recommendation

In the spec, enumerate and evaluate these trigger options:

**Option A: Manual only (status quo)**

- Pro: zero API quota risk; user controls exactly when data refreshes.
- Con: dashboard can be stale for hours between hand edits; friction for daily
  use workflow.
- Recommended for: users with API-paranoid setups or slow connections.

**Option B: Focus-triggered re-import (RECOMMENDED)**

- Trigger: Tauri's `on_window_event` with `WindowEvent::Focused(true)` fires
  when the app regains focus (user switches back from the spreadsheet browser tab).
- Behavior: run `check_duplicate_import` for each tracked tab; only call the
  Sheets API if the checksum may have changed (i.e., always, since we cannot
  know without fetching — but the early-exit in `import_rows_with_options` via
  `check_duplicate_import` means the DB write is a no-op when data is identical).
- Debounce: minimum 60 seconds between focus-triggered syncs to avoid a burst
  when the user alt-tabs rapidly. Persist `last_focus_sync_at` in `app_setting`.
- API cost: ~13 calls/focus event × ≤20 focus events/hour = 260 req/hour =
  4.3 req/min — comfortably within the 60 req/min/user quota.
- No config needed: sensible default; the user can disable via a toggle stored
  in `app_setting` (`sheets_focus_sync_enabled`, default `"true"`).

**Option C: Background poll**

- Trigger: `tokio::time::interval` in a Tauri background task.
- Cadence options: 5 min, 15 min, 30 min. At 15 min and 13 tabs: 52 calls/hour
  = 0.87 req/min — well within quota even combined with focus triggers.
- Complexity: requires a Tauri background task, a cancellation token (to stop
  on disconnect), and a UI indicator for "syncing in background." Harder to
  test. Adds ambient battery/network drain on laptops.
- Recommended: DEFER to a follow-on plan after focus-sync proves stable.

**Spec section required**: a table comparing A/B/C on API cost, complexity, and
offline behavior; a clear "recommended default" callout (B), and a note that C
is deferred.

**Verify**: `ls specs/021-realtime-sync/` → directory exists with `spec.md`
started (at minimum the trigger section written).

### Step 3: Define the incremental import and checksum-diff flow

The spec must describe the full read-side sync loop. Use exact function names
from the codebase:

1. **Fetch sheet data**: call `SheetsClient::get_sheet_values` for each tab in
   `sheets_last_import` (one API call per tab, sequentially, not concurrently —
   `send_with_retry` already handles transient 429s with backoff).
2. **Parse rows**: `parse_rows_with_layout` (import.rs) → `Vec<ImportedRow>`.
3. **Compute checksum**: `compute_checksum(&rows)` (import.rs:102).
4. **Early-exit check**: `check_duplicate_import(pool, sheet_name, &checksum)`
   (import.rs:152) — if `true`, skip the DB write entirely for that tab (no
   write, no lock contention). This is the key deduplication safety net; it
   means a focus-trigger sync on an unchanged sheet is a read-only network
   operation with zero DB side-effects.
5. **UPSERT with three-way merge**: `import_rows_with_options` (import.rs:199)
   — opens a SQLite `BEGIN IMMEDIATE` transaction (important: after plan 002
   lands, this will be wrapped in the outer atomic transaction), runs
   `apply(source_amount, local_amount, sheet_amount)` per field per row
   (reconcile.rs:53), writes the `value` and `source` fields, and records any
   `conflict` via `record_conflict` (import.rs:11).
6. **Diff-delete**: rows in `sync_log` for this `source_sheet` but absent from
   `current_ids` are deleted (import.rs:362–391).
7. **ConflictGate surfaces automatically**: `ConflictGate` (ConflictGate.tsx)
   polls `getImportConflicts()` on mount. After a background sync creates new
   conflicts, the UI needs to be notified. The spec must decide: (a) emit a
   Tauri event (`tauri::Emitter::emit`) that the frontend listens to via
   `listen` to re-render ConflictGate, OR (b) use `invalidateCommands()` (the
   existing cache-bust mechanism in `src/lib/useCommand.ts`) to trigger a
   re-fetch. Option (b) requires ConflictGate to subscribe to the invalidation
   bus rather than a static `useEffect` on mount — document which approach and
   why.

**Verify**: `npm run typecheck` → exit 0 (no new code changed yet; verifying
baseline is clean before authoring spec sections that reference types).

### Step 4: Define the write-back safety gate criteria

The spec must enumerate ALL conditions that must be satisfied before
`WRITE_BACK_ENABLED` is flipped from `false` to `true`. This is the most
important section of the spec. Base it on ADR-0003 and the existing write-back
infrastructure:

**Gate criteria (all must hold simultaneously):**

1. **Plans 001 and 002 are DONE**: the Economia side-by-side import is correct
   and the import transaction is atomic. Write-back on a partially-imported
   state could corrupt the wrong year's column.

2. **Per-cell checksum verification before write**: `plan_write_back` already
   produces `CellWrite.current` (the sheet's current value). Before calling
   `SheetsClient::update_cell_values`, the executor must re-fetch the cell
   range and compare against `CellWrite.current`. If the sheet changed between
   preview and apply (concurrent edit), abort the batch and surface a new import.
   This must be implemented as a `verify_then_write` function (not yet in
   write_back.rs) — spec it here, implement it in the follow-on plan.

3. **Human approval for every write batch**: `applyWriteBack` in the UI
   (WriteBackPreview.tsx:151) already requires explicit button click. This must
   remain synchronous with human intent — never triggered automatically by the
   sync loop. The sync loop only reads.

4. **No write to formula-owned columns**: `Saldo` (chained formula column) and
   `Data` (structural) must never appear in a write-back batch. This constraint
   is documented in `docs/claude-design-prompt.md:189–190` but is not enforced
   in code. The gate requires a blocklist in `plan_write_back` that filters
   these columns from `CellWrite` output before it reaches the UI.

5. **Conflict queue is empty before write**: if `SELECT COUNT(*) FROM
import_conflict WHERE resolved_at IS NULL` > 0, the write-back button must
   be disabled with a clear message. Writing to a sheet when the local state has
   unresolved conflicts risks propagating the wrong value.

6. **Integration test coverage for the round-trip**: at least one test that
   imports synthetic rows, edits one locally, runs a re-import with a sheet
   change, verifies the three-way merge decision, then calls `plan_write_back`
   and asserts the diff is correct.

**Spec section required**: a "Gate criteria" table with each criterion, the
file/function where it must be enforced, and the test that verifies it.

**Verify**: `npm run rust:check` → exit 0 (Rust baseline still clean; no code
changed, confirming the spec doesn't inadvertently describe a build-breaking
change).

### Step 5: Define open questions and defer multi-device / offline

Create `specs/021-realtime-sync/open-questions.md` with the following
enumerated questions. Each entry must have: the question, why it matters,
the owner (human decision vs. implementation research), and the resolution
criterion.

**Q1: Sheets API quota per OAuth project vs. per user**
The 60 req/min limit is per-user per OAuth client. If multiple Neko Finance
instances share the same client ID (all users of a public build), the per-
project limit (300 req/min) may be hit in aggregate. Resolution: confirm
whether this app uses a personal client ID (single user, per OAuth consent
screen configuration) or a shared SaaS client ID. If personal, Q1 is moot.

**Q2: Offline / no-network behavior during focus sync**
When the focus trigger fires but there is no network, `send_with_retry` will
exhaust 3 attempts with backoff and return an error. The sync must not show a
disruptive error modal for a background sync failure (it should log silently
and expose a subtle "last synced: X min ago" indicator). Resolution: spec the
UX for silent failure + retry on next focus.

**Q3: Multi-device / simultaneous edits**
If the user runs Neko Finance on two machines (home and work), both sync from
the same sheet. Writes from machine A land in the sheet; machine B syncs and
sees them as `ApplySheet` decisions. This is safe on the READ side (three-way
merge handles it). The WRITE side is dangerous: if machine B has pending write-
back items and machine A already wrote different values to those cells, the
per-cell checksum verification (gate criterion 2) catches the divergence and
aborts. Resolution: gate criterion 2 must be in place before write-back is
enabled on any multi-device setup.

**Q4: Conflict backlog management**
If the user never opens ConflictGate and a background sync keeps appending
new conflicts (e.g., they edit the sheet description daily AND have a local
edit on the same row), the `import_conflict` table grows unbounded. Resolution:
add a `MAX_OPEN_CONFLICTS = 50` guard in `import_rows_with_options` that pauses
further imports and surfaces a banner, or auto-resolve old conflicts in favor of
the sheet after a TTL (e.g., 7 days). Document tradeoffs.

**Q5: Token expiry during background sync**
`ensure_valid_token` in `token_store.rs` already refreshes the access token if
expired. A background poll must call it before each sync cycle. If the refresh
token itself is revoked (user revokes OAuth consent), the sync must degrade
gracefully (stop the poll, emit a "re-authenticate" notification, not panic).
Resolution: verify `ensure_valid_token` returns `Err` (not panics) on revoke;
add a handler in the background task that disables polling and emits a Tauri
event.

**Verify**: `ls specs/021-realtime-sync/` → shows `spec.md` and
`open-questions.md`; `npm run privacy:scan` → exit 0 (no private data in new
files).

### Step 6: Author the complete spec.md

`specs/021-realtime-sync/spec.md` must contain these sections in order:

1. **Summary**: one paragraph. "Real-time-ish sync means the app reads the
   sheet automatically (focus trigger or background poll) and applies the three-
   way merge via the existing `reconcile` kernel, surfacing conflicts in
   ConflictGate. Write-back remains human-approved and gated — the sync loop
   never writes to the sheet automatically."

2. **Trigger/cadence decision** (from step 2): table + recommendation.

3. **Incremental import flow** (from step 3): numbered steps referencing exact
   function names + the ConflictGate notification approach decision.

4. **Write-back safety gate** (from step 4): enumerated criteria table.

5. **Proposed schema additions** (new `app_setting` keys only — no new tables
   needed for the read-only sync phase):
   - `sheets_focus_sync_enabled` (`"true"` | `"false"`, default `"true"`)
   - `sheets_sync_cadence_secs` (integer string, default `"0"` = disabled;
     for future background poll; `"900"` = 15 min)
   - `sheets_last_focus_sync_at` (ISO-8601 timestamp, for debounce)

6. **Proposed Tauri events** (emitted from Rust, listened in React):
   - `sync://import-complete` — payload: `{ sheet: string, txns_updated: number, conflicts: number }`. Triggers `invalidateCommands()` + ConflictGate re-fetch in the frontend.
   - `sync://error` — payload: `{ reason: string }`. Silent log only (no modal).

7. **Implementation phases** (recommended order for a follow-on plan):
   - Phase 1 (read-only): focus-trigger sync, debounce, `sync://import-complete` event, ConflictGate subscription, `app_setting` UI toggle. No write-back changes.
   - Phase 2 (write-back enablement): per-cell re-verify, formula-column blocklist, conflict-queue guard, integration test coverage, flip `WRITE_BACK_ENABLED = true`.

8. **Open questions**: link to `open-questions.md`.

9. **Non-goals of this spike**: background poll implementation, Mia/copilot
   write tools, multi-user SaaS quota management.

**Verify**: `npm run check` → exit 0 (full gate; all checks pass; no source
was modified by the spike).

### Step 7: Update plans/README.md

Mark plan 021 status as `DONE` in the table row.

**Verify**: `grep "021" plans/README.md` → shows `| 021 | ... | DONE |`.

## Test plan

This is a spike — no new production code is written, so no new tests are
written in this plan. However, the spec (step 6, section 4) MUST enumerate
the integration tests that the follow-on implementation plan must write. The
spec is the test plan for the implementation.

Specifically, the spec must list:

- **Round-trip test**: import synthetic rows → edit `amount` locally → re-import
  with changed sheet amount → assert `MergeDecision::Conflict` for that field →
  assert `import_conflict` table has one unresolved row.
- **Checksum early-exit test**: import once → call `import_rows_with_options`
  again with identical rows → assert return value is `0` and no DB writes
  occurred (verify via `sync_log` timestamp unchanged).
- **Focus-debounce test**: emit two `Focused(true)` events within 30 seconds →
  assert only one API call was made (mock the HTTP client).
- **Write-back gate test**: assert `plan_write_back` output contains zero
  entries for `Saldo` and `Data` column headers.
- **Conflict-queue guard test**: fill `import_conflict` with `MAX_OPEN_CONFLICTS`
  unresolved rows → trigger another import → assert import is paused and a UI
  banner key is set in `app_setting`.

Existing test to use as pattern for Rust tests: `src-tauri/src/google_sheets/reconcile.rs` lines 73–163 (the inline `#[cfg(test)]` module with `apply_*` and `reconcile_*` tests). Match that style.

Existing test to use as pattern for frontend tests: `src/features/reconcile/ConflictGate.test.tsx`.

## Done criteria

ALL must hold when the spike is complete:

- [ ] `specs/021-realtime-sync/spec.md` exists and contains all eight sections
      listed in step 6.
- [ ] `specs/021-realtime-sync/open-questions.md` exists and contains Q1–Q5.
- [ ] `npm run check` exits 0 (no source files modified; all existing checks pass).
- [ ] `npm run privacy:scan` exits 0 (no private data in spec files).
- [ ] `grep "WRITE_BACK_ENABLED" src-tauri/src/google_sheets/write_back.rs` still
      returns `pub const WRITE_BACK_ENABLED: bool = false;` (flag untouched).
- [ ] `git diff --name-only HEAD` shows only files under `specs/021-realtime-sync/`
      and `plans/README.md` — no source files.
- [ ] `plans/README.md` row for plan 021 shows status `DONE`.

## STOP conditions

Stop and report back (do not improvise) if:

- `WRITE_BACK_ENABLED` in `src-tauri/src/google_sheets/write_back.rs:14` is
  anything other than `false` at the start of the spike — someone may have
  already enabled write-back, which changes the scope of this spike entirely.
- The three-way merge kernel (`reconcile.rs`) has been replaced or significantly
  refactored since this plan was written — the incremental import flow in step 3
  depends on its exact semantics.
- `check_duplicate_import` no longer queries `sync_log` by `(source_sheet,
checksum)` — this is the incremental-sync deduplication mechanism; a change
  here invalidates the checksum-diff flow design.
- `plans/001` or `plans/002` are marked `REJECTED` — the spike's write-back
  gate criteria assume those correctness fixes land first; rejecting them
  changes the prerequisite analysis.
- Any step's verification fails after a reasonable fix attempt (e.g., `npm run
check` fails because the spec file itself was accidentally placed in a scanned
  source directory).
- The spec, to be accurate, requires documenting a private method name, token
  value, spreadsheet URL, or personal data — stop and sanitize first.

## Maintenance notes

- **After the spike lands**: create a follow-on implementation plan (plan 022
  or next available slot) using this spec as its requirement document. Phase 1
  (read-only focus sync) can be executed independently of Phase 2 (write-back
  enablement).
- **ADR-0003 is the law**: any future agent implementing the background poll or
  the write-back gate must re-read `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md`
  before touching `WRITE_BACK_ENABLED`. The flag is not a feature toggle — it
  is a one-way gate that changes the system of record.
- **Quota math must be re-verified** if the method's canonical spreadsheet
  shape changes (e.g., more than 13 tabs). The `send_with_retry` client caps
  retries at 3; a burst of 14+ tabs in a focus sync could still hit 429 on the
  first call — the spec should recommend sequential (not concurrent) tab imports
  with a minimum 100 ms gap between calls as defensive quota hygiene.
- **ConflictGate subscription model** (step 3 decision point) affects the
  implementation of plan 022. If the team chooses the Tauri-event approach
  (`sync://import-complete`), ConflictGate.tsx must add a `listen` call;
  if they choose `invalidateCommands()`, the existing cache-bust infrastructure
  in `src/lib/useCommand.ts` is sufficient but ConflictGate must opt in to the
  invalidation signal. Document the choice and rationale in the spec.
- **`Saldo` column blocklist**: this is a correctness invariant (`Saldo` is a
  formula chain in the spreadsheet; overwriting it from write-back would silently
  corrupt every subsequent day's balance). The blocklist must be unit-tested
  before `WRITE_BACK_ENABLED` is set to `true`. See `docs/claude-design-prompt.md:189`.
