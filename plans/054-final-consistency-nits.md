# Plan 054: Final consistency nits — SUM(ABS) in effective_daily_ceiling + tag-exclude parity in annual savings

> **Executor instructions**: Two small consistency fixes from the final
> convergence sign-off (the app is otherwise clean — no P0/P1/P2). Each gets a
> regression test. Run every verification command. STOP if a fix changes a
> realized number unexpectedly. When done, flip the plan-054 row in
> `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat 131d5fe..HEAD -- src-tauri/src/commands/forecast_cmds.rs`

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Category**: bug (consistency)
- **Depends on**: none
- **Planned at**: commit `131d5fe`, 2026-06-21

## Why this matters

The final whole-app sign-off found NO P0/P1/P2 bugs. Two P3 consistency nits
remain — both are "correct today by a storage invariant" but inconsistent with
the patterns established elsewhere, so closing them removes latent risk and
finishes the work cleanly.

## Current state (verify line numbers — they shifted after plans 052/053)

1. **`effective_daily_ceiling`** (`src-tauri/src/commands/forecast_cmds.rs`, the
   prev-month diário-average fallback): the SQL uses
   `ABS(COALESCE((SELECT SUM(amount) ...), 0))` — an OUTER `ABS` of the SIGNED
   sum. Everywhere else in the expense-aggregation family this was changed to
   per-row `SUM(ABS(amount))` (plan 041 month_grid, plan 049
   realized_monthly_baseline, plan 053 daily_spend_today) because imported
   expenses are stored negative and manual positive — a mixed-sign day/period
   cancels before the outer ABS. This is the last spot still on the old pattern.
2. **`realized_annual_savings`** (and `projected_annual_savings` if it has the
   same shape) in `forecast_cmds.rs`: aggregates income/expense rows WITHOUT the
   `exclude_from_totals` tag filter, while `load_year_events` (feeding
   annual_metrics) DOES apply `NOT EXISTS (... transaction_tag ... tag.exclude_from_totals=1)`.
   So a transaction tagged "Ignorar" is excluded from the metric path but still
   counted in realized/projected annual savings → the two diverge.

## Commands

`npm run rust:check` · `npm run typecheck` · `npm run test:run` · `npm run check`

## Scope

In scope: `src-tauri/src/commands/forecast_cmds.rs` + its tests. Out of scope:
the Performance formula (LOCKED), classify(), the Saldo chain, any flag, the 028
gates, the economia=annotation model (052).

## Steps

1. **effective_daily_ceiling → SUM(ABS(amount))**: change the outer-ABS to
   per-row ABS, matching month*grid/realized_monthly_baseline/daily_spend_today.
   \_Verify*: a test where the prior month has an imported (negative) + a manual
   (positive) daily expense → the ceiling reflects the sum of magnitudes, not the
   cancelled signed sum. `npm run rust:check`.
2. **annual savings tag-exclude parity**: add the same `exclude_from_totals`
   `NOT EXISTS` filter used in `load_year_events` to `realized_annual_savings`
   (and `projected_annual_savings` if applicable), so excluded-tag rows are
   dropped consistently across the metric and the savings/guardrail paths.
   _Verify_: a test where an excluded-tag expense/income is NOT counted in
   realized_annual_savings, matching load_year_events. `npm run rust:check`.

## Done criteria

- `grep -n "ABS(COALESCE" src-tauri/src/commands/forecast_cmds.rs` → no
  expense-sum occurrence remains (all are `SUM(ABS(...))`).
- `realized_annual_savings` (+ projected) carry the `exclude_from_totals` filter.
- `npm run rust:check` + `npm run check` → exit 0; the 2 regression tests pass.

## STOP conditions

- If adding the tag-exclude filter to annual savings changes a realized number
  for the current data in a way that contradicts the sheet, STOP and report.
- Do NOT touch the Performance formula, the economia model, or any flag.

## Maintenance notes

- After this, the `SUM(ABS(amount))` invariant for expense aggregation holds in
  ALL query sites — document that invariant near one of them so future SQL
  follows it.
