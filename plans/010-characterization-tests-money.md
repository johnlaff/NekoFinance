# Plan 010: Characterization tests for money/forecast SQL helpers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/commands.rs src-tauri/src/google_sheets/write_back.rs src/screens/dashboard/PrevisibilidadeCard.tsx src/screens/dashboard/PerformanceCard.tsx src/screens/dashboard/MonthLedgerCard.tsx src/features/pockets/PocketsManager.tsx src/test/commands.ts`
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Five async SQL helpers — `realized_annual_savings`, `realized_annual_economia`,
`effective_daily_ceiling`, `realized_monthly_baseline`, and `load_write_back_txns` —
have no direct unit tests. They are currently tested only through higher-level
integration points (`forecast_dto`, `dashboard_summary`), which means a refactoring
of `commands.rs` (plan 011) or a subtle SQL change could silently break a financial
calculation without any targeted test failing. Four dashboard cards (`PrevisibilidadeCard`,
`PerformanceCard`, `MonthLedgerCard`, `PocketsManager`) have zero frontend tests.
This plan pins the current behavior of those helpers and cards as characterization
tests, giving plan 011 a safe net to stand on.

## Current state

### Rust test conventions (`src-tauri/src/commands.rs`)

All Rust tests live in `mod tests` at line 2542 of `commands.rs`:

```rust
// commands.rs:2542-2543
#[cfg(test)]
mod tests {
    use super::*;
```

Tests use `#[tokio::test]` for async. The shared helpers (re-use them; do NOT
duplicate):

- `fixture_pool()` (line 2822): creates an in-memory SQLite pool and runs all
  migrations via `sqlx::migrate!("./migrations").run(&pool)`.
- `insert_realized(pool, ttype, amount, date)` (line 3144): inserts a
  realized (non-projected) transaction.
- `insert_projection(pool, ttype, amount, date, payment_method, is_fixed)` (line 2971):
  inserts a projected transaction.
- `insert_liquid_account(pool, balance)` (line 2833): inserts a `bank` liquid account.
- `insert_reserve_account(pool, balance)` (line 3513): inserts a `savings` reserve account.
- `insert_sheet_balance(pool, sheet, date, cents)` (line 3132): inserts a row in
  `sheet_daily_balance`.

**Key invariants confirmed in code:**
- `amount` is always stored as positive magnitude (integer cents ≥ 0). Functions
  use `ABS()` in SQL or `.abs()` in Rust when reading back; do not insert negative
  amounts in the new tests unless the test is specifically about non-canonical sign.
- `realized_annual_savings` (line 503): counts `income` and `expense` rows where
  `date >= year_start` AND `substr(date,1,7) < cur_ym` (complete months only;
  current month excluded). Returns `(renda, poupança=renda−saída)`. Ignores
  `is_projection` flag (stale/frozen at import time).
- `realized_annual_economia` (line 528): counts `transfer` rows whose
  `to_account_id` maps to an account with `liquidity IN ('reserve','illiquid')`,
  same date window (complete months, ignores `is_projection`). Returns the sum.
- `realized_monthly_baseline` (line 575): median of monthly expense totals for
  the last 6 **complete** months (months before `cur_ym`), ignoring `is_projection`.
  Returns 0 when no complete months exist.
- `effective_daily_ceiling` (line 612): if an active explicit `daily_budget` with
  `amount > 0` exists, returns it directly (priority). Otherwise, returns
  `SUM(expense where is_fixed=0 AND is_projection=0 AND payment_method<>'credit'
  AND substr(date,1,7) = prev_ym) / days_in_prev_month`. Returns 0 if no prior month.
- `load_write_back_txns` (line 1919): for a given `year` (i32):
  - Loads all `income` and `expense` (non-credit) transactions with
    `substr(date,1,4) = year`.
  - `income` → `RowKind::Entrada`; fixed expense → `RowKind::Saida`; variable
    expense → `RowKind::Diario`.
  - If a credit card account exists with `closing_day` and `due_day` set,
    collapses all credit expenses into lumps at `cycle_due_date(purchase_date,
    closing, due)` — only those whose due date falls in `year` are included.
  - If no credit card with a cycle exists, credit expenses are included at
    their own dates as `RowKind::Saida`.

### One existing direct test for `realized_annual_savings` (line 3397)

```rust
// commands.rs:3396-3407
#[tokio::test]
async fn realized_annual_ignores_stale_is_projection_flag() {
    let pool = fixture_pool().await;
    insert_projection(&pool, "income", 500_000, "2026-05-05", "", 0).await;
    insert_projection(&pool, "expense", 480_000, "2026-05-10", "debit", 0).await;

    let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
    let (income, savings) = realized_annual_savings(&pool, today).await.unwrap();
    assert_eq!(income, 500_000);
    assert_eq!(savings, 20_000); // 500_000 − 480_000
}
```

No direct tests exist yet for `realized_annual_economia`, `effective_daily_ceiling`,
`realized_monthly_baseline`, or `load_write_back_txns`.

### Frontend test conventions

Pattern file: `src/screens/dashboard/DailyCheckinCard.test.tsx`

```tsx
// DailyCheckinCard.test.tsx:1-8
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DailyCheckinCard } from "./DailyCheckinCard";
import type { DashboardSummary } from "../../lib/api";
import { mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
```

`src/test/commands.ts` exports `mockCommands(handlers)` (line 21),
`mockInvoke` (line 19), and canonical fixture objects `FORECAST`, `POCKETS`,
`EMPTY_POCKETS`, `MONTH_GRID`, `SUMMARY`. Use these; do not duplicate fixture
data when the existing fixture covers the case.

Key types (from `src/lib/api.ts`):
- `Forecast` (line 179): includes `annual_savings: AnnualSavings`,
  `coverage: MonthCoverage[]`, `baseline_outflow_cents`, `trusted_through_month`,
  `months: MonthMetric[]`.
- `MonthCoverage` (line 168): `{ year, month, projected_outflow_cents,
  baseline_outflow_cents, coverage_bps, is_complete, estimated_missing_cents }`.
- `AnnualSavings` (line 154): includes `registered_economia_cents`,
  `realized_income_cents`, `realized_savings_cents`, `realized_rate_bps`.
- `MonthGridDay` used by `getMonthGrid` Tauri command (`get_month_grid`).
- `Pockets` (line ~96 of `src/test/commands.ts`): `{ liquid_cents, reserve_cents,
  restricted_cents, illiquid_cents, net_worth_cents, accounts[] }`.

**React Compiler is enabled**: do NOT add `memo`, `useMemo`, or `useCallback` anywhere.

The `isTauri` guard in `PocketsManager` (line 104, 112, 128, 133) causes `getPockets()`
and `createAccount()` to be skipped in the test environment (where `__TAURI_INTERNALS__`
is set but `isTauri` may be false). Check what `isTauri` evaluates to in tests. If the
form renders as disabled, test the disabled state rather than submit flow.

### In-scope file locations

- `src-tauri/src/commands.rs` — Rust helper functions and the `mod tests` block
  (append inside the existing `mod tests`).
- `src/screens/dashboard/PrevisibilidadeCard.test.tsx` — create new.
- `src/screens/dashboard/PerformanceCard.test.tsx` — create new.
- `src/screens/dashboard/MonthLedgerCard.test.tsx` — create new.
- `src/features/pockets/PocketsManager.test.tsx` — create new.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Rust tests (all) | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass |
| Rust tests (filter) | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- helpers_char` | targeted tests pass |
| Frontend tests (all) | `npm run test:run` | all pass |
| Frontend tests (filter) | `npm run test:run -- --reporter=verbose PrevisibilidadeCard` | targeted tests pass |
| Typecheck | `npm run typecheck` | exit 0, no errors |
| Lint | `npm run lint` | exit 0 |
| Full gate (optional, use last) | `npm run check` | exit 0 |

## Scope

**In scope** (the only files you should create or modify):

- `src-tauri/src/commands.rs` — append new `#[tokio::test]` functions inside the
  existing `mod tests { ... }` block at the bottom (before the closing `}`).
- `src/screens/dashboard/PrevisibilidadeCard.test.tsx` — create.
- `src/screens/dashboard/PerformanceCard.test.tsx` — create.
- `src/screens/dashboard/MonthLedgerCard.test.tsx` — create.
- `src/features/pockets/PocketsManager.test.tsx` — create.

**Out of scope** (do NOT touch):

- Any production source file (`commands.rs` logic, `.tsx` components, `lib/api.ts`,
  `write_back.rs`, `forecast/mod.rs`, etc.) — tests only.
- `src/test/commands.ts` — if a fixture needs a small extension, add it there ONLY
  if the existing fixtures genuinely do not cover the case; otherwise reuse them.
  If you do extend it, note this in the STOP check for drift.
- `plans/README.md` — update only the status cell for plan 010 when done.
- Migrations — do not add or change any.

## Git workflow

- Branch: `advisor/010-characterization-tests-money`
- Commit style: conventional commits as seen in `git log` (e.g.
  `test: characterization tests for realized_annual_economia`).
- One commit per logical group (Rust helpers, then each card) is fine, or one
  commit at the end — executor's choice. Do NOT push or open a PR unless
  explicitly instructed.

## Steps

### Step 1: Append Rust characterization tests for `realized_annual_economia`

Open `src-tauri/src/commands.rs`. Locate the closing `}` of `mod tests` (currently
the last `}` in the file, near line 3832). Insert the following tests **before** that
closing brace. Use the existing helpers (`fixture_pool`, `insert_realized`,
`insert_projection`, `insert_reserve_account`, `insert_liquid_account`) — do not copy
their bodies.

Tests to add (in order):

1. **`economia_counts_complete_months_only`** — seed two complete months (March, April)
   with transfers to a reserve account, and one partial month (June, the current month).
   Assert only the complete months count in `realized_annual_economia`.

2. **`economia_skips_transfers_to_liquid_accounts`** — seed a transfer to a liquid
   account (not reserve or illiquid) in a complete month. Assert `realized_annual_economia`
   returns 0 for that transfer.

3. **`economia_ignores_stale_is_projection_flag`** — seed a transfer to a reserve
   account dated in a complete month but with `is_projection=1`. Assert it still counts
   (same staleness rule as `realized_annual_savings`: date window wins).

Shape for each test:

```rust
#[tokio::test]
async fn economia_counts_complete_months_only() {
    let pool = fixture_pool().await;
    // Need a reserve account to be the transfer target.
    // insert_reserve_account creates one; its id is needed for the transfer.
    // Retrieve it after insertion:
    insert_reserve_account(&pool, 0).await;
    let (reserve_id,): (String,) =
        sqlx::query_as("SELECT id FROM account WHERE liquidity='reserve' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
    // Complete month (March):
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
         VALUES (?1,'transfer',50_000,'2026-03-15',?2,0)",
    )
    .bind(uuid::Uuid::new_v4().to_string()).bind(&reserve_id)
    .execute(&pool).await.unwrap();
    // Current (incomplete) month — must NOT count:
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
         VALUES (?1,'transfer',30_000,'2026-06-05',?2,0)",
    )
    .bind(uuid::Uuid::new_v4().to_string()).bind(&reserve_id)
    .execute(&pool).await.unwrap();

    let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
    let economia = realized_annual_economia(&pool, today).await.unwrap();
    assert_eq!(economia, 50_000, "só março (mês completo) conta; junho fica de fora");
}
```

Apply the same pattern for the other two tests, adjusting the assertion and
the data inserted. For test 3 (stale flag), use `is_projection=1` in the INSERT
but a date in a complete past month.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia_counts_complete_months_only economia_skips_transfers_to_liquid_accounts economia_ignores_stale_is_projection_flag 2>&1 | tail -5`
→ `test result: ok. 3 passed; 0 failed`

### Step 2: Append Rust characterization tests for `realized_monthly_baseline`

Add these tests inside `mod tests`:

1. **`baseline_is_median_of_last_six_complete_months`** — insert 7 complete months of
   expenses with values that have a known median (e.g., 100_000, 200_000, 300_000,
   150_000, 250_000, 180_000, 400_000 from oldest to newest; the LIMIT 6 takes the 6
   most recent, sorted ascending → median of `[150_000, 180_000, 200_000, 250_000,
   300_000, 400_000]` = `(200_000 + 250_000) / 2 = 225_000`). Today = first day of
   the month AFTER the last expense month.

2. **`baseline_returns_zero_when_no_complete_months`** — empty pool (or only current
   month data). Assert `realized_monthly_baseline` returns 0.

3. **`baseline_ignores_current_month`** — insert expenses only in the current month
   (say, June, with `today` in June). Assert 0.

4. **`baseline_odd_count_uses_middle_value`** — insert exactly 3 complete months with
   values 100_000, 200_000, 300_000. Assert median = 200_000 (middle).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- baseline_is_median baseline_returns_zero baseline_ignores_current_month baseline_odd_count 2>&1 | tail -5`
→ `test result: ok. 4 passed; 0 failed`

### Step 3: Append Rust characterization tests for `effective_daily_ceiling`

Add these tests inside `mod tests`:

1. **`daily_ceiling_falls_back_to_prior_month_avg`** — insert non-fixed, non-credit,
   non-projected expenses in the prior complete month. Assert the ceiling equals
   `SUM / days_in_that_month`. (May 2026 has 31 days; `today = 2026-06-13`.)

2. **`daily_ceiling_prefers_active_budget_over_fallback`** — insert both a prior-month
   expense history AND an active `daily_budget` row. Assert the ceiling equals the
   `daily_budget.amount`, ignoring the calculated fallback.

3. **`daily_ceiling_zero_when_no_prior_month`** — empty pool, `today = 2026-06-13`.
   Assert 0.

4. **`daily_ceiling_excludes_fixed_and_credit_from_avg`** — insert expenses in the
   prior month: some with `is_fixed=1`, some with `payment_method='credit'`, and some
   plain variable non-credit. Assert only the plain variable ones contribute to the sum.

Note: an active `daily_budget` row requires a `person` row (FK). Use
`insert_liquid_account` to get a person, then query `SELECT id FROM person LIMIT 1`
to get the `person_id` for the budget insert. The schema for `daily_budget`:
`(id, person_id, amount, start_date, status, free_income)`. Set `status='active'`.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- daily_ceiling_falls_back daily_ceiling_prefers_active daily_ceiling_zero daily_ceiling_excludes 2>&1 | tail -5`
→ `test result: ok. 4 passed; 0 failed`

### Step 4: Append Rust characterization tests for `load_write_back_txns`

Add these tests inside `mod tests`:

1. **`write_back_txns_income_and_variable_expense`** — insert one `income` row and one
   variable `expense` (is_fixed=0, payment_method='debit') in year 2026. Call
   `load_write_back_txns(&pool, 2026)`. Assert one `Entrada` and one `Diario` item
   in the result, both with the correct `amount_cents`.

2. **`write_back_txns_fixed_expense_maps_to_saida`** — insert a fixed expense
   (`is_fixed=1`, `payment_method='debit'`). Assert it maps to `RowKind::Saida`.

3. **`write_back_txns_credit_no_card_falls_to_own_date`** — no credit card account;
   insert a `payment_method='credit'` expense in 2026. Assert it appears in the result
   as `RowKind::Saida` at its own date (the no-card branch at line 1999).

4. **`write_back_txns_transfer_excluded`** — insert a `transfer` transaction; assert it
   does NOT appear in the result (transfers go to the Economia sheet, not the daily grid).

5. **`write_back_txns_wrong_year_excluded`** — insert income dated 2025, call with
   `year = 2026`. Assert the result is empty.

Shape reference for assertions on `WriteBackTxn`:

```rust
use crate::google_sheets::import::RowKind;
// WriteBackTxn is defined in google_sheets::write_back and re-exported through
// the type alias used in commands.rs — confirm the import path at line 1919.
// The field names are: date (String), kind (RowKind), amount_cents (i64).
assert!(result.iter().any(|t| t.kind == RowKind::Entrada && t.amount_cents == income_amount));
```

Note: `RowKind` derives `PartialEq`. If it does not, add `== RowKind::Entrada` will
fail; in that case compare via `matches!()` or a string representation. STOP if
`RowKind` does not implement `PartialEq` — do not derive it in production code without
confirming it doesn't break anything.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- write_back_txns 2>&1 | tail -5`
→ `test result: ok. 5 passed; 0 failed`

### Step 5: Run all Rust tests to confirm no regressions

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -10`
→ `test result: ok. N passed; 0 failed` (N ≥ previous count + 12 new tests)

### Step 6: Create `PrevisibilidadeCard.test.tsx`

Create `src/screens/dashboard/PrevisibilidadeCard.test.tsx`.

Model after `DailyCheckinCard.test.tsx`. The component is a pure render (no Tauri
calls; props-only). No `vi.mock` or `mockCommands` needed.

Import from `src/test/commands.ts`:
- `FORECAST` for the standard case (has coverage with one incomplete month,
  `baseline_outflow_cents > 0`, `annual_savings.registered_economia_cents = 250000`,
  `annual_savings.realized_income_cents = 5000000`).
- `EMPTY_FORECAST` for the no-data/no-baseline case.

Tests to write:

1. **`renders_incomplete_month_warning`** — `render(<PrevisibilidadeCard forecast={FORECAST} />)`.
   The FORECAST fixture has one incomplete month (August 2026, `is_complete: false`).
   Assert `screen.getByText(/A partir de/)` is present (the warning paragraph, line 83
   of the component). Assert the calculated `economizadoPct` is rendered: with
   `registered_economia_cents = 250000` and `realized_income_cents = 5000000`,
   `pct = Math.round(250000 / 5000000 * 100) = 5`. Assert `screen.getByText(/5%/)`.

2. **`renders_neutral_when_no_baseline`** — use `EMPTY_FORECAST` which has
   `baseline_outflow_cents = 0`. Assert the "Ainda não há meses realizados" text
   (line 64 of the component) is present. Assert the warning paragraph is absent.

3. **`renders_ok_when_all_months_complete`** — build a variant of `FORECAST` where
   `coverage = [{ ..., is_complete: true }]` and `trusted_through_month = '2026-07'`.
   Assert "Seus meses futuros estão completos" text (line 76-77).

4. **`renders_trusted_through_label`** — use `FORECAST` (which has
   `trusted_through_month = '2026-07'`). Assert `screen.getByText(/confiável até/)` is
   present.

**Verify**: `npm run test:run -- --reporter=verbose PrevisibilidadeCard 2>&1 | tail -10`
→ all 4 tests pass

### Step 7: Create `PerformanceCard.test.tsx`

Create `src/screens/dashboard/PerformanceCard.test.tsx`.

The component is also props-only. Use `FORECAST` from `src/test/commands.ts`.

The FORECAST `months` array (see `src/test/commands.ts` lines 186-211) has two
entries: month 6 (`savings_rate_bps: 2500`) and month 7 (`savings_rate_bps: 1000`).
`FORECAST.today = '2026-06-10'`, so `ym = '2026-06'`. The filter keeps months
`>= '2026-06'`; both months qualify. The `incompleteKeys` Set is built from
`coverage` (one item: month 8, incomplete). Month 8 is not in `months`, so neither
rendered cell is incomplete.

Tests to write:

1. **`renders_performance_for_upcoming_months`** — render with `FORECAST`. Assert
   `screen.getByText('junho')` and `screen.getByText('julho')` (month labels) are
   present. Assert `screen.getByText(/economizado 25%/)` for June
   (`Math.round(2500 / 100) = 25`). Assert `screen.getByText(/economizado 10%/)` for
   July (`Math.round(1000 / 100) = 10`).

2. **`marks_incomplete_month_with_incompleto_label`** — build a forecast where a month
   in `months` has a matching key in `coverage[].is_complete = false`. E.g., add
   month 8 to `months` and ensure `coverage` has `{ month: 8, is_complete: false }`.
   Assert `screen.getByText('incompleto')` appears.

3. **`returns_null_when_no_upcoming_months`** — use `EMPTY_FORECAST` (months = []).
   Assert the component renders nothing: `const { container } = render(...)`,
   assert `container.firstChild === null`.

**Verify**: `npm run test:run -- --reporter=verbose PerformanceCard 2>&1 | tail -10`
→ all 3 tests pass

### Step 8: Create `MonthLedgerCard.test.tsx`

Create `src/screens/dashboard/MonthLedgerCard.test.tsx`.

The component calls `getMonthGrid(year, month)` via `useCommand`, which calls
the Tauri command `get_month_grid`. Use `vi.mock` + `mockCommands`.

Import `MONTH_GRID` from `src/test/commands.ts`. The MONTH_GRID fixture has 4 days
(sparse June 2026 data). The `footerOf()` function is internal; test its outcome
through the rendered footer rows.

From the MONTH_GRID fixture:
- `income_cents` total = 700_000 (day June 25).
- `fixed_out_cents` total = 250_000 (day June 15).
- `daily_out_cents` total = 4_300 (day June 15).
- `saidaTotal = 250_000 + 4_300 = 254_300`.
- `performance = 700_000 − 254_300 = 445_700`.

Tests to write:

1. **`renders_month_name_and_grid_rows`** — mock `get_month_grid` with `MONTH_GRID`.
   `today = '2026-06-10'`. After `waitFor`, assert `screen.getByText(/Junho de 2026/)`.
   Assert at least one row is present.

2. **`footer_shows_correct_totals`** — same setup. After `waitFor`, assert footer shows
   income `R$ 7.000,00`, saída total `R$ 2.543,00`, and performance `R$ 4.457,00`.
   (Money component renders magnitude without minus; use `sign="auto"` for performance.)

3. **`shows_empty_state_when_no_data`** — mock `get_month_grid` with `[]`. After
   `waitFor`, assert `screen.getByText(/Mês sem lançamentos/)`.

4. **`month_nav_changes_the_loaded_month`** — use `userEvent`. Mock both
   `get_month_grid` calls (for June and July). Click the "Próximo mês" button. After
   `waitFor`, assert `screen.getByText(/Julho de 2026/)`.

Note: the component starts with `useState(todayYm)`, so the initial month is derived
from `today` prop. Pass `today='2026-06-10'` consistently.

**Verify**: `npm run test:run -- --reporter=verbose MonthLedgerCard 2>&1 | tail -10`
→ all 4 tests pass

### Step 9: Create `PocketsManager.test.tsx`

Create `src/features/pockets/PocketsManager.test.tsx`.

`PocketsManager` calls `getPockets()` via `useEffect` and `createAccount()` on submit.
Both are guarded by `isTauri` (lines 35, 51 of the component). In the test environment,
`__TAURI_INTERNALS__` is set but `isTauri` is `typeof window.__TAURI_INTERNALS__ !==
'undefined'` — which evaluates to `true` in tests (setup.ts sets it). Verify this
assumption: if inputs are `disabled` in the rendered output, `isTauri` is actually
`false` in tests; adjust by checking the actual disabled state in your first render.

Import `POCKETS`, `EMPTY_POCKETS`, `mockCommands`, `mockInvoke` from
`src/test/commands.ts`.

Tests to write:

1. **`renders_existing_pockets_list`** — mock `get_pockets: POCKETS`. After `waitFor`,
   assert `screen.getByText('Conta corrente')` (the first account name) and
   `screen.getByText('Poupança')` are present.

2. **`shows_nothing_above_form_when_no_pockets`** — mock `get_pockets: EMPTY_POCKETS`.
   After `waitFor`, assert the pocket list is absent. Assert the form is still present.

3. **`form_is_present_with_nome_and_tipo_fields`** — render with any mock.
   Assert `screen.getByPlaceholderText('Ex.: Bolso demo')` and a select/combobox for
   type are present (do not assert specific option labels — they may change).

4. **`shows_error_on_get_pockets_failure`** — mock `get_pockets: new Error('db locked')`.
   After `waitFor`, assert an error message contains "Não foi possível carregar os
   bolsos." (from `safeErrorMessage` at line 39 of the component, with the fallback
   passed as second arg).

If `isTauri` is `false` in tests (inputs disabled), add test 4a: assert that the
submit button has the disabled attribute when `!isTauri`.

**Verify**: `npm run test:run -- --reporter=verbose PocketsManager 2>&1 | tail -10`
→ all tests pass (minimum 4)

### Step 10: Run all frontend tests

**Verify**: `npm run test:run 2>&1 | tail -10`
→ `Tests X passed (X includes all previous + new tests), 0 failed`

### Step 11: Typecheck and lint

**Verify (both must pass)**:
```
npm run typecheck 2>&1 | tail -5   # → exit 0, no errors
npm run lint 2>&1 | tail -5        # → exit 0
```

### Step 12: Update plan status

Edit `plans/README.md`. Change the status cell for plan 010 from `TODO` to `DONE`.

**Verify**: `grep "010" plans/README.md`
→ line contains `DONE`

## Test plan

### Rust tests (append to `mod tests` in `src-tauri/src/commands.rs`)

| Test name | Helper(s) under test | What it pins |
|-----------|----------------------|--------------|
| `economia_counts_complete_months_only` | `realized_annual_economia` | current-month exclusion |
| `economia_skips_transfers_to_liquid_accounts` | `realized_annual_economia` | liquidity filter |
| `economia_ignores_stale_is_projection_flag` | `realized_annual_economia` | date-window wins |
| `baseline_is_median_of_last_six_complete_months` | `realized_monthly_baseline` | LIMIT 6 + median |
| `baseline_returns_zero_when_no_complete_months` | `realized_monthly_baseline` | zero-data path |
| `baseline_ignores_current_month` | `realized_monthly_baseline` | month boundary |
| `baseline_odd_count_uses_middle_value` | `realized_monthly_baseline` | odd-length median |
| `daily_ceiling_falls_back_to_prior_month_avg` | `effective_daily_ceiling` | fallback path |
| `daily_ceiling_prefers_active_budget_over_fallback` | `effective_daily_ceiling` | budget priority |
| `daily_ceiling_zero_when_no_prior_month` | `effective_daily_ceiling` | new-user zero |
| `daily_ceiling_excludes_fixed_and_credit_from_avg` | `effective_daily_ceiling` | filter correctness |
| `write_back_txns_income_and_variable_expense` | `load_write_back_txns` | Entrada + Diario |
| `write_back_txns_fixed_expense_maps_to_saida` | `load_write_back_txns` | Saida for fixed |
| `write_back_txns_credit_no_card_falls_to_own_date` | `load_write_back_txns` | no-card branch |
| `write_back_txns_transfer_excluded` | `load_write_back_txns` | transfer exclusion |
| `write_back_txns_wrong_year_excluded` | `load_write_back_txns` | year filter |

Total new Rust tests: 16.

Structural model: `realized_annual_ignores_stale_is_projection_flag` (line 3397)
for async style; `dashboard_daily_budget_prefers_explicit_active_budget` (line 3591)
for inserting a `daily_budget` row.

### Frontend tests (new files)

| File | Tests | Structural model |
|------|-------|-----------------|
| `PrevisibilidadeCard.test.tsx` | 4 | `DailyCheckinCard.test.tsx` |
| `PerformanceCard.test.tsx` | 3 | `DailyCheckinCard.test.tsx` |
| `MonthLedgerCard.test.tsx` | 4 | `DashboardScreen.test.tsx` |
| `PocketsManager.test.tsx` | 4 | `DailyCheckinCard.test.tsx` |

Total new frontend tests: ≥ 15.

**Rust test run**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -5`
→ 16 more tests pass than before this plan.

**Frontend test run**: `npm run test:run 2>&1 | tail -5`
→ ≥ 15 more tests pass than before this plan.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; the 16 new
  Rust test names listed in the Test plan table all appear in the output as `ok`.
- [ ] `npm run test:run` exits 0; the 4 new `.test.tsx` files all appear in the output.
- [ ] `npm run typecheck` exits 0 with no errors.
- [ ] `npm run lint` exits 0.
- [ ] `git diff --name-only HEAD~1..HEAD` (or `git status`) shows ONLY the 5
  in-scope files modified/created (plus `plans/README.md`). No production source
  files changed.
- [ ] `plans/README.md` status for plan 010 reads `DONE`.

## STOP conditions

Stop immediately and report back (do not improvise) if:

- The code at the "Current state" locations does not match the excerpts — e.g.,
  `realized_annual_economia` is no longer at line 528, or its SQL differs from
  what is shown above. The codebase has drifted; the plan may need updating.
- `RowKind` does not implement `PartialEq` — you cannot compare `t.kind == RowKind::Entrada`
  without it. Do NOT `#[derive(PartialEq)]` on a production type without human review.
- `insert_reserve_account` does not create a person row, causing FK violations when you
  insert a `transfer` that references `to_account_id` — the pool may need `insert_liquid_account`
  called first to establish a person. Confirm this before writing all 3 economia tests.
- Any step's verify command fails twice after a reasonable fix attempt (e.g., wrong import
  path, SQL quoting issue, fixture shape mismatch).
- A test requires touching a production file to make it testable (e.g., exporting a
  private function). Stop; the advisor may need to reassess scope.
- `isTauri` evaluates to `false` in the test environment and the `PocketsManager`
  form renders all inputs as disabled — in this case, document the actual behavior and
  test the disabled state rather than submit flow, but report so the advisor can note
  the testability gap.
- The `write_back_txns_credit_no_card_falls_to_own_date` test finds the credit expense
  with `is_projection` set unexpectedly (the existing tests at line 2971 set
  `is_projection=1`; the helper `insert_realized` at line 3144 sets `is_projection=0`).
  Use `insert_realized` or insert directly with explicit `is_projection=0` to avoid
  the card-detection branch filtering it out.

## Maintenance notes

- Plan 011 (split `commands.rs`) depends on these characterization tests being green.
  The executor of plan 011 should run `cargo test ... -- economia_ baseline_ daily_ceiling_ write_back_txns_` first to confirm the net is intact before and after the split.
- The `realized_monthly_baseline` tests use fixed "today" dates (June 2026). If a test
  using a dynamic "today" is added later, the LIMIT 6 window will shift and the
  median fixture values will need to be recomputed. Keep fixture months hardcoded to
  2025–2026 range for stability.
- `effective_daily_ceiling` is called from two places: `forecast_dto` (line 811) and
  `dashboard_summary` (line 1361). The characterization tests here exercise the
  function directly. If those callers are refactored (plan 011), re-run these tests.
- If `WRITE_BACK_ENABLED` is ever flipped to `true`, the `build_write_back_plan`
  integration path will need direct tests too (currently its OAuth dependency makes
  direct testing impractical). This plan defers that; revisit in a future test plan.
- The `PocketsManager` component guards on `isTauri`; if the isTauri detection logic
  changes, the test assertions may need adjustment.
