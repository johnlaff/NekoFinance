# Plan 053: Bug bundle — daily_spend_today SUM(ABS) + update amount guard + write-back audit/clamp P3s

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 5ada3ae..HEAD -- src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/transactions.rs src-tauri/src/commands/write_back_cmds.rs src/screens/TotaisScreen.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Package**: H
- **Planned at**: commit `5ada3ae`, 2026-06-21

## Why this matters

Five related sign/guard bugs survived the Package-G sweep. Bug 1 causes `daily_spend_today` to under-report on mixed-import-and-manual days: outer `ABS(SUM(signed))` partially cancels before taking absolute value, so the "Diário hoje" tile shows a number smaller than actual spend. Bug 2 lets `update_transaction_cmd` silently store `amount = 0` or a negative value, violating the positive-magnitude invariant enforced at create time. Bugs 3 and 4 affect write-back audit accuracy: derived reimbursement rows get their `source_amount` clobbered on income realign, and a type-change on an itemized transaction leaves stale `line_items` whose sum no longer matches the parent total. Bug 5 lets `ytdPct` in `TotaisScreen` render ">100% acumulado" on edge months, which is confusing. All fixes are small, isolated, and each gets a regression test.

## Current state

### Files and roles

- `src-tauri/src/commands/forecast_cmds.rs` — `dashboard_summary` inner fn (testable) at line 1145; the buggy daily-spend query at lines 1177–1185; correct `SUM(ABS(amount))` exemplars at lines 182, 237, 1062–1063.
- `src-tauri/src/commands/transactions.rs` — `create_transaction_inner` amount guard at line 363; `update_transaction_cmd` at lines 584–671 (missing guard; line-item clear condition at lines 633–641).
- `src-tauri/src/commands/write_back_cmds.rs` — `load_write_back_txns` derived-exclusion exemplar at line 19; `record_write_back_audit` at line 594; the buggy `"entrada"` UPDATE at lines 623–632; `ensure_reserve_account` at line 980.
- `src/screens/TotaisScreen.tsx` — `ytdPct` computation at lines 229–231; rendered at line 315.

### Bug 1 — `daily_spend_today` mixed-sign cancellation

`forecast_cmds.rs:1177–1185` (inside `dashboard_summary`):

```rust
// forecast_cmds.rs:1177
let daily_spend: (i64,) = sqlx::query_as(
    "SELECT ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                          WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                            AND (payment_method IS NULL OR payment_method <> 'credit')), 0))",
)
.bind(&today)
.fetch_one(pool)
.await
.map_err(|e| format!("query daily spend: {e}"))?;
```

Problem: `SUM(amount)` over a mix of imported expenses (stored negative, e.g. `-5000`) and manual expenses (stored positive, e.g. `3000`) produces a signed partial sum (`-2000`) before `ABS()` is applied → reports `2000` instead of the correct `8000`.

Fix pattern (match `realized_monthly_baseline` at `forecast_cmds.rs:237` and `month_grid` at lines 1062–1063):

```rust
"SELECT COALESCE((SELECT SUM(ABS(amount)) FROM \"transaction\" \
                  WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                    AND (payment_method IS NULL OR payment_method <> 'credit')), 0)"
```

`ABS()` moves inside `SUM` so each row contributes its magnitude. Outer `COALESCE` replaces `NULL` (zero rows) with `0`. The wrapping `ABS(COALESCE(...))` becomes unnecessary; remove it so the query result type remains `(i64,)`.

Note: the comment block at lines 1171–1176 says "ABS() é defesa-em-profundidade" — update it to reflect the corrected pattern (`SUM(ABS(amount))`).

### Bug 2 — `update_transaction_cmd` missing amount guard

`create_transaction_inner` rejects non-positive amounts at `transactions.rs:363`:

```rust
// transactions.rs:363
if amount_cents <= 0 {
    return Err("valor deve ser positivo (magnitude)".into());
}
```

`update_transaction_cmd` at `transactions.rs:584–671` has no such guard. After the `txn_type` validation at line 596, `amount_cents` flows unchecked into the `UPDATE` at line 644.

Fix: add the guard immediately after the type check (line 598), before any DB access:

```rust
// after: if !matches!(txn_type.as_str(), "income" | "expense") { … }
if amount_cents <= 0 {
    return Err("valor deve ser positivo (magnitude)".into());
}
```

### Bug 3 — `record_write_back_audit` "entrada" arm clobbers derived rows

`write_back_cmds.rs:623–632`:

```rust
"entrada" => {
    sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'income'",
    )
    .bind(c.value_cents)
    .bind(&now)
    .bind(&c.date)
    .execute(&mut *tx)
    .await
}
```

This matches ALL `type='income'` rows for the date, including `id LIKE 'derived:%'` (reimbursement rows synthesised by the importer). `load_write_back_txns` explicitly excludes derived rows at line 19:

```rust
// write_back_cmds.rs:19
"… AND id NOT LIKE 'derived:%'",
```

Fix: add the same exclusion to the audit UPDATE:

```sql
WHERE date = ?3 AND type = 'income' AND id NOT LIKE 'derived:%'
```

### Bug 4 — `update_transaction_cmd` line_items not cleared on type change

`transactions.rs:633–641` — the stale-items clear fires only when `old_amount != amount_cents`:

```rust
// transactions.rs:633
if let Some((old_amount, item_count)) = current
    && item_count > 0
    && old_amount != amount_cents
{
    sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
        …
}
```

A type change (e.g. `income → expense` with the same amount) leaves line_items whose `Σ` still matches the parent total numerically, but whose semantic (income line items on an expense row) is wrong and will confuse write-back. The condition must also trigger on a type change.

Before the `current` query the command already has the new `txn_type` in scope. Load the old type alongside the old amount:

```rust
let current: Option<(i64, i64, String)> = sqlx::query_as(
    r#"SELECT t.amount, COUNT(li.id), t.type
       FROM "transaction" t
       LEFT JOIN line_item li ON li.transaction_id = t.id
       WHERE t.id = ?1
       GROUP BY t.amount, t.type"#,
)
…
if let Some((old_amount, item_count, old_type)) = current
    && item_count > 0
    && (old_amount != amount_cents || old_type != txn_type)
{
    // DELETE line_items …
}
```

### Bug 5 — `ytdPct` unclamped in `TotaisScreen.tsx`

`TotaisScreen.tsx:229–231`:

```ts
const ytdPct = Math.round(
  (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
);
```

No upper bound. When `registered_economia_cents > realized_income_cents` (large one-off transfer in a low-income month) `ytdPct` exceeds 100.

`TotaisScreen.tsx:315`:

```tsx
sublabel={`no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`}
```

Fix: clamp at 100 and add a visual indicator when the raw value was truncated:

```ts
const ytdPctRaw = Math.round(
  (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
);
const ytdPct = Math.min(ytdPctRaw, 100);
const ytdPctLabel =
  ytdPctRaw > 100
    ? `no ano: >100% acumulado · meta 20–30% (média anual)`
    : `no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`;
```

Then use `ytdPctLabel` in the `sublabel` prop.

### Bug 6 — `ensure_reserve_account` check-then-insert race (P3, optional)

`write_back_cmds.rs:980–1006`: `ensure_reserve_account` does a `SELECT` then a conditional `INSERT` outside any transaction. Two concurrent import calls (unlikely in practice — Tauri commands are not parallel by default) could both see no reserve account and both insert. The `account` table has no `UNIQUE` constraint on `(liquidity='reserve')`, so the second insert succeeds, leaving two reserve accounts.

The safe fix is to pass `&mut tx` from `store_economia_entries` into `ensure_reserve_account` so the check-and-insert runs inside the same `sqlx::Transaction`. However this requires changing the signature, which affects callers. If the change is simple, do it; otherwise note it as a follow-up.

### Conventions

- Money = positive-magnitude integer cents. `amount` stores the magnitude; sign is encoded in `type`. `create_transaction_inner` enforces this; `update_transaction_cmd` must mirror it.
- Imported expenses arrive negative from the sheet (`-amount_out`). Aggregations over mixed-source expense rows must use `SUM(ABS(amount))`, never `ABS(SUM(amount))`. Established exemplars: `forecast_cmds.rs:237` and `month_grid` at lines 1062–1063.
- `derived:%` rows are synthesised, not imported 1:1 from the sheet. They must never be realigned by write-back. Exclusion pattern: `AND id NOT LIKE 'derived:%'`.
- React Compiler is ON. Do not add `useMemo`/`useCallback` around the new `ytdPctRaw`/`ytdPctLabel` locals — they are plain scalar computations.
- Static `CSSProperties` objects should be hoisted outside the component. `ytdPct` changes are purely in the JSX sublabel string, no new style objects needed.

## Commands you will need

| Purpose         | Command                                       | Expected on success |
| --------------- | --------------------------------------------- | ------------------- |
| Rust type+lint  | `npm run rust:check`                          | exit 0, no errors   |
| TS typecheck    | `npm run typecheck`                           | exit 0, no errors   |
| Lint            | `npm run lint`                                | exit 0              |
| Unit tests      | `npm run test:run`                            | all pass            |
| Rust unit tests | `cd src-tauri && cargo test 2>&1 \| tail -20` | test result: ok     |
| Full gate       | `npm run check`                               | exit 0, all green   |
| Targeted test   | `cd src-tauri && cargo test daily_spend 2>&1` | test result: ok     |

All commands verified in this repo.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/forecast_cmds.rs` — Bug 1 fix + regression test
- `src-tauri/src/commands/transactions.rs` — Bug 2 + Bug 4 fixes + regression tests
- `src-tauri/src/commands/write_back_cmds.rs` — Bug 3 fix + optional Bug 6 + regression test
- `src/screens/TotaisScreen.tsx` — Bug 5 fix

**Out of scope** (do NOT touch, even though they look related):

- `src/screens/totaisStatus.ts` — status thresholds are unrelated; leave untouched.
- `src-tauri/src/commands/mod.rs` — `realized_monthly_baseline` sign bug was already fixed in plan 049; do not re-fix here.
- Any change to the `forecast/mod.rs` performance formula — locked in plan 051; do not touch.
- Any change to the import pipeline (`google_sheets/import.rs`) — out of scope.
- Any UI changes beyond `TotaisScreen.tsx` line 315 area.

## Git workflow

- Branch: `advisor/053-sign-guard-audit-bugs`
- Commit per logical fix (one commit per bug, or group Rust fixes in one commit and the TS fix in another).
- Message style follows repo convention — conventional commits observed in `git log`:
  `fix: daily_spend_today SUM(ABS) — mixed-sign cancellation bug`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Drift check

Run the drift check command from the header. If any in-scope file changed since commit `5ada3ae`, compare the "Current state" excerpts against the live code. If the excerpts no longer match, STOP and report.

**Verify**: `git diff --stat 5ada3ae..HEAD -- src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/transactions.rs src-tauri/src/commands/write_back_cmds.rs src/screens/TotaisScreen.tsx` → either empty output (no drift) or a list you must reconcile before proceeding.

### Step 2: Fix Bug 1 — `daily_spend_today` SUM(ABS)

In `src-tauri/src/commands/forecast_cmds.rs` around line 1177, replace:

```rust
let daily_spend: (i64,) = sqlx::query_as(
    "SELECT ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                          WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                            AND (payment_method IS NULL OR payment_method <> 'credit')), 0))",
)
```

with:

```rust
let daily_spend: (i64,) = sqlx::query_as(
    "SELECT COALESCE((SELECT SUM(ABS(amount)) FROM \"transaction\" \
                      WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                        AND (payment_method IS NULL OR payment_method <> 'credit')), 0)",
)
```

Also update the comment block at lines 1171–1176 to say `SUM(ABS(amount))` instead of the "outer ABS" description, so future readers are not misled.

**Verify**: `cd src-tauri && cargo test 2>&1 | grep -E "FAILED|error\[" | head -20` → no output (all pass).

### Step 3: Add regression test for Bug 1

In `forecast_cmds.rs` `mod tests` (after the last `#[tokio::test]` in the file), add a test named `daily_spend_today_sums_magnitudes_not_signed_amounts`. Pattern: use the same `pool()` and `insert_expense` helpers already in scope.

Test setup:

- Insert one imported expense for today's date: `amount = -50_00` (negative, simulating import).
- Insert one manual expense for today's date: `amount = 30_00` (positive, simulating manual entry). Both: `type='expense'`, `is_fixed=0`, `is_projection=0`, `payment_method=NULL`.
- Call `dashboard_summary(&p, today)`.

Assertions:

- `result.daily_spend_today_cents == 80_00` (sum of magnitudes: 5000 + 3000).
- The wrong result before the fix would be `20_00` (ABS(-5000 + 3000) = 2000), so the assertion distinguishes the two clearly.

Note: `dashboard_summary` requires a seed (needs at least one account with balance, or the `projection_seed` query returns 0 — which is fine). Check whether `projection_seed` or `effective_daily_ceiling` hit required tables: if the test pool with only `transaction` rows causes a DB error, seed with a minimal `INSERT INTO account …` matching the schema (see `write_back_cmds.rs:1144–1151` for a minimal account insert). If `dashboard_summary` always returns `Ok` with zero for missing ancillary tables, no extra setup is needed.

**Verify**: `cd src-tauri && cargo test daily_spend_today_sums_magnitudes 2>&1` → `test result: ok. 1 passed`.

### Step 4: Fix Bug 2 — `update_transaction_cmd` missing amount guard

In `src-tauri/src/commands/transactions.rs`, inside `update_transaction_cmd` (starts at line 584), after the type validation block that ends at line 598, add:

```rust
if amount_cents <= 0 {
    return Err("valor deve ser positivo (magnitude)".into());
}
```

This must appear before any `let now = …` or DB access — the function should reject bad input before touching the pool, mirroring `create_transaction_inner:363`.

**Verify**: `cd src-tauri && cargo check 2>&1 | grep "^error" | head -5` → no output.

### Step 5: Add regression test for Bug 2

In `transactions.rs` `mod tests`, add two tests:

**`update_transaction_cmd_rejects_zero_amount`**:

- Insert a transaction with `amount=1000` using `insert_txn`.
- Call a thin helper that replicates the guard logic (or call the public function directly if accessible — in this file, `update_transaction_cmd` takes a `State`, so write a minimal inner helper that calls the guard check directly, similar to how `run_update_items` wraps the item-update logic).
- Alternatively, extract the guard + DB logic into a `pub(crate) async fn update_transaction_inner(pool, id, txn_type, amount_cents, …)` parallel to `create_transaction_inner`, and test that. If that extraction feels too large, test the guard by calling `update_transaction_cmd` via the existing `test_pool` — note that `State<'_, SqlitePool>` cannot be constructed in unit tests; instead, expose an inner function `pub(crate) async fn update_transaction_inner` at the same level as `create_transaction_inner` that does the real work, and have `update_transaction_cmd` delegate to it (the same pattern already used in this file).

Either approach is acceptable. The test must verify:

- `update_transaction_inner(&pool, "tx-guard", "expense", 0, …).await` returns `Err` containing "positivo".
- The row's `amount` in the DB remains `1000` (unchanged).

**`update_transaction_cmd_rejects_negative_amount`**:

- Same setup; call with `amount_cents = -500`; assert `Err` + row unchanged.

**Verify**: `cd src-tauri && cargo test update_transaction_cmd_rejects 2>&1` → `test result: ok. 2 passed`.

### Step 6: Fix Bug 4 — line_items not cleared on type change

In `transactions.rs`, locate the `current` query at line 622. Modify it to also fetch the old `type`:

Change:

```rust
let current: Option<(i64, i64)> = sqlx::query_as(
    r#"SELECT t.amount, COUNT(li.id)
       FROM "transaction" t
       LEFT JOIN line_item li ON li.transaction_id = t.id
       WHERE t.id = ?1
       GROUP BY t.amount"#,
)
.bind(&id)
.fetch_optional(&mut *tx)
.await
.map_err(|e| format!("update (load items): {e}"))?;
if let Some((old_amount, item_count)) = current
    && item_count > 0
    && old_amount != amount_cents
{
```

To:

```rust
let current: Option<(i64, i64, String)> = sqlx::query_as(
    r#"SELECT t.amount, COUNT(li.id), t.type
       FROM "transaction" t
       LEFT JOIN line_item li ON li.transaction_id = t.id
       WHERE t.id = ?1
       GROUP BY t.amount, t.type"#,
)
.bind(&id)
.fetch_optional(&mut *tx)
.await
.map_err(|e| format!("update (load items): {e}"))?;
if let Some((old_amount, item_count, old_type)) = current
    && item_count > 0
    && (old_amount != amount_cents || old_type != txn_type)
{
```

**Verify**: `cd src-tauri && cargo check 2>&1 | grep "^error" | head -5` → no output.

### Step 7: Add regression test for Bug 4

In `transactions.rs` `mod tests`, add `update_transaction_cmd_clears_items_on_type_change`:

- Insert a transaction: `type='income'`, `amount=1000`.
- Insert two `line_item` rows pointing to it (`transaction_id`).
- Simulate the corrected `update_transaction_cmd` flow: open a transaction, run the updated `current` query with `t.type`, trigger the clear condition with `txn_type="expense"` (different type, same amount).
- Commit.

Assertions:

- `COUNT(*) FROM line_item WHERE transaction_id = ?` → `0` (items cleared).
- `amount FROM "transaction" WHERE id = ?` → `1000` (unchanged, amount was not modified).

**Verify**: `cd src-tauri && cargo test update_transaction_cmd_clears_items_on_type_change 2>&1` → `test result: ok. 1 passed`.

### Step 8: Fix Bug 3 — `record_write_back_audit` "entrada" includes derived rows

In `write_back_cmds.rs:625–626`, change:

```sql
"UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
 WHERE date = ?3 AND type = 'income'"
```

to:

```sql
"UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
 WHERE date = ?3 AND type = 'income' AND id NOT LIKE 'derived:%'"
```

**Verify**: `cd src-tauri && cargo check 2>&1 | grep "^error" | head -5` → no output.

### Step 9: Add regression test for Bug 3

In `write_back_cmds.rs` `mod tests`, add `audit_entrada_skips_derived_rows`:

Setup:

- Insert a regular income row: `id='inc-1'`, `type='income'`, `date='2026-03-01'`, `source_amount=5000`.
- Insert a derived row: `id='derived:reimb-1'`, `type='income'`, `date='2026-03-01'`, `source_amount=1000`.
- Build a `CellWrite` with `kind="entrada"`, `date="2026-03-01"`, `value_cents=9900`.
- Call `record_write_back_audit(&p, "Sheet1", &[&cell]).await.unwrap()`.

Assertions:

- `source_amount FROM "transaction" WHERE id='inc-1'` → `9900` (realigned).
- `source_amount FROM "transaction" WHERE id='derived:reimb-1'` → `1000` (unchanged).

Use the `pool()` helper and `CellWrite` struct already in scope in this test module (see `write_back_cmds.rs:1173` for an existing `CellWrite` construction example).

**Verify**: `cd src-tauri && cargo test audit_entrada_skips_derived_rows 2>&1` → `test result: ok. 1 passed`.

### Step 10: Fix Bug 5 — `ytdPct` clamp in `TotaisScreen.tsx`

In `src/screens/TotaisScreen.tsx`, replace lines 228–231:

```ts
const a = forecast.annual_savings;
const ytdPct = Math.round(
  (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
);
```

with:

```ts
const a = forecast.annual_savings;
const ytdPctRaw = Math.round(
  (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
);
const ytdPct = Math.min(ytdPctRaw, 100);
const ytdPctLabel =
  ytdPctRaw > 100
    ? `no ano: >100% acumulado · meta 20–30% (média anual)`
    : `no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`;
```

Then at line 315, replace:

```tsx
sublabel={`no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`}
```

with:

```tsx
sublabel = { ytdPctLabel };
```

No new imports needed. `ytdPctRaw`, `ytdPct`, and `ytdPctLabel` are plain scalar computations — do not wrap in `useMemo` (React Compiler is ON).

**Verify**: `npm run typecheck` → exit 0.

### Step 11: (Optional) Bug 6 — `ensure_reserve_account` concurrency

Assess effort: open `write_back_cmds.rs:1008–1025` (`store_economia_entries`). The function calls `ensure_reserve_account(pool)` at line 1020 — outside the `tx` that starts at line 1025.

If the fix is straightforward (change `ensure_reserve_account` to accept `&mut sqlx::Transaction<'_, sqlx::Sqlite>` instead of `&SqlitePool`, update callers): implement it. The only caller is `store_economia_entries` at line 1020.

If the signature change cascades into more than ~20 lines of churn, skip and leave this note in `plans/README.md`.

**Verify** (if done): `cd src-tauri && cargo check 2>&1 | grep "^error" | head -5` → no output.

### Step 12: Full gate

Run the complete check suite.

**Verify**: `npm run check` → exit 0, all green.

### Step 13: Update plan index

Add a row for plan 053 in `plans/README.md` in the execution-order table with status `DONE` once all steps pass.

## Test plan

| Test name                                              | File                             | What it covers                                                      |
| ------------------------------------------------------ | -------------------------------- | ------------------------------------------------------------------- |
| `daily_spend_today_sums_magnitudes_not_signed_amounts` | `forecast_cmds.rs` `mod tests`   | Bug 1 regression: mixed import (−) + manual (+) → sum of magnitudes |
| `update_transaction_cmd_rejects_zero_amount`           | `transactions.rs` `mod tests`    | Bug 2 regression: `amount=0` → Err, row unchanged                   |
| `update_transaction_cmd_rejects_negative_amount`       | `transactions.rs` `mod tests`    | Bug 2 regression: `amount<0` → Err, row unchanged                   |
| `update_transaction_cmd_clears_items_on_type_change`   | `transactions.rs` `mod tests`    | Bug 4 regression: type change with same amount → line_items cleared |
| `audit_entrada_skips_derived_rows`                     | `write_back_cmds.rs` `mod tests` | Bug 3 regression: `derived:%` rows not clobbered by entrada realign |

Structural patterns:

- Rust tests: model after existing `mod tests` in each file — `pool()` → `sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap()` + `sqlx::migrate!("./migrations").run(&p).await.unwrap()`.
- The `update_transaction_cmd_clears_items_on_type_change` test mirrors `update_transaction_cmd_clears_items_and_updates_amount_atomically` at `transactions.rs:895` but triggers the type-change branch instead of the amount-change branch.
- TS: no new unit tests needed for Bug 5 (pure scalar arithmetic with `Math.min`); the existing Playwright visual smoke (`npm run e2e`) covers the rendered sublabel.

**Verify all tests**: `cd src-tauri && cargo test 2>&1 | tail -5` → `test result: ok. N passed; 0 failed`.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `cd src-tauri && cargo test 2>&1 | grep -E "FAILED|^error"` returns no output
- [ ] `cd src-tauri && cargo test daily_spend_today_sums_magnitudes` → 1 passed
- [ ] `cd src-tauri && cargo test update_transaction_cmd_rejects` → 2 passed
- [ ] `cd src-tauri && cargo test update_transaction_cmd_clears_items_on_type_change` → 1 passed
- [ ] `cd src-tauri && cargo test audit_entrada_skips_derived_rows` → 1 passed
- [ ] `grep -n "ABS(COALESCE((SELECT SUM(amount)" src-tauri/src/commands/forecast_cmds.rs` → no matches (old pattern gone)
- [ ] `grep -n "WHERE date = .* AND type = 'income'" src-tauri/src/commands/write_back_cmds.rs` → any remaining match includes `AND id NOT LIKE 'derived:%'`
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `npm run check` exits 0
- [ ] `plans/README.md` status row for plan 053 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the excerpts (e.g., the daily-spend query at `forecast_cmds.rs:1177` no longer looks like `ABS(COALESCE((SELECT SUM(amount)…`). The codebase has drifted since this plan was written.
- Any `cargo test` step fails twice after a reasonable fix attempt.
- Exposing `update_transaction_inner` as `pub(crate)` for testability requires touching more than `transactions.rs` (e.g., the Tauri command registration in `lib.rs` must change).
- The `ensure_reserve_account` refactor (Bug 6, optional) cascades into files outside `write_back_cmds.rs`.
- `npm run check` does not go green after all fixes are applied and tests pass individually.

## Maintenance notes

- Bug 1 fix brings `daily_spend_today` into the same sign-safe family as `month_grid` and `realized_monthly_baseline` (both already using `SUM(ABS(amount))`). If any new expense aggregation query is added to `forecast_cmds.rs`, apply the same pattern.
- Bug 2 guard: if `update_transaction_cmd` is later extended to accept `transfer` type, the guard must remain (transfers also use positive magnitudes).
- Bug 3 exclusion: any future `record_write_back_audit` arm that touches `type='income'` must include `AND id NOT LIKE 'derived:%'`. Consider extracting a shared SQL fragment or a Rust constant `EXCLUDE_DERIVED` for clarity.
- Bug 4 fix: the `current` query now returns a 3-tuple. If the query is later extended (e.g., to also fetch `payment_method`), keep the GROUP BY aligned with the selected non-aggregate columns.
- Bug 5 (ytdPct): the ">100%" case is intentionally preserved as a visible signal rather than silently capped. If in a future iteration the method defines a canonical cap, update the label copy accordingly.
- Bug 6 (optional): if deferred, track it as a follow-up in `plans/README.md`. The risk is low in practice (Tauri's async executor serialises commands on the main thread by default), but a concurrent background import could theoretically trigger it.
- Reviewer: scrutinise the `GROUP BY t.amount, t.type` in the updated `current` query (Step 6). SQLite's `COUNT(li.id)` aggregates correctly in this grouping because `t.id` is constant per `WHERE t.id = ?1`. Confirm the `GROUP BY` change doesn't affect the fetch semantics (it shouldn't, since we're fetching at most one row).

```

```
