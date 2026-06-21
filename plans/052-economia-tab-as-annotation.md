# Plan 052: Complete economia=Saída — Economia tab as a metric annotation (no Saldo double-count; Economizado% source)

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
> git diff --stat 5ada3ae..HEAD -- \
>   src-tauri/src/commands/write_back_cmds.rs \
>   src-tauri/src/commands/sheets_import.rs \
>   src-tauri/src/commands/forecast_cmds.rs \
>   src-tauri/src/forecast/mod.rs \
>   src-tauri/migrations/
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M-L
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Package**: H
- **Planned at**: commit `5ada3ae`, 2026-06-21

## Why this matters

The owner's model is: savings is recorded as a grid **Saída** (expense row → `FixedOut`/`Daily` →
already in `cost_of_living` → hits Performance and Saldo **once**). The Economia spreadsheet tab
is the owner's **manual % annotation** (`Economizado% = Economia/Entradas`), not a second money
movement. Today the Economia tab is empty, so these bugs are dormant. The moment the owner fills
any Economia-tab cell, `store_economia_entries` inserts a `type='transfer'` row keyed
`economia:YYYY-MM` pointing to the reserve account. `load_forecast_events` and
`load_realized_month_events` both load transfers to reserve as `EventKind::Economia`; `signed()`
returns `−amount` for `EventKind::Economia`. Because the **same savings** is already a grid Saída
(counted in Saldo via `cost_of_living`), the Saldo will subtract it a second time. Additionally,
`savings_rate_bps` and `realized_annual_economia` read only from transfers-to-reserve, so a user
saving purely through grid Saídas (no explicit reserve transfer) will show Economizado% = 0.

Three fixes together close all three bugs without changing Performance, the manual reserve-transfer
flow (plan 003), or any other model decisions.

## Current state

### Model decision (locked since plan 051)

From `plans/README.md` lines 115–119:

> Performance formula in `forecast/mod.rs` — DECISION LOCKED, FINAL via plans 040 → 046 → 051.
> Formula: `income − cost_of_living`. Economia is NOT a separate deduction: the savings expense row
> is already in `cost_of_living`; the Economia-tab transfer is a savings-rate annotation that feeds
> `savings_rate_bps` only, not Performance. Do NOT re-add `− economia` to the formula.

### Bug 1 (P0-dormant): Saldo double-count via `store_economia_entries`

`src-tauri/src/commands/write_back_cmds.rs` lines 1008–1065 — `store_economia_entries` writes a
`type='transfer'` row into the `transaction` table for every non-zero Economia-tab entry:

```rust
// write_back_cmds.rs:1039-1050
sqlx::query(
    "INSERT INTO \"transaction\" (id, type, amount, description, date, to_account_id, is_projection, created_at, updated_at) \
     VALUES (?1, 'transfer', ?2, 'Economia (importada da aba Economia)', ?3, ?4, ?5, ?6, ?6) \
     ON CONFLICT(id) DO UPDATE SET amount=excluded.amount, date=excluded.date, \
       is_projection=excluded.is_projection, updated_at=excluded.updated_at",
)
.bind(&id)      // "economia:YYYY-MM"
.bind(cents)
.bind(&date)
.bind(reserve)  // to_account_id = reserve account
```

`load_cashflow_events` (`forecast_cmds.rs:584-616`) loads all future transactions including
`type='transfer'` rows; `map_cashflow_row` → `classify()` (`forecast/mod.rs:255-261`) maps
`transfer + to_account liquidity='reserve'` → `EventKind::Economia`. `signed()` returns
`−e.amount_cents` for `EventKind::Economia` (`forecast/mod.rs:228`):

```rust
// forecast/mod.rs:222-229
fn signed(e: &CashflowEvent) -> i64 {
    match e.kind {
        EventKind::Income => e.amount_cents,
        EventKind::FixedOut | EventKind::Daily | EventKind::Economia => -e.amount_cents,
    }
}
```

`load_realized_month_events` (`forecast_cmds.rs:648-674`) similarly loads realized transactions
including transfers. **Result**: when the Economia tab is non-empty, the same savings amount is
subtracted from the Saldo twice — once from the grid Saída already recorded, once from the
`economia:YYYY-MM` transfer row.

### Bug 2 (P1): Economizado% = 0 for grid-Saída savers

`forecast/mod.rs` lines 356–406 — `economia` is accumulated only from `EventKind::Economia`
events (transfers to reserve); `savings_rate_bps = economia * 10_000 / income`:

```rust
// forecast/mod.rs:356-406
let mut economia = 0i64;
// ...
EventKind::Economia => economia += e.amount_cents,
// ...
let savings_rate_bps = if income > 0 {
    economia * 10_000 / income
} else {
    0
};
```

When there is no `EventKind::Economia` event (Economia tab empty, owner saves via grid Saída only),
`economia = 0` → `savings_rate_bps = 0` → Economizado% shows 0, diverging from the spreadsheet.

`realized_annual_economia` (`forecast_cmds.rs:158-198`) has the same issue — it queries only
`type='transfer'` rows to reserve:

```rust
// forecast_cmds.rs:181-196
let row: (i64,) = sqlx::query_as(
    "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
     LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date >= ?1 AND t.date < ?2 \
       AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid') \
       ...",
)
```

### Bug 3 (P1): safe_to_spend guardrail over-constrains grid-Saída savers

`forecast_cmds.rs` lines 813–822 — `annual_economia` is sourced from `realized_annual_economia`
(transfers to reserve only). This is passed directly to `forecast::safe_to_spend_today` as the
savings numerator for the guardrail:

```rust
// forecast_cmds.rs:816-823
let annual_economia = realized_annual_economia(pool, today_naive).await?;
let sts = forecast::safe_to_spend_today(
    &fc,
    annual_income,
    annual_economia,      // ← 0 when no reserve-transfers exist
    SAVINGS_TARGET_BPS,
    reserve_floor_cents,
);
```

When `annual_economia = 0` and `annual_savings_amt > 0` (there IS real surplus — the owner is
saving via grid Saídas — but no reserve-transfer rows), the savings guardrail fires with the wrong
numerator and may incorrectly constrain `safe_to_spend_today`.

### Callers of `store_economia_entries`

Two callers, both in `src-tauri/src/commands/sheets_import.rs`:

1. Line 307 — XLSX import path (inside `import_xlsx` command, Economia sheet branch)
2. Line 500 — live Sheets API import path (`import_economia_sheet` command)

### Existing reserve-transfer flow (plan 003 — keep as-is)

The manual "Economia" transaction type (plan 003) creates `type='transfer'` rows pointing to a
reserve/illiquid account when the user manually records a reserve deposit inside Neko. This is a
**real money movement** and correctly hits the Saldo + counts as savings. It is **distinct** from
the Economia-tab annotation and must remain unchanged. The new `economia_annotation` table covers
only spreadsheet-tab imports, not manual transfers.

### Existing write-back audit test (keep passing)

`write_back_cmds.rs` lines 1222–1280 (`economia_write_back_audit_realigns_source_amount`) tests
the round-trip of `economia:YYYY-MM` transaction ids. After this plan, those ids go away (the
annotation table uses its own PK). The write-back audit path must be updated to match the new
storage shape (see Step 5 below).

### Migration naming convention

Existing migrations follow `YYYYMMDDNNNNNN_<slug>.sql` (numeric timestamp prefix). The latest file
is `20260621000003_line_item_section.sql`. New migration: `20260621000004_economia_annotation.sql`.

### Repo conventions

- Functional-core/imperative-shell: pure calculations in `forecast/mod.rs`; IO in
  `commands/forecast_cmds.rs` and `commands/write_back_cmds.rs`.
- SQLite migrations at `src-tauri/migrations/`; picked up by `sqlx::migrate!("./migrations")`.
- Error handling: `Result<T, String>` at the command boundary; `?` propagation inside.
- Exemplar for DB helper functions: `ensure_reserve_account` at `write_back_cmds.rs:980-1006`.
- Exemplar for migration + test pattern: `forecast_cmds.rs:1245-1380` (in-memory pool, `sqlx::migrate!`).

## Commands you will need

| Purpose          | Command                                                             | Expected on success    |
| ---------------- | ------------------------------------------------------------------- | ---------------------- |
| Rust check+test  | `npm run rust:check`                                                | exit 0, all tests pass |
| Full gate        | `npm run check`                                                     | exit 0                 |
| Typecheck only   | `npm run typecheck`                                                 | exit 0, no errors      |
| Unit tests only  | `npm run test:run`                                                  | exit 0, all pass       |
| Lint             | `npm run lint`                                                      | exit 0                 |
| Rust test filter | `cargo test -p neko-finance-lib economia_annot` (from `src-tauri/`) | named tests pass       |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/migrations/20260621000004_economia_annotation.sql` (create)
- `src-tauri/src/commands/write_back_cmds.rs` — replace `store_economia_entries` body; remove
  `ensure_reserve_account` call from that function (keep `ensure_reserve_account` itself, still used
  by plan 003's manual-transfer flow if present; confirm by grepping before removing)
- `src-tauri/src/commands/forecast_cmds.rs` — update `realized_annual_economia` to read from
  annotation table + real reserve-transfers; update `annual_savings` DTO wiring
- `src-tauri/src/forecast/mod.rs` — update `month_metrics_for` / `project_with_metrics` to accept
  and use annotation amounts for `savings_rate_bps` + `economia_cents`; **leave `signed()` and
  Performance formula completely untouched**
- `src-tauri/src/commands/sheets_import.rs` — update callers of `store_economia_entries` (no
  signature change needed if the function itself handles the new table)

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/forecast/mod.rs` lines 222–229 (`signed()`) — do NOT change. Economia real
  transfers (plan 003 manual flow) still correctly subtract from Saldo via `signed()`. Only the
  annotation-imported rows stop hitting the transaction table.
- `src-tauri/src/forecast/mod.rs` lines 377–384 (Performance formula) — do NOT change. Decision
  LOCKED per plan 051.
- The manual reserve-transfer path (plan 003) — any `type='transfer'` row pointing to a
  `reserve`/`illiquid` account that was manually created by the user still counts as a real money
  movement and should continue to flow through `EventKind::Economia` → `signed()` → Saldo.
- Frontend TypeScript files — `economia_cents` and `savings_rate_bps` field names on the DTO are
  unchanged; the shape shifts only in value, not in keys.
- `src-tauri/src/google_sheets/write_back.rs` — the `plan_economia_write_back` function reads the
  Economia-tab values to plan a write-back; it does not need to change. The audit-realign path
  (which matched `id = economia:YYYY-MM`) does need updating — see Step 5.

## Git workflow

- Branch: `advisor/052-economia-annotation`
- Commit style (match repo): `fix: <present-tense description> (plano 052)` — e.g.
  `fix: economia tab → annotation table, remove Saldo double-count (plano 052)`
- One logical commit per step is fine; squash at PR time if preferred.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Write the forward migration

Create `src-tauri/migrations/20260621000004_economia_annotation.sql`:

```sql
-- Plan 052: Economia-tab values are a metric annotation (% = Economia/Entradas),
-- not a money movement. Storing them in `transaction` caused a Saldo double-count
-- when the same savings was already a grid Saída. This table holds the raw
-- annotation values; it does NOT affect the Saldo chain.
CREATE TABLE IF NOT EXISTS economia_annotation (
    profile_id   TEXT    NOT NULL DEFAULT '',
    year         INTEGER NOT NULL,
    month        INTEGER NOT NULL CHECK (month BETWEEN 1 AND 12),
    amount_cents INTEGER NOT NULL CHECK (amount_cents >= 0),
    updated_at   TEXT    NOT NULL,
    PRIMARY KEY (profile_id, year, month)
);
```

Notes:

- `profile_id` defaults to `''` (empty string) to match the single-profile app with no join
  complexity. When multi-profile support is added, a migration can backfill with real profile ids.
- `amount_cents >= 0`: annotation values are magnitudes (the spreadsheet column holds positive
  numbers); zero means "no savings recorded for this month" and can coexist with a row (explicit
  zero from the sheet = "owner saw the month and saved nothing").
- No `is_projection` column: the annotation is a static historical annotation from the sheet,
  not a projected cashflow event.

**Verify**: `npm run rust:check` → exit 0 (migration is picked up by `sqlx::migrate!` in all
test pools; if the file has a syntax error, any test using `sqlx::migrate!` will fail with a
migration error).

### Step 2: Replace `store_economia_entries` to write the annotation table

In `src-tauri/src/commands/write_back_cmds.rs`, replace the body of
`store_economia_entries` (lines 1008–1065). The new body writes to `economia_annotation`
instead of `transaction`. The function signature stays identical:

```rust
pub(crate) async fn store_economia_entries(
    pool: &SqlitePool,
    entries: &[(i32, u32, i64)],  // (year, month, amount_cents)
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let mut count = 0usize;

    for (year, month, cents) in entries {
        if *cents > 0 {
            sqlx::query(
                "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
                 VALUES ('', ?1, ?2, ?3, ?4) \
                 ON CONFLICT(profile_id, year, month) DO UPDATE SET \
                   amount_cents=excluded.amount_cents, updated_at=excluded.updated_at",
            )
            .bind(year)
            .bind(*month as i64)
            .bind(cents)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("upsert annotation {year}-{month:02}: {e}"))?;
        } else {
            // Zero/blank cell = owner removed the annotation; delete the row.
            sqlx::query(
                "DELETE FROM economia_annotation WHERE profile_id='' AND year=?1 AND month=?2",
            )
            .bind(year)
            .bind(*month as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("delete annotation {year}-{month:02}: {e}"))?;
        }
        count += 1;
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(count)
}
```

Also remove the old `ensure_reserve_account` call that existed inside the previous body.
Before removing, grep to confirm `ensure_reserve_account` has no other callers left that depend
on the `store_economia_entries` code path:

```
grep -rn "ensure_reserve_account" src-tauri/src/
```

If `ensure_reserve_account` is still called elsewhere (e.g. from the manual Economia transfer
created by plan 003), keep the function. If it is now dead code, delete it too. Either way,
confirm the grep result before deciding.

Also remove the old `economia_write_back_audit_realigns_source_amount` test (lines 1230–1280 in
`write_back_cmds.rs`) — it tests the now-gone `economia:YYYY-MM` transaction round-trip. A
replacement test is added in Step 6.

**Verify**: `npm run rust:check` → exit 0. The two callers of `store_economia_entries` in
`sheets_import.rs` still compile because the function signature is unchanged.

### Step 3: Update `realized_annual_economia` to include the annotation

In `src-tauri/src/commands/forecast_cmds.rs`, replace the body of `realized_annual_economia`
(lines 158–198). The new implementation sums:

1. The `economia_annotation` table for the same date window (the owner's sheet annotation).
2. Real reserve/illiquid transfers (`type='transfer'`, `a.liquidity IN ('reserve','illiquid')`)
   that were created manually by the user in Neko (plan 003 flow). These are disjoint from the
   annotation: the annotation table only holds rows written by `store_economia_entries` (sheet
   import); manual transfers never appear there.

Both sides cover the same "complete months" window. Keep the January-window shift identical to the
existing logic so the guardrail stays symmetric with `realized_annual_savings`.

```rust
pub(crate) async fn realized_annual_economia(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Same window as realized_annual_savings — keep symmetric (Jan shift to prior December).
    let is_january = cur_ym == format!("{}-01", today_naive.year());
    let (lower_date, upper_date, lower_ym, lower_m, lower_y) = if is_january {
        let prev = today_naive.year() - 1;
        (
            format!("{prev}-12-01"),
            format!("{}-01-01", today_naive.year()),
            format!("{prev}-12"),
            12i64,
            prev as i64,
        )
    } else {
        (
            format!("{}-01-01", today_naive.year()),
            format!("{cur_ym}-01"),
            format!("{}-01", today_naive.year()),
            1i64,
            today_naive.year() as i64,
        )
    };

    // Side A: annotation from the Economia tab (sheet import via store_economia_entries).
    // Window = complete months: year/month < current year-month (or the Jan-shifted window).
    // For the non-January case: months from January (month=1) up to but not including cur_month.
    // For the January case: only December of the prior year (month=12, year=prev).
    let annotation_sum: (i64,) = if is_january {
        sqlx::query_as(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM economia_annotation \
             WHERE year = ?1 AND month = 12",
        )
        .bind(today_naive.year() as i64 - 1)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("annotation economia (jan): {e}"))?
    } else {
        let cur_month = today_naive.month() as i64;
        sqlx::query_as(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM economia_annotation \
             WHERE year = ?1 AND month < ?2",
        )
        .bind(today_naive.year() as i64)
        .bind(cur_month)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("annotation economia: {e}"))?
    };

    // Side B: real manual reserve transfers created by the user in Neko (plan 003 flow).
    // These are regular transaction rows; the same exclude_from_totals guard applies.
    let transfer_sum: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid') \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&lower_date)
    .bind(&upper_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized economia transfers: {e}"))?;

    Ok(annotation_sum.0 + transfer_sum.0)
}
```

Important: `annotation_sum.0 + transfer_sum.0` is a correct sum without double-count because:

- `economia_annotation` rows come only from `store_economia_entries` (sheet import).
- Manual reserve transfers are `type='transfer'` rows in `transaction`, never in
  `economia_annotation`.
- There is no path that writes the same savings to both tables simultaneously.

**Verify**: `npm run rust:check` → exit 0.

### Step 4: Wire annotation into `month_metrics_for` / `project_with_metrics`

The `savings_rate_bps` and `economia_cents` fields in `MonthMetric` currently depend on
`EventKind::Economia` events accumulated per month. For the annotation path, there are no
`EventKind::Economia` events (since `store_economia_entries` no longer writes `transaction` rows).

The cleanest approach is to pass the annotation amounts as an additional argument to the functions
that compute per-month metrics. Two options — choose the one that fits the existing call sites:

**Option A (preferred — minimal surface change)**: pass a `&HashMap<(i32, u32), i64>` (keyed by
`(year, month)`, values in cents) to `month_metrics_for` and `project_with_metrics`. Inside the
function body, after accumulating `EventKind::Economia` from events (which still captures manual
reserve transfers), **add** the annotation value for the same `(year, month)` key. The two sources
are additive and disjoint.

In `forecast/mod.rs`, update the `month_metrics_for` signature:

```rust
pub fn month_metrics_for(
    today: NaiveDate,
    events: &[CashflowEvent],
    months: &[(i32, u32)],
    annotation: &std::collections::HashMap<(i32, u32), i64>,  // NEW
) -> Vec<MonthMetric>
```

Inside the accumulation loop, after `EventKind::Economia => economia += e.amount_cents`:

```rust
// Add annotation amount for this month (from the Economia tab import).
// Manual reserve transfers are already counted via EventKind::Economia above.
economia += annotation.get(&(year, month)).copied().unwrap_or(0);
```

Do the same for `project_with_metrics` if it also accumulates `economia` per month (check the
function — it likely delegates to `month_metrics_for` or duplicates the loop; adapt accordingly).

Update all call sites of `month_metrics_for` in `forecast_cmds.rs`:

- `annual_metrics` (line ~1004): load annotation from DB, build the `HashMap`, pass it.
- Any other call site — grep: `grep -n "month_metrics_for\|project_with_metrics"
src-tauri/src/commands/forecast_cmds.rs`

For `annual_metrics`, add a helper query before the call:

```rust
let annotation_rows: Vec<(i64, i64)> = sqlx::query_as(
    "SELECT month, amount_cents FROM economia_annotation WHERE year = ?1",
)
.bind(year as i64)
.fetch_all(pool)
.await
.map_err(|e| format!("annotation for year {year}: {e}"))?;

let annotation: std::collections::HashMap<(i32, u32), i64> = annotation_rows
    .into_iter()
    .map(|(m, c)| ((year, m as u32), c))
    .collect();
```

For `dashboard_summary` / `get_dashboard_summary` (wherever `project_with_metrics` is called),
load the annotation similarly for the current year.

**Verify**: `npm run rust:check` → exit 0. The `forecast/mod.rs` unit tests for `savings_rate_bps`
(e.g. `T5.2` at line ~827) will need updating: pass `&HashMap::new()` (or a populated map) as the
new `annotation` argument. Update those test call sites; the expected values stay the same.

### Step 5: Update write-back audit for the new storage shape

In `src-tauri/src/google_sheets/write_back.rs`, find the `economia` audit branch that currently
realigns `source_amount` on rows matched by `id = 'economia:YYYY-MM'`. After this plan, there are
no `economia:YYYY-MM` rows in `transaction`. The write-back audit for the Economia tab should
instead record the annotation write in a way that prevents false conflict on re-import.

Two sub-options:

- If the write-back audit for Economia only exists to prevent the staleness conflict (no other
  side effects), and the annotation table naturally has no `source_amount` concept, simply
  **remove the economia branch from the write-back audit** and ensure `store_economia_entries`'s
  upsert logic (ON CONFLICT DO UPDATE) naturally handles re-imports idempotently.
- If the write-back audit records the proposed value for conflict detection, add a parallel
  `economia_annotation` upsert inside the audit path instead.

Grep first to understand the scope:

```
grep -n "economia" src-tauri/src/google_sheets/write_back.rs | head -40
```

Adapt minimally. Do not introduce a new `source_amount`-style staleness mechanism for the
annotation table unless the existing write-back conflict logic specifically requires it.

**Verify**: `npm run rust:check` → exit 0.

### Step 6: Add regression tests proving all three bugs are fixed

Add tests in `src-tauri/src/commands/write_back_cmds.rs` (for Bug 1) and
`src-tauri/src/commands/forecast_cmds.rs` (for Bugs 2 and 3). Model after the existing
in-memory pool pattern at `forecast_cmds.rs:1245-1253`.

**Test A — Bug 1 regression (no Saldo double-count)**

In `write_back_cmds.rs` tests:

```
// After store_economia_entries writes an annotation for 2026-06 with 50000 cents:
// - transaction table has NO row with id='economia:2026-06'
// - economia_annotation table has a row (year=2026, month=6, amount_cents=50000)
// - A forecast built from events loaded by load_cashflow_events sees NO EventKind::Economia
//   event for this annotation (the Saldo is not affected).
```

Concretely:

1. Create in-memory pool, run migrations.
2. Call `store_economia_entries(&pool, &[(2026, 6, 50000)]).await`.
3. Assert: `SELECT COUNT(*) FROM "transaction" WHERE id='economia:2026-06'` returns 0.
4. Assert: `SELECT amount_cents FROM economia_annotation WHERE year=2026 AND month=6` returns 50000.
5. Call `load_cashflow_events(&pool, today, horizon).await` with a date in the future.
6. Assert the returned events contain no `EventKind::Economia` event for June 2026 (the annotation
   must not appear as a cashflow event).

**Test B — Bug 2 regression (Economizado% reflects annotation)**

In `forecast_cmds.rs` tests:

```
// When annotation has 25000 cents for 2026-03 and income is 100000 cents:
// savings_rate_bps for March should be 2500 (25%).
```

Concretely:

1. In-memory pool, run migrations.
2. Insert `economia_annotation (year=2026, month=3, amount_cents=25000)`.
3. Insert an income transaction for 2026-03 of 100000 cents.
4. Build the annotation HashMap for year 2026.
5. Call `month_metrics_for(today, &events, &[(2026, 3)], &annotation)`.
6. Assert `metrics[0].savings_rate_bps == 2500` and `metrics[0].economia_cents == 25000`.

**Test C — Bug 3 regression (realized_annual_economia includes annotation)**

In `forecast_cmds.rs` tests:

```
// realized_annual_economia sums annotation + real transfers, not just transfers.
```

Concretely:

1. In-memory pool, run migrations.
2. Insert `economia_annotation` rows for Jan–May 2026 (5 months × 10000 cents each).
3. Insert a manual reserve transfer for 2026-03 of 8000 cents (plan 003 flow: account with
   liquidity='reserve', transaction type='transfer').
4. Call `realized_annual_economia(&pool, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()).await`.
5. Assert result = 5 × 10000 + 8000 = 58000 cents (annotation + manual transfer, no double-count).

**Test D — annotation and real transfer are disjoint (no double-count)**

Verify that calling `store_economia_entries` does NOT create a `transaction` row that would be
picked up by `realized_annual_economia`'s transfer-side query:

1. In-memory pool.
2. Call `store_economia_entries(&pool, &[(2026, 3, 20000)]).await`.
3. Call `realized_annual_economia` with a date in June 2026.
4. Assert result = 20000 (annotation only, no phantom transfer).
5. Also assert `SELECT COUNT(*) FROM "transaction" WHERE type='transfer'` returns 0.

**Verify**: `cargo test -p neko-finance-lib economia_annot 2>&1` (from `src-tauri/`) → all 4 new
tests pass. Then `npm run rust:check` → exit 0.

### Step 7: Final gate

**Verify**: `npm run check` → exit 0 (covers typecheck + lint + unit tests + rust:check + privacy
scan).

## Test plan

New tests to add (all in the `#[cfg(test)]` blocks of the files being changed):

| Test name                                      | File                 | Covers                       |
| ---------------------------------------------- | -------------------- | ---------------------------- |
| `annotation_does_not_create_transaction_row`   | `write_back_cmds.rs` | Bug 1: no Saldo entry        |
| `annotation_not_loaded_as_cashflow_event`      | `write_back_cmds.rs` | Bug 1: no Saldo deduction    |
| `savings_rate_reflects_annotation`             | `forecast_cmds.rs`   | Bug 2: Economizado% non-zero |
| `realized_annual_economia_includes_annotation` | `forecast_cmds.rs`   | Bug 3: guardrail correct     |
| `annotation_and_transfer_no_double_count`      | `forecast_cmds.rs`   | Bugs 1+3: additive, disjoint |

Structural pattern: model after `forecast_cmds.rs:1245-1380` (in-memory pool + `sqlx::migrate!`).

Existing tests to update (signature change in `month_metrics_for`):

- All call sites of `month_metrics_for` in `forecast/mod.rs` unit tests (the `T5.2` block around
  line 827) must pass `&HashMap::new()` (or a populated map) as the new `annotation` arg.

**Verification command**: `npm run rust:check` → exit 0, 5 new tests pass, 0 existing tests
regress.

## Done criteria

- [ ] `npm run rust:check` exits 0; 5 new tests pass and all pre-existing tests pass
- [ ] `npm run check` exits 0
- [ ] `SELECT COUNT(*) FROM "transaction" WHERE id LIKE 'economia:%'` on a fresh import of a
      non-empty Economia tab returns 0 (no `economia:YYYY-MM` rows in the transaction table)
- [ ] `SELECT COUNT(*) FROM economia_annotation` on that same import returns the number of
      non-zero Economia-tab months imported
- [ ] `grep -rn "economia:YYYY-MM\|id.*economia.*format" src-tauri/src/` returns no matches
      referencing the old transaction-id pattern from `store_economia_entries` (the id scheme is gone)
- [ ] `grep -n "economia" src-tauri/src/forecast/mod.rs | grep "signed\|Performance"` confirms
      `signed()` and the Performance formula are untouched (no new `EventKind::Economia` path added
      or removed that would affect real reserve transfers)
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row for plan 052 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the excerpted locations does not match (e.g. `store_economia_entries` no longer
  exists at `write_back_cmds.rs:1008`, or `realized_annual_economia` was moved or renamed).
- Any step's `npm run rust:check` fails after a single reasonable fix attempt.
- A grep reveals that `ensure_reserve_account` is called from paths not covered by this plan
  (beyond the old `store_economia_entries` body and any plan 003 manual-transfer path).
- The `plan_economia_write_back` function in `write_back.rs` turns out to read from the
  `transaction` table by `id LIKE 'economia:%'` in a way that would silently produce empty
  write-back plans after this change (audit that path before finalizing Step 5).
- You discover that `project_with_metrics` in `forecast/mod.rs` does NOT delegate to
  `month_metrics_for` and maintains a separate `economia` accumulation loop — then both functions
  need the `annotation` parameter, and the call sites in the dashboard path also need updating.
- Any existing test that was passing before Step 2 starts failing for a reason unrelated to the
  signature change in `month_metrics_for` (indicates an unintended side effect).

## Maintenance notes

- **Ripple with plan 003** (manual Economia/transfer launch): manual reserve transfers remain real
  money movements (`type='transfer'`, `to_account liquidity='reserve'`). They flow through
  `EventKind::Economia` → `signed()` → Saldo, and through the transfer-side query in
  `realized_annual_economia`. This is correct and intentional. If plan 003 is executed after
  this plan, verify its tests still pass and that it does not introduce `economia:YYYY-MM` style
  ids in `transaction`.
- **Ripple with plan 005** (safe-to-spend / registered economia): `AnnualSavingsDto.registered_economia_cents`
  is now sourced from `realized_annual_economia` which includes the annotation. The DTO field name
  and meaning are unchanged; the value is now more complete. No DTO rename needed.
- **Ripple with plan 033** (Economizado% thresholds): thresholds in `saldoHeatmap.ts` are
  unchanged (they compare `savings_rate_bps` against fixed bps values). No change needed.
- **Ripple with plan 044/045** (side-by-side Economia parity): `parse_economia_sheet` and the
  XLSX import path are untouched; only the storage destination of their output changes. If plan 044
  or 045 modified the parse path, re-verify `store_economia_entries` callers still compile.
- **Future multi-profile support**: `economia_annotation.profile_id` defaults to `''`. When
  multi-profile is added, a new migration should backfill the profile_id and add a proper FK.
  Do not add the FK in this plan — the `profile` table may not yet have stable ids in all deploy
  environments.
- **Reviewer focus areas for the PR**: (a) confirm `signed()` is unmodified; (b) confirm the
  Performance formula is unmodified; (c) confirm no `EventKind::Economia` event is emitted for
  annotation amounts; (d) confirm the annotation + real-transfer sum in `realized_annual_economia`
  has no path that counts the same amount twice.

```

```
