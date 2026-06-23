# Plan 058: Restore past balances in Calendário, O ano, and Este mês

> **Executor instructions**: Follow step by step; run every verification command and confirm
> the expected result. If a "STOP condition" occurs, stop and report. When done, update this
> plan's row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src/lib/api.ts src/screens/YearGridScreen.tsx src/screens/AnnualScreen.tsx src/screens/TotaisScreen.tsx`
> On any change, compare the "Current state" excerpts to the live code before proceeding.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug (regression introduced by the 2026 redesign, PR #84)
- **Planned at**: commit `da2d3e9`, 2026-06-22

## Why this matters

Three redesigned screens stopped showing **past** balances, hiding data the user already has
in the spreadsheet. The user's north-star principle is *"o app supera a planilha, nunca menos"*
— the app must never show less than the sheet. Today (assume today = 2026-06-22):

- **Calendário** (`YearGridScreen`): daily saldo shows "—" for past days (1–21 June); only
  today-forward (22–30) have values.
- **O ano** (`AnnualScreen`): the "Saldo fim" (end-of-month balance) column is "—" for Jan–May.
- **Este mês** (`TotaisScreen`): the "Resultado nos últimos meses" trend shows only the
  current month.

**Root cause**: all three rely solely on `get_forecast` (`getForecast`), whose `daily[]`,
`month_end[]`, and `months[]` arrays are built by the engine starting at **today** and project
forward (`project_with_metrics` loops `let mut day = today; while day <= horizon_end`). Past
days/months are never emitted. The redesign also dropped the `getMonthGrid` binding from
`src/lib/api.ts` (commit said "forecast covers it" — it does not cover the past). The Rust
commands that DO expose realized history still exist and are untouched:

- `month_grid(pool, year, month)` → `Vec<MonthGridDayDto>` — every day of any month with
  `balance_cents: Option<i64>` from the imported sheet `Saldo` column (past or future).
  (`src-tauri/src/commands/forecast_cmds.rs:1126`)
- `annual_metrics(pool, year, today)` → all 12 months' realized metrics, not bound to today.
  (`src-tauri/src/commands/forecast_cmds.rs:1081`) — `AnnualScreen` already fetches this for
  metrics (line ~370), just not for the Saldo-fim column.

## Current state

- `src/lib/api.ts` — `getMonthGrid` binding was removed; the Rust command `get_month_grid`
  (registered name) / `month_grid` still exists. There is currently **no** `MonthGridDay` type
  or `getMonthGrid` function here. (Other bindings like `getForecast`, `getAnnualMetrics`,
  `tagTotalsForMonth` follow the pattern `export function x(...) { return invoke("snake_cmd", {...}); }`.)
- The Rust DTO (in `forecast_cmds.rs`, around line 1108) is:
  ```rust
  pub struct MonthGridDayDto {
      pub date: String,
      pub day: u32,
      pub income_cents: i64,
      pub fixed_out_cents: i64,
      pub daily_out_cents: i64,
      pub balance_cents: Option<i64>,
  }
  ```
  Confirm the exact Tauri command name registered for it (grep `invoke_handler` /
  `#[tauri::command]` near `month_grid` in `src-tauri/src/`); earlier code used
  `invoke("get_month_grid", { year, month })`. **Use whatever name is actually registered.**
- `src/screens/YearGridScreen.tsx` (~line 88–91): builds `balanceMap` from
  `getForecast` → `forecast.daily[]`; past dates aren't in the map → cells render "—".
- `src/screens/AnnualScreen.tsx`: fetches `getAnnualMetrics` (~line 370) AND `getForecast`;
  the "Saldo fim" column is built from `forecast.month_end[]` (`buildEndMap`, ~line 385) and a
  guard `const showEndBal = year === forecastYear && endBal !== undefined` (~line 234) →
  false for Jan–May.
- `src/screens/TotaisScreen.tsx` (~line 382): reads `forecast.months[]`; for a past month
  `months.findIndex(...)` is -1 (~line 387) and the trend slice (~line 434–435) only has
  current+future months.
- Conventions: data is fetched via the `useCommand(key, fetcher)` hook returning
  `{data, error, loading}`; multiple commands per screen is normal (see AnnualScreen already
  using two). Money is integer cents; format with `fmtBRL`/`formatBRL`. Strict TS
  (`noUncheckedIndexedAccess`) — guard array indexing.

## Commands you will need

| Purpose      | Command                         | Expected on success |
|--------------|---------------------------------|---------------------|
| Typecheck    | `npm run typecheck`             | exit 0              |
| Lint         | `npm run lint`                  | exit 0              |
| Unit test    | `npm run test:run`              | all pass            |
| E2E          | `npm run e2e`                   | all pass            |
| Rust (binding sanity) | `npm run rust:check`   | exit 0 (only if you touch Rust — you should NOT need to) |
| Find cmd name | `grep -rn "month_grid" src-tauri/src` | shows the registered command name |

## Scope

**In scope:**
- `src/lib/api.ts` — re-add the `getMonthGrid` binding + `MonthGridDay` type.
- `src/screens/YearGridScreen.tsx` — use month-grid balances for the displayed month's days.
- `src/screens/AnnualScreen.tsx` — fill "Saldo fim" for past months.
- `src/screens/TotaisScreen.tsx` — feed the trend with past months from `getAnnualMetrics`.
- The corresponding `*.test.tsx` for the three screens + `src/test/commands.ts` (add a
  `MONTH_GRID`-style mock fixture for the new command).

**Out of scope (do NOT touch):**
- Any Rust file. The commands already exist; if a command is genuinely missing or unregistered,
  that is a STOP condition (do not add Rust).
- The forecast formula / engine. This plan only sources *past* balances from existing commands;
  it must not change how *future* projection works.
- `getMonthGrid`'s removal from knip: re-adding the binding makes it used again; if `npm run
  deadcode` (knip) complains it is unused, that means a screen isn't actually calling it — fix
  the wiring, don't re-delete.

## Git workflow

- Branch: `advisor/058-restore-historical-balances`
- Message style: `fix(screens): restore past balances in Calendário/O ano/Este mês (redesign regression)`

## Steps

### Step 1: Re-add the `getMonthGrid` binding

In `src/lib/api.ts`, add the type + function mirroring the Rust DTO and the existing binding
style. Use the **actual registered command name** (verify via grep, likely `get_month_grid`):
```ts
export interface MonthGridDay {
  date: string;
  day: number;
  income_cents: number;
  fixed_out_cents: number;
  daily_out_cents: number;
  balance_cents: number | null;
}
export function getMonthGrid(year: number, month: number): Promise<MonthGridDay[]> {
  return invoke("get_month_grid", { year, month });
}
```
**Verify**: `npm run typecheck` → exit 0.

### Step 2: Add a mock fixture for the command

In `src/test/commands.ts`, add a `MONTH_GRID` fixture and register `get_month_grid` in the
mock command map (follow how `get_forecast` / `get_annual_metrics` are mocked there). Include a
few past days with non-null `balance_cents` and a couple with `null`.
**Verify**: `npm run typecheck` → exit 0.

### Step 3: Calendário — past-day balances

In `src/screens/YearGridScreen.tsx`, for the **displayed month**, fetch
`getMonthGrid(year, month)` via `useCommand` and build a date→`balance_cents` map from it.
Render each day's saldo from the month-grid map; for the **current + future** days keep using
`forecast.daily` (the projection) so today-forward still reflects the live forecast. Concretely:
overlay — month-grid balance for days `< today`, forecast balance for days `>= today`. Days
with `balance_cents === null` in the grid (not imported) still show "—".
**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 4: O ano — Saldo fim for past months

In `src/screens/AnnualScreen.tsx`, populate the "Saldo fim" column for past months from
realized history. Approach: for each past month of the displayed year, take the **last day with
a non-null `balance_cents`** from `getMonthGrid(year, month)` as that month's end balance; for
the current + future months keep the existing `forecast.month_end[]` source. Adjust the
`showEndBal` guard (~line 234) so past months with a realized end balance also render.
- To avoid 12 calls, you MAY fetch only the past months of the displayed year (Jan..current-1).
- If you find this too chatty and want a single query instead, that is a documented
  alternative but it requires a new Rust command — which is **out of scope**; do the
  per-past-month `getMonthGrid` approach here.
**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 5: Este mês — trend over past months

In `src/screens/TotaisScreen.tsx`, also fetch `getAnnualMetrics(currentYear)` and **merge** its
`months[]` with `forecast.months[]`, deduping by `(year, month)` and preferring the forecast
version for current+future months and the annual-metrics version for past months. Feed the
merged list to the "Resultado nos últimos meses" trend so it shows the real prior months.
**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 6: Tests

Update/extend `src/screens/YearGridScreen.test.tsx`, `AnnualScreen.test.tsx`,
`TotaisScreen.test.tsx` (create if absent; pattern: `DashboardScreen.test.tsx`) so that with
the `MONTH_GRID` / annual-metrics mocks containing realized past data, the screens render past
balances (not "—") and the trend includes >1 month.
**Verify**: `npm run test:run` → all pass.

## Test plan

- Calendário: with a month-grid mock having `balance_cents` for past days, assert those day
  cells show a formatted R$ value (not "—").
- O ano: with realized months, assert the "Saldo fim" cell for a past month shows a value.
- Este mês: with `getAnnualMetrics` returning ≥3 realized months, assert the trend renders >1
  month.
- Keep the e2e green (the existing Calendário/O ano/Este mês e2e assert headings + tabs).

## Done criteria

- [ ] `src/lib/api.ts` exports `getMonthGrid` + `MonthGridDay`
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; new/updated tests assert past balances render in all three screens
- [ ] `npm run e2e` exits 0
- [ ] `npm run deadcode` (knip) shows no NEW unused exports (getMonthGrid is now used)
- [ ] No Rust files modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

- No Tauri command for the month grid is registered (grep finds the function but not a
  `#[tauri::command]` wrapper / handler entry) → STOP; this plan assumes the command exists.
- `forecast.month_end` / `forecast.daily` / `forecast.months` shapes differ from the excerpts → STOP.
- Wiring a screen would require changing the forecast engine → STOP (out of scope).

## Maintenance notes

- This re-couples the historical screens to `sheet_daily_balance` (via `month_grid`) for the
  past and `forecast` for the future. If a future change unifies past+future into one command,
  collapse these two sources then.
- Reviewer: confirm the today-boundary overlay is correct (no double source for `today`), and
  that empty/never-imported days still show "—" rather than `R$ 0,00`.
- Deferred (not here): a single `get_monthly_end_balances(year)` Rust command would make O ano
  one query instead of N; raise as a separate plan if the per-month fetch is too slow.
