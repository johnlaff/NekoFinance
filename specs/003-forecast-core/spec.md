# Spec: Forecast Core — Projected Running Balance

## Summary

Implement the deterministic **projected running-balance engine** that is the heart of the
methodology: a forward-looking, day-by-day projected cash balance computed from realized
transactions and future projections, plus the derived decision metrics (projected month-end
balance, future deficit, safe-to-spend, performance, cost of living, real daily average, savings
rate). This replaces the current `get_dashboard_summary.balance`, which returns the **current**
account sum (`SUM(account.balance)`) — a backward-looking number that contradicts the method.

The engine is a **pure functional core** (no IO) with an imperative shell adapter, fully unit-tested
(TDD), and is the **authoritative financial calculator** (the copilot never computes balances).

## Motivation

The methodology is forecast-first: the hero number is the **projected end-of-month balance** ("how
much will be left over, or missing"), and every entry instantly changes that projection. Today the
dashboard shows `SUM(account.balance)` (the present), which is the opposite of the method — it
cannot answer "can I spend / should I wait" because it never looks forward. Schema `001` already
provides transactions, projections (`is_projection`), per-day dual tracking (`daily_checkin`:
Régua 1 débito vs Régua 2 credit), accounts, and reserve — but no engine chains them into a
projected balance. This spec adds that engine; it unblocks the dashboard, the daily speedometer,
and every copilot decision tool.

## User Stories

### US1 — Projected running balance (the chain)

**As** the primary user
**I want** a day-by-day projected cash balance over a forward horizon
**So that** I can see what my balance will be on any future day, like the spreadsheet does.

**Acceptance**: A pure function computes `saldo[d] = saldo[d-1] + Σincome[d] − (Σfixed_out[d] +
Σdaily[d])` for each day in the horizon, chaining continuously across month and year boundaries
(the last day of a month seeds the first day of the next). Inputs are realized transactions +
projections; output is an ordered series of `{date, projected_balance}`. INTEGER cents throughout.
Deterministic: identical inputs always yield identical output. Tested before implementation.

### US2 — Today's seed reconciled to real cash

**As** the primary user
**I want** the projection to start from my real current cash balance today
**So that** the forecast reflects reality, not a stale or guessed starting point.

**Acceptance**: The seed = sum of **liquid cash accounts** at "today" (for this slice, the existing
`type IN ('bank','wallet','savings')`). The day-0 projected balance equals that seed. The engine
takes the seed as an explicit input (the shell supplies it), so the core stays IO-free. _(A richer
account liquidity classification — restricted vouchers, illiquid funds — is a later slice; this
slice keeps the current cash-account set and documents the seam.)_

### US3 — Projected month-end balance (the hero)

**As** the primary user
**I want** the projected balance at the end of the current and upcoming months
**So that** I know in advance whether each month ends in the green or the red.

**Acceptance**: For each month in the horizon the engine returns the projected end-of-month balance.
`get_dashboard_summary` exposes the projected end-of-current-month as the hero figure (replacing the
current-balance field; see US8).

### US4 — Future deficit ("the hole")

**As** the primary user
**I want** to know the deepest negative point in my projected future and when it occurs
**So that** I can size exactly how much "new money" is needed to stay solvent.

**Acceptance**: The engine returns the minimum projected balance across the horizon and its date. If
no day is negative, it returns the minimum positive trough. Used by future debt/credit decision tools.

### US5 — Safe-to-spend today

**As** the primary user
**I want** to know how much more I can spend today without pushing any future day negative
**So that** I get a real-time "can I spend / parcel / wait" answer.

**Acceptance**: The engine returns the maximum additional outflow that can be added today such that
the minimum projected balance over the horizon stays ≥ 0. Deterministic; tested against the deficit
in US4.

### US6 — Monthly decision metrics (Totais)

**As** the primary user
**I want** Performance, Cost of living, Real daily average, and Savings rate per month
**So that** I can judge whether my month is structurally healthy, not just the cash trough.

**Acceptance**: From the same event stream the engine computes, per month: **Performance** =
income − all outflows; **Cost of living** = fixed outflows + daily + card; **Real daily average** =
realized daily spend ÷ elapsed days; **Savings rate %** = saved ÷ income, flagged against the 20–30%
target band. Cash balance and performance are reported as **distinct** numbers (a month can end with
a small negative balance while performance is healthy, or vice-versa).

### US7 — Credit invoices and dual tracking in the projection

**As** a user who currently spends mostly on credit
**I want** credit consumption to land in the projection as a lump on the invoice due date, while
débito daily spend hits day by day
**So that** the projection stays correct whether I pay by débito or credit.

**Acceptance**: Future credit consumption is reflected as a fixed-outflow event on the card
**due day** (Régua 2 / `credit_spend` aggregated per cycle), while débito daily spend (Régua 1 /
`daily_spend`) reduces the balance on the day it occurs. _(A first-class invoice entity with
per-item owner attribution and reimbursement links is a later slice; this slice consumes the
already-tracked credit aggregates and projects them at the due date.)_

### US8 — Replace the dashboard balance

**As** the primary user
**I want** the dashboard summary to show the projected balance, not the current account sum
**So that** the headline number matches the forecast-first method.

**Acceptance**: `get_dashboard_summary` returns the **projected** balance (computed via the engine)
in place of `SUM(account.balance)`. The response shape stays compatible with the existing frontend
fields; only the value's meaning changes (documented). No regression in unrelated summary fields.

### US9 — Demo fixture for end-to-end verification

**As** a developer
**I want** a synthetic seed dataset that exercises the engine
**So that** the projected balance can be seen running in the real app, not only in unit tests.

**Acceptance**: A fixture (synthetic, privacy-clean) seeds accounts + transactions + projections
producing a known projected curve (including at least one future negative trough), so the dashboard
renders a real forecast under `npm run tauri dev`.

## Non-functional requirements

- **Functional core / imperative shell**: the projection is a pure Rust module with no DB/IO; the
  Tauri command is a thin adapter that loads rows and supplies the seed (Constitution P3, P7).
- **TDD is mandatory** (Constitution NFR; AGENTS finance-math rule): tests precede implementation and
  cover the chaining formula, month/year boundary continuity, the future deficit, safe-to-spend, the
  performance-vs-cash distinction, and the credit-lump-at-due-date behavior.
- **Deterministic & authoritative** (Constitution P3): the engine — never the LLM — computes balances.
- **Money = INTEGER cents; dates = ISO 8601 TEXT; IDs = UUID v4** (consistent with `001`).
- **Privacy** (Constitution P1): this spec and all fixtures are synthetic and data-free; the personal
  mapping lives only in the local methodology pack.
- **Scope boundary (small reversible slice, Constitution P7)**: account liquidity classes (voucher /
  pension / FGTS), the first-class invoice entity, owner/payer/beneficiary/responsible attribution
  (Constitution P5), reimbursement net-zero links, and what-if scenario branches are **out of scope
  here** and tracked as later slices. This slice only adds the engine + the dashboard swap + a fixture.
