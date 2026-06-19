# Plan 007: Correctness: engine/date edge cases + effect/transaction hygiene

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/google_sheets/import.rs src-tauri/src/commands.rs src/lib/useCommand.ts src-tauri/src/conflicts.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Five small, independent correctness defects spread across the Rust core and one
React hook. None is a data-loss path by itself, but together they silently
produce wrong dates (wrong cycle due-date when `closing_day = 0`), wrong
timestamps (all rows misdated to 2025 when the layout year cannot be detected),
an overstated projection seed (Economia transfers excluded from the gap query),
a stale-closure risk in the hook, and a non-atomic pair of SQL writes that can
leave a conflict record half-resolved on crash. Fixing them now is cheap
(all five are surgical one-location changes) and prevents harder-to-diagnose
bugs once more data is live in the app.

## Current state

### Relevant files

- `src-tauri/src/forecast/mod.rs` — pure forecast engine; `cycle_due_date()` lives here (lines 283–306); no IO, deterministic, easy to test.
- `src-tauri/src/google_sheets/import.rs` — row parser; two call sites of `layout.year.unwrap_or(2025)` (lines 552 and 674); both silently fall back to 2025 if `layout.year` is `None`.
- `src-tauri/src/commands.rs` — imperative shell; `projection_seed()` gap query (lines 465–473) excludes `'transfer'` transactions.
- `src/lib/useCommand.ts` — SWR-lite hook; `fetcher` omitted from the `useEffect` deps (lines 68–69).
- `src-tauri/src/conflicts.rs` — conflict resolution; `resolve()` (lines 54–113) issues two sequential `execute()` calls with no surrounding transaction.

### Code excerpts as of commit d183bbf

**Fix 1 — `cycle_due_date` (`forecast/mod.rs:283–284`)**

```rust
// line 283
pub fn cycle_due_date(checkin_date: NaiveDate, closing_day: u32, due_day: u32) -> NaiveDate {
    let (cycle_close_year, cycle_close_month) = if checkin_date.day() <= closing_day {
```

When `closing_day = 0`: `checkin_date.day()` is always ≥ 1, so `day() <= 0`
is always false → the `else` branch always fires → the cycle is always treated
as closed last month, regardless of the actual checkin date. A card configured
with `closing_day = 0` (which `u32` admits) produces consistently wrong due
dates.

**Fix 2 — hardcoded year fallback (`import.rs:552` and `import.rs:674`)**

```rust
// line 552
let year = layout.year.unwrap_or(2025);

// line 674
let year = layout.year.unwrap_or(2025);
```

`layout.year` comes from `layout_detect::parse_year_from_name(sheet_name)`,
which returns `None` for any sheet whose name is not a bare integer (e.g.
`"Finanças"`, `"Sheet1"`, or a future year named differently). On `None`, both
call sites silently fall back to `2025`, misdating every row to that year with
no warning to the caller or user.

**Fix 3 — `projection_seed` gap query excludes transfers (`commands.rs:465–467`)**

```rust
// lines 465–467
let gap: (i64,) = sqlx::query_as(
    "SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
     FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense')",
)
```

The gap compensates for realized transactions that occurred between the last
imported `sheet_daily_balance` row and today. `'transfer'` rows (Economia —
moving money to a reserve/savings account) are excluded. Since a transfer
reduces the liquid balance (amount leaves the spending account), excluding it
overstates the seed by the sum of any Economia transfers in the gap window.

The comment at lines 493–494 explicitly defers the Economia treatment as a P2
review item; this step resolves it.

**Fix 4 — stale `fetcher` closure in `useCommand` (`useCommand.ts:39–69`)**

```ts
// lines 39, 43, 68–69
useEffect(() => {
  …
  fetcher()
  …
  // eslint-disable-next-line react-hooks/exhaustive-deps -- fetcher is a stable module-level wrapper
}, [cmd]);
```

The suppression comment claims `fetcher` is always a stable module-level
wrapper. That assumption holds for every current call site (all callers pass
an arrow that calls a module-level `invoke` wrapper). However, the contract is
not enforced at the API boundary: any caller who passes an unstable inline
arrow (e.g. `() => invoke("cmd", args)` inside a component body) will silently
use a stale closure because `fetcher` is missing from the deps array.

The React Compiler (enabled in this repo — see `vite.config.ts`) will not
automatically fix missing explicit deps inside a `useEffect`; it handles
re-renders of components, not effect dependency arrays.

The safe fix is to document and enforce the stable-identity contract at the
API boundary (JSDoc + explicit invariant comment), or alternatively expose a
second overload that accepts a stable `cmd` key alongside an inline fetcher.
Because changing the deps array would change runtime behavior (callers who
currently pass unstable arrows would start re-running the effect on every
render, which is also wrong), the minimal correct fix is to:
1. Strengthen the JSDoc so callers know they must pass a referentially stable
   `fetcher`, and
2. Replace the vague eslint-disable comment with one that explicitly states
   the invariant, so future readers understand the assumption rather than just
   seeing a suppression.

Do NOT add `fetcher` to the deps array without also auditing every call site —
that would break the cache semantics for callers who already pass inline arrows.
Do NOT add `useCallback` or `useMemo` wrappers in callers — the React Compiler
(enabled) handles that; manual wrapping conflicts with it.

**Fix 5 — non-atomic sequential writes in `conflicts.resolve()` (`conflicts.rs:54–113`)**

```rust
// lines 54–113 (abridged)
match field.as_str() {
    "amount" => {
        …
        sqlx::query("UPDATE \"transaction\" …").execute(pool).await?; // write 1
    }
    "description" => {
        …
        sqlx::query("UPDATE \"transaction\" …").execute(pool).await?; // write 1
    }
    …
}
sqlx::query("UPDATE import_conflict SET resolved_at=…").execute(pool).await?; // write 2
```

Two separate `execute(pool)` calls with no surrounding transaction. If the
process crashes between write 1 and write 2, the transaction row is updated but
the conflict is left un-resolved. The inverse (write 2 succeeds, write 1 fails)
is harder since write 1 is first, but both share the same crash window. Wrap
both writes in a single `sqlx` transaction.

### Repo conventions

- **Error handling**: functions return `Result<T, String>` (not `anyhow`) — see
  `conflicts.rs:33` and `commands.rs:433`. Match this style.
- **Money**: integer cents; amounts are positive magnitude — `forecast/mod.rs:18`.
- **Functional-core / imperative-shell**: `forecast/mod.rs` is pure (no IO);
  `commands.rs` is the shell. Keep it that way — do not introduce IO into
  `forecast/mod.rs`.
- **No manual memo/useCallback/useMemo**: React Compiler is enabled
  (`vite.config.ts`). Do NOT add manual memoization; it conflicts with the
  compiler's output.
- **Sqlx transactions**: open with `pool.begin().await` → `let mut tx = …`;
  bind queries with `&mut *tx`; close with `tx.commit().await`. See the pattern
  in `google_sheets/import.rs` (the `import_rows` function uses this style).

## Commands you will need

| Purpose          | Command                                                                 | Expected on success          |
|------------------|-------------------------------------------------------------------------|------------------------------|
| Rust check       | `npm run rust:check`                                                    | exit 0 (fmt + clippy + test) |
| Rust tests only  | `cargo test --manifest-path src-tauri/Cargo.toml --locked`             | all pass                     |
| TS typecheck     | `npm run typecheck`                                                     | exit 0, no errors            |
| TS lint          | `npm run lint`                                                          | exit 0                       |
| TS unit tests    | `npm run test:run`                                                      | all pass                     |
| Full gate        | `npm run check`                                                         | exit 0                       |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/forecast/mod.rs` — Fix 1: guard/clamp `closing_day`
- `src-tauri/src/google_sheets/import.rs` — Fix 2: replace silent fallback; add test
- `src-tauri/src/commands.rs` — Fix 3: include transfers in gap query; add test
- `src/lib/useCommand.ts` — Fix 4: strengthen JSDoc + invariant comment
- `src-tauri/src/conflicts.rs` — Fix 5: wrap sequential writes in one transaction

**Out of scope** (do NOT touch):

- `src-tauri/src/google_sheets/layout_detect.rs` — `parse_year_from_name` already returns `Option<i32>`; the problem is the consumer, not the producer.
- Any other file in `src-tauri/src/` or `src/` — these are independent surgical fixes; scope creep would make the diff harder to review and risks breaking unrelated functionality.
- Schema migrations in `src-tauri/migrations/` — no schema change is required.
- `plans/README.md` line for plan 007 — update it after all five steps pass.

## Git workflow

- Branch: `advisor/007-correctness-edge-cases`
- Commit style: conventional commits, matching repo history (e.g.
  `fix: clamp closing_day=0 in cycle_due_date`,
  `fix: fail loudly when layout year is undetected (import.rs)`,
  `fix: include transfers in projection_seed gap query`,
  `fix: document fetcher stable-identity contract in useCommand`,
  `fix: wrap conflict resolve() writes in one transaction`).
  One commit per fix is fine; or a single `fix: correctness edge cases (plan 007)`
  if the reviewer prefers it.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Guard `closing_day = 0` in `cycle_due_date`

**File**: `src-tauri/src/forecast/mod.rs`, function `cycle_due_date` starting at line 283.

Add a guard at the top of the function body that clamps `closing_day` to the
range `1..=28`. If `closing_day` is 0, treat it as 1 (the earliest valid
closing day); if it exceeds 28, clamp to 28 (safe across all months). This is
a defensive normalisation: the method's reference behaviour does not define a
closing day of 0, so 0 is either a misconfiguration or a default-initialised
value.

Suggested shape (add before the `if checkin_date.day() <= closing_day` block):

```rust
// Clamp closing_day to a valid calendar day present in every month.
// closing_day = 0 is not a valid cycle boundary; treat it as 1.
// closing_day > 28 would skip February; cap at 28.
let closing_day = closing_day.clamp(1, 28);
```

Then add a regression test in the existing `#[cfg(test)]` block (around line
916, after the existing T7.2d test). Name it `cycle_due_date_closing_day_zero`
and assert that a `closing_day = 0` input does NOT always return the prior
month. Suggested case: checkin `2026-01-15`, `closing_day = 0`, `due_day = 10`
→ after clamping to 1, checkin day 15 > closing day 1, so the cycle closed last
month (December), due date is January 10. Assert `due == d("2026-01-10")`.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked cycle_due_date` → all `cycle_due_date_*` tests pass, including the new one.

---

### Step 2: Fail loudly when layout year is undetected in `parse_rows_with_layout`

**File**: `src-tauri/src/google_sheets/import.rs`, lines 552 and 674.

Both occurrences of `layout.year.unwrap_or(2025)` must be replaced. The safest
approach consistent with the repo's `Result<_, String>` convention is to make
`parse_rows_with_layout` return `Result<Vec<ImportedRow>, String>` (and update
its callers) so it can propagate an error when `layout.year` is `None`.

**However**, check the callers before changing the signature:

- `commands.rs:238` — `let imported_rows = import::parse_rows_with_layout(…);`
- `commands.rs:377` — `let imported_rows = import::parse_rows_with_layout(…);`
- `import.rs` internal tests (lines ~945–1690) — all pass a layout with
  `year: Some(2025)`.

If changing the return type is too disruptive (many test call sites), an
acceptable alternative is to keep the return type as `Vec<ImportedRow>` but
log a structured warning and return an empty `Vec` (skip all rows) rather than
silently misdating them. The critical constraint is: **do not date rows to a
wrong year**. Returning no rows is preferable to returning rows with year 2025
when the actual year is different.

Preferred approach (change return type to `Result`):

1. Change the function signature to:
   ```rust
   pub fn parse_rows_with_layout(
       rows: &[Vec<String>],
       layout: &SheetLayout,
       mappings: &[(String, i32)],
       notes: &[Vec<String>],
   ) -> Result<Vec<ImportedRow>, String>
   ```
2. Replace `layout.year.unwrap_or(2025)` at line 552 with:
   ```rust
   let year = layout.year.ok_or_else(|| {
       format!("cannot parse rows: layout for sheet '{}' has no detected year (sheet name must be a 4-digit year)", layout.sheet_name)
   })?;
   ```
3. Apply the same change at line 674 (the `parse_daily_balances_with_layout`
   function or the second occurrence — check which function owns line 674 at
   your live checkout).
4. Propagate the `?` through callers in `commands.rs` (both call sites already
   return `Result<_, String>`, so `?` compiles directly).
5. Update every test in `import.rs` that calls `parse_rows_with_layout` to
   unwrap the `Result` (`.unwrap()`) — they all supply `year: Some(2025)`, so
   they will still pass.

Add one new test that verifies the error path: a `SheetLayout` with
`year: None` must return `Err(…)` (not produce dated rows).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked google_sheets` → all tests pass, including the new `year_none_returns_error` test.

---

### Step 3: Include transfers in the `projection_seed` gap query

**File**: `src-tauri/src/commands.rs`, lines 465–473.

Change the `IN ('income','expense')` filter to also include `'transfer'`:

```rust
// Before
"SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
 FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense')",

// After
"SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
 FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense','transfer')",
```

A `transfer` (Economia) reduces the liquid spending balance (money leaves the
cash account and goes to a reserve). Its `amount` is a positive magnitude. The
existing `CASE` expression already maps non-`income` types to `-amount`, so a
transfer will correctly subtract from the running total. This matches the
method's definition: Economia is a signed outflow from the liquid balance.

Also remove (or update) the now-stale comment at lines 493–494 that reads:
> `transfer` é IGNORADO (não há linha Economia explícita ainda) — a poupança real virá do saldo da reserva quando o slice de Economia existir; até lá o net é um proxy conservador (review P2).

Replace it with a comment that reflects the corrected behaviour:
> `transfer` (Economia) é incluído: reduz o saldo líquido tanto quanto uma despesa.
> O CASE expression abaixo trata corretamente como saída (−amount).

Add a regression test in the `#[cfg(test)]` block of `commands.rs` (after the
existing `projection_seed_folds_realized_gap_up_to_today` test at line 3433).
Name it `projection_seed_gap_includes_transfer_economia`. Use `insert_realized`
with `ttype = "transfer"` in the gap window and assert the seed is reduced by
the transfer amount.

Example:
```rust
#[tokio::test]
async fn projection_seed_gap_includes_transfer_economia() {
    let pool = fixture_pool().await;
    insert_sheet_balance(&pool, "2026", "2026-06-10", 500_000).await;
    // Economia transfer between seed date and today: should reduce seed.
    insert_realized(&pool, "transfer", 50_000, "2026-06-11").await;

    let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
    // 500_000 − 50_000 = 450_000
    assert_eq!(projection_seed(&pool, today).await.unwrap(), 450_000);
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked projection_seed` → all `projection_seed_*` tests pass, including the new one.

---

### Step 4: Document and enforce the stable-fetcher contract in `useCommand`

**File**: `src/lib/useCommand.ts`, function `useCommand`, lines 35–76.

Do NOT change the deps array. Do NOT add `useCallback`/`useMemo`. The fix is
purely documentary + comment precision:

1. Expand the JSDoc on `useCommand` to explicitly state the `fetcher` contract:

```ts
/**
 * SWR-lite for Tauri commands: returns the last successful response
 * synchronously (no skeleton flash on re-navigation) and revalidates in the
 * background on every mount. Scope is deliberately tiny — one cache entry per
 * command name; argumentful commands can join when a screen needs them.
 *
 * @param cmd   - Stable string key identifying the Tauri command. Changing
 *                `cmd` discards cached data and triggers a fresh load.
 * @param fetcher - MUST be referentially stable across renders (a module-level
 *                  arrow or a function defined outside the component). Passing
 *                  an inline arrow `() => invoke(…)` will NOT re-run the effect
 *                  on change — the first fetcher reference is captured and kept.
 *                  If you need per-render arguments, encode them into `cmd`
 *                  (e.g. `"month:2026-07"`) and define a stable fetcher that
 *                  reads the key, or use `invalidateCommands()` after writes.
 */
```

2. Replace the existing eslint-disable comment at line 68–69:

```ts
// eslint-disable-next-line react-hooks/exhaustive-deps -- fetcher is a stable module-level wrapper
```

with a more explicit invariant comment:

```ts
// INVARIANT: fetcher must be referentially stable (module-level arrow or stable
// function ref). Adding fetcher to deps would cause re-runs for callers that
// inline their arrow, breaking the "no skeleton flash on remount" contract.
// See JSDoc on useCommand for the stable-fetcher requirement.
// eslint-disable-next-line react-hooks/exhaustive-deps
```

No logic changes; this step is documentation only. The existing tests in
`src/lib/useCommand.test.ts` cover all current behaviour and must continue to
pass unchanged.

**Verify**: `npm run typecheck && npm run lint && npm run test:run -- useCommand` → all pass, no new type errors, lint clean.

---

### Step 5: Wrap `conflicts.resolve()` writes in one SQLite transaction

**File**: `src-tauri/src/conflicts.rs`, function `resolve`, lines 33–114.

The function currently accepts `pool: &SqlitePool`. Change it to open a
transaction internally so both writes are atomic:

1. Change the pool borrow to open a transaction at the top of the function body
   (after the initial `SELECT` that loads the conflict row):

```rust
let mut tx = pool.begin().await.map_err(|e| format!("begin tx: {e}"))?;
```

2. Re-bind every subsequent `sqlx::query(…).execute(pool)` call to use
   `&mut *tx` instead of `pool`:

   - The `UPDATE "transaction"` inside the `match` arms (write 1).
   - The `UPDATE import_conflict SET resolved_at=…` at the end (write 2).

3. Commit at the end:

```rust
tx.commit().await.map_err(|e| format!("commit resolve: {e}"))?;
Ok(())
```

Note: the initial `SELECT` (loading the conflict row) can stay on `pool`
directly — it is a read-only query and does not need to be inside the write
transaction.

The function signature `pub async fn resolve(pool: &SqlitePool, id: &str, choice: &str) -> Result<(), String>` does not change. Callers in
`conflicts.rs:127–133` are unaffected.

The existing tests (`resolve_sheet_writes_sheet_value_and_aligns_base`,
`resolve_local_keeps_local_value_but_aligns_base_to_sheet`, etc.) exercise
the full round-trip through an in-memory SQLite pool and will catch any
regression.

Add one new test `resolve_is_atomic_on_simulated_crash` that:
- Seeds a transaction and a conflict.
- Replaces the pool with a pool that has WAL mode enabled (or simply verifies
  that after `resolve()` succeeds both the `transaction` row and the
  `import_conflict` row are updated in the same query — the existing tests
  already implicitly verify this).
- **Simpler alternative**: just verify that calling `resolve()` twice on the
  same conflict ID (the second call should be a no-op because `resolved_at IS
  NULL` is false) does not corrupt data.

```rust
#[tokio::test]
async fn resolve_idempotent_second_call_is_noop() {
    let p = pool().await;
    seed_txn(&p, "t1", 10000, "x").await;
    seed_conflict(&p, "c1", "t1", "amount", "8000", "10000", "12000").await;

    resolve(&p, "c1", "sheet").await.unwrap();
    // Second call: conflict already resolved (resolved_at IS NOT NULL → row not found).
    // Must not error and must not corrupt the already-resolved state.
    resolve(&p, "c1", "sheet").await.unwrap();
    assert_eq!(amount_of(&p, "t1").await, (12000, Some(12000)));
    assert!(list_conflicts(&p).await.unwrap().is_empty());
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked conflicts` → all `conflicts::tests::*` pass, including the new test.

---

### Step 6: Full gate

Run the full gate to confirm all five fixes compose cleanly:

**Verify**: `npm run check` → exit 0, all checks green.

## Test plan

New tests (all regression/bug-fix tests):

| Test name | File | What it guards |
|---|---|---|
| `cycle_due_date_closing_day_zero` | `forecast/mod.rs` | Fix 1: `closing_day = 0` no longer always routes to prior month |
| `year_none_returns_error` (or `parse_rows_year_none_errors`) | `import.rs` | Fix 2: `layout.year = None` returns `Err`, not misdated rows |
| `projection_seed_gap_includes_transfer_economia` | `commands.rs` | Fix 3: Economia transfer in gap window reduces the seed |
| `resolve_idempotent_second_call_is_noop` | `conflicts.rs` | Fix 5: idempotent double-resolve, also exercises the transaction path |

Fix 4 (`useCommand`) has no new test — it is a documentation-only change that does not alter observable runtime behaviour. The existing five tests in `useCommand.test.ts` must continue to pass unchanged.

**Structural pattern for new Rust tests**: model after the existing tests in
each file's `#[cfg(test)]` block. For `forecast/mod.rs`, follow the `T7.2*`
block at line 914. For `commands.rs`, follow `projection_seed_folds_realized_gap_up_to_today` at line 3433. For `conflicts.rs`, follow `resolve_sheet_writes_sheet_value_and_aligns_base` at line 186.

**Run tests per-step**: use the filter commands in "Commands you will need" to
run only the affected module's tests after each step, then run the full gate at
the end (Step 6).

## Done criteria

Machine-checkable. ALL must hold before marking this plan DONE:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked cycle_due_date` exits 0; a test named `cycle_due_date_closing_day_zero` exists and passes.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked google_sheets` exits 0; a test that passes `year: None` to `parse_rows_with_layout` exists and asserts `Err(…)`.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked projection_seed` exits 0; a test named `projection_seed_gap_includes_transfer_economia` exists and passes.
- [ ] `npm run typecheck` exits 0.
- [ ] `npm run lint` exits 0.
- [ ] `npm run test:run` exits 0 (all TS tests pass; `useCommand` tests unchanged).
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked conflicts` exits 0; a test named `resolve_idempotent_second_call_is_noop` exists and passes.
- [ ] `grep -n "unwrap_or(2025)" src-tauri/src/google_sheets/import.rs` returns no matches.
- [ ] `grep -n "IN ('income','expense')" src-tauri/src/commands.rs` in the `projection_seed` gap query returns no matches (the query now includes `'transfer'`).
- [ ] `npm run check` exits 0.
- [ ] `git diff --name-only HEAD` shows only files in the in-scope list (or their test blocks within the same file).
- [ ] `plans/README.md` status row for plan 007 updated to DONE.

## STOP conditions

Stop and report (do not improvise) if:

- Any code excerpt in "Current state" does not match the live file at the line numbers cited — the plan was written for commit `d183bbf` and the codebase may have moved.
- `parse_rows_with_layout` already returns `Result` in the live code (meaning Fix 2 was partially applied) — reconcile before proceeding.
- The `transfer` row type behaves differently in the gap window than described (e.g. a `to_account_id` join is required for the `transfer` to affect the liquid balance) — do not assume the simple inclusion is correct; investigate and report.
- Changing `conflicts.resolve()` to open an internal transaction requires touching `commands.rs` or any caller in a way that breaks the `Result<(), String>` contract — report rather than changing the function signature.
- A step's `cargo test` or `npm run` verification fails twice after a reasonable fix attempt.
- Any fix appears to require touching a file outside the in-scope list.

## Maintenance notes

- **Fix 1** (`cycle_due_date`): if the credit-card configuration UI ever adds
  validation that prevents `closing_day = 0` from reaching the engine, the
  clamp in `cycle_due_date` becomes redundant but harmless. Leave it as
  defence-in-depth.
- **Fix 2** (`layout.year`): the assumption is that every valid data tab has a
  4-digit year as its name. If a future product decision introduces tabs named
  differently (e.g. `"2026-Q1"`), `parse_year_from_name` in
  `layout_detect.rs` must be extended — do not silently fall back to a
  hardcoded year.
- **Fix 3** (gap query): when the Economia import slice lands (plan 003 or a
  later slice that imports the `Economia` tab), re-verify that `transfer`
  rows in the gap window still have the correct sign. The method records
  Economia as a positive-magnitude outflow from the spending balance, which
  the current `CASE` expression handles correctly — but a different
  representation (e.g. signed amounts) would break this.
- **Fix 4** (`useCommand` JSDoc): if a caller ever needs to pass a
  per-render fetcher (e.g. one that captures `ownerId` from component state),
  the correct pattern is to encode the argument into `cmd` (as is already done
  for `"month:2026-07"` in the existing test). The JSDoc now explains this.
- **Fix 5** (`conflicts.resolve` transaction): if the conflict resolution flow
  ever expands (e.g. resolving multiple fields in one call), the transaction
  boundary here already covers it — just add more writes inside the same `tx`.
- **Reviewer**: in the PR, pay particular attention to Fix 3's CASE expression
  to confirm the sign is correct for `'transfer'`, and to Fix 2's return-type
  change propagation to both `commands.rs` call sites.
