# Plan 037: P1 flow fixes — tag-exclude out of Saldo projection; credit-lump write-back audit; Jan-1 economia symmetry

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat e62ecb6..HEAD -- src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/write_back_cmds.rs`
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
- **Package**: D
- **Planned at**: commit `e62ecb6`, 2026-06-20

## Why this matters

Three related correctness bugs all land in the same two Rust files. Bug 1: plan 034
added an `exclude_from_totals` filter to `load_cashflow_events`, which feeds the
forward Saldo chain (balance trajectory, deepest deficit, Horizonte). A future expense
tagged "Ignorar" now silently vanishes from the projected balance even though the cash
will leave the account — the correct intent is: excluded tags suppress METRIC aggregates
only, not the cash Saldo. Bug 2: the write-back audit (`record_write_back_audit`) has
no realignment path for the credit-lump case, so the `source_amount` baseline of the
grouped credit transactions is never updated after a write-back — the next import
raises a spurious conflict on every cycle, hitting the user's most common write
operation. Bug 3: `realized_annual_economia` lacks the Jan-1 window shift that
`realized_annual_savings` has, so on the first day of the year the savings guardrail
compares a December-window income figure against an economia figure that still reads
zero — an inconsistency in the guardrail that can produce a misleading "can spend"
result during the year transition.

All three have regression tests. Fixing them together keeps the two files coherent.

## Current state

### File roles

- `src-tauri/src/commands/forecast_cmds.rs` — all forward-projection SQL helpers and
  forecast Tauri commands; 999 lines.
- `src-tauri/src/commands/write_back_cmds.rs` — write-back plan builder, Google Sheets
  write command, and post-write audit helper; 873 lines.

### Bug 1 — `load_cashflow_events` (forecast_cmds.rs lines 349–380)

The function that feeds the balance trajectory currently filters out excluded-tag
transactions via `NOT EXISTS`:

```rust
// forecast_cmds.rs:349-380
pub(crate) async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let horizon = horizon_end.format("%Y-%m-%d").to_string();

    let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?1 AND t.date <= ?2 \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",     // <-- BUG: kills future txns from the cash Saldo
    )
```

This filter MUST NOT be in `load_cashflow_events` — that function supplies cash events
to `load_forecast_events` (line 390: `let mut events = load_cashflow_events(...)`)
which drives `forecast_dto` and `dashboard_summary`. Removing it restores cash fidelity.

The filter belongs (and is correctly present) in the METRIC functions:

- `load_realized_month_events` (lines 412–438) — used by Performance/Custo de vida of
  the current month.
- `load_year_events` (lines 733–755) — used by the annual metrics view.
- `realized_annual_economia` (lines 158–181) — used by the savings guardrail.

The `projection_seed` (lines 69–79) is already correct — it does NOT filter by tag,
so the seed includes real cash flows.

### Bug 2 — `record_write_back_audit` "saida" arm (write_back_cmds.rs lines 510–521)

The audit function has four match arms for cell kinds. The `"saida"` arm:

```rust
// write_back_cmds.rs:510-521
"saida" => {
    sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
           AND (payment_method IS NULL OR payment_method <> 'credit')",
    )
    .bind(c.value_cents)
    .bind(&now)
    .bind(&c.date)
    .execute(&mut *tx)
    .await
}
```

The credit-lump case: credit purchases (`type='expense'`, `payment_method='credit'`,
`is_fixed=0`) are aggregated by `cycle_due_date` in `load_write_back_txns` (lines
63–93) and written as a single Saída lump on the due date. After write-back, the audit
must realign the `source_amount` of all credit transactions whose `cycle_due_date`
equals `c.date` — but the current `"saida"` arm excludes `payment_method='credit'`
transactions, so it matches zero rows for the lump → the source baseline stays stale →
next import flags a spurious conflict.

How the lump date is computed (lines 63–93 of `write_back_cmds.rs` and the
`forecast::cycle_due_date` helper): each credit purchase on date `d` has its due date
computed from the card's `closing_day`/`due_day`. The lump value on a given due date is
`SUM(ABS(amount))` of all purchases whose computed due date equals that date.

The fix must realign `source_amount` on those source credit transactions (not on any
single-row `is_fixed=1` row, which may not exist for a credit-only Saída cell). The
simplest correct approach: add a separate arm or sub-query inside `"saida"` that also
updates `payment_method='credit'` rows grouped by their computed due date = `c.date`.

Because the due-date computation is done in Rust (`forecast::cycle_due_date`), not in
SQL, and the app only supports one card (enforced by `ORDER BY created_at, id LIMIT 1`
in `load_write_back_txns`), the SQL realignment can read the card's closing/due days
from the `account` table and apply the cycle formula inline, or simply match
`payment_method='credit'` rows for dates in the closing window and filter by computed
due date. The simplest safe option: add a second UPDATE inside the `"saida"` arm that
targets `payment_method='credit'` rows whose computed due-date (looked up from the
card) matches `c.date`. See "Step 2" for the concrete implementation shape.

The comment on line 476–478 already acknowledges the gap:

```rust
// write_back_cmds.rs:476-478
/// `cells` are the cells effectively WRITTEN (already filtered by `changed`). We map each one to
/// transactions by `(date, type, is_fixed)` derived from `kind` — the same criterion as
/// `load_write_back_txns`. Credit card (lump at due date) has no 1:1 importable row, so the
/// realignment focuses on direct-debit rows (income/expense in debit); the round-trip test covers this.
```

The note "round-trip test covers this" turns out to be aspirational — no such test
exists yet. This plan adds it.

### Bug 3 — `realized_annual_economia` missing Jan-1 shift (forecast_cmds.rs lines 158–181)

`realized_annual_savings` (lines 115–151) has an explicit Jan-1 branch:

```rust
// forecast_cmds.rs:120-138
let is_january = cur_ym == format!("{}-01", today_naive.year());
let (lower, upper) = if is_january {
    (
        format!("{}-12-01", today_naive.year() - 1),
        format!("{}-01-01", today_naive.year()),
    )
} else {
    (
        format!("{}-01-01", today_naive.year()),
        format!("{cur_ym}-01"),
    )
};
```

`realized_annual_economia` (lines 158–181) uses a fixed `year_start` with no
equivalent shift:

```rust
// forecast_cmds.rs:162-163
let year_start = format!("{}-01-01", today_naive.year());
let cur_ym = today_naive.format("%Y-%m").to_string();
// ...
.bind(&year_start)
.bind(format!("{cur_ym}-01"))
```

On Jan 1: `year_start = "YYYY-01-01"` and `upper = "YYYY-01-01"` — the range is empty,
so `economia = 0` while the savings function uses the prior December window and sees
actual income. The guardrail that uses both can produce an inconsistent state.

The fix: replicate the `is_january` branch in `realized_annual_economia` exactly as in
`realized_annual_savings`, using the same variable names and logic.

### Repo conventions

- Tests use `sqlx::sqlite::SqlitePoolOptions::new().connect("sqlite::memory:")` +
  `sqlx::migrate!("./migrations").run(&p)` — see `src-tauri/src/tags.rs:182–188` and
  `src-tauri/src/conflicts.rs:149–156`.
- Async tests use `#[tokio::test]`.
- Test module goes in a `#[cfg(test)] mod tests { ... }` block at the bottom of the
  relevant `.rs` file.
- Functional-core style: all three functions under test are `pub(crate) async fn` that
  take `&SqlitePool` — call them directly in tests, no Tauri State needed.
- `INSERT INTO "transaction"` requires at minimum: `id, type, amount, date,
is_projection`. For tags: also insert into `tag` and `transaction_tag`. For the
  credit lump: insert into `account` (`type='credit_card'`, `closing_day`, `due_day`).
- Money = positive-magnitude integer cents (`amount` is always stored positive;
  sign comes from `type`).

## Commands you will need

| Purpose               | Command              | Expected on success        |
| --------------------- | -------------------- | -------------------------- |
| Rust typecheck + lint | `npm run rust:check` | exit 0, no errors/warnings |
| All unit tests        | `npm run test:run`   | all pass                   |
| Full gate             | `npm run check`      | exit 0                     |

> Run `npm run rust:check` after each step; run `npm run test:run` after all tests are
> written to confirm the new tests pass.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/forecast_cmds.rs`
- `src-tauri/src/commands/write_back_cmds.rs`

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/google_sheets/write_back.rs` — sheet-level write logic; the audit
  function under repair is in `write_back_cmds.rs`, not here.
- `src-tauri/src/tags.rs` — tag schema is correct and unchanged.
- Any frontend file (`src/`) — no UI change required.
- `src-tauri/migrations/` — no schema change required.
- Any other Rust source file — keep the blast radius minimal.

## Git workflow

- Branch: `advisor/037-package-d-flow-fixes`
- Commit style from `git log`: `fix: <lower-case description>` (conventional commits).
  Example from recent history: `fix: aderência — piso de reserva = custo de vida × meses`
- One commit per logical fix, or one commit for all three if they are small — your
  call; the key is each commit must leave `rust:check` green.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Remove the exclude-filter from `load_cashflow_events`

Open `src-tauri/src/commands/forecast_cmds.rs`. Locate `load_cashflow_events` at
line 349. The SQL query (lines 359–368) contains a `NOT EXISTS` sub-select that
filters out excluded-tag transactions. Remove the `AND NOT EXISTS (...)` clause
entirely, leaving only:

```sql
SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection,
       COALESCE(a.liquidity,'')
FROM "transaction" t LEFT JOIN account a ON a.id = t.to_account_id
WHERE t.date > ?1 AND t.date <= ?2
```

Do not remove the same filter from `load_realized_month_events` (line 425),
`load_year_events` (line 745), or `realized_annual_economia` (line 168) — those are
intentional and correct.

**Verify**: `npm run rust:check` → exit 0, no new warnings.

### Step 2: Add credit-lump realignment in `record_write_back_audit`

Open `src-tauri/src/commands/write_back_cmds.rs`. Locate `record_write_back_audit` at
line 479. Inside the `for c in cells` loop, find the `"saida"` arm (lines 510–521).

After the existing `"saida"` UPDATE (which targets non-credit debit expenses) executes,
add a second UPDATE that realigns the `source_amount` of credit purchases whose
computed due date equals `c.date`. The Rust shape to produce inside the `"saida"` arm:

```rust
"saida" => {
    // 1) Debit/fixed expenses — unchanged.
    let debit_updated = sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE date = ?3 AND type = 'expense' AND is_fixed = 1 \
           AND (payment_method IS NULL OR payment_method <> 'credit')",
    )
    .bind(c.value_cents)
    .bind(&now)
    .bind(&c.date)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("realign source_amount (debit saida): {e}"))?;

    // 2) Credit lump: realign all credit purchases whose cycle_due_date falls on c.date.
    //    The due date for a purchase on day D with closing_day CL and due_day DU is computed
    //    by `forecast::cycle_due_date`. We replicate the grouping logic:
    //    read the first complete card, then update matching credit rows.
    let card: Option<(i64, i64)> = sqlx::query_as(
        "SELECT closing_day, due_day FROM account \
         WHERE type='credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("query card for audit: {e}"))?;

    let credit_updated = if let Some((closing, due)) = card {
        // Purchases that could due on c.date come from roughly the prior billing cycle.
        // For safety: select all credit expense rows and filter in Rust by cycle_due_date.
        let candidates: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, date FROM \"transaction\" \
             WHERE type='expense' AND payment_method='credit'",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("query credit candidates: {e}"))?;

        let due_str = c.date.as_str(); // "YYYY-MM-DD"
        let matching_ids: Vec<String> = candidates
            .into_iter()
            .filter_map(|(id, date_str)| {
                let d = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
                let computed =
                    forecast::cycle_due_date(d, closing as u32, due as u32);
                if computed.format("%Y-%m-%d").to_string() == due_str {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        let mut n = 0usize;
        for id in &matching_ids {
            sqlx::query(
                "UPDATE \"transaction\" SET source_amount = NULL, updated_at = ?1 \
                 WHERE id = ?2",
            )
            // source_amount=NULL signals "baseline is now the lump total, not individual row"
            // — the next import will see local==sheet for the aggregated cell and raise no conflict.
            // Alternatively, set source_amount=individual_row_amount if the import compares
            // per-row. CHECK "Step 2 design note" below before choosing.
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("realign credit source_amount: {e}"))?;
            n += 1;
        }
        n
    } else {
        0
    };

    Ok(sqlx::sqlite::SqliteQueryResult::default()) // dummy — rows_affected counted above
}
```

> **Step 2 design note — what value to write into `source_amount` for credit rows**:
>
> The 3-way merge in `src-tauri/src/google_sheets/reconcile.rs` compares
> `source_amount` (base) against `amount` (local edit) and the sheet value. For the
> credit-lump cell the sheet value is `SUM(individual purchases)` and there is no
> per-row sheet column — the per-row `source_amount` has no meaningful "base" to track.
> Setting `source_amount = NULL` on each purchase row signals "no prior sheet baseline
> tracked" which the reconciler treats as "base equals local" (no conflict). This is
> the correct semantic: after a write-back the lump total is the new authoritative base,
> and individual purchases are below the tracking granularity.
>
> If after inspecting `reconcile.rs` you find that `source_amount = NULL` is handled
> differently (e.g. treated as "base=0" → always conflicts), switch to writing the
> individual row's `amount` as `source_amount` instead. Verify with the round-trip test
> in Step 4.
>
> **STOP** if the reconciler's NULL-handling is ambiguous — report back rather than guess.

Because `sqlx` does not allow returning an arbitrary `SqliteQueryResult` from inside a
`match` arm without the single-result constraint, refactor the arm to accumulate the
row count into `realigned` directly rather than returning a dummy result. The actual
pattern: compute `debit_updated.rows_affected() + credit_updated as u64` and add to
`realigned` before the `_ => continue` arm. Adjust the surrounding bookkeeping
accordingly. Keep the transaction (`&mut *tx`) flowing through all queries.

**Verify**: `npm run rust:check` → exit 0.

### Step 3: Replicate Jan-1 window shift in `realized_annual_economia`

Open `src-tauri/src/commands/forecast_cmds.rs`. Locate `realized_annual_economia` at
line 158. The current body (lines 162–180) uses a fixed `year_start` with no Jan-1
branch. Replace the date-binding block so it matches `realized_annual_savings` exactly:

Target shape (insert after line 161, replacing lines 162–163):

```rust
let cur_ym = today_naive.format("%Y-%m").to_string();
let is_january = cur_ym == format!("{}-01", today_naive.year());
let (lower, upper) = if is_january {
    (
        format!("{}-12-01", today_naive.year() - 1),
        format!("{}-01-01", today_naive.year()),
    )
} else {
    (
        format!("{}-01-01", today_naive.year()),
        format!("{cur_ym}-01"),
    )
};
```

Then bind `lower` and `upper` in the query (replacing the existing `.bind(&year_start)`
and `.bind(format!("{cur_ym}-01"))` calls):

```rust
.bind(&lower)
.bind(&upper)
```

Remove the now-unused `year_start` variable.

**Verify**: `npm run rust:check` → exit 0.

### Step 4: Write three regression tests

Add a `#[cfg(test)] mod tests { ... }` block at the bottom of
`src-tauri/src/commands/forecast_cmds.rs` (after the last function), and a `#[cfg(test)]
mod tests { ... }` block at the bottom of `src-tauri/src/commands/write_back_cmds.rs`.

Use the standard in-memory pool helper (same pattern as `src-tauri/src/tags.rs:182–188`):

```rust
async fn pool() -> sqlx::SqlitePool {
    let p = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    p
}
```

#### Test A — `forecast_cmds.rs` tests block: excluded-tag future expense still affects projected balance

```
test name: excluded_tag_expense_still_lowers_projected_balance
```

Setup:

1. Insert one future expense (`date = tomorrow`, `type = 'expense'`, `amount = 5000`,
   `is_projection = 1`).
2. Insert a tag with `exclude_from_totals = 1`.
3. Insert a `transaction_tag` row linking the expense to that tag.

Assert: `load_cashflow_events(&p, today, horizon)` returns a non-empty list that
includes the tagged expense (i.e. the expense is NOT filtered out).

Then verify the converse for metrics: call `load_realized_month_events(&p, month_start,
today)` with a past tagged expense and assert it IS excluded (the filter there is
intentional). This guards against accidentally removing the filter from the wrong function.

#### Test B — `forecast_cmds.rs` tests block: Jan-1 economia uses prior-December window

```
test name: jan1_economia_uses_prior_december_window
```

Setup:

1. Set `today_naive = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()`.
2. Insert a `transfer` to an `account` with `liquidity = 'reserve'` dated `2025-12-15`
   (magnitude `20000`).

Assert: `realized_annual_economia(&p, today_naive).await.unwrap()` returns `20000`
(not 0, which the broken version would return because the year window is empty on Jan 1).

Also assert that `realized_annual_savings(&p, today_naive).await.unwrap().1` is
non-zero when there is prior-December income (guard that the symmetry is actually
maintained — both use December). Insert an income row `2025-12-10, amount=100000`
alongside the transfer.

#### Test C — `write_back_cmds.rs` tests block: credit-lump write-back raises no spurious conflict

```
test name: credit_lump_writeback_realigns_source_amount
```

Setup:

1. Insert an `account` row with `type='credit_card'`, `closing_day=25`, `due_day=5`.
2. Insert two credit purchases: `date='2026-05-20', type='expense', payment_method='credit',
amount=3000, is_fixed=0` and `date='2026-05-22', type='expense', payment_method='credit',
amount=2000, is_fixed=0`. Both should due on `2026-06-05` per `cycle_due_date`.
3. Construct a `CellWrite` with `kind="saida"`, `date="2026-06-05"`,
   `value_cents=5000`, `a1="F5"`, `changed=true`.
4. Call `record_write_back_audit(&p, "2026", &[&cell]).await`.

Assert: after the call, both credit purchase rows have `source_amount IS NULL` (or the
value you chose in Step 2 — match whichever the implementation sets).

Assert (conflict-suppression property): read `source_amount` of both rows and confirm
neither equals a stale prior value that would trigger a 3-way-merge conflict if the
next import brings `sheet_value = 5000` as the lump total.

> Note: `CellWrite` is defined in `write_back_cmds.rs` (find its struct definition and
> import it accordingly within the test module via `use super::*`).

**Verify**: `npm run test:run` → all tests pass, including the 3 new ones. Then
`npm run check` → full gate green.

## Test plan

Summary of new tests:

| File                 | Test name                                             | What it guards                                                           |
| -------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------ |
| `forecast_cmds.rs`   | `excluded_tag_expense_still_lowers_projected_balance` | Bug 1 regression: excluded-tag future expense visible in cash projection |
| `forecast_cmds.rs`   | `jan1_economia_uses_prior_december_window`            | Bug 3 regression: Jan-1 economia window symmetry                         |
| `write_back_cmds.rs` | `credit_lump_writeback_realigns_source_amount`        | Bug 2 regression: credit lump write-back clears stale source baseline    |

Model the test structure after `src-tauri/src/tags.rs` (in-memory pool, `#[tokio::test]`,
pure function calls with `&pool`).

Run with: `npm run test:run` → exit 0, 3 new tests passing.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 with no new warnings.
- [ ] `npm run test:run` exits 0; exactly 3 new `#[tokio::test]` functions exist and pass:
      `excluded_tag_expense_still_lowers_projected_balance`,
      `jan1_economia_uses_prior_december_window`,
      `credit_lump_writeback_realigns_source_amount`.
- [ ] `grep -n "NOT EXISTS" src-tauri/src/commands/forecast_cmds.rs` — the filter
      appears in `load_realized_month_events` (≈line 425), `load_year_events` (≈line 744),
      and `realized_annual_economia` (≈line 168), but NOT in `load_cashflow_events`
      (≈line 349). Confirm by line number.
- [ ] `grep -n "is_january" src-tauri/src/commands/forecast_cmds.rs` — appears in BOTH
      `realized_annual_savings` and `realized_annual_economia`.
- [ ] `grep -n "payment_method.*credit\|credit.*payment_method" src-tauri/src/commands/write_back_cmds.rs`
      — the audit function now has a code path that updates `payment_method='credit'` rows.
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD`).
- [ ] `plans/README.md` status row for plan 037 updated to DONE.

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the `load_cashflow_events` query (≈lines 359–368) does not contain the
  `NOT EXISTS` block described in "Current state" — the filter may have already been
  removed by another PR, making Step 1 a no-op (confirm and skip).
- `realized_annual_economia` already has an `is_january` branch — Bug 3 may be
  pre-fixed; confirm and skip Step 3.
- The `record_write_back_audit` function's `"saida"` arm (≈lines 510–521) has
  significantly changed shape since this plan was written.
- `CellWrite` is not accessible from a test module via `use super::*` (it may be in a
  different module); locate it and adjust the import rather than inventing a replacement.
- Step 2's design note about `source_amount = NULL` vs `source_amount = amount` cannot
  be resolved by reading `reconcile.rs` — report the ambiguity.
- Any step's `npm run rust:check` fails twice after a reasonable fix attempt.
- The fix requires touching `src-tauri/src/google_sheets/reconcile.rs` or any
  out-of-scope file to be correct.

## Maintenance notes

- **Bug 1 interaction with future plans**: if a plan adds a separate "projected cash
  including all txns" vs "metric-visible txns" concept, `load_cashflow_events` is the
  canonical cash-only view — keep it filter-free.
- **Bug 2 long-term**: the credit-lump realignment in Rust (loop + `cycle_due_date`)
  is O(n) over all credit rows. For large histories (hundreds of purchases) this is
  fine; at thousands of rows, push the cycle computation into SQL using a date-range
  window instead. Deferred out of this plan.
- **Bug 3**: if the method ever changes the "meses completos" definition (e.g. including
  the current month's partial data), both `realized_annual_savings` and
  `realized_annual_economia` must be updated together — they must stay symmetric.
- A reviewer should scrutinize: (a) that the `NOT EXISTS` removal in Step 1 only touches
  `load_cashflow_events` and no other function; (b) that the credit-realignment query in
  Step 2 uses the same `cycle_due_date` function as `load_write_back_txns` (not a
  hand-rolled date arithmetic that could drift); (c) that the Jan-1 branch in Step 3
  copies the exact variable names from `realized_annual_savings` to keep the two
  functions visually comparable.
