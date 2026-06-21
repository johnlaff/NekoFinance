# Plan 055: Write-back↔re-import identity (no duplicate) + economia write-back tag-filter + sync-cadence debounce

> **Executor instructions**: Three fixes from the Opus confirmatory sign-off
> (verify + fidelity verdicts were "ship"; these are the last bug items — one P2
> duplication + two P3). Each gets a regression test. STOP if the identity
> re-key risks corrupting an existing imported row. When done, flip the plan-055
> row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat 6bae118..HEAD -- src-tauri/src/google_sheets/ src-tauri/src/commands/write_back_cmds.rs src-tauri/src/sync_task.rs`

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (the identity fix touches the import/write-back round-trip)
- **Category**: bug
- **Depends on**: none
- **Planned at**: commit `6bae118`, 2026-06-21

## Why this matters

The Opus sign-off found the app clean on fidelity (total) and verification (no
regressions), with NO P0/P1. Three remaining items: one P2 data-duplication on
the manual-entry → write-back → re-import round-trip, and two P3 consistency
nits. Closing them reaches a truly zero-bug state for the user's workflow.

## Current state (verify line numbers — re-locate by content)

1. **[P2] Manual row duplicated on write-back→re-import.** A transaction created
   in-app gets a UUID id and is NOT recorded in `sync_log` against a sheet
   position. If it is written back to a (previously empty) grid cell and the
   sheet is later re-imported, the importer computes a DETERMINISTIC `row_id`
   (`sha256("txn-v1|{sheet}|{date}|{kind}|{slot}")`) for that cell and inserts a
   SECOND row — the diff-delete never removes the original UUID row (it isn't in
   `sync_log`). Result: the manual transaction + its re-imported twin = a
   duplicate (double-counts in totals/Saldo).
   - Files: `src-tauri/src/google_sheets/import.rs` (`row_id`, the UPSERT +
     diff-delete + `sync_log` keying), `src-tauri/src/commands/write_back_cmds.rs`
     (`apply_write_back` — where a manual row's value lands in a sheet cell).
2. **[P3] economia write-back column omits the tag-exclude filter.**
   `load_economia_by_month` (the source for the Economia-column write-back, in
   `write_back_cmds.rs`) sums reserve transfers WITHOUT the `exclude_from_totals`
   `NOT EXISTS` filter that `realized_annual_economia` applies. So an
   "Ignorar"-tagged reserve transfer is excluded from the metric but still
   written to the sheet's Economia column — inconsistent.
3. **[P3] sync-cadence debounce written on every interval tick.** In `run_probe`
   (`src-tauri/src/sync_task.rs`), the focus-debounce timestamp
   (`sheets_last_focus_probe_at`) is written on EVERY probe including the timed
   interval tick; the 60s focus-debounce then suppresses the next interval tick,
   so a configured 30s interval effectively polls ~60s. Harmless to data, wrong
   cadence.

## Commands

`npm run rust:check` · `npm run typecheck` · `npm run test:run` · `npm run e2e` · `npm run check`

## Scope

In scope: `import.rs`, `write_back_cmds.rs`, `sync_task.rs` + tests. Out of scope:
the Performance formula (LOCKED), classify(), the Saldo chain mechanics, the 028
gates, the economia=annotation model.

## Steps

1. **Identity fix (P2)** — choose the cleanest of: (a) at write-back of a manual
   (UUID-id, not-in-`sync_log`) transaction into a sheet cell, RE-KEY it to the
   deterministic `row_id` for that (sheet, date, kind, slot) AND record it in
   `sync_log` so the next import treats it as the SAME row (UPSERT, not insert);
   OR (b) make the import dedup by (date, kind, amount[, description]) so a
   re-imported cell that matches an existing manual row updates it in place
   rather than inserting a twin. Prefer (a) — it makes the manual row a
   first-class sheet-backed row after write-back. Whatever the choice, ensure no
   EXISTING imported row is mis-rekeyed. Test: create a manual txn → write it
   back to an empty cell → re-import the sheet → exactly ONE row exists (no
   duplicate); Saldo/totals unchanged.
2. **economia write-back tag-filter (P3)** — add the same `exclude_from_totals`
   `NOT EXISTS` filter to `load_economia_by_month` so an "Ignorar"-tagged reserve
   transfer is NOT written to the Economia column (parity with the metric path).
   Test.
3. **sync debounce (P3)** — only write `sheets_last_focus_probe_at` on
   FOCUS-triggered probes, not on the interval-loop tick, so the configured
   interval cadence holds. Test the decision (focus path writes it; interval path
   does not).

## Done criteria

- A manual txn written back + re-imported yields exactly one row (regression test).
- `load_economia_by_month` carries the `exclude_from_totals` filter.
- The interval tick does not write the focus-debounce key.
- `npm run rust:check` + `npm run check` → exit 0; the 3 regression tests pass.

## STOP conditions

- If re-keying a manual row to the deterministic `row_id` could collide with or
  overwrite an existing imported row, STOP and report (prefer the import-dedup
  approach instead).
- Do NOT change the Performance formula, the economia model, or any flag/gate.

## Maintenance notes

- After this, the manual-entry → write-back → re-import round-trip is
  idempotent (no twins) — note this invariant near `row_id`.
