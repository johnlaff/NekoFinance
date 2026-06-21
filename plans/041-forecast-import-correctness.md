# Plan 041: Forecast/import correctness: is_projection (today + edit + checksum) + month_grid sign

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
>
> ```
> git diff --stat d3922d2..HEAD -- \
>   src-tauri/src/google_sheets/import.rs \
>   src-tauri/src/commands/transactions.rs \
>   src-tauri/src/commands/forecast_cmds.rs
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

Four bugs—three P1, one P2—cause the forecast/import pipeline to silently misclassify
rows and produce wrong totals. Bug 1: every row dated _today_ is classified as a
projection (future) instead of realized, so today's spending appears in the "forecast"
pane rather than the "realized" pane. Bug 2: the dedup checksum includes the derived
`is_projection` field, which depends on the wall-clock date of import, so re-opening
the app the next day computes a different checksum for an _unchanged_ sheet and
triggers a full spurious re-import. Bug 3: editing a future transaction's date to today
leaves `is_projection = 1` in the database—a ghost "projected" entry that is actually
realized—because `update_transaction_cmd` never recomputes `is_projection`. Bug 4:
`month_grid` sums the `amount` column for expenses without `ABS()`; imported expenses
are stored as negative magnitudes (the importer writes `-amount_out`) while manual
entries are positive, so a month containing both shows a wrong (under-counted) Saída
total. Together these bugs corrupt daily-ledger totals, dedup logic, and the forecast.

## Current state

### Relevant files

- `src-tauri/src/google_sheets/import.rs` — sheet parser, `classify_row`, checksum,
  `ImportedRow`, `compute_checksum_with_options` (bugs 1 and 2).
- `src-tauri/src/commands/transactions.rs` — `update_transaction_cmd` (bug 3),
  `create_transaction_inner` (reference logic for `is_projection`).
- `src-tauri/src/commands/forecast_cmds.rs` — `month_grid` (bug 4),
  `dashboard_summary` (exemplar that already uses `ABS()`).

### Bug 1 — `classify_row` misclassifies today (`import.rs` lines 95–105)

```rust
// import.rs:95-105
pub fn classify_row(date_str: &str, date_direction: &str) -> Result<bool, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_past = date_str < today.as_str();   // BUG: strict-less; today is NOT past

    match date_direction {
        "past_only" => Ok(false),
        "future_only" => Ok(true),
        "both" => Ok(!is_past),                // "both": today → is_past=false → is_projection=true ❌
        _ => Err(format!("unknown date_direction: {date_direction}")),
    }
}
```

The fix is `date_str <= today.as_str()` (or equivalently `date_str < today.as_str()` →
renamed to `date_str > today.as_str()` for `is_projection`). Today is _realized_, not
projected. The `date_direction = "past_only"` branch always returns `false` so it is
not affected; only the `"both"` branch is wrong.

The existing test (`test_classify_row_past` at line 1477) passes "2020-01-15" and
"2099-12-31" — it does not cover _today_, so this bug was not caught.

### Bug 2 — checksum includes `is_projection` (`import.rs` lines 116–132)

```rust
// import.rs:116-132
fn compute_checksum_with_options(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.date.as_bytes());
        hasher.update(row.amount.to_le_bytes());
        if descriptions_trusted {
            hasher.update(row.description.as_bytes());
        }
        hasher.update([row.is_projection as u8]);  // BUG: derived field in checksum
        hasher.update(row.kind.as_str().as_bytes());
        hasher.update(row.raw_note.as_bytes());
    }
    hex::encode(hasher.finalize())
}
```

`is_projection` is a derived field computed by `classify_row` from the _current date_
at import time. The underlying sheet data (date, amount, description, kind, note) has
not changed, but the checksum does, so `check_duplicate_import` misses the duplicate
and triggers a full re-import on every calendar day. Fix: remove
`hasher.update([row.is_projection as u8])` from the hash. The remaining fields (date,
amount, description, kind, raw_note) are all source-data fields that genuinely reflect
sheet changes.

The existing `test_compute_checksum` test (line 1699) does not pin `is_projection`
across dates, so the bug was not caught.

### Bug 3 — `update_transaction_cmd` does not recompute `is_projection` (`transactions.rs` lines 476–517)

```rust
// transactions.rs:492-509
let affected = sqlx::query(
    r#"UPDATE "transaction"
       SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
           is_fixed = ?6, date = ?7, updated_at = ?8
       WHERE id = ?1 AND source_amount IS NULL"#,
)
// ...
// No is_projection recompute — if date changes from future to today/past,
// is_projection remains 1 (ghost "Previsto" in the forecast). ❌
```

Reference (correct) logic from `create_transaction_inner` (line 329):

```rust
// transactions.rs:329
let is_projection = start > chrono::Local::now().date_naive();
```

The fix: add `is_projection = ?9` to the UPDATE SET clause and bind the recomputed
value. The recompute mirrors `create_transaction_inner`: `new_date > Local::now().date_naive()`.

### Bug 4 — `month_grid` sums Saída without `ABS()` (`forecast_cmds.rs` lines 887–900)

```rust
// forecast_cmds.rs:888-900
let flows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
    "SELECT date, \
            COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN amount ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN amount ELSE 0 END), 0) \
     FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 GROUP BY date",
)
```

Imported expenses are stored with a negative sign (`-amount_out` at import.rs line
1138–1139); manual expenses are positive (`create_transaction_inner` requires
`amount_cents > 0`). Both are `type = 'expense'`. Summing them raw gives a mixed-sign
total. Compare with `dashboard_summary` at line 1007, which correctly uses `ABS()`:

```rust
// forecast_cmds.rs:1007-1011
"SELECT ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                      WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                        AND (payment_method IS NULL OR payment_method <> 'credit')), 0))"
```

Fix: wrap the expense CASE expressions in `ABS()` so both imported (negative) and
manual (positive) expenses contribute their magnitude.

### Repo conventions

- **Functional-core / imperative-shell**: pure logic in core functions, I/O at the
  adapter boundary. `classify_row` is already pure (no I/O); keep it that way.
- **Money = positive-magnitude integer cents**: `transaction.amount` is magnitude;
  direction comes from `type`. `ABS()` in queries is defense-in-depth (importer bug
  aside, it documents the intent).
- **React Compiler ON / no manual memo**: not relevant here (Rust only).
- **Method-neutral language**: no references to proprietary app/course names in code
  comments or docs. Use "method" or "spreadsheet" as generic terms.
- **Existing test pattern**: unit tests for pure functions live in
  `#[cfg(test)] mod tests` at the bottom of the same file (see `import.rs:1473`,
  `transactions.rs:519`, `forecast_cmds.rs:1071`). Integration tests that need a
  pool use the `pool()` / `test_pool()` helper that creates an in-memory SQLite and
  runs migrations (`sqlx::migrate!("./migrations").run(&pool)`).

## Commands you will need

| Purpose         | Command                                  | Expected on success |
| --------------- | ---------------------------------------- | ------------------- |
| Rust check      | `npm run rust:check`                     | exit 0, no errors   |
| Unit tests      | `npm run test:run`                       | all pass            |
| Full gate       | `npm run check`                          | exit 0              |
| Rust tests only | `cd src-tauri && cargo test 2>&1`        | all pass            |
| Filter test     | `cd src-tauri && cargo test <test_name>` | named test passes   |

Run commands from `/home/john/dev/neko-finance` (repo root), except where noted.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/google_sheets/import.rs` — bugs 1 and 2, plus their regression tests
- `src-tauri/src/commands/transactions.rs` — bug 3, plus its regression test
- `src-tauri/src/commands/forecast_cmds.rs` — bug 4, plus its regression test

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/forecast/mod.rs` — pure engine; this plan does not change projection
  logic there.
- `src/` (React frontend) — no frontend changes required; `is_projection` and
  `month_grid` are backend-only in this plan.
- Plan 040 (Performance formula) — a separate plan; do not conflate.
- Any migration files — no schema changes required; `is_projection` already exists in
  `transaction`.
- `src-tauri/src/commands/forecast_cmds.rs` functions other than `month_grid` — the
  `dashboard_summary` `ABS()` is already correct; do not touch it.

## Git workflow

- Branch: `advisor/041-forecast-import-correctness`
- One commit per logical step (or one commit for all four fixes if the reviewer prefers
  a single atomic unit). Match the observed style: `fix: <short description>`.
  Example from `git log`: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD`.
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Fix Bug 1 — `classify_row` strict-less comparison

In `src-tauri/src/google_sheets/import.rs`, function `classify_row` (lines 95–105),
change the comparison on line 97 from:

```rust
let is_past = date_str < today.as_str();
```

to:

```rust
let is_past = date_str <= today.as_str();
```

This makes today _realized_ (`is_past = true` when `date_str == today`) and thus
`is_projection = false` for the `"both"` direction.

No other callers of `classify_row` exist in the codebase that would be broken: the
`"past_only"` branch always returns `Ok(false)` regardless of `is_past`, and
`"future_only"` always returns `Ok(true)`.

**Verify**: `cd src-tauri && cargo test test_classify_row` → existing tests still pass;
no compilation errors.

### Step 2: Add regression test for Bug 1 (today classified as realized)

In the `#[cfg(test)] mod tests` block at the bottom of
`src-tauri/src/google_sheets/import.rs` (after line 1491), add:

```rust
#[test]
fn classify_row_today_is_realized() {
    // A row dated today must be realized (is_projection = false), not projected.
    // Bug 1: the old `<` comparison made today a projection.
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(
        !classify_row(&today, "both").unwrap(),
        "today must be realized (is_projection=false) in 'both' mode"
    );
    // "past_only" and "future_only" are direction overrides; unchanged by this fix.
    assert!(!classify_row(&today, "past_only").unwrap());
    assert!(classify_row(&today, "future_only").unwrap());
}
```

**Verify**: `cd src-tauri && cargo test classify_row_today_is_realized` → 1 test passes.

### Step 3: Fix Bug 2 — remove `is_projection` from checksum

In `src-tauri/src/google_sheets/import.rs`, function `compute_checksum_with_options`
(lines 116–132), remove the line:

```rust
        hasher.update([row.is_projection as u8]);
```

After the fix the function body hashes only: `date`, `amount`
(when `descriptions_trusted`), `description`, `kind`, `raw_note` — all source-data
fields. The comment block above `parse_note_markers` already documents that
`raw_note` enters the checksum so that edits to cell notes trigger re-import; that
comment is unaffected.

IMPORTANT: the public wrapper `compute_checksum` (line 112) calls
`compute_checksum_with_options(rows, true)` and is used by the existing
`test_compute_checksum` test. After this change, `test_compute_checksum` still passes
(the two calls use identical rows with `is_projection: false`, so the checksum is
still deterministic and the two results are still equal).

**Verify**: `cd src-tauri && cargo test test_compute_checksum` → still passes.

### Step 4: Add regression test for Bug 2 (checksum stable across dates)

In the `#[cfg(test)] mod tests` block of `import.rs`, add after the fix in step 3:

```rust
#[test]
fn checksum_excludes_is_projection_field() {
    // Bug 2: is_projection is date-relative (computed from today), so including it
    // in the checksum caused the same unchanged sheet to produce a different checksum
    // on a different calendar day → daily spurious full re-import.
    // Fix: is_projection must NOT affect the checksum.
    let row_as_future = ImportedRow {
        date: "2099-01-15".into(),
        amount: 50000,
        description: "Gasto fixo".into(),
        is_projection: true,   // "future" classification
        kind: RowKind::Saida,
        raw_note: String::new(),
    };
    let row_as_past = ImportedRow {
        date: "2099-01-15".into(),
        amount: 50000,
        description: "Gasto fixo".into(),
        is_projection: false,  // same source data, different derived classification
        kind: RowKind::Saida,
        raw_note: String::new(),
    };
    // Same source data → same checksum regardless of is_projection.
    assert_eq!(
        compute_checksum(&[row_as_future]),
        compute_checksum(&[row_as_past]),
        "checksum must not depend on is_projection (derived field)"
    );
}
```

**Verify**: `cd src-tauri && cargo test checksum_excludes_is_projection_field` →
1 test passes.

### Step 5: Fix Bug 3 — recompute `is_projection` on date edit

In `src-tauri/src/commands/transactions.rs`, function `update_transaction_cmd` (lines
476–517), update the SQL UPDATE and its bindings to include `is_projection`.

Current SQL (lines 492–497):

```rust
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, updated_at = ?8
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
```

Replace with:

```rust
    let new_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| format!("data inválida: {e}"))?;
    let is_projection = new_date > chrono::Local::now().date_naive();
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
```

Then renumber the bindings: `.bind(&now)` moves from `?8` to `?9`, and
`.bind(is_projection as i64)` is added as `?8`. The final binding block should be:

```rust
    .bind(&id)           // ?1
    .bind(&txn_type)     // ?2
    .bind(amount_cents)  // ?3
    .bind(&description)  // ?4
    .bind(&payment_method) // ?5
    .bind(is_fixed as i64) // ?6
    .bind(&date)           // ?7
    .bind(is_projection as i64) // ?8  ← new
    .bind(&now)            // ?9  ← was ?8
```

`NaiveDate` is already imported via `use chrono::NaiveDate` at the top of the file
(used in `create_transaction_inner` on line 298). Confirm with a quick grep before
editing: `grep -n 'NaiveDate' src-tauri/src/commands/transactions.rs`.

The `source_amount IS NULL` guard already ensures only manual (non-imported) entries
are editable, so this recompute does not affect imported rows.

**Verify**: `cd src-tauri && cargo test` → all existing tests pass, no compilation
errors.

### Step 6: Add regression test for Bug 3 (date edit recomputes is_projection)

In the `#[cfg(test)] mod tests` block of `transactions.rs` (after line 519), add an
integration test. Model the test after `update_transaction_items_cmd_sets_total` (line
610): use the `test_pool()` helper, insert a transaction, call the inner logic, then
assert the DB state.

Because `update_transaction_cmd` takes `State<'_, SqlitePool>` (not directly testable),
write a local helper that replicates the updated logic inline (matching the pattern of
`run_update_items` at line 555):

```rust
async fn run_update_txn_date(
    pool: &SqlitePool,
    id: &str,
    new_date: &str,
) -> Result<(), String> {
    use chrono::NaiveDate;
    let new_date_parsed = NaiveDate::parse_from_str(new_date, "%Y-%m-%d")
        .map_err(|e| format!("data inválida: {e}"))?;
    let is_projection = new_date_parsed > chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
    .bind(id)
    .bind("expense")
    .bind(1000_i64)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(0_i64)
    .bind(new_date)
    .bind(is_projection as i64)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("update: {e}"))?
    .rows_affected();
    if affected == 0 {
        return Err("not found".into());
    }
    Ok(())
}

#[tokio::test]
async fn update_transaction_date_to_today_clears_is_projection() {
    let pool = test_pool().await;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let future = "2099-12-31";

    // Insert a future transaction (is_projection = 1).
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
         VALUES ('tx-upd', 'expense', 1000, ?1, 0, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(future)
    .execute(&pool)
    .await
    .unwrap();

    // Edit the date to today.
    run_update_txn_date(&pool, "tx-upd", &today)
        .await
        .unwrap();

    let (is_projection,): (i64,) =
        sqlx::query_as("SELECT is_projection FROM \"transaction\" WHERE id = 'tx-upd'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        is_projection, 0,
        "editing date to today must clear is_projection (Bug 3)"
    );
}
```

**Verify**: `cd src-tauri && cargo test update_transaction_date_to_today_clears_is_projection`
→ 1 test passes.

### Step 7: Fix Bug 4 — wrap Saída/Diário SUM in ABS() in `month_grid`

In `src-tauri/src/commands/forecast_cmds.rs`, function `month_grid` (lines 877–931),
update the SQL at lines 888–900. The current query is:

```rust
    let flows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT date, \
                COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 GROUP BY date",
    )
```

Replace with (wrapping the two expense CASE expressions in `ABS()`):

```rust
    let flows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT date, \
                COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
                COALESCE(ABS(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN amount ELSE 0 END)), 0), \
                COALESCE(ABS(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN amount ELSE 0 END)), 0) \
         FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 GROUP BY date",
    )
```

Note: `ABS()` wraps the entire `SUM(...)`, not each `CASE` branch. This preserves the
COALESCE structure and ensures a zero sum stays zero.

Incomes (`amount > 0`, `type = 'income'`) are NOT wrapped — they are always positive
and the formula is correct for them already.

**Verify**: `cd src-tauri && cargo test` → all existing tests pass, no compilation
errors.

### Step 8: Add regression test for Bug 4 (month_grid mixed-sign Saída)

In the `#[cfg(test)] mod tests` block of `forecast_cmds.rs` (after line 1071), add:

```rust
#[tokio::test]
async fn month_grid_expense_total_is_magnitude_regardless_of_sign() {
    // Bug 4: imported expenses are stored negative (-amount_out); manual are positive.
    // month_grid must return the magnitude (ABS) so both sources add up correctly.
    let p = pool().await;

    // Simulate an imported expense (negative amount, is_fixed=1 = Saída).
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
         VALUES ('imp-exp', 'expense', -150000, '2026-03-15', 1, 0, '2026-03-15T00:00:00Z', '2026-03-15T00:00:00Z')",
    )
    .execute(&p)
    .await
    .unwrap();

    // Simulate a manual expense (positive amount, is_fixed=1 = Saída).
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
         VALUES ('man-exp', 'expense', 80000, '2026-03-15', 1, 0, '2026-03-15T00:00:00Z', '2026-03-15T00:00:00Z')",
    )
    .execute(&p)
    .await
    .unwrap();

    let grid = month_grid(&p, 2026, 3).await.unwrap();
    let day15 = grid.iter().find(|d| d.date == "2026-03-15").unwrap();

    // fixed_out must be the sum of magnitudes: 150_000 + 80_000 = 230_000.
    // Before fix: -150_000 + 80_000 = -70_000 (wrong sign, wrong value).
    assert_eq!(
        day15.fixed_out_cents, 230_000,
        "month_grid fixed_out must be magnitude regardless of storage sign (Bug 4)"
    );
    assert_eq!(day15.daily_out_cents, 0);
    assert_eq!(day15.income_cents, 0);
}
```

**Verify**: `cd src-tauri && cargo test month_grid_expense_total_is_magnitude_regardless_of_sign`
→ 1 test passes.

### Step 9: Final gate

Run the full quality gate from the repo root:

```
npm run rust:check
npm run test:run
```

If the project-level check command covers Rust too:

```
npm run check
```

**Verify**: exit 0, all tests pass including the 4 new regression tests.

## Test plan

Four new regression tests, one per bug, in the file where the bug lives:

| Test name                                                  | File                     | Covers                                    |
| ---------------------------------------------------------- | ------------------------ | ----------------------------------------- |
| `classify_row_today_is_realized`                           | `import.rs` tests        | Bug 1: today → `is_projection=false`      |
| `checksum_excludes_is_projection_field`                    | `import.rs` tests        | Bug 2: checksum same across dates         |
| `update_transaction_date_to_today_clears_is_projection`    | `transactions.rs` tests  | Bug 3: date edit recomputes is_projection |
| `month_grid_expense_total_is_magnitude_regardless_of_sign` | `forecast_cmds.rs` tests | Bug 4: ABS() on mixed-sign expenses       |

Structural pattern for pure unit tests: `import.rs:1477–1495` (`test_classify_row_past`,
`test_classify_row_future`).

Structural pattern for integration tests (pool + migrations): `forecast_cmds.rs:1121`
(`excluded_tag_expense_still_lowers_projected_balance`) and `transactions.rs:610`
(`update_transaction_items_cmd_sets_total`).

**Verification command**: `cd src-tauri && cargo test 2>&1 | tail -20` →
output should include the 4 new test names under `test result: ok`.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `cd src-tauri && cargo test` exits 0; exactly 4 new tests exist and pass
      (`classify_row_today_is_realized`, `checksum_excludes_is_projection_field`,
      `update_transaction_date_to_today_clears_is_projection`,
      `month_grid_expense_total_is_magnitude_regardless_of_sign`)
- [ ] `grep -n 'date_str < today' src-tauri/src/google_sheets/import.rs` returns no
      matches (old strict-less comparison is gone)
- [ ] `grep -n 'is_projection as u8' src-tauri/src/google_sheets/import.rs` returns
      no matches (`is_projection` no longer in checksum)
- [ ] `git diff --name-only` shows only the 3 in-scope Rust files (plus
      `plans/README.md` if updated); no other files modified
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the file:line locations in "Current state" does not match the excerpts
  (the codebase has drifted since this plan was written).
- `classify_row` has been refactored to take a `NaiveDate` rather than `&str` — the
  fix in Step 1 still applies logically, but the line numbers and surrounding code will
  differ; verify before editing.
- `update_transaction_cmd` already includes `is_projection` in its SET clause (someone
  already fixed Bug 3 independently).
- `NaiveDate` is NOT imported in `transactions.rs` (would mean the module was
  significantly restructured); grep first as instructed in Step 5.
- Step 9 gate fails after two fix attempts at the test.
- Any fix appears to require touching an out-of-scope file (migration, forecast engine,
  frontend).

## Maintenance notes

- **Checksum stability**: the `compute_checksum_with_options` function is called both
  from the async import path and from the sync `compute_import_checksum` helper used
  by the shell for pre-flight dedup checks. Both calls go through the same function
  after this fix — no divergence risk. If a new source field is added to `ImportedRow`
  in the future, add it to the checksum deliberately.
- **`update_transaction_cmd` scope**: the `source_amount IS NULL` guard means this
  update path only touches manual (app-created) transactions. Imported transactions are
  not editable via this path; their `is_projection` is controlled by the importer on
  re-import. If a future plan lifts this guard (allow editing imported amounts), it
  must also handle `is_projection` recompute there.
- **`month_grid` vs `forecast.daily`**: `month_grid` is the "ledger view" (all days,
  any month); `forecast.daily` starts from today and uses the projection engine. They
  use different code paths. If `forecast.daily` is found to have a similar mixed-sign
  bug, that is a separate fix (check `load_cashflow_events` and `map_cashflow_row`
  in `forecast_cmds.rs`).
- **`is_projection` staleness**: the broader staleness problem (field is frozen at
  import time and becomes wrong when the user does not re-import for days) is
  acknowledged in several comments in `forecast_cmds.rs` (e.g. line 113:
  "NÃO filtra `is_projection`: ele é congelado no import … o flag congelado"). This
  plan fixes the _creation-time_ and _edit-time_ classification bugs (Bugs 1 and 3);
  the stale-flag problem for rows that were correctly classified at import but age
  without re-import is a separate, lower-priority concern.
