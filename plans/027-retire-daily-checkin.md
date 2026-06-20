# Plan 027: Retire the vestigial `daily_checkin` table (spike 024 → Option A)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan in
> `plans/README.md` (and flip plan 024 to DONE — Option A implemented) unless a
> reviewer told you they maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 7041f6e..HEAD -- src-tauri/src/commands/forecast_cmds.rs src-tauri/src/lib.rs src-tauri/src/commands/mod.rs`
> If any in-scope file changed since this plan was written, re-open the cited
> spots and compare before proceeding; on a mismatch, treat it as a STOP.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW (the table has no production writer → it is always empty in
  production, so every read of it already returns 0/nothing; removing the reads
  is behavior-preserving for real users)
- **Category**: tech-debt
- **Depends on**: none (plan 022 already removed the credit-side surfacing)
- **Planned at**: commit `7041f6e`, 2026-06-20
- **Decision source**: spike `plans/024-spike-daily-checkin-decision.md` — Option A

## Why this matters

`daily_checkin (date, daily_spend, credit_spend)` is vestigial. The spike (024)
confirmed: **no production code writes to it** — the only `INSERT`s are the demo
seed and tests. Every real daily check-in is recorded as a normal Diário
`transaction` instead. The table is still _read_ in two fallback branches that,
with an always-empty table, are dead code; and its `credit_spend` column is the
last structural remnant of the "credit accumulates daily" idea that the project
already decided to drop (credit is a lump on the due date — plan 022). Option A
removes the whole table to stay faithful and delete the dead paths.

## Current state (verified at `7041f6e` — re-open and re-confirm line numbers)

### No production writer

`INSERT INTO daily_checkin` exists only at:

- `src-tauri/src/lib.rs:342` — demo/sample seed.
- `src-tauri/src/commands/mod.rs:246` and `:1447` — test fixtures.

### Two production readers (both fallbacks; empty table → no-op)

1. `src-tauri/src/commands/forecast_cmds.rs:346` — in `load_cashflow_events`:
   ```rust
   "SELECT date, daily_spend, credit_spend FROM daily_checkin WHERE date > ?1 AND date <= ?2",
   ```
   then (~:357–373) it loops the rows, folding `daily_spend` into a Diário
   event and (`if credit_spend > 0`) `credit_spend` into a due-day lump.
2. `src-tauri/src/commands/forecast_cmds.rs:941` — the daily-spend query's
   `CASE … ELSE` branch:
   ```sql
   … ELSE COALESCE((SELECT SUM(daily_spend) FROM daily_checkin WHERE date = ?1), 0) …
   ```
   (the `WHEN EXISTS(<Diário transaction today>) THEN <sum of today's Diário> …`
   branch is the real path).

### Tests that exercise the table (must be removed with it)

- `src-tauri/src/commands/mod.rs` ~:1394–1443 — `T7.2` credit-cycle test that
  seeds `credit_spend` into `daily_checkin` and asserts it lands as a due-day
  lump (the exact "Régua 2" behavior being retired).
- The `daily_checkin` INSERT helpers at `mod.rs:246` and the test at `:1447` and
  any assertion of the `daily_spend` fallback.

### Migration + docs

- `src-tauri/migrations/20240608000010_daily_checkin.sql` — creates the table.
- `docs/adr/0001-dual-tracking-daily-credit.md` — documents the Régua 1/Régua 2
  dual tracking (the Régua 2 half is what is being retired).
- `CONTEXT.md` — carries `daily_checkin` vocabulary.

## Commands (verification gates)

`npm run rust:check` · `npm run typecheck` · `npm run test:run` · `npm run e2e`
· `npm run doctor` · `npm run check`

## Scope

**In scope**: `forecast_cmds.rs` (remove both reads), `lib.rs` (remove the seed
insert + its sample rows), `commands/mod.rs` (remove the daily_checkin tests +
helpers), a new forward `DROP TABLE` migration, `docs/adr/0001-*.md`,
`CONTEXT.md`, `plans/README.md`.

**Out of scope**: the forecast math itself, the credit→`FixedOut` `classify()`
routing (faithful — do not touch), `write_back.rs`, any other table.

## Steps

1. **Remove the `load_cashflow_events` daily_checkin read** (`forecast_cmds.rs`
   ~:346–373): delete the `SELECT … FROM daily_checkin …` query and the loop
   that folds its rows into Diário events / due-day lumps. Keep the rest of the
   event loading (transactions, recurrences) intact.
   - _Verify_: `npm run rust:check` compiles; forecast tests still pass.

2. **Simplify the daily-spend query** (`forecast_cmds.rs` ~:935–945): drop the
   `ELSE COALESCE((SELECT SUM(daily_spend) FROM daily_checkin …), 0)` fallback
   so daily spend comes only from Diário transactions. If the `CASE` now has a
   single arm, reduce it to the direct transaction-sum (returning 0 when there
   is no Diário transaction that day, matching today's empty-table behavior).
   - _Verify_: `cargo test … dashboard` daily-spend tests pass (update any test
     that asserted the fallback path).

3. **Remove the demo seed insert** (`lib.rs` ~:342 + the sample `daily_checkin`
   rows around it). The demo already seeds transactions; do not add new sample
   data. Confirm the seed still runs and the demo dashboard is non-empty.
   - _Verify_: `npm run rust:check`.

4. **Remove the tests** in `commands/mod.rs`: the `T7.2` credit-cycle test
   (~:1394–1443), the `daily_checkin` INSERT helper/test at `:246` and `:1447`,
   and any `daily_spend`-fallback assertion. Do not weaken unrelated assertions
   in those test fns — if a test mixes daily_checkin with still-valid coverage,
   keep the valid part and drop only the daily_checkin portion.
   - _Verify_: `npm run rust:check` (all tests compile + pass).

5. **Add the drop migration**: create
   `src-tauri/migrations/20240620000001_drop_daily_checkin.sql` with
   `DROP TABLE IF EXISTS daily_checkin;` (forward-only; match the existing
   migration file style/header). Confirm it is the highest-numbered migration.
   - _Verify_: a fresh DB migrates clean (`npm run rust:check` runs the
     migration-applied tests).

6. **Update ADR-0001** (`docs/adr/0001-dual-tracking-daily-credit.md`): record
   that the "Régua 2 / daily-accruing credit" half is retired (credit is a lump
   on the due date — see plan 022); the daily debit ritual ("Régua 1") remains,
   recorded as Diário `transaction` rows, not a separate table. Note the table
   was dropped (plan 027). Method-neutral language.
   - _Verify_: doc reads coherently; no dangling references to `daily_checkin`
     as a live mechanism.

7. **Update `CONTEXT.md`**: remove/adjust the `daily_checkin` vocabulary lines
   to reflect that the daily ritual is a transaction, not a dedicated table.

8. **Flip the index**: `plans/README.md` — plan 027 row → DONE; plan 024 row →
   `DONE — Option A` (spike resolved + implemented).

## Test plan

- No NEW behavior to test (this is a removal). The existing forecast and
  dashboard tests are the safety net — they must stay green after the reads are
  removed, proving the table was not contributing to real output.
- After removal, grep proves the table is gone:
  `grep -rn "daily_checkin" src-tauri/src` → returns nothing (the only allowed
  hit is the new drop-migration file under `migrations/`).

## Done criteria (machine-checkable)

- `grep -rn "daily_checkin" src-tauri/src` → no matches.
- `grep -rn "credit_spend\|daily_spend FROM daily_checkin" src-tauri/src` → no
  matches.
- `npm run rust:check` → exit 0 (fmt + clippy `-D warnings` + all tests).
- `npm run check` → exit 0 (incl. `test:run`, `e2e`, privacy scan).
- `npm run doctor` → no new findings.

## STOP conditions

- If removing the `daily_checkin` read **changes any forecast/dashboard test
  result** (i.e. the table was actually contributing output) — STOP and report;
  Option A assumes the table is always empty in production.
- If a test in `commands/mod.rs` couples daily_checkin coverage with unrelated,
  still-valid assertions you cannot cleanly separate — STOP and report rather
  than deleting real coverage.
- If you find any **production** write to `daily_checkin` (outside the demo seed
  - tests) — STOP; the vestigial premise is wrong.
- Do not touch `write_back.rs` or `classify()`.

## Maintenance notes

- This supersedes the Régua-2 half of ADR-0001. A future "explicit daily ritual
  gesture" feature (spike 024 Option C) would be a _new_ design decision, not a
  revival of this table.
- Forward-only migration: existing installs simply drop the (empty) table on
  next launch; no data migration needed.
