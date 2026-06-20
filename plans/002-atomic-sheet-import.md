# Plan 002: Make the sheet import atomic (single SQLite transaction)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/commands.rs src-tauri/src/google_sheets/import.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The sheet import orchestration in `import_sheet_data` and `import_local_xlsx`
(in `commands.rs`) performs the layout INSERT, the per-mapping INSERTs, the
row import (`import_rows_with_options`), and the balance-series store
(`store_balance_series`) as **separate executes on the shared pool** — not
inside one transaction. A crash or process kill between any two of those
phases leaves partial state in the database: for example, transactions written
but no balance series (so the projection seed stays wrong), or layout/mapping
rows saved while no transactions were imported. Because the balance column is
the projection seed for the entire dashboard, a half-written import produces
a silently wrong financial view until the user re-imports. Wrapping all four
phases in one `pool.begin() … commit()` per sheet makes the import
all-or-nothing: either every row, every mapping, and the balance series land
together, or nothing changes.

## Current state

### Files and roles

- `src-tauri/src/commands.rs` — Tauri command handlers; contains
  `import_sheet_data` (~lines 168–261) and `import_local_xlsx` (~lines
  297–405). Both execute layout writes and call import functions as separate
  non-transactional operations against the shared `SqlitePool`.
- `src-tauri/src/google_sheets/import.rs` — pure import domain; contains
  `import_rows_with_options` (~lines 199–397) and `store_balance_series`
  (~lines 739–767). Each currently opens **its own** internal transaction and
  commits before returning.

### Critical excerpts (re-verify with the drift check)

**`import_rows_with_options` signature (import.rs:199–205):**

```rust
pub async fn import_rows_with_options(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
) -> Result<usize, String> {
```

It calls `pool.begin()` at line 226 and `tx.commit()` at line 394.

**`store_balance_series` signature (import.rs:739–744):**

```rust
pub async fn store_balance_series(
    pool: &SqlitePool,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
```

It calls `pool.begin()` at line 744 and `tx.commit()` at line 765.

**`import_sheet_data` orchestration (commands.rs:199–261) — the non-atomic
sequence:**

```rust
// Layout INSERT (separate execute)
sqlx::query("INSERT OR REPLACE INTO sheet_layout …")
    .execute(pool.inner())   // line ~209 — no surrounding transaction
    .await?;

for m in &mappings {
    sqlx::query("INSERT OR REPLACE INTO sheet_mapping …")
        .execute(pool.inner())   // line ~221 — ditto
        .await?;
}

// Row import — opens and commits its OWN transaction
let count = import::import_rows_with_options(&pool, …).await?; // line ~242

// Balance series — opens and commits its OWN transaction
import::store_balance_series(&pool, &sheet_name, &balances).await?; // line ~258
```

**`import_local_xlsx` (commands.rs:341–396) — identical shape per sheet:**

```rust
sqlx::query("INSERT OR REPLACE INTO sheet_layout …")
    .execute(pool.inner())  // line ~351
    .await?;
for m in &mappings { … .execute(pool.inner()) … }  // line ~363

let count = import::import_rows_with_options(&pool, …).await?; // line ~379

import::store_balance_series(&pool, sheet_name, &balances).await?; // line ~396
```

**`check_duplicate_import` (import.rs:152–166):** reads `sync_log` via the
pool; called inside `import_rows_with_options` before the transaction opens.
Must stay as a pool-level read (before the outer transaction) to avoid a
read-your-writes false negative.

**`resolve_profile_id` (import.rs:403–447):** already accepts
`&mut sqlx::SqliteConnection`, which is the deref target of
`&mut sqlx::Transaction<'_, sqlx::Sqlite>`. No signature change needed.

**`record_conflict` (import.rs:11–49):** already accepts
`&mut sqlx::Transaction<'_, sqlx::Sqlite>`. No signature change needed.

### Relevant conventions

- Money is integer cents; amounts are positive magnitude — no behavior changes
  in this plan.
- The `functional-core / imperative-shell` style means IO (DB writes) lives in
  the adapter (`commands.rs` is the shell; `import.rs` is the core). Threading
  a transaction through `import.rs` functions is consistent with this style
  — the functions remain pure domain logic, and the shell controls the
  transaction boundary.
- Existing sqlx pattern for passing a connection: `&mut sqlx::SqliteConnection`
  (dereferences from `&mut *tx`). See `resolve_profile_id` at
  `import.rs:403–447` as the structural exemplar.
- Existing sqlx pattern for a `Transaction` wrapping multiple sub-calls:
  `pool.begin()` → `&mut *tx` → `tx.commit()`. See `store_economia_entries`
  in `commands.rs:2359–2397` as a multi-step atomic commit example.
- sqlx version: 0.9 (`src-tauri/Cargo.toml:26`).
  `sqlx::Transaction<'_, sqlx::Sqlite>` derefs to `&mut sqlx::SqliteConnection`
  via `&mut *tx`.
- `pool.inner()` (used in Tauri command handlers) returns `&SqlitePool`.

## Commands you will need

| Purpose            | Command                                                           | Expected on success          |
| ------------------ | ----------------------------------------------------------------- | ---------------------------- |
| Rust check         | `npm run rust:check`                                              | exit 0 (fmt + clippy + test) |
| Rust tests only    | `cargo test --manifest-path src-tauri/Cargo.toml --locked`        | all pass                     |
| Filtered Rust test | `cargo test --manifest-path src-tauri/Cargo.toml --locked import` | all pass                     |
| Typecheck (full)   | `npm run typecheck`                                               | exit 0                       |
| Full gate          | `npm run check`                                                   | exit 0                       |
| Privacy scan       | `npm run privacy:scan`                                            | exit 0                       |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/google_sheets/import.rs` — add transaction-accepting variants
  of `import_rows_with_options` and `store_balance_series`; add the new atomic
  import test.
- `src-tauri/src/commands.rs` — refactor `import_sheet_data` and
  `import_local_xlsx` to open one transaction per sheet, pass it through, and
  commit after all phases succeed.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/layout_detect.rs` — no behavior change in
  layout detection.
- The existing public signatures `import_rows`, `import_rows_with_options`,
  `store_balance_series` — keep them for the tests and any future callers.
  Add `_in_tx` variants alongside, or change the internal bodies to delegate
  to a shared inner function; do not remove the existing public API.
- Rust migrations in `src-tauri/migrations/` — no schema change.
- Frontend TypeScript — this is a pure Rust/SQLite change.
- Plan 004 concerns (per-row owner split writes) — do not pre-empt them.

## Git workflow

- Branch: `advisor/002-atomic-sheet-import`
- Commit style: `fix: <description>` (match the repo's recent `fix:` prefix
  convention, e.g. `fix: wrap sheet import in single SQLite transaction`).
- Commit per logical unit (one commit per step is fine); do NOT push or open a
  PR unless explicitly instructed.

## Steps

### Step 1: Add internal transaction-accepting variants in `import.rs`

The goal is to introduce two `pub(crate)` functions that accept an already-open
`sqlx::Transaction<'_, sqlx::Sqlite>` and do the work without committing — the
caller (commands.rs) owns the commit/rollback.

**1a. Refactor `import_rows_with_options`**

Extract the body of `import_rows_with_options` (lines 199–397 of import.rs)
into a new `async fn import_rows_core(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, …) -> Result<usize, String>` (private, no `pub`).

The extracted body already uses `&mut *tx` for every sqlx call; the only
change is that the transaction is received, not created. Do **not** move the
`check_duplicate_import` call inside — it is a pool-level read and must run
before the transaction opens (see Current state).

Keep the existing `import_rows_with_options(pool, …)` public function intact;
change its body to:

```rust
pub async fn import_rows_with_options(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }
    let checksum = if options.descriptions_trusted {
        compute_checksum(rows)
    } else {
        compute_checksum_with_options(rows, false)
    };
    if check_duplicate_import(pool, sheet_name, &checksum).await? {
        return Ok(0);
    }
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let n = import_rows_core(&mut tx, sheet_name, rows, profile_id, options, &checksum).await?;
    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(n)
}
```

Add a new `pub(crate)` function for use from commands.rs:

```rust
/// Import rows into an already-open transaction. The caller is responsible
/// for commit/rollback. The checksum duplicate check must be done by the
/// caller BEFORE opening the transaction.
pub(crate) async fn import_rows_with_options_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
    options: ImportRowsOptions,
    checksum: &str,
) -> Result<usize, String> {
    import_rows_core(tx, sheet_name, rows, profile_id, options, checksum).await
}
```

**1b. Refactor `store_balance_series`**

Similarly extract its body into a private `store_balance_series_core` that
accepts `tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>`. Keep the public
`store_balance_series(pool, …)` delegating to the core after begin/commit.
Add:

```rust
pub(crate) async fn store_balance_series_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sheet_name: &str,
    series: &[DailyBalance],
) -> Result<usize, String> {
    store_balance_series_core(tx, sheet_name, series).await
}
```

**1c. Expose `compute_checksum_with_options` (or the checksum selection
logic) in a `pub(crate)` helper**

The commands.rs caller needs to compute the checksum before opening the outer
transaction (for the idempotency check). Expose:

```rust
pub(crate) fn compute_import_checksum(rows: &[ImportedRow], descriptions_trusted: bool) -> String {
    compute_checksum_with_options(rows, descriptions_trusted)
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked import`
→ all existing import tests still pass (no behavior change yet).

---

### Step 2: Wrap `import_sheet_data` in a single transaction

In `commands.rs`, `import_sheet_data` (lines ~199–261):

Replace the current sequence of separate `.execute(pool.inner())` +
independent function calls with a single transaction. The new shape:

```rust
// 1. Compute checksum and check duplicate BEFORE opening the transaction
//    (pool-level read; must not be inside the tx to avoid a stale read).
let imported_rows = import::parse_rows_with_layout(&rows, &layout_ref, &mappings_list, &notes);
let options = import::ImportRowsOptions { descriptions_trusted };
let checksum = import::compute_import_checksum(&imported_rows, options.descriptions_trusted);
if import::check_duplicate_import(&pool, &sheet_name, &checksum).await? {
    return Ok(0);  // idempotent — nothing to do
}

// 2. Begin the single outer transaction.
let mut tx = pool.begin().await.map_err(|e| format!("begin import: {e}"))?;

// 3. Layout save (if newly detected).
if layout_was_newly_detected {
    sqlx::query("INSERT OR REPLACE INTO sheet_layout …")
        .execute(&mut *tx)
        .await?;
    for m in &mappings {
        sqlx::query("INSERT OR REPLACE INTO sheet_mapping …")
            .execute(&mut *tx)
            .await?;
    }
}

// 4. Row import (no internal transaction — uses our tx).
let count = import::import_rows_with_options_in_tx(
    &mut tx, &sheet_name, &imported_rows, &profile_id, options, &checksum,
).await?;

// 5. Balance series (no internal transaction — uses our tx).
let balance_offset = import::get_balance_offset_for_sheet(&pool, &sheet_name).await?;
let balances = import::parse_balance_series(&rows, &layout_ref, balance_offset);
import::store_balance_series_in_tx(&mut tx, &sheet_name, &balances).await?;

// 6. Single commit.
tx.commit().await.map_err(|e| format!("commit import: {e}"))?;

Ok(count)
```

Notes:

- `get_balance_offset_for_sheet` is a read-only query; call it against `&pool`
  (pool-level read) before or after the tx — it does not write. Calling it
  after `tx` is opened is fine because it does not conflict with any in-tx
  write. If you prefer, call it before `pool.begin()` alongside the layout
  detection; either order is correct because it only reads `sheet_mapping`
  (which is not being concurrently modified by another writer in this path).
- `get_layout_for_sheet` and `get_active_mappings_for_sheet` are also reads;
  call them against `&pool` before `pool.begin()` as today.
- `descriptions_trusted` flag: set it from the `(_, descriptions_trusted)`
  tuple that comes from the notes fetch (the existing `let descriptions_trusted`
  variable is already in scope).
- Preserve the existing early-return for `rows.len() < 3`.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked import`
→ all existing import tests still pass.

---

### Step 3: Wrap each sheet iteration of `import_local_xlsx` in a single transaction

In `commands.rs`, `import_local_xlsx` (lines ~297–405):

Apply the same pattern to each loop iteration (per sheet):

```rust
for sheet_name in &sheet_names {
    // … economia branch unchanged …

    if let Ok(range) = workbook.worksheet_range(sheet_name) {
        // … parse rows, skip if < 3 …

        // Layout detection (pool-level reads/writes deferred to tx below)
        let detected_layout = match import::get_layout_for_sheet(&pool, sheet_name).await? {
            Some(l) => (l, false),    // (layout, is_new)
            None => match layout_detect::detect_layout(&rows, sheet_name) {
                Ok(d) => (d, true),
                Err(_) => continue,
            },
        };
        let (layout, is_new_layout) = detected_layout;

        let mappings = import::get_active_mappings_for_sheet(&pool, sheet_name).await?;
        let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &[]);
        if imported_rows.is_empty() {
            continue;
        }

        let options = import::ImportRowsOptions { descriptions_trusted: false };
        let checksum = import::compute_import_checksum(&imported_rows, false);
        if import::check_duplicate_import(&pool, sheet_name, &checksum).await? {
            continue;
        }

        let mut tx = pool.begin().await.map_err(|e| format!("begin import: {e}"))?;

        if is_new_layout {
            sqlx::query("INSERT OR REPLACE INTO sheet_layout …")
                .execute(&mut *tx).await?;
            for m in &generate_mappings(&layout) {
                sqlx::query("INSERT OR REPLACE INTO sheet_mapping …")
                    .execute(&mut *tx).await?;
            }
        }

        let count = import::import_rows_with_options_in_tx(
            &mut tx, sheet_name, &imported_rows, &profile_id, options, &checksum,
        ).await?;

        let balance_offset = import::get_balance_offset_for_sheet(&pool, sheet_name).await?;
        let balances = import::parse_balance_series(&rows, &layout, balance_offset);
        import::store_balance_series_in_tx(&mut tx, sheet_name, &balances).await?;

        tx.commit().await.map_err(|e| format!("commit import: {e}"))?;

        total += count;
        sheets_imported.push(format!("{sheet_name} ({count} rows)"));
    }
}
```

Note on `get_active_mappings_for_sheet` in the `is_new_layout` branch: when
the layout is new, the mappings were just written inside `tx` and are not yet
visible via the pool. In that case, generate the mapping list from
`layout_detect::generate_mappings(&layout)` for the `parse_rows_with_layout`
call, rather than fetching from the pool. (This matches what the original code
did — it called `import::get_active_mappings_for_sheet` after the inserts,
which was also pool-level and therefore could not see the un-committed
rows — but that path worked because SQLite WAL mode allows readers to see the
last committed version; inside a transaction the writes are not yet committed.
The safe approach: stash `generate_mappings` output before beginning `tx`.)
If you examine the existing code at lines 355–366 and 373, you will see that
`get_active_mappings_for_sheet` is called AFTER the individual mapping inserts
with `pool.inner()` — those inserts were auto-committed, so the fetch worked.
Under the new scheme, move the mapping generation (from `generate_mappings`)
before `pool.begin()` and use the generated list directly.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked import`
→ all existing import tests still pass.

---

### Step 4: Add an all-or-nothing regression test

In `src-tauri/src/google_sheets/import.rs`, inside `mod tests`, add one new
async test. Model its structure after the existing `reimport_after_edit_replaces_instead_of_duplicating` test (lines 1563–1582).

The new test must prove that if `store_balance_series_in_tx` returns an error
after rows have been written in the same transaction, the transaction rolls back
and zero new rows remain in the database.

Because a real mid-import error is hard to inject, test the next-best thing:
**two separate full-import calls where the second import uses the same checksum
(duplicate check) to verify the dedupe gate works correctly across both
transaction phases.** Then also test the explicit all-or-nothing property:

```rust
#[tokio::test]
async fn atomic_import_rolls_back_on_balance_error() {
    let pool = test_pool().await;
    let rows = vec![imported("2026-03-01", 50_000)];

    // First: a normal import succeeds.
    let checksum = compute_import_checksum(&rows, true);
    assert!(!check_duplicate_import(&pool, "2026", &checksum).await.unwrap());
    {
        let mut tx = pool.begin().await.unwrap();
        let n = import_rows_with_options_in_tx(
            &mut tx, "2026", &rows, "p1", ImportRowsOptions::default(), &checksum,
        )
        .await
        .unwrap();
        assert_eq!(n, 1);
        // Simulate a balance-series failure by NOT calling store_balance_series_in_tx
        // and instead explicitly rolling back — simulates a mid-import crash.
        tx.rollback().await.unwrap();
    }

    // After rollback: zero transactions, balance series empty.
    assert_eq!(count_transactions(&pool).await, 0);
    let (bal_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bal_count, 0, "balance series must also be absent after rollback");

    // Duplicate-check must still return false (the rolled-back import must not
    // have written a sync_log entry that would block the retry).
    assert!(
        !check_duplicate_import(&pool, "2026", &checksum).await.unwrap(),
        "rolled-back import must not poison the duplicate-check gate"
    );
}
```

Add a second test that proves a successful two-phase import (rows + balance
series) in a single transaction commits both atomically:

```rust
#[tokio::test]
async fn atomic_import_commits_rows_and_balance_together() {
    let pool = test_pool().await;
    let rows = vec![imported("2026-04-01", 30_000)];
    let series = vec![DailyBalance {
        date: "2026-04-01".into(),
        balance_cents: 30_000,
        is_projection: false,
    }];
    let checksum = compute_import_checksum(&rows, true);

    let mut tx = pool.begin().await.unwrap();
    import_rows_with_options_in_tx(
        &mut tx, "2026", &rows, "p1", ImportRowsOptions::default(), &checksum,
    )
    .await
    .unwrap();
    store_balance_series_in_tx(&mut tx, "2026", &series).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(count_transactions(&pool).await, 1);
    let (bal_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sheet_daily_balance")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bal_count, 1, "balance series committed alongside transactions");
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked import`
→ all existing tests pass AND the two new tests pass (look for
`atomic_import_rolls_back_on_balance_error` and
`atomic_import_commits_rows_and_balance_together` in the output).

---

### Step 5: Full quality gate

Run the complete gate to confirm nothing else broke:

**Verify**: `npm run rust:check` → exit 0 (rustfmt + clippy + all Rust tests).

Then run the full gate:

**Verify**: `npm run check` → exit 0.

---

## Test plan

### New tests (Step 4, in `src-tauri/src/google_sheets/import.rs`, `mod tests`)

| Test name                                         | What it covers                                                                                                                                                 |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `atomic_import_rolls_back_on_balance_error`       | Explicit rollback after row-import phase leaves zero transactions and zero balance rows; duplicate-check gate not poisoned by the rolled-back sync_log entries |
| `atomic_import_commits_rows_and_balance_together` | Successful single-transaction import commits both rows and balance series; both are readable after commit                                                      |

### Structural pattern

Model after `reimport_after_edit_replaces_instead_of_duplicating` (import.rs:1563)
and `slot_identity_is_positional_known_limitation` (import.rs:1722) for the
async test setup (`test_pool().await`, `imported()` helper, `count_transactions()`).

### Existing tests that must still pass (no regression)

All tests in `mod tests` of `import.rs` — especially:

- `reimport_identical_dataset_is_noop` — idempotency must still work
- `import_bootstraps_default_profile_when_id_is_unknown` — profile resolution still in inner tx
- `reimport_preserves_transaction_identity_and_enrichment` — no re-insert
- `conflict_recorded_when_both_changed` — 3-way merge unchanged
- All `slot_*` and `replace_is_scoped_to_its_own_sheet` tests

### Verification command

```
cargo test --manifest-path src-tauri/Cargo.toml --locked import
```

Expected: all previously passing tests pass; 2 new tests pass.

## Done criteria

All must hold before marking this plan DONE:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked import` exits 0; the two new tests (`atomic_import_rolls_back_on_balance_error`, `atomic_import_commits_rows_and_balance_together`) appear in the output and pass.
- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests).
- [ ] `npm run check` exits 0 (full gate).
- [ ] `grep -n "pool.inner()" src-tauri/src/commands.rs` returns no matches in the layout or mapping INSERT lines inside `import_sheet_data` or `import_local_xlsx` (they now use `&mut *tx` instead).
- [ ] `grep -n "store_balance_series\b" src-tauri/src/commands.rs` returns no matches where the call goes directly to `pool`/`pool.inner()` inside the import functions (it must go through `store_balance_series_in_tx`).
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD` shows only `src-tauri/src/commands.rs` and `src-tauri/src/google_sheets/import.rs`).
- [ ] `plans/README.md` status row for plan 002 updated to DONE.

## STOP conditions

Stop and report back (do not improvise) if:

- The line numbers or code excerpts in "Current state" do not match the live
  code (drift from a concurrent commit).
- A `cargo test` step fails with a compile error that cannot be fixed by
  adjusting the new `_in_tx` function signatures (e.g., a lifetime issue
  from passing `&mut Transaction` through two call frames).
- `store_balance_series_in_tx` cannot share the outer transaction because
  SQLite returns a `database is locked` error (would indicate the connection
  pool is configured with more than 1 connection for the test DB, or that
  the WAL mode/locking semantics changed — check `SqlitePoolOptions` in
  `test_pool()`).
- Any existing test in `import.rs` fails after the refactor, even after a
  reasonable fix attempt.
- The fix requires touching `layout_detect.rs`, `reconcile.rs`, or any
  migration file.
- `npm run privacy:scan` or `npm run check` reports a new failure unrelated
  to this plan's changes (stop, note it, and report separately).

## Maintenance notes

- **Plan 004 (owner splits + credit payment method) depends on this plan.**
  Plan 004 adds more per-row writes inside the import path. Those writes must
  be threaded through `import_rows_core` (or a new inner function) so they
  participate in the single outer transaction established by `commands.rs`.
  When implementing plan 004, do not re-open a second `pool.begin()` inside
  `import_rows_core` — use the `tx` already in scope.
- **Reviewer scrutiny in the PR:** confirm that `check_duplicate_import` and
  `get_balance_offset_for_sheet` are still called against `&pool` (not inside
  the transaction), and that neither `import_rows_with_options_in_tx` nor
  `store_balance_series_in_tx` calls `tx.commit()` — only the caller in
  `commands.rs` commits.
- **`import_local_xlsx` economia branch:** the `store_economia_entries` call
  in the `import_local_xlsx` economia branch (line ~324) already wraps its own
  internal transaction; that branch is out of scope for this plan and should
  not be changed.
- **SQLite WAL mode:** the app enables WAL via pragmas at startup; the
  single-writer, single-connection-per-import constraint is safe. If a
  background writer is added in the future, revisit the connection pool
  size and the locking behavior of long-running import transactions.
