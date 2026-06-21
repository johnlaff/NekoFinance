# Plan 049: P1: update_transaction_cmd atomicity + realized_monthly_baseline ABS sign

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 2132297..HEAD -- src-tauri/src/commands/transactions.rs src-tauri/src/commands/forecast_cmds.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `2132297`, 2026-06-21

## Why this matters

Two data-integrity bugs were identified in the post-Package-F sign-off.

**Bug 1 — non-atomic update**: `update_transaction_cmd` clears `line_item` rows and then updates the parent `transaction` total as two separate auto-commit statements against the raw pool. A crash between them leaves the database in an inconsistent state: the items are gone but the parent total still reflects the old itemised sum, or the new total is written against no items at all. All other multi-table writes in this codebase (delete, budget upsert with categories) already use `pool.begin()` … `tx.commit()`. This function is the lone outlier.

**Bug 2 — mixed-sign baseline**: `realized_monthly_baseline` computes the median monthly spend using `SUM(amount)` over `type = 'expense'` rows. Imported expenses are stored as negative amounts (`-amount_out`, see `import.rs:1158`), while manually-entered expenses are stored as positive amounts. When both exist in the same month the raw sum partially cancels itself, producing a median that is wrong in magnitude and potentially negative. This baseline feeds `reserve_floor` (= baseline × 6 months), which in turn limits `safe_to_spend_today`. A corrupted baseline therefore distorts the primary guardrail shown on the dashboard. The identical bug was fixed for `month_grid` in plan 041 by switching to `ABS`.

## Current state

### Relevant files

- `src-tauri/src/commands/transactions.rs` — Tauri command layer for transactions; contains `update_transaction_cmd` (lines 582–656) and the `#[cfg(test)]` block (from line 707).
- `src-tauri/src/commands/forecast_cmds.rs` — forecast/metrics command layer; contains `realized_monthly_baseline` (lines 227–254), `reserve_floor` (lines 529–541), and the `#[cfg(test)]` block (from line 1238).
- `src-tauri/src/google_sheets/import.rs` — sheet importer; stores outflows as `-amount_out` (line 1158).

### Bug 1 — excerpt from `transactions.rs`

The conditional DELETE (line 626) and the UPDATE (line 633) each call `.execute(pool.inner())`, which auto-commits immediately. There is no enclosing `sqlx::Transaction`.

```rust
// transactions.rs:622–655
    if let Some((old_amount, item_count)) = current
        && item_count > 0
        && old_amount != amount_cents
    {
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind(&id)
            .execute(pool.inner())          // ← auto-commit #1
            .await
            .map_err(|e| format!("update (clear stale items): {e}"))?;
    }

    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
           WHERE id = ?1"#,
    )
    ...
    .execute(pool.inner())                  // ← auto-commit #2 (separate)
```

The correct pattern used everywhere else (e.g. `delete_transaction_cmd`, lines 533–575):

```rust
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|e| format!("delete (begin): {e}"))?;

    sqlx::query(...)
        .execute(&mut *tx)   // ← bound to transaction
        ...

    tx.commit()
        .await
        .map_err(|e| format!("delete (commit): {e}"))?;
```

### Bug 2 — excerpt from `forecast_cmds.rs`

`realized_monthly_baseline` at lines 233–241:

```rust
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT SUM(amount) FROM \"transaction\" \
         WHERE type='expense' AND date < ?1 \
         GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6",
    )
    .bind(format!("{cur_ym}-01"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("baseline: {e}"))?;
```

`SUM(amount)` is correct only for all-positive rows. Imported expenses are stored negative (`import.rs:1158`: `amount: -amount_out`). The fix is `SUM(ABS(amount))`, matching the treatment in `month_grid` (lines 1057–1060):

```rust
        // month_grid (forecast_cmds.rs:1059):
        COALESCE(SUM(CASE WHEN type='expense' AND ... THEN ABS(amount) ELSE 0 END), 0)
```

### Convention: functional-core / imperative-shell

Domain logic lives in pure functions injected with `today_naive: NaiveDate` (deterministic, testable without Tauri `State`). The `#[tauri::command]` wrappers are thin adapters that call the inner functions. Tests call the inner functions directly against an in-memory SQLite pool created by `sqlx::migrate!("./migrations")`. See `test_pool()` / `pool()` helpers in the existing `#[cfg(test)]` blocks.

### Convention: React Compiler is ON (frontend only)

Not relevant to this plan — all changes are Rust.

## Commands you will need

| Purpose         | Command                                                 | Expected on success        |
| --------------- | ------------------------------------------------------- | -------------------------- |
| Rust check      | `npm run rust:check`                                    | exit 0, no warnings        |
| Unit tests      | `npm run test:run`                                      | all pass (including 2 new) |
| Full gate       | `npm run check`                                         | exit 0                     |
| Cargo test only | `cd src-tauri && cargo test 2>&1`                       | all pass                   |
| Filter one test | `cd src-tauri && cargo test <test_name> -- --nocapture` | named test passes          |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/transactions.rs` — wrap DELETE + UPDATE in `update_transaction_cmd` in a single `sqlx::Transaction`; add one regression test.
- `src-tauri/src/commands/forecast_cmds.rs` — change `SUM(amount)` to `SUM(ABS(amount))` in `realized_monthly_baseline`; add one regression test.

**Out of scope** (do NOT touch):

- `src-tauri/src/google_sheets/import.rs` — the negative sign on imported expenses is intentional and correct; do not change storage convention.
- `src-tauri/src/forecast/mod.rs` — `classify()` and all forecast engine code; the sign convention is handled at the query level.
- Any frontend file — no UI change required.
- `src-tauri/src/commands/forecast_cmds.rs` functions other than `realized_monthly_baseline` — `reserve_floor`, `projected_annual_savings`, `realized_annual_savings` already use `CASE WHEN type='income' THEN amount ELSE 0 END` patterns that exclude expenses from their SUM.
- `month_grid` — already fixed in plan 041 with `ABS`; do not re-touch.

## Git workflow

- Branch: `advisor/049-update-atomicity-baseline-sign`
- One commit per bug fix, or a single commit covering both; use conventional commit style matching the repo (`fix: …` prefix). Example from `git log`: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Wrap `update_transaction_cmd` DELETE + UPDATE in one `sqlx::Transaction`

Open `src-tauri/src/commands/transactions.rs`. The entire body of `update_transaction_cmd` (lines 593–655) runs against `pool.inner()` directly. Restructure it as follows:

1. After the `is_projection` derivation (after line 605), open a transaction:

   ```rust
   let mut tx = pool
       .inner()
       .begin()
       .await
       .map_err(|e| format!("update (begin): {e}"))?;
   ```

2. Replace the `current` query's `.fetch_optional(pool.inner())` with `.fetch_optional(&mut *tx)`.

3. Replace the conditional DELETE's `.execute(pool.inner())` with `.execute(&mut *tx)`.

4. Replace the UPDATE's `.execute(pool.inner())` with `.execute(&mut *tx)`.

5. After the `if affected == 0` early-return guard (line 654), add:

   ```rust
   tx.commit()
       .await
       .map_err(|e| format!("update (commit): {e}"))?;
   ```

6. Remove the lone `Ok(())` at line 655 — the commit line above is the new success return path; add `Ok(())` after the `tx.commit()` call.

The resulting shape mirrors `delete_transaction_cmd` (lines 533–576). The `current` SELECT (read for the conditional), the conditional DELETE, and the UPDATE all run inside the same transaction. The early-return on `affected == 0` causes an implicit rollback (the `tx` is dropped without commit), which is correct — same pattern as `delete_transaction_cmd`.

**Verify**: `cd src-tauri && cargo check 2>&1 | grep -c "^error"` → `0`

### Step 2: Add regression test for `update_transaction_cmd` atomicity

In the `#[cfg(test)]` block of `transactions.rs` (from line 707), add a new test after the existing `update_transaction_items_cmd_*` tests. The test must verify that after a successful `update_transaction_cmd` call that triggers the item-clear path (value changed, items existed), the `line_item` rows are gone AND the parent `transaction.amount` reflects the new value — i.e., both sides of the formerly non-atomic write are consistent.

Use the existing `test_pool()` and `insert_txn()` helpers. There is no public inner function for `update_transaction_cmd` (it takes `State<'_, SqlitePool>`), so replicate the relevant logic directly in the test — the same approach used by `run_update_items` in the existing test block (lines 741–796), which duplicates the command body to make it testable.

Test outline:

```rust
#[tokio::test]
async fn update_transaction_cmd_clears_items_and_updates_amount_atomically() {
    let pool = test_pool().await;
    // Insert a parent transaction with amount 1000.
    insert_txn(&pool, "tx-upd", 1000).await;
    // Insert two line_item rows for it.
    sqlx::query(
        "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
         VALUES ('li-a', 'tx-upd', 600, 'Part A', 0, 1), \
                ('li-b', 'tx-upd', 400, 'Part B', 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Execute the same logic as update_transaction_cmd with a new amount (2000 ≠ 1000).
    // Open a transaction, clear stale items, update parent — same sequence as the fixed command.
    let new_amount: i64 = 2000;
    let mut tx = pool.begin().await.unwrap();
    let current: Option<(i64, i64)> = sqlx::query_as(
        r#"SELECT t.amount, COUNT(li.id)
           FROM "transaction" t
           LEFT JOIN line_item li ON li.transaction_id = t.id
           WHERE t.id = ?1
           GROUP BY t.amount"#,
    )
    .bind("tx-upd")
    .fetch_optional(&mut *tx)
    .await
    .unwrap();
    if let Some((old_amount, item_count)) = current
        && item_count > 0
        && old_amount != new_amount
    {
        sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
            .bind("tx-upd")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    sqlx::query(
        r#"UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1"#,
    )
    .bind("tx-upd")
    .bind(new_amount)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Both sides must be consistent after commit.
    let (amount,): (i64,) =
        sqlx::query_as(r#"SELECT amount FROM "transaction" WHERE id = 'tx-upd'"#)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(amount, 2000, "parent amount updated to new value");

    let item_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-upd'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        item_count.0, 0,
        "stale line_items cleared when amount changed"
    );
}
```

**Verify**: `cd src-tauri && cargo test update_transaction_cmd_clears_items -- --nocapture 2>&1 | tail -5` → `test ... ok`

### Step 3: Fix `realized_monthly_baseline` to use `SUM(ABS(amount))`

Open `src-tauri/src/commands/forecast_cmds.rs`. At lines 233–241, change the SQL from:

```sql
SELECT SUM(amount) FROM "transaction"
WHERE type='expense' AND date < ?1
GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6
```

to:

```sql
SELECT SUM(ABS(amount)) FROM "transaction"
WHERE type='expense' AND date < ?1
GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6
```

Only the `SUM(amount)` → `SUM(ABS(amount))` token changes. No other line in this function changes.

**Verify**: `cd src-tauri && cargo check 2>&1 | grep -c "^error"` → `0`

### Step 4: Add regression test for `realized_monthly_baseline` mixed-sign

In the `#[cfg(test)]` block of `forecast_cmds.rs` (from line 1238), add a new test after the existing `month_grid_expense_total_is_magnitude_regardless_of_sign` test (ends around line 1416). Use the existing `pool()` and `insert_expense()` helpers.

Test outline:

```rust
#[tokio::test]
async fn realized_monthly_baseline_sums_magnitudes_not_signed_amounts() {
    // One imported expense (stored negative) + one manual expense (stored positive)
    // in the same past month. The baseline must equal the sum of magnitudes, not
    // the algebraic sum (which would partially cancel and produce a wrong result).
    let p = pool().await;
    // today is in a later month so the test month is "complete" (before cur_ym-01).
    let today = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

    // Imported expense: negative amount, simulating -amount_out from import.rs:1158.
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
         VALUES ('imp-bl', 'expense', -90000, '2026-04-10', 0)",
    )
    .execute(&p)
    .await
    .unwrap();

    // Manual expense: positive amount.
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
         VALUES ('man-bl', 'expense', 60000, '2026-04-20', 0)",
    )
    .execute(&p)
    .await
    .unwrap();

    // Only one month in the window → median = that month's total.
    // Correct: ABS(-90000) + 60000 = 150_000.
    // Wrong (before fix): -90000 + 60000 = -30_000.
    let baseline = realized_monthly_baseline(&p, today).await.unwrap();
    assert_eq!(
        baseline, 150_000,
        "realized_monthly_baseline must sum magnitudes (ABS), not signed amounts"
    );

    // reserve_floor = baseline × RESERVE_MIN_MONTHS (6) = 900_000.
    // Verify the floor is positive and coherent (not negative from the buggy baseline).
    let floor = reserve_floor(&p, today).await.unwrap();
    assert!(
        floor >= 150_000 * RESERVE_MIN_MONTHS,
        "reserve_floor must be at least baseline × RESERVE_MIN_MONTHS"
    );
}
```

**Verify**: `cd src-tauri && cargo test realized_monthly_baseline_sums_magnitudes -- --nocapture 2>&1 | tail -5` → `test ... ok`

### Step 5: Run full test suite and quality gate

**Verify**:

1. `npm run rust:check` → exit 0
2. `npm run test:run` → all tests pass, including the 2 new ones
3. `npm run check` → exit 0

## Test plan

Two new regression tests, one per bug:

| Test name                                                           | File                                                       | Cases covered                                                                                                             |
| ------------------------------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `update_transaction_cmd_clears_items_and_updates_amount_atomically` | `src-tauri/src/commands/transactions.rs` (in `mod tests`)  | Items present + amount changed → DELETE and UPDATE both committed; both sides consistent                                  |
| `realized_monthly_baseline_sums_magnitudes_not_signed_amounts`      | `src-tauri/src/commands/forecast_cmds.rs` (in `mod tests`) | Imported (negative) + manual (positive) expense in same month → baseline = sum of magnitudes; `reserve_floor` is positive |

Structural model:

- `update_transaction_cmd_clears_items_and_updates_amount_atomically` — modelled after the existing `run_update_items` inline helper (lines 741–796 in `transactions.rs`), which replicates command-body logic in a test-local function.
- `realized_monthly_baseline_sums_magnitudes_not_signed_amounts` — modelled after `month_grid_expense_total_is_magnitude_regardless_of_sign` (lines 1382–1416 in `forecast_cmds.rs`), which tests the same sign-family bug in `month_grid`.

Run with: `cd src-tauri && cargo test 2>&1` → all pass, 2 new tests present.

## Done criteria

- [ ] `npm run rust:check` exits 0, no new warnings
- [ ] `cd src-tauri && cargo test 2>&1` exits 0; output contains `update_transaction_cmd_clears_items_and_updates_amount_atomically ... ok` and `realized_monthly_baseline_sums_magnitudes_not_signed_amounts ... ok`
- [ ] `grep -n "\.execute(pool\.inner())" src-tauri/src/commands/transactions.rs` no longer appears inside the `update_transaction_cmd` function body (lines 584–660)
- [ ] `grep -n "SUM(amount)" src-tauri/src/commands/forecast_cmds.rs` returns no match in `realized_monthly_baseline` (line ~234)
- [ ] `git diff --name-only` shows only the two in-scope files (plus `plans/README.md` for the status update)
- [ ] `plans/README.md` status row for plan 049 updated from `TODO` to `DONE`

## STOP conditions

Stop and report back (do not improvise) if:

- The code at lines 622–655 of `transactions.rs` does not match the excerpts above — the function may have been refactored since this plan was written.
- The code at lines 233–241 of `forecast_cmds.rs` does not show `SUM(amount)` — the bug may have already been fixed, or the function was restructured.
- Wrapping in a transaction causes a `cargo check` error because `pool` is `State<'_, SqlitePool>` (a `Deref` wrapper) — you may need `pool.inner().begin()` rather than `pool.begin()`. Confirm against the `delete_transaction_cmd` pattern at line 533 which uses `pool.inner().begin()`.
- Either regression test fails after a fix attempt — the database schema or helper may differ from what this plan assumes.
- The fix appears to require touching `import.rs` or `forecast/mod.rs`.

## Maintenance notes

- The `update_transaction_cmd` function takes `State<'_, SqlitePool>` (Tauri wrapper), not a bare `&SqlitePool`. The pattern is always `pool.inner().begin()`, not `pool.begin()`. All existing multi-table commands follow this — verify against `delete_transaction_cmd` at line 533.
- After this fix, `update_transaction_cmd` and `update_transaction_items_cmd` are both atomic. If a future plan adds more columns to the `transaction` table update, or adds additional cascaded deletes, keep them inside the same `tx` scope.
- The `realized_monthly_baseline` fix narrows to exactly `SUM(ABS(amount))`. The `effective_daily_ceiling` function (lines 287–297) already uses `ABS(COALESCE(SUM(amount), 0))` for the same reason; this plan brings `realized_monthly_baseline` into alignment.
- If a future plan introduces a sign normalisation at insert time (storing all amounts as positive magnitudes), the `ABS()` wrapping becomes a no-op rather than a fix — review both sites then.
- The `reserve_floor` test (second test, assertion on `floor >= baseline * 6`) is a loose bound. A tighter bound would require seeding an `account` with `liquidity = 'reserve'`; the loose bound is sufficient to confirm the floor is not corrupted by a negative baseline.
