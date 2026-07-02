# Plan 028: Sync Phase 2 — enable human-approved write-back, safely

> **Executor instructions**: Follow this plan step by step. This plan flips the
> master switch that lets the app write to the user's REAL Google Sheet (their
> source of record). Treat every step as safety-critical. Run every verification
> command and confirm the expected result before moving on. The flag flip
> (Step 9) is the LAST step and only after every prior gate is green. If any
> STOP condition occurs, stop and report — do not improvise. When done, update
> the plan-028 row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat 75b9a3d..HEAD -- src-tauri/src/google_sheets/write_back.rs src-tauri/src/commands/write_back_cmds.rs src-tauri/src/oauth/pkce.rs src/features/sheets/WriteBackPreview.tsx`
> If any in-scope file changed since this plan was written, re-open the cited
> spots and compare before proceeding; on a mismatch, treat it as a STOP.

## Status

- **Priority**: P1 (the headline remaining feature; high value, high risk)
- **Effort**: L
- **Risk**: HIGH — writes to the user's live financial spreadsheet
- **Depends on**: 026 (read-side sync — reuses its Drive `modifiedTime` probe)
- **Planned at**: commit `75b9a3d`, 2026-06-20
- **Constraint**: ADR-0003 — every material write needs a structured diff,
  validation, and explicit human approval; concurrent-edit detection must force
  re-review. `WRITE_BACK_ENABLED` is a one-way gate.

## Why this matters

The write-back machinery is structurally complete but disabled behind
`WRITE_BACK_ENABLED = false` (`write_back.rs:14`). Phase 1 (plan 026) shipped
read-side background sync. Phase 2 makes the **human-approved** write-back path
actually usable. The investigation found the core is sound (pure diff planner,
preview/apply split, re-read at apply time), but **must not be enabled** until
the gates below are closed — otherwise the first live write can 403 (no scope),
silently overwrite a concurrent human edit, or corrupt a formula column whose
only protection today is an indirect DB flag.

## Current state (verified — re-open and re-confirm line numbers)

- **Master flag**: `pub const WRITE_BACK_ENABLED: bool = false`
  (`src-tauri/src/google_sheets/write_back.rs:14`); `ensure_write_back_enabled()`
  (~:17) gates `apply_write_back` + `apply_economia_write_back`. Preview paths
  are flag-independent (read-only).
- **Pure planners**: `plan_write_back` (`write_back.rs:118`) and
  `plan_economia_write_back` (~~:213) → `Vec<CellWrite>`. `CellWrite` (~~:36) has
  `current`, `proposed`, `value_cents`, `changed`. Apply filters `changed`
  (`write_back_cmds.rs:303-315`).
- **Write API**: `SheetsClient::batch_update_values`
  (`google_sheets/mod.rs:200-232`) — one `values:batchUpdate` POST,
  `valueInputOption=RAW`, each cell written as a numeric `f64` (cents/100.0).
- **Apply path**: `apply_write_back` (`write_back_cmds.rs:283-316`) →
  `build_write_back_plan` does a FRESH `get_sheet_values` re-read + re-plan
  before writing (implicit staleness handling, but no forced re-review).
- **Formula protection (today, structural only)**: `kind_offset`
  (`write_back.rs:74-84`) returns offsets only for `amount_in`/`amount_out`/
  `amount_daily`; Saldo's mapping is generated `is_active=0`
  (`layout_detect.rs:197`) and `get_active_mappings_for_sheet` filters to active;
  `plan_economia_write_back` targets only `econ_col` (Entradas/% are formulas —
  per its docstring). NO explicit, tested blocklist in the planners.
- **OAuth scopes**: `pkce.rs:91-98` requests `spreadsheets.readonly` +
  `drive.metadata.readonly` — **NO write scope**. Test at `pkce.rs:156` asserts
  `spreadsheets.readonly`.
- **Frontend**: `WriteBackPreview.tsx` reads `writeBackEnabled()`, renders
  `ApprovalDiffCard` per changed cell, single batch Approve. No loading guard, no
  staleness warning, no confirm dialog, no ConflictGate precondition.
- **Gaps confirmed**: `apply_write_back` does NOT check `import_conflict` (unresolved
  conflicts), does NOT compare a stored checksum/revision to force re-review, does
  NOT inspect per-range batch errors, writes no audit row to `sync_log`.

## Commands (verification gates)

`npm run rust:check` · `npm run typecheck` · `npm run lint` · `npm run test:run`
· `npm run e2e` · `npm run doctor` · `npm run check`

## Scope

**In scope**: `oauth/pkce.rs` (+ token scope-mismatch detection in
`oauth/token_store.rs`), `google_sheets/write_back.rs` (explicit blocklist +
tests), `commands/write_back_cmds.rs` (conflict guard, staleness re-verify,
batch error inspection, audit log), `google_sheets/mod.rs` (batchUpdate response
inspection + chunking), `WriteBackPreview.tsx` + `api.ts` (UX hardening), the
flag flip, and tests. **Out of scope**: changing the diff/aggregation math,
`classify()`, the import pipeline, multi-credit-card support (warn only — see
Step 8), full programmatic rollback (deferred — see Maintenance).

## Steps

### Step 1 — OAuth write scope + re-consent (BLOCKER)

In `pkce.rs` (~:91-98) add `https://www.googleapis.com/auth/spreadsheets` to the
requested scopes (it supersedes `spreadsheets.readonly`; keep
`drive.metadata.readonly` for the Phase-1 probe). Update the scope assertion test
at `pkce.rs:156` to require the write scope.

Add **scope-mismatch detection**: a stored token from before this change carries
only readonly scopes → a write will 403. In `token_store.rs` (where the token +
its granted scopes are stored/loaded) record the granted scope string; expose a
check the apply path uses to detect "token lacks write scope" and return a
typed, user-actionable error ("Re-autorize para habilitar a escrita") that the
frontend turns into a re-consent prompt — NOT a raw 403 string.

- _Verify_: `cargo test … pkce` (scope test updated + passes); `npm run rust:check`.
- _STOP_ if the OAuth client config can't be granted the write scope.

### Step 2 — Explicit formula-column blocklist in the planners (BLOCKER)

Defense-in-depth over the structural `is_active` flag. In `write_back.rs`:

- Add `const FORMULA_ONLY_FIELDS: &[&str] = &["balance", "date"];` and, in
  `plan_write_back`, **reject any mapping whose `target_field` is in that set**
  before `kind_offset` resolves it — so even if a mapping were wrongly
  `is_active=1`, Saldo/Data are never written.
- In `plan_economia_write_back`, assert the resolved `econ_col` is neither the
  `Entradas` column nor the `%` column (verify by the header label at that index)
  and never write outside `econ_col`; bail with a clear error otherwise.
- _Tests (mandatory, per spike 021)_: (a) seed a mappings list that INCLUDES
  `balance` (offset 4) and `date` (offset 0) as active, plus transactions, and
  assert `plan_write_back` emits ZERO `CellWrite` for those offsets; (b) assert
  `plan_economia_write_back` emits no `CellWrite` at the Entradas/% column index.
- _Verify_: `cargo test … write_back` (new tests pass).

### Step 3 — Conflict-queue guard (ADR-0003)

In `apply_write_back` AND `apply_economia_write_back` (before any write), run
`SELECT COUNT(*) FROM import_conflict WHERE resolved_at IS NULL`; if `> 0`,
return a typed error ("Resolva os conflitos de importação antes de enviar"). The
frontend (Step 5) disables Approve and explains when conflicts are pending.

- _Verify_: a test that seeds an unresolved `import_conflict` and asserts apply
  returns the guard error without calling the Sheets client.

### Step 4 — Staleness re-verify: force re-review (ADR-0003 core gate)

Today apply silently re-plans from a fresh read. Instead, make the user's
approval bind to what they saw:

- At **preview** time, capture and return the spreadsheet's Drive `modifiedTime`
  (reuse `get_file_modified_time` from plan 026) as a `preview_revision` token.
- At **apply** time, the frontend passes that token back; the apply re-reads
  `modifiedTime` and, if it ADVANCED since preview, **aborts with a typed
  "sheet changed since preview — re-review" error** instead of writing. Only when
  the sheet is unchanged does it proceed (it still re-plans + writes only
  `changed` cells as a second safety net).
- _Verify_: a test where the modifiedTime advanced → apply returns the
  re-review error and writes nothing.

### Step 5 — Frontend approval hardening (`WriteBackPreview.tsx`)

- **Loading guard**: disable Approve while an apply is in flight (no double-submit).
- **Staleness warning**: stamp `previewedAt` + carry the `preview_revision`; on
  Approve, if the backend returns the Step-4 re-review error, surface a clear
  "A planilha mudou — gere o preview de novo" and auto-trigger a fresh preview.
- **Second-confirmation**: a Confirmar/Cancelar dialog before the live write
  ("Enviar N célula(s) para a planilha?") so a single misclick can't write.
- **ConflictGate precondition**: call `getImportConflicts()`; if pending, disable
  Approve with an explanation (mirrors Step 3 backend guard).
- **Post-apply reset**: clear the diff (or re-run preview) after success so the
  UI reflects the new sheet state and a second identical send can't fire.
- **a11y**: `aria-live="polite"`/`role="status"` on the success message; surface
  `value_cents` (formatted R$) so "what will be written" is unambiguous.
- _Verify_: `npm run test:run` (add component tests: loading-guard disables
  Approve; conflicts-pending disables Approve; re-review error re-previews);
  `npm run e2e`; `npm run doctor` (no new findings).

### Step 6 — batchUpdate response inspection + chunking

In `batch_update_values` (`mod.rs:200-232`): inspect the `values:batchUpdate`
response for per-range outcome and `totalUpdatedCells`; on a mismatch vs the
requested ranges, return an error naming the failure rather than reporting
success. Chunk the `updates` slice if it exceeds a safe ranges-per-request bound
(e.g. 500) so a full annual write can't exceed the API limit; report partial
progress on a mid-sequence failure.

- _Verify_: a unit test for the chunking boundary; `npm run rust:check`.

### Step 7 — Write-back audit to `sync_log`

After a successful apply, record what was written (sheet, ranges/cells, new
checksum) to `sync_log` so the next import recognizes the values as the new base
(updates `source_*`) instead of re-surfacing them as a fresh change/conflict.

- _Verify_: a round-trip test (apply → re-import) asserts the written cells do
  NOT produce a spurious `import_conflict`.

### Step 8 — Multi-credit-card edge: warn (not fix)

`load_write_back_txns` collapses the credit lump using `LIMIT 1` on cards with
`closing_day`+`due_day`. If the user has >1 such card, or a card missing cycle
days, surface a non-blocking warning in the preview ("Mais de um cartão com
ciclo — confira a data da fatura antes de enviar"). Do NOT attempt multi-card
support here (out of scope).

- _Verify_: a test asserting the warning flag is set with two qualifying cards.

### Step 9 — Flip the flag (LAST — only after Steps 1-8 are green)

Set `WRITE_BACK_ENABLED = true` (`write_back.rs:14`). Keep `valueInputOption=RAW`
with a numeric value — this is correct and safe (a numeric write stays numeric,
preserves the cell's display format, and cannot be reinterpreted as a formula;
do NOT switch to USER_ENTERED). The per-write human approval (Steps 5) remains —
flipping the flag enables the approve-to-write path, it does NOT auto-write.

- _Verify_: full `npm run check` exit 0; the apply tests now exercise the real
  (mocked) client path; `grep -n "WRITE_BACK_ENABLED" write_back.rs` shows `true`.

## Test plan

New tests, each next to existing write-back tests: blocklist (Step 2, x2),
conflict guard (Step 3), staleness re-review (Step 4), frontend guards (Step 5,
x3), chunking (Step 6), audit round-trip (Step 7), multi-card warning (Step 8),
scope assertion (Step 1). The existing pure-planner tests must stay green.

## Done criteria (machine-checkable)

- `grep -n "auth/spreadsheets" src-tauri/src/oauth/pkce.rs` → write scope present.
- `grep -n "FORMULA_ONLY_FIELDS" src-tauri/src/google_sheets/write_back.rs` → present.
- `grep -n "WRITE_BACK_ENABLED" …/write_back.rs` → `= true`.
- `npm run rust:check` → exit 0 (incl. all new tests).
- `npm run check` → exit 0 (incl. `test:run`, `e2e`, privacy scan).
- `npm run doctor` → no new findings.

## STOP conditions

- If flipping the scope can't get the OAuth consent (client config) — STOP.
- If any test shows `plan_write_back` can emit a `CellWrite` for `balance`/`date`
  after Step 2 — STOP (the blocklist is the safety floor).
- If the staleness gate (Step 4) can't reliably detect a concurrent edit — STOP;
  do NOT flip the flag with the gate unproven.
- Do NOT change the diff/aggregation math or `classify()`.
- Do NOT switch `valueInputOption` away from RAW.

## Maintenance notes

- **Suggested split for review safety**: implement Steps 1-4 + 6-8 (backend
  safety, flag still false) as PR-A, then Steps 5 + 9 (frontend hardening + the
  flag flip) as PR-B. Reviewing the live-write enablement separately from the
  guards reduces risk.
- **Re-consent UX**: every existing user must re-authorize once (new scope). The
  Step-1 detection turns this into a clear prompt rather than a runtime 403.
- **Deferred (not in this plan)**: full programmatic rollback (would require
  snapshotting every overwritten cell pre-write); multi-credit-card write-back;
  per-cell `userEnteredValue.formulaValue` pre-read (our static blocklist +
  structural mappings already cover the known formula columns — revisit if the
  sheet schema gains user-defined formula columns).
