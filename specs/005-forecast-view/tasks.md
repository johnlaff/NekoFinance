# Tasks: Forecast View — Daily Projection on the Dashboard

## Phase 1 — Shared event assembly (refactor under test)

- [x] T1.1 Extract `load_cashflow_events(pool, today, horizon_end)` from `dashboard_summary`
- [x] T1.2 Existing dashboard integration tests stay green (regression gate)

## Phase 2 — get_forecast (TDD)

- [x] T2.1 Tests (red): `forecast_dto` on fixture DB — chain, per-day flows, safe-to-spend,
      deficit Some/None, empty-DB flat line
- [x] T2.2 Implement DTOs + `forecast_dto` + `get_forecast`; register command (green)
- [x] T2.3 `cargo` gates green (fmt, clippy -D warnings, tests)

## Phase 3 — Dashboard reorientation

- [x] T3.1 Frontend tests: safe-to-spend callout, deficit notice (presence/absence), daily table
      rows + today marker + negative styling
- [x] T3.2 `lib/api.ts` types + `getForecast()`; DashboardScreen third fetch
- [x] T3.3 Hero: safe-to-spend line; deficit danger notice (icon + text)
- [x] T3.4 Daily projection table card (Data/Entrada/Saída/Diário/Saldo) replaces recent-txn card
- [x] T3.5 CSS: `.dash-safe`, `.dash-deficit`, `.fc-table` (tokens only)

## Phase 4 — Gates

- [x] T4.1 `npm run check` fully green
- [x] T4.2 Update spec/tasks checkboxes; commit slice
