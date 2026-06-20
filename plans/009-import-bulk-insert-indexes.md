# Plan 009: Bulk-insert import + index-friendly date filters

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/google_sheets/import.rs src-tauri/src/commands.rs src-tauri/migrations/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: perf
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The sheet importer issues one `INSERT OR REPLACE` per row for `sheet_daily_balance`
(~365 calls per year-sheet imported) and at minimum 3–5 statements per imported
transaction row. A year with 400 rows therefore triggers ~1,600 round-trips inside a
single transaction, each paying the overhead of statement preparation. Replacing these
with batched multi-row `VALUES` clauses eliminates the per-row overhead and cuts
wall-clock import time proportionally.

Five date-filtered queries in `commands.rs` use `substr(t.date, 1, 4) = '2026'` to
match a year. SQLite cannot use the existing `idx_transaction_date` B-tree index for
that expression — it must scan every row and evaluate the function. Replacing those
predicates with `date >= '2026-01-01' AND date < '2027-01-01'` lets SQLite seek
directly into the index range, which matters when the transaction table grows across
multiple imported years.

## Current state

### Relevant files

- `src-tauri/src/google_sheets/import.rs` — importer; contains
  `import_rows_with_options` (line 199) and `store_balance_series` (line 739).
- `src-tauri/src/commands.rs` — Tauri command handlers and query helpers; contains
  five `substr(…date…, 1, 4) = ?` year predicates (lines 1162, 1923, 2003, 2221).
- `src-tauri/migrations/20240608000008_indexes_core.sql` — defines
  `idx_transaction_date ON "transaction"(date)`.
- `src-tauri/migrations/20240608000010_daily_checkin.sql` — defines
  `idx_daily_checkin_person_date ON daily_checkin(person_id, date)`.

### Excerpt A — `store_balance_series` row-by-row loop (import.rs:752–763)

```rust
    for b in series {
        sqlx::query(
            "INSERT OR REPLACE INTO sheet_daily_balance (sheet_name, date, balance_cents, is_projection) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(sheet_name)
        .bind(&b.date)
        .bind(b.balance_cents)
        .bind(b.is_projection as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert balance: {e}"))?;
    }
```

### Excerpt B — `import_rows_with_options` per-row INSERT (import.rs:266–280)

The `None` branch inside the `for row in rows` loop issues one INSERT per new row:

```rust
                sqlx::query(
                    "INSERT INTO \"transaction\" (id, type, amount, description, date, is_fixed, is_projection, source_amount, source_description, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?3, ?4, ?8, ?8)",
                )
                .bind(&txn_id)
                // … six more .bind() calls …
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("insert row {row:?}: {e}"))?;
```

The `Some` branch (update) always issues exactly one UPDATE. There is also one
`INSERT … ON CONFLICT DO UPDATE` into `sync_log` per row (import.rs:345–359).

### Excerpt C — year filter using `substr` (commands.rs:1162)

```rust
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE substr(t.date, 1, 4) = ?1",
```

Bound with `.bind(format!("{year:04}"))`. The index `idx_transaction_date` is
skipped because `substr()` wraps the indexed column.

### All five `substr(…date…, 1, 4)` occurrences (lines as of d183bbf)

| Line | Function                                       | SQL fragment                                                |
| ---- | ---------------------------------------------- | ----------------------------------------------------------- |
| 1162 | `load_year_events`                             | `WHERE substr(t.date, 1, 4) = ?1`                           |
| 1923 | `load_write_back_txns` (income/expense branch) | `WHERE substr(date, 1, 4) = ?1`                             |
| 2003 | `load_write_back_txns` (credit no-card branch) | `WHERE … AND substr(date,1,4) = ?1`                         |
| 2221 | `load_economia_by_month`                       | `WHERE substr(t.date, 1, 4) = ?1 AND t.type = 'transfer' …` |

There is also a `substr(date, 1, 4)` at line 2219 inside the same query
(`SELECT substr(t.date, 6, 2), …`) — that one is in the SELECT list and
GROUP BY, not the WHERE clause; leave it as-is or rewrite it to `strftime('%m', t.date)`
for consistency, but it is not a filter bug.

### Existing indexes (confirmed from migrations)

```sql
-- migration 20240608000008_indexes_core.sql
CREATE INDEX IF NOT EXISTS idx_transaction_date ON "transaction"(date);

-- migration 20240608000010_daily_checkin.sql
CREATE INDEX IF NOT EXISTS idx_daily_checkin_person_date ON daily_checkin(person_id, date);
```

`idx_transaction_date` is already present and is exactly the index that range
predicates (`date >= '2026-01-01' AND date < '2027-01-01'`) will use. No new
migration is needed for the transaction table.

`daily_checkin` queries at lines 756, 1378, 1403 filter by `date` only (no
`person_id`). The composite `(person_id, date)` index cannot be used for these
lookups without a leading `person_id` predicate. A standalone `idx_daily_checkin_date`
index would help — but `daily_checkin` is a sparse table (one row per day where a
check-in was recorded) and is read-mostly at O(days-in-range) scale, so the gain is
minimal compared to the transaction table. **Confirm the table size before adding an
index** (see Step 1 below). If `daily_checkin` has fewer than ~5,000 rows in the
target user's database, skip the index and document that in Maintenance notes.

### Repo conventions that apply here

- **Money is always integer cents.** `amount` in `transaction` is stored as an
  integer. Do not introduce floating-point at any point in the new batch code.
- **Amounts are positive magnitude.** The sign is carried by the `type` column
  (`income` / `expense` / `transfer`). Callers call `.abs()` before binding.
- **Functional-core / imperative-shell.** SQL helpers in `commands.rs` are pure
  query functions. Keep the bulk-insert logic inside the existing helpers rather
  than inventing a new abstraction layer.
- **No manual `memo` / `useMemo` / `useCallback`.** React Compiler is enabled —
  this rule applies to the frontend only and is not relevant here, but included for
  completeness in case you are also touching the frontend.
- **Prefer editing existing files over new abstractions.** Do not add new modules
  for these changes.
- **Style exemplar for SQLx query patterns** — model all new queries after the
  existing patterns in `src-tauri/src/google_sheets/import.rs` (use
  `sqlx::query(…).bind(…).execute(&mut *tx)`; no `query!` macro because the schema
  is dynamically migrated).
- **The existing transaction wrapper must not be duplicated.** `import_rows_with_options`
  already opens a single `pool.begin()` at line 226 and commits at line 394. The
  balance-series writer opens its own transaction at line 744. Do not nest transactions
  or add new `pool.begin()` calls inside these functions. Plan 002 will eventually
  wrap the outer call site; do not preempt that.

## Commands you will need

| Purpose                 | Command                                                    | Expected on success |
| ----------------------- | ---------------------------------------------------------- | ------------------- |
| Rust typecheck + clippy | `npm run rust:check`                                       | exit 0, no warnings |
| Rust unit tests only    | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass            |
| Full gate               | `npm run check`                                            | exit 0              |
| Lint                    | `npm run lint`                                             | exit 0              |
| Typecheck (frontend)    | `npm run typecheck`                                        | exit 0              |

`npm run rust:check` runs `cargo fmt --check`, `cargo clippy`, and `cargo test` — use
it as the single Rust gate.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/google_sheets/import.rs` — batch the balance-series insert loop.
- `src-tauri/src/commands.rs` — replace the four `substr(…date…, 1, 4) = ?` WHERE
  predicates with range predicates.
- `src-tauri/migrations/` — add one new migration **only if** you confirm in Step 1
  that a standalone `idx_daily_checkin_date` is warranted. Migration file name must
  follow the existing `YYYYMMDDNNNNNN_description.sql` pattern and sort after the
  newest existing migration (`20240612000010_drop_unused_fts.sql`).

**Out of scope** (do NOT touch, even though they look related):

- The outer transaction-wrapper call site — that belongs to plan 002.
- Any change to the `parse_rows_with_layout` logic or the `ImportedRow` struct.
- The `substr(date, 1, 7)` (month-level) filters at lines 513, 537, 583–584, 636.
  Those work correctly with a range-scan because the `date` index is still useful
  for the `date >= year_start` lower bound, and the `substr` in the GROUP BY
  (line 584) is in the SELECT list, not the WHERE clause.
- The `substr(t.date, 6, 2)` in the SELECT list at line 2219 — not a WHERE predicate.
- Frontend files, Tauri config, migrations unrelated to indexes.
- `plans/README.md` column widths or plan titles (only update your row's Status).

## Git workflow

- Branch: `advisor/009-bulk-insert-indexes`
- Commit style observed in `git log`: conventional commits, e.g.
  `perf: batch balance-series insert and fix year-filter indexes`
  Use one commit per logical unit (one for the insert batch, one for the filter fix,
  one for the migration if any).
- Do NOT push or open a PR unless instructed.

## Steps

### Step 0: Confirm `daily_checkin` row count

Before touching any file, check whether a `daily_checkin` date index is worthwhile.
Run:

```sql
-- Use sqlite3 on the local DB (path varies per installation):
-- SELECT COUNT(*) FROM daily_checkin;
```

Since this is a Tauri desktop app with a local SQLite file, you cannot query it
directly in CI. Make a judgment based on design: the table accumulates at most one row
per day per person. For a solo user with a few years of data that is at most ~1,000–
3,000 rows. At that scale a full-table scan is negligible. **Skip the migration** and
document this in Maintenance notes. Proceed to Step 1.

If you have evidence the table is materially larger (e.g. a test fixture with tens of
thousands of rows), you may add the migration — see the note in "Scope" above. The
step still proceeds to Step 1 without blocking.

**Verify**: No command — this is a design decision. Record your conclusion as a
one-line comment in the Maintenance notes section before moving on.

---

### Step 1: Batch the `store_balance_series` INSERT loop

Open `src-tauri/src/google_sheets/import.rs` and locate `store_balance_series`
(starts at line 739 as of d183bbf). The body currently loops over `series` and issues
one `INSERT OR REPLACE` per element.

Replace the loop with a chunk-batched approach. SQLite has a maximum of 32,766 bound
parameters per statement. With 4 parameters per row, the safe chunk size is
**`32766 / 4 = 8191`** — use `8000` as a round safe limit.

For each chunk, build a SQL string with the correct number of `VALUES (?,?,?,?)`
placeholders, then bind all values in order. Example shape (do not copy verbatim —
adapt to the actual variable names in the function):

```rust
    const CHUNK: usize = 8000;
    for chunk in series.chunks(CHUNK) {
        if chunk.is_empty() {
            continue;
        }
        let placeholders: String = (0..chunk.len())
            .map(|i| {
                let b = i * 4;
                format!("(?{}, ?{}, ?{}, ?{})", b + 1, b + 2, b + 3, b + 4)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT OR REPLACE INTO sheet_daily_balance \
             (sheet_name, date, balance_cents, is_projection) VALUES {placeholders}"
        );
        let mut q = sqlx::query(&sql);
        for b in chunk {
            q = q
                .bind(sheet_name)
                .bind(&b.date)
                .bind(b.balance_cents)
                .bind(b.is_projection as i64);
        }
        q.execute(&mut *tx)
            .await
            .map_err(|e| format!("bulk insert balance: {e}"))?;
    }
```

The `DELETE` before the loop (line 746) and the `tx.commit()` after (line 765) must
remain unchanged.

**Verify**: `npm run rust:check` → exit 0, no warnings.

---

### Step 2: Write a regression test for `store_balance_series` batching

Add a `#[tokio::test]` inside the existing `mod tests` block in `import.rs` (after
line 881). The test must:

1. Create an in-memory SQLite pool and run migrations — model after `fixture_pool()`
   in `commands.rs:2822–2831`.
2. Build a `series` vec with **400 elements** (one per day of a year — more than a
   single small batch to exercise the loop).
3. Call `store_balance_series(&pool, "Jan", &series).await` — expect `Ok(400)`.
4. Query `SELECT COUNT(*) FROM sheet_daily_balance WHERE sheet_name = 'Jan'` and
   assert it equals 400.
5. Call `store_balance_series` again with an empty slice — expect `Ok(0)` and the
   table still has 400 rows (delete + commit with no inserts must not corrupt).

This test guards against the "empty chunk panics" edge case and the "re-import
replaces correctly" invariant.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked store_balance_series` → 1 test passes.

---

### Step 3: Replace `substr(date, 1, 4) = ?` in `load_year_events`

Open `src-tauri/src/commands.rs`. Locate `load_year_events` (line 1157).

Current WHERE clause (line 1162):

```sql
WHERE substr(t.date, 1, 4) = ?1
```

Bound with `.bind(format!("{year:04}"))`.

Replace with:

```sql
WHERE t.date >= ?1 AND t.date < ?2
```

And change the bind calls to:

```rust
.bind(format!("{year:04}-01-01"))
.bind(format!("{}-01-01", year + 1))
```

The query result set is identical: only dates whose first four characters equal the
year string satisfy the range, and all valid ISO-8601 dates in the transaction table
satisfy that structure by the parser invariant at the import boundary.

**Verify**: `npm run rust:check` → exit 0.

---

### Step 4: Replace `substr(date, 1, 4) = ?` in `load_write_back_txns` (income/expense branch)

Still in `commands.rs`. Locate `load_write_back_txns` (line 1919). The first query
(line 1921–1928):

```sql
SELECT type, date, amount, is_fixed FROM "transaction"
WHERE substr(date, 1, 4) = ?1
  AND NOT (type='expense' AND payment_method='credit')
```

Bound with `.bind(format!("{year:04}"))`.

Replace `WHERE substr(date, 1, 4) = ?1` with `WHERE date >= ?1 AND date < ?2` and
update the binds:

```rust
.bind(format!("{year:04}-01-01"))
.bind(format!("{}-01-01", year + 1))
```

**Verify**: `npm run rust:check` → exit 0.

---

### Step 5: Replace `substr(date,1,4) = ?` in `load_write_back_txns` (credit no-card branch)

Still in `commands.rs`. Locate the credit no-card branch inside `load_write_back_txns`
(line 2001–2008):

```sql
SELECT date, amount FROM "transaction"
WHERE type='expense' AND payment_method='credit' AND substr(date,1,4) = ?1
```

Bound with `.bind(format!("{year:04}"))`.

Replace with the same range pattern:

```sql
WHERE type='expense' AND payment_method='credit' AND date >= ?1 AND date < ?2
```

Binds:

```rust
.bind(format!("{year:04}-01-01"))
.bind(format!("{}-01-01", year + 1))
```

**Verify**: `npm run rust:check` → exit 0.

---

### Step 6: Replace `substr(t.date, 1, 4) = ?` in `load_economia_by_month`

Still in `commands.rs`. Locate `load_economia_by_month` (line 2217). The query
(lines 2218–2223):

```sql
SELECT substr(t.date, 6, 2), COALESCE(SUM(ABS(t.amount)), 0) FROM "transaction" t
LEFT JOIN account a ON a.id = t.to_account_id
WHERE substr(t.date, 1, 4) = ?1 AND t.type = 'transfer'
  AND a.liquidity IN ('reserve','illiquid')
GROUP BY substr(t.date, 6, 2)
```

Replace only the WHERE predicate `substr(t.date, 1, 4) = ?1` with
`t.date >= ?1 AND t.date < ?2`. Leave the `substr(t.date, 6, 2)` in the SELECT and
GROUP BY unchanged — those are not WHERE filters and do not prevent index use.

Updated binds:

```rust
.bind(format!("{year:04}-01-01"))
.bind(format!("{}-01-01", year + 1))
```

**Verify**: `npm run rust:check` → exit 0.

---

### Step 7: Write a unit test confirming year-range semantics

Add a `#[tokio::test]` to `mod tests` in `commands.rs` (after the last existing test).
The test must:

1. Create a `fixture_pool()`.
2. Insert two transactions via raw SQL:
   - `('t1', 'income', 100000, '2026-06-15', 0, 0)` (in year 2026)
   - `('t2', 'income', 200000, '2027-01-01', 0, 0)` (boundary: first day of 2027,
     must NOT appear in 2026 results)
3. Call `load_year_events(&pool, 2026).await` and assert the result contains exactly
   one event with `amount == 100000`.
4. Call `load_year_events(&pool, 2027).await` and assert exactly one event with
   `amount == 200000`.

This is a regression test for the off-by-one at the year boundary (the exclusive upper
bound `< '2027-01-01'` correctly excludes 2027-01-01, unlike `substr(…) = '2026'` which
would have excluded it anyway — but the test documents the contract explicitly).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked load_year_events` → at least 1 test passes.

---

### Step 8: Grep-confirm no remaining `substr.*date.*1, 4` year predicates in WHERE

Run:

```
grep -rn "substr.*date.*1, 4\|substr.*date.*1,4" src-tauri/src/commands.rs src-tauri/src/google_sheets/import.rs
```

Expected: zero matches in WHERE contexts. (You may see matches in SELECT or GROUP BY
clauses — those are not filter bugs and are intentionally left unchanged.)

If any matches remain, fix them before proceeding.

**Verify**: Command above → 0 matches in WHERE predicates (manually inspect any
remaining matches to confirm they are in SELECT/GROUP BY only).

---

### Step 9: Full gate

Run the complete quality gate:

```
npm run check
```

Expected: exit 0, no errors, no lint warnings, all tests pass.

**Verify**: `npm run check` → exit 0.

---

### Step 10: Update `plans/README.md`

Change the Status cell for plan 009 from `TODO` to `DONE`.

**Verify**: `git diff plans/README.md` shows only the Status change for row 009.

## Test plan

### New tests to write

1. **`store_balance_series_batches_400_rows`** (in `import.rs`, inside `mod tests`):
   - Happy path: 400-element series is stored, COUNT(\*) = 400.
   - Re-import: calling again with empty slice leaves 400 rows intact.
   - Pattern: model after `fixture_pool()` in `commands.rs:2822–2831`; use
     `sqlx::migrate!("./migrations").run(&pool)` to bootstrap schema.

2. **`load_year_events_year_boundary`** (in `commands.rs`, inside `mod tests`):
   - 2026-06-15 is in 2026.
   - 2027-01-01 is in 2027, not in 2026.

### Existing tests to keep green

- All existing tests in `import.rs::tests` (lines 881–end) — none touch
  `store_balance_series`, but the migration-run in the new test fixture must not break
  them.
- All existing `#[tokio::test]` in `commands.rs::tests` — the year-filter change must
  not alter their expected outputs (they use dates like `2026-01-01` / `2026-06-15`
  which are correctly within the year range).

**Verification command**: `cargo test --manifest-path src-tauri/Cargo.toml --locked` → all pass.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (fmt, clippy, tests).
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; new tests
      `store_balance_series_batches_400_rows` and `load_year_events_year_boundary` exist
      and pass.
- [ ] `grep -rn "substr.*date.*1, 4\|substr.*date.*1,4" src-tauri/src/commands.rs src-tauri/src/google_sheets/import.rs`
      returns zero matches in WHERE clauses.
- [ ] `npm run check` exits 0 (full gate including frontend typecheck, lint, privacy scan).
- [ ] `git diff --name-only` shows only files from the in-scope list (import.rs,
      commands.rs, optionally one new migration, and plans/README.md).
- [ ] `plans/README.md` row for plan 009 shows `DONE`.

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the excerpted locations does not match what is live in the file (drift
  since d183bbf). The drift check command at the top of this file will reveal this.
- `npm run rust:check` fails after a step with an error you cannot resolve in one
  targeted fix attempt. Do not accumulate broken states across steps.
- The SQLite parameter limit (32,766 per statement) appears to be causing a runtime
  error in the batch insert. The chunk size of 8,000 (× 4 params = 32,000) should be
  well within bounds, but if an assertion fires, stop and report rather than
  increasing the chunk size without understanding the cause.
- Any step requires modifying `src-tauri/src/forecast/` or any other file outside
  the in-scope list — stop and report; the advisory session did not anticipate that
  dependency.
- The year-filter replacement changes the output of any existing test (the semantics
  are identical for well-formed ISO-8601 dates, but if a test fixture uses malformed
  dates like `"20260615"` or `"2026/06/15"`, the range will behave differently from
  `substr` — stop and report rather than adjusting the fixture silently).

## Maintenance notes

- **`daily_checkin` index decision**: as of plan authoring, `daily_checkin` is a
  sparse table (at most one row per day per user, so O(years × 365) rows for a solo
  user). A full scan is negligible at that scale. No standalone date index was added.
  If the app ever supports multi-household data or daily automated check-ins at high
  frequency, revisit adding `CREATE INDEX idx_daily_checkin_date ON daily_checkin(date)`.
- **Year+1 arithmetic**: `year + 1` in Rust is `i32` addition — this works for any
  year representable in the database (practical range 2000–2100). No overflow risk.
- **Plan 002 coordination**: plan 002 will wrap the outer import call site in an
  additional transaction. That plan should be applied on top of this one; the batch
  insert logic inside `store_balance_series` already opens and commits its own
  transaction, which plan 002 may or may not collapse (verify at that time).
- **Reviewer focus areas in PR**:
  - Confirm the `year + 1` exclusive upper bound is correct (i.e., `< '2027-01-01'`
    does not drop the last day of 2026).
  - Confirm the chunking math in `store_balance_series` (chunk_size × params_per_row
    ≤ 32,766 at all times).
  - Confirm that `substr(t.date, 6, 2)` in the SELECT/GROUP BY of `load_economia_by_month`
    was intentionally left unchanged.
- **Future work explicitly deferred**: batching the per-row transaction INSERT
  (`import_rows_with_options` None branch, lines 266–280) was considered. Unlike the
  balance series — which is replace-all with uniform rows — the transaction import
  interleaves INSERT for new rows with UPDATE for existing rows and conflict recording,
  making a simple multi-row VALUES batch unsafe without restructuring the merge loop.
  Deferring to a later plan (post-plan 004, when the merge logic is stable).
