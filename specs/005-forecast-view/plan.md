# Plan: Forecast View — Daily Projection on the Dashboard

## Backend

```
commands.rs
  load_cashflow_events(pool, today, horizon_end) -> Vec<CashflowEvent>   ← extracted from
      dashboard_summary (transactions + credit-cycle lumps); single source of event mapping
  dashboard_summary(...)        → now calls load_cashflow_events (behavior unchanged)
  forecast_dto(pool, today)     → events → forecast::project → ForecastDto (testable, no State)
  get_forecast (#[tauri::command]) → thin wrapper with Local::now()
```

DTO (serde in shell only; core stays serde-free):

```rust
ForecastDayDto { date: String, income_cents, fixed_out_cents, daily_out_cents, balance_cents }
DayPointDto    { date: String, balance_cents }
MonthEndDto    { year, month, balance_cents }
ForecastDto    { today, horizon_end, safe_to_spend_today_cents,
                 deepest_deficit: Option<DayPointDto>, daily: Vec<ForecastDayDto>,
                 month_end: Vec<MonthEndDto> }
```

Per-day flows: aggregate `CashflowEvent`s by (date, kind) in the shell and zip with `fc.daily`
(both keyed by date; engine emits one `DayPoint` per day in horizon).

## Frontend

- `lib/api.ts`: `ForecastDay`, `DayPoint`, `MonthEnd`, `Forecast` interfaces + `getForecast()`.
- `DashboardScreen`: third parallel fetch. Hero gains the safe-to-spend line; a danger notice
  appears when `deepest_deficit < 0`. Main card becomes the daily projection table; the Mia aside
  stays. (Recent transactions live in the Transações screen since 004.)
- New CSS: `.dash-safe` (callout), `.dash-deficit` (notice), `.fc-table` today-row highlight —
  tokens only, money in `--font-money`.

## Testing

- Rust (TDD, red first): fixture DB → `forecast_dto` asserts (a) day-0 balance = seed, (b) chain
  drops by the expense on its date, (c) per-day flows match, (d) safe-to-spend = trough floor,
  (e) deficit present when negative / `None`-not-negative case, (f) dashboard regression tests
  still green after extraction.
- Frontend: callout renders formatted value; deficit notice only when negative; table renders one
  row per day, today marked, negative saldo styled; mocks via `test/commands.ts`.

## Risks

1. **Date alignment** between flow aggregation and `fc.daily` — both derive from the same event
   vector and horizon; zip by date map, assert in tests.
2. **Empty DB** → seed 0, flat line, safe-to-spend 0 — covered by a test (first-run experience).
3. **Dashboard layout regression** — visual pass + existing tests updated in the same change.
