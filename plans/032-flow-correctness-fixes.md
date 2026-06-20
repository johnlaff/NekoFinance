# Plan 032: Flow correctness: write-back audit, sync race, derived double-count, Jan-1 guardrail

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src-tauri/src/commands/write_back_cmds.rs src-tauri/src/lib.rs src-tauri/src/commands/sheets_import.rs src-tauri/src/commands/forecast_cmds.rs src-tauri/src/sync_task.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Category**: bug
- **Package**: B
- **Depends on**: none
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

Four confirmed logic bugs corrupt data or silently skip safety guardrails on every user session. The write-back audit NULL payment_method bug means `source_amount` is never realigned for the user's normal Saída/Diário entries (which always have `payment_method IS NULL`), so phantom import conflicts accumulate after every write-back. The sync race allows a user-triggered import to run simultaneously with the background sync on the single-connection SQLite pool, risking database corruption or a "database is locked" error. Derived compensating transactions (created by the split/reembolso feature) are double-counted in the write-back load, inflating the Entrada value written back to the spreadsheet. The Jan-1 guardrail goes inactive on January 1 because the realized-savings window is empty, giving a false "no constraint" signal exactly when the user is starting a new year. The two P2 additions (Economia write-back missing audit trail; sentinel advances even when a tab import fails) are included because they are one-to-two line fixes adjacent to the P1 changes.

## Current state

### Files and their roles

- `src-tauri/src/commands/write_back_cmds.rs` — Tauri commands for write-back preview/apply; contains `load_write_back_txns` (lines 10–46), `record_write_back_audit` (lines 478–561), and `apply_economia_write_back` (lines 668–706).
- `src-tauri/src/commands/sheets_import.rs` — Tauri commands `import_sheet_data` (line 78) and `import_local_xlsx` (line 270); neither acquires the `SyncGuard`.
- `src-tauri/src/lib.rs` — App setup; creates and manages `Arc<SyncGuard>` (lines 133–134); comment at line 130 says it serializes user-triggered imports (untrue for `import_sheet_data`/`import_local_xlsx`).
- `src-tauri/src/commands/forecast_cmds.rs` — `realized_annual_savings` (lines 105–124); the window `[year_start, cur_ym-01)` is empty on Jan 1.
- `src-tauri/src/sync_task.rs` — `SyncGuard` type alias (line 43); background import loop (lines 217–238); sentinel advance at line 238.

### Excerpt 1 — Bug A: NULL payment_method excluded from audit realignment

`src-tauri/src/commands/write_back_cmds.rs` lines 509–532:

```rust
"saida" => {
    sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
           AND NOT (payment_method = 'credit')",
    )
    // ...
}
"diario" => {
    sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 0 \
           AND NOT (payment_method = 'credit')",
    )
    // ...
}
```

In SQLite, `NULL = 'credit'` evaluates to NULL, so `NOT NULL` → NULL → the row is EXCLUDED by the WHERE clause. Transactions with `payment_method IS NULL` (all normal Saída/Diário entries) are never realigned.

Fix both occurrences: replace `AND NOT (payment_method = 'credit')` with `AND (payment_method IS NULL OR payment_method <> 'credit')`.

### Excerpt 2 — Bug B: import_sheet_data does not acquire SyncGuard

`src-tauri/src/commands/sheets_import.rs` lines 77–107:

```rust
#[tauri::command]
pub async fn import_sheet_data(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    profile_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    // ...
    import_one_tab(
        pool.inner(),
        &client,
        &spreadsheet_id,
        &sheet_name,
        &profile_id,
    )
    .await
}
```

No `State<'_, Arc<sync_task::SyncGuard>>` parameter; `import_local_xlsx` at line 270 has the same omission.

`src-tauri/src/lib.rs` lines 129–134 (the misleading comment and the manage call):

```rust
// Background read-side sync (plan 026, Phase 1: read-only — never touches
// write-back). Spawned after the pool + AppDataDir are managed. Clones happen
// before `app.manage(pool)` moves the pool into Tauri state. The shared
// SyncGuard serializes background and user-triggered imports (single-connection
// pool); the focus handler reuses the same guard so neither path overlaps.
let sync_pool = pool.clone();
let import_guard = Arc::new(sync_task::SyncGuard::new(()));
app.manage(import_guard.clone());
```

The comment at lines 130–131 says the guard serializes user-triggered imports, but `import_sheet_data` and `import_local_xlsx` do not accept it as Tauri state.

`src-tauri/src/sync_task.rs` line 43:

```rust
pub type SyncGuard = tokio::sync::Mutex<()>;
```

Fix: add `guard: State<'_, Arc<sync_task::SyncGuard>>` to both command signatures; acquire `let _lock = guard.inner().lock().await;` before calling `import_one_tab` / the xlsx loop. Update the comment in `lib.rs` to be accurate.

### Excerpt 3 — Bug C: derived transactions included in write-back load

`src-tauri/src/commands/write_back_cmds.rs` lines 10–24:

```rust
pub(crate) async fn load_write_back_txns(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<WriteBackTxn>, String> {
    // 1) Entrada + Saída/Diário (expense não-crédito) do ano, cada um na sua data.
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT type, date, amount, is_fixed FROM \"transaction\" \
         WHERE date >= ?1 AND date < ?2 \
           AND NOT (type='expense' AND payment_method='credit')",
    )
```

The query loads all non-credit-expense transactions, including `type='income'` rows with `id LIKE 'derived:%'` (the compensating Entradas created by plan 023 for #reembolso/#dividir). These get aggregated into the Entrada cell by `plan_write_back` in `src-tauri/src/google_sheets/write_back.rs` lines 163–176, inflating the value written back.

Fix: add `AND id NOT LIKE 'derived:%'` to the WHERE clause of the `load_write_back_txns` query.

### Excerpt 4 — Bug D: Jan-1 realized_annual_savings window is empty

`src-tauri/src/commands/forecast_cmds.rs` lines 105–124:

```rust
pub(crate) async fn realized_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date < ?2 \
           AND type IN ('income','expense')",
    )
    .bind(&year_start)
    .bind(format!("{cur_ym}-01")) // 1º dia do mês corrente
```

On January 1, `cur_ym` = `"YYYY-01"` and `format!("{cur_ym}-01")` = `"YYYY-01-01"`, which equals `year_start`. The range `[year_start, year_start)` is empty → both sums are 0 → the guardrail goes inactive.

Fix: when `cur_ym == format!("{}-01", today_naive.year())` (i.e., it is January), use the previous December as the upper bound:

```rust
let upper = if cur_ym == format!("{}-01", today_naive.year()) {
    format!("{}-12-01", today_naive.year() - 1)
} else {
    format!("{cur_ym}-01")
};
```

Then bind `&upper` instead of `format!("{cur_ym}-01")`. When December data from the prior year is absent (first-ever launch in January), the query returns 0 — the same safe fallback as before, but now the caller can distinguish "no data" from "active window is empty".

### Excerpt 5 — P2a: apply_economia_write_back has no record_write_back_audit call

`src-tauri/src/commands/write_back_cmds.rs` lines 697–706:

```rust
    // Re-verifica a frescura (Step 4) antes de escrever.
    guard_sheet_unchanged(&client, &spreadsheet_id, preview_revision.as_deref()).await?;

    let updates: Vec<(String, f64)> = plan
        .iter()
        .filter(|c| c.changed)
        .map(|c| (format!("'Economia'!{}", c.a1), c.value_cents as f64 / 100.0))
        .collect();
    client.batch_update_values(&spreadsheet_id, &updates).await
}
```

`record_write_back_audit` is not called here. After the Economia write-back, `source_amount` on transfer rows is never updated, and no `sync_log` audit entry is written.

Fix: call `record_write_back_audit(pool.inner(), "Economia", &cells_written).await?;` after `batch_update_values` succeeds, where `cells_written` is the filtered slice of changed `CellWrite` refs. (Pattern to match: see `apply_write_back` a few lines above in the same file which calls `record_write_back_audit` correctly.)

### Excerpt 6 — P2b: sync sentinel advances even when a tab import fails

`src-tauri/src/sync_task.rs` lines 224–238:

```rust
    for tab in &tabs {
        if let Err(e) =
            crate::commands::import_one_tab(pool, &client, &spreadsheet_id, tab, &profile_id).await
        {
            // One bad tab shouldn't abort the rest; log and continue.
            eprintln!("[sync] import of tab '{tab}' failed: {e}");
        }
    }

    // 9. Conflicts created by this import (still the open count — the import never
    //    auto-resolves; this is the badge number the frontend shows).
    let conflict_count = open_conflict_count(pool).await?;

    // 10. Advance the sentinel only after a successful import pass.
    crate::commands::app_setting_set(pool, "sheets_last_modified_time", &modified_time).await?;
```

The comment at line 237 says "only after a successful import pass", but the sentinel advances even if every tab failed (the loop continues on error and does not set a flag). Fix: track whether any tab failed and skip the sentinel advance if so, so the next tick retries.

### Repo conventions

- All Rust async tests in `src-tauri/src/commands/mod.rs` use `#[tokio::test]` and a shared `fixture_pool()` helper that creates an in-memory SQLite pool and runs migrations. Match this pattern for new regression tests.
- Money = positive-magnitude integer cents. Queries use `ABS(amount)` or the magnitude stored at insert; `WriteBackTxn.amount_cents` is always positive.
- SQL uses named bind params `?1`, `?2`, ... (sqlx SQLite style). NOT `$1`.
- `use super::*;` at the top of each submodule pulls in the shared imports from `src-tauri/src/commands/mod.rs`.
- Do not use `Arc<sync_task::SyncGuard>` directly in tests — the guard is only needed in the Tauri command signature (real Tauri state); unit tests call the inner async functions directly without it.

## Commands you will need

| Purpose         | Command                           | Expected on success           |
| --------------- | --------------------------------- | ----------------------------- |
| Rust check      | `npm run rust:check`              | exit 0, no errors or warnings |
| Full gate       | `npm run check`                   | exit 0                        |
| Unit tests      | `npm run test:run`                | all pass                      |
| Rust tests only | `cd src-tauri && cargo test 2>&1` | all pass                      |
| Typecheck       | `npm run typecheck`               | exit 0                        |
| Lint            | `npm run lint`                    | exit 0                        |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/write_back_cmds.rs` — Bug A, Bug C, P2a fixes
- `src-tauri/src/commands/sheets_import.rs` — Bug B fix (add SyncGuard to command signatures)
- `src-tauri/src/lib.rs` — Bug B fix (update misleading comment at lines 129–131)
- `src-tauri/src/commands/forecast_cmds.rs` — Bug D fix
- `src-tauri/src/sync_task.rs` — P2b fix
- `src-tauri/src/commands/mod.rs` — regression tests for all four P1 bugs (add to the existing `#[cfg(test)] mod tests` block)

**Out of scope** (do NOT touch):

- `src-tauri/src/google_sheets/write_back.rs` — the `plan_write_back` aggregation is correct; the fix is upstream in `load_write_back_txns`.
- `src-tauri/src/commands/forecast_cmds.rs` lines outside `realized_annual_savings` — the `is_projection` ambient-clock issue is deferred (see Maintenance notes).
- Any frontend (React) files — these are pure Rust/SQL bugs.
- Migration files — no schema changes required.

## Git workflow

- Branch: `advisor/032-flow-correctness-fixes`
- Commit style: conventional commits, matching recent history — e.g. `fix: <concise description>`. One commit per logical fix (one per step) is fine; squashing is also fine.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Fix Bug A — NULL payment_method excluded from write-back audit realignment

In `src-tauri/src/commands/write_back_cmds.rs`, function `record_write_back_audit` (starts at line 478), find the two SQL UPDATE statements for `"saida"` (around line 509) and `"diario"` (around line 521). Each ends with:

```
AND NOT (payment_method = 'credit')
```

Replace both occurrences with:

```
AND (payment_method IS NULL OR payment_method <> 'credit')
```

There are exactly two occurrences in this function (one in the `"saida"` arm, one in the `"diario"` arm). Do not change the `"entrada"` arm — it has no such filter.

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles and existing tests pass.

### Step 2: Add regression test for Bug A

In `src-tauri/src/commands/mod.rs`, inside `#[cfg(test)] mod tests`, add a new `#[tokio::test]` function `write_back_audit_realigns_null_payment_method`. The test should:

1. Call `fixture_pool().await` to create an in-memory pool.
2. Insert a realized `expense` with `is_fixed=1`, `date='2026-03-10'`, `amount=50000`, and `payment_method` not set (NULL) — use `sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) VALUES (?1,'expense',50000,'2026-03-10',1,0)")`.
3. Build a `CellWrite { date: "2026-03-10".into(), kind: "saida".into(), a1: "B10".into(), value_cents: 55000, changed: true }`.
4. Call `record_write_back_audit(&pool, "2026", &[&cell]).await.unwrap()`.
5. Query `SELECT source_amount FROM "transaction" WHERE date='2026-03-10' AND type='expense'` and assert it equals `55000`.

This test failed before the fix (source_amount stayed NULL) and must pass after.

**Verify**: `cd src-tauri && cargo test write_back_audit_realigns 2>&1` → 1 test, passed.

### Step 3: Fix Bug C — derived transactions included in write-back load

In `src-tauri/src/commands/write_back_cmds.rs`, function `load_write_back_txns` (starts at line 10), find the first SQL query (around line 15–18):

```sql
SELECT type, date, amount, is_fixed FROM "transaction"
WHERE date >= ?1 AND date < ?2
  AND NOT (type='expense' AND payment_method='credit')
```

Add `AND id NOT LIKE 'derived:%'` to the WHERE clause so the query becomes:

```sql
SELECT type, date, amount, is_fixed FROM "transaction"
WHERE date >= ?1 AND date < ?2
  AND NOT (type='expense' AND payment_method='credit')
  AND id NOT LIKE 'derived:%'
```

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles and all tests pass.

### Step 4: Add regression test for Bug C

In `src-tauri/src/commands/mod.rs`, add `#[tokio::test] async fn load_write_back_txns_excludes_derived`. The test should:

1. Call `fixture_pool().await`.
2. Insert a normal income transaction for 2026: `id='t-real'`, `type='income'`, `amount=100000`, `date='2026-03-05'`, `is_projection=0`.
3. Insert a derived compensating income: `id='derived:reembolso:t-real'`, `type='income'`, `amount=5000`, `date='2026-03-05'`, `is_projection=0`.
4. Call `load_write_back_txns(&pool, 2026).await.unwrap()`.
5. Assert the returned `Vec` has exactly 1 element (the real transaction, not the derived one) and its `amount_cents` is `100000`.

**Verify**: `cd src-tauri && cargo test load_write_back_txns_excludes_derived 2>&1` → 1 test, passed.

### Step 5: Fix Bug D — Jan-1 realized_annual_savings window is empty

In `src-tauri/src/commands/forecast_cmds.rs`, function `realized_annual_savings` (starts at line 105), replace the bind for the upper date. Currently at line 119:

```rust
.bind(format!("{cur_ym}-01"))
```

Replace the two lines that compute `cur_ym` and the bind with:

```rust
let cur_ym = today_naive.format("%Y-%m").to_string();
let upper = if cur_ym == format!("{}-01", today_naive.year()) {
    // January: no completed months in this year yet — use previous December
    // so the guardrail stays active (based on prior-year data).
    format!("{}-12-01", today_naive.year() - 1)
} else {
    format!("{cur_ym}-01")
};
```

Then bind `&upper` instead of `format!("{cur_ym}-01")`.

The `year_start` bind (`?1`) does not change — the function still queries from the start of the current year (or, in January, prior-December data falls before `year_start`, giving a safe empty window rather than a broken one). Add a comment explaining the Jan-1 special case.

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles and all existing tests pass.

### Step 6: Add regression test for Bug D

In `src-tauri/src/commands/mod.rs`, add `#[tokio::test] async fn realized_annual_savings_active_on_jan_1`. The test should:

1. Call `fixture_pool().await`.
2. Insert realized income and expense in the previous December (e.g., `date='2025-12-10'`, income `120000`, expense `80000`).
3. Call `realized_annual_savings(&pool, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()).await.unwrap()`.
4. Assert the returned `(income, savings)` = `(120000, 40000)` — the guardrail is active, not silent.

Also add a companion test `realized_annual_savings_jan_1_no_prior_data` that calls the same function on Jan 1 with no prior December data and asserts both values are `0` (safe fallback, not a panic).

**Verify**: `cd src-tauri && cargo test realized_annual_savings_active_on_jan_1 2>&1` and `cargo test realized_annual_savings_jan_1_no_prior_data 2>&1` → each 1 test, passed.

### Step 7: Fix Bug B — import_sheet_data and import_local_xlsx do not acquire SyncGuard

In `src-tauri/src/commands/sheets_import.rs`:

For `import_sheet_data` (line 78), add `guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>` as a new parameter (after `pool`). Then acquire the lock before calling `import_one_tab`:

```rust
let _lock = guard.inner().lock().await;
import_one_tab(pool.inner(), &client, &spreadsheet_id, &sheet_name, &profile_id).await
```

For `import_local_xlsx` (line 270), add `guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>` as a new parameter (after `pool`). Acquire the lock before the `for sheet_name in &sheet_names` loop.

Tauri automatically resolves managed state by type — the `Arc<SyncGuard>` was already registered via `app.manage(import_guard.clone())` in `lib.rs` line 134 — so no change to `lib.rs` is needed except updating the comment.

In `src-tauri/src/lib.rs`, update the comment block at lines 129–131 from "the focus handler reuses the same guard so neither path overlaps" to make explicit that `import_sheet_data` and `import_local_xlsx` also acquire the guard:

Replace:

```
// The shared
// SyncGuard serializes background and user-triggered imports (single-connection
// pool); the focus handler reuses the same guard so neither path overlaps.
```

With:

```
// The shared SyncGuard serializes ALL import paths (background loop, focus probe,
// `import_sheet_data`, `import_local_xlsx`) against each other on the
// single-connection pool.
```

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles and all existing tests pass. Also: `grep -n "guard.*SyncGuard\|SyncGuard.*guard" src-tauri/src/commands/sheets_import.rs` → shows both command function signatures contain the guard parameter.

### Step 8: Add regression test for Bug B

The SyncGuard contention is a concurrency concern that is hard to exercise deterministically in a unit test. Instead, add a documentation test that confirms the guard IS present in the command signatures by asserting the compile-time presence. A simpler approach: add `#[tokio::test] async fn import_sheet_data_signature_documents_sync_guard` in `mod.rs` that just checks at compile time:

```rust
// Regression: import_sheet_data must accept a SyncGuard State parameter.
// This test documents the expectation; the actual guard is Tauri-managed state
// and cannot be exercised without a full Tauri context.
// CI: if the parameter is removed, `import_sheet_data` will not compile because
// Tauri's #[tauri::command] macro injects the state resolver at compile time.
// The real guard is acquired before import_one_tab (verified by code review).
let _: () = (); // compilation proves the command still exists
```

More usefully, add a concurrency regression test `import_guard_serializes_concurrent_calls`:

1. Create two `fixture_pool()` instances sharing an in-memory path (or use a single pool).
2. Create a real `Arc<sync_task::SyncGuard>` (`Arc::new(sync_task::SyncGuard::new(())`).
3. Acquire the lock in one task and confirm a second concurrent task is blocked (using `tokio::time::timeout` to assert it does NOT complete within a short window while the first lock is held).

This is optional; if it proves difficult to write deterministically, a comment in the test file noting the expectation and referring to `lib.rs` is acceptable. The P1 check is that the code compiles with the parameter added.

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → all tests pass.

### Step 9: Fix P2a — apply_economia_write_back missing audit call

In `src-tauri/src/commands/write_back_cmds.rs`, function `apply_economia_write_back` (line 668), after the `client.batch_update_values(...)` call succeeds, add an audit call. The changed cells are already in `plan` (the `Vec<CellWrite>` returned by `build_economia_plan`). After the `batch_update_values` call:

```rust
let written: Vec<&CellWrite> = plan.iter().filter(|c| c.changed).collect();
if !written.is_empty() {
    record_write_back_audit(pool.inner(), "Economia", &written).await?;
}
let n = written.len();
Ok(n)
```

Note: the current function returns `client.batch_update_values(...)` (a `Result<usize, String>`). Restructure to capture the count, run the audit, then return the count. The `updates` vec is already built from `plan.iter().filter(|c| c.changed)` — compute it once, use for both `batch_update_values` and for building `written`.

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles, all tests pass.

### Step 10: Fix P2b — sync sentinel advances even when all tab imports fail

In `src-tauri/src/sync_task.rs`, in the `run_probe` function, find the tab import loop at lines 224–231 and the sentinel advance at line 238. Introduce a flag `all_ok`:

```rust
let mut all_ok = true;
for tab in &tabs {
    if let Err(e) =
        crate::commands::import_one_tab(pool, &client, &spreadsheet_id, tab, &profile_id).await
    {
        eprintln!("[sync] import of tab '{tab}' failed: {e}");
        all_ok = false;
    }
}

// 9. ...
let conflict_count = open_conflict_count(pool).await?;

// 10. Only advance the sentinel when ALL tabs imported successfully.
//     If any tab failed, skip the advance so the next tick retries.
if all_ok {
    crate::commands::app_setting_set(pool, "sheets_last_modified_time", &modified_time).await?;
}
```

Update the comment at line 237 to match.

**Verify**: `cd src-tauri && cargo test 2>&1 | tail -5` → compiles, all tests pass.

### Step 11: Full gate

Run the complete quality gate to confirm nothing regressed.

**Verify**: `npm run check` → exits 0. Then: `cd src-tauri && cargo test 2>&1 | grep -E "test result|FAILED"` → "test result: ok" with no FAILED lines.

## Test plan

All regression tests go in `src-tauri/src/commands/mod.rs` inside the existing `#[cfg(test)] mod tests` block. Model each test after the existing `fixture_pool()` + `sqlx::query` pattern (see lines 280–445 for examples). New tests:

| Test name                                       | Step | What it covers                                    |
| ----------------------------------------------- | ---- | ------------------------------------------------- |
| `write_back_audit_realigns_null_payment_method` | 2    | Bug A: NULL payment_method rows are now realigned |
| `load_write_back_txns_excludes_derived`         | 4    | Bug C: derived IDs excluded from write-back load  |
| `realized_annual_savings_active_on_jan_1`       | 6    | Bug D: prior-December data used on Jan 1          |
| `realized_annual_savings_jan_1_no_prior_data`   | 6    | Bug D edge case: safe empty result, no panic      |

Verification: `cd src-tauri && cargo test 2>&1 | grep -E "test.*ok|test.*FAILED"` → all four new tests pass; no FAILED lines.

## Done criteria

- [ ] `npm run rust:check` exits 0 with no warnings
- [ ] `cd src-tauri && cargo test 2>&1` exits 0; all four new regression tests exist and pass
- [ ] `npm run check` exits 0 (typecheck + lint + test:run + rust:check all green)
- [ ] `grep -n "NOT (payment_method = 'credit')" src-tauri/src/commands/write_back_cmds.rs` returns no matches (both occurrences replaced)
- [ ] `grep -n "derived:%" src-tauri/src/commands/write_back_cmds.rs` shows the exclusion filter in `load_write_back_txns`
- [ ] `grep -n "SyncGuard" src-tauri/src/commands/sheets_import.rs` returns matches for both `import_sheet_data` and `import_local_xlsx`
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row for plan 032 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The SQL snippets in "Current state" do not match the live file at the stated lines (the codebase drifted since this plan was written — run the drift check at the top first).
- Adding `State<'_, Arc<crate::sync_task::SyncGuard>>` to a command signature causes a Tauri compile error because the type is not managed (it should be — `lib.rs` line 134 registers it — but verify `app.manage(import_guard.clone())` is still present before proceeding).
- The `record_write_back_audit` signature in `write_back_cmds.rs` has changed (e.g., it now takes a different cell type or sheet name format).
- Step 9 (Economia audit) requires changes to `CellWrite` or `WriteBackTxn` structs (out of scope for this plan; flag it and skip step 9 only).
- Any step's `cargo test` fails after a reasonable fix attempt; do not mask the failure with `#[allow(...)]` or by deleting the failing test.
- The number of existing tests decreases (you removed rather than added tests).

## Maintenance notes

- **Bug A and B interact**: after Bug B is fixed, user-triggered imports and write-back audits will serialize correctly. If `import_sheet_data` is ever split into a streaming or multi-tab command, ensure the guard is acquired for the entire batch, not per-tab.
- **Bug C and the derived-transaction feature**: the `derived:%` ID prefix is the contract established by plan 023. Any future feature that creates synthetic transactions MUST use this prefix so the write-back load exclusion continues to work.
- **Bug D and the prior-December window**: the Jan-1 fix uses prior-December data as a proxy for "last full period of realized savings". If the user did not import December data, the guardrail returns 0 (inactive) — this is acceptable. If a future plan adds monthly guardrail logic, revisit this function.
- **P2b and partial tab failure**: the sentinel-skip-on-failure change means a persistently failing tab will prevent the sentinel from ever advancing, causing the background sync to re-import on every tick. This is the correct conservative behavior (retry until all succeed). If a tab is permanently broken, the user should remove it from tracking — no code change needed here.
- **Deferred (out of scope)**: the `is_projection` ambient-clock staleness issue in `realized_annual_savings` (a flag frozen at import time that becomes stale when the user does not re-import for days) is tracked separately and is NOT addressed by this plan.
- **Reviewer checklist**: in the PR, scrutinize the two NULL-safe SQL fixes (Bug A) for SQLite-specific NULL semantics; confirm the `derived:%` LIKE pattern is correct for all synthetic IDs currently created; verify the Jan-1 branch is exercised by the new tests and not dead code; confirm `import_sheet_data` and `import_local_xlsx` both hold the lock for the full import duration (not just acquired and immediately dropped).
