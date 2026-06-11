# Spec: Forecast View — Daily Projection on the Dashboard

## Summary

Surface the forecast engine (spec `003`) in the UI. Today the dashboard shows only the projected
month-end number; the engine's other outputs — the day-by-day projected balance, the deepest future
deficit, and safe-to-spend-today — are computed and tested but invisible. This spec adds a
`get_forecast` Tauri command exposing the projection as a DTO, and reorients the dashboard around
it: a "pode gastar até X" callout, a deficit warning when the month dips negative, and the daily
chained projection table (Data / Entrada / Saída / Diário / Saldo) that is the methodology's core
reading surface.

## Motivation

The methodology's daily ritual is reading the chained daily balance: every future day's projected
saldo, with entradas and saídas on the days they happen, so problems are visible weeks before they
arrive. The engine already produces this; without a screen it cannot drive any decision. This is
the last slice between the current build and a genuinely usable first release.

## User Stories

### US1 — Forecast exposed as a command

**As** the frontend
**I want** a `get_forecast` command returning the projection for the current month
**So that** any screen can render forecast data without recomputing finance math in JS.

**Acceptance**: `get_forecast` returns `{ today, horizon_end, safe_to_spend_today_cents,
deepest_deficit?, daily[], month_end[] }` where `daily[]` rows carry per-day flows
(`income_cents`, `fixed_out_cents`, `daily_out_cents`) plus the chained `balance_cents`. Dates are
ISO 8601 strings at the boundary (DTO mapping in the shell; the pure core stays serde-free). The
event assembly (transactions + credit-cycle lumps from check-ins) is shared with
`get_dashboard_summary` via one tested function — no duplicated mapping logic. Covered by
integration tests on a fixture DB before implementation (TDD).

### US2 — Safe-to-spend callout

**As** the primary user
**I want** the dashboard to tell me how much I can still spend today without any future day going
negative
**So that**"posso gastar?" has an instant, deterministic answer.

**Acceptance**: The dashboard hero area shows "pode gastar até X hoje" using
`safe_to_spend_today_cents` (0 floors at R$ 0,00, never negative). Money in tabular mono.

### US3 — Future deficit warning

**As** the primary user
**I want** an explicit warning when the projection dips below zero, with the date and depth
**So that** I can size the hole before it happens.

**Acceptance**: When `deepest_deficit.balance_cents < 0`, the dashboard shows a danger-tone notice
with the date (DD/MM) and the amount. Not color-only: icon + text per design-system rules. No
notice renders when the whole horizon stays ≥ 0.

### US4 — Daily projection table

**As** the primary user
**I want** the remaining days of the current month listed as Data / Entrada / Saída / Diário /
Saldo
**So that** the app reads like the methodology's daily sheet, but always up to date.

**Acceptance**: The dashboard's main card lists one row per day from today to month-end: per-day
inflow, fixed outflow, daily outflow (— when zero), and the chained projected balance. Today's row
is visually marked (and labeled "hoje"); negative balances render in the danger money color (with
the minus sign carrying the meaning, not color alone). Replaces the recent-transactions card on the
dashboard (that list now lives in the Transações screen).

## Non-functional requirements

- **Functional core / imperative shell**: no change to `forecast/mod.rs` semantics; the shell maps
  events → DTO. Per-day flow sums are shell aggregation, tested.
- **TDD**: Rust integration tests for `get_forecast` (chain values, flows, safe-to-spend, deficit
  presence/absence) and shared-assembly regression tests precede implementation; frontend component
  tests cover the callout, the warning, and the table.
- **Determinism**: all figures come from the engine; the frontend formats, never computes.
- **Scope boundary**: month selector/multi-month navigation, charts, and Régua-1/2 speedometers are
  later slices. Current month only.
