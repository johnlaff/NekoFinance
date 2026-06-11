# Tasks: Forecast Core — Projected Running Balance

> TDD: each implementation task is preceded by a failing test. Pure core first, shell last.

## Phase 1 — Core module skeleton ✅

- [x] T1.1 Create `src-tauri/src/forecast/mod.rs`; register module in `lib.rs`
- [x] T1.2 Define types: `EventKind`, `CashflowEvent`, `DayPoint`, `MonthEnd`, `MonthMetric`, `Forecast`
- [x] T1.3 ~~Add `chrono`~~ (already a dependency); stub `project(seed, today, events, horizon_end)` returning empty
- [x] T1.4 Confirm `cargo test` compiles with a trivial passing test

## Phase 2 — Daily chaining (US1) ✅ (TDD red→green, 5 tests, clippy+fmt clean)

- [x] T2.1 Test: single month — `saldo[d] = saldo[d-1] + income − (fixed_out + daily)`; day-0 = seed
- [x] T2.2 Test: month boundary — last day of month seeds first day of next
- [x] T2.3 Test: **year boundary** (Dec → Jan) continuity
- [x] T2.4 Test: determinism — same input yields identical output; empty events → flat seed line
- [x] T2.5 Implement the chaining loop to green

## Phase 3 — Month-end hero + future deficit (US3, US4) ✅

- [x] T3.1 Test: `month_end[]` returns the projected balance on each month's last day
- [x] T3.2 Test: `deepest_deficit` = min projected balance + its date (negative trough)
- [x] T3.3 Test: all-positive horizon → `deepest_deficit` is the minimum positive trough
- [x] T3.4 Implement month-end + deficit reducers to green

## Phase 4 — Safe-to-spend today (US5) ✅

- [x] T4.1 Test: `safe_to_spend_today` = max extra outflow today keeping `min(future) >= 0`
- [x] T4.2 Test: when already negative ahead → safe-to-spend is 0 (not negative)
- [x] T4.3 Test: cross-check — spending exactly `safe_to_spend_today` makes the trough touch 0
- [x] T4.4 Implement to green

## Phase 5 — Monthly metrics / Totais (US6) ✅

- [x] T5.1 Test: `performance = income − all_out`; `cost_of_living = fixed_out + daily + card`
- [x] T5.2 Test: `real_daily_avg` = realized daily ÷ elapsed days; `savings_rate_bps` vs 20–30% band
- [x] T5.3 Test: **cash ≠ performance** — a month ends slightly negative in cash while performance is positive
- [x] T5.4 Implement metric reducers to green

## Phase 6 — Credit dual-tracking (US7) ✅ (core behavior; cycle aggregation lands in Phase 7 mapping)

- [x] T6.1 Test: a `Daily` (Régua 1, débito) event reduces balance on its day
- [x] T6.2 Test: a credit cycle aggregate becomes one `FixedOut` lump on the card `due_day` (Régua 2)
- [x] T6.3 Test: future credit lumps depress future months without touching the daily line
- [x] T6.4 Implement credit-lump **aggregation** (Σ `credit_spend` per cycle → due_day) in the mapping (Phase 7)

## Phase 7 — Shell adapter, seed, and dashboard swap (US2, US8) ✅

- [x] T7.1 Test: row→event mapping — income/expense(is_fixed)/projection/daily_checkin → `CashflowEvent[]`
- [x] T7.2 Test: credit cycle aggregation (Σ `credit_spend` per cycle) → lump at `due_day`
- [x] T7.3 Implement mapping in `commands.rs` (shell)
- [x] T7.4 Implement seed = Σ balance of liquid accounts (`type IN ('bank','wallet','savings')`) at today
- [x] T7.5 Replace `get_dashboard_summary.balance` (`SUM(account.balance)`) with the projected value
- [x] T7.6 Integration test: `get_dashboard_summary` on a fixture DB returns the **projected** balance

## Phase 8 — Fixture, frontend, gates (US9)

- [x] T8.1 Create a synthetic fixture (accounts + transactions + projections + a future negative trough)
- [x] T8.2 Update the dashboard hero label/semantics: Saldo → **Saldo projetado de fim de mês**
- [~] T8.3 Verify end-to-end under `npm run tauri dev` (forecast curve renders from the fixture) — **não verificável em ambiente headless; adiada para validação com display**
- [x] T8.4 Run `npm run check` — typecheck, lint, test, build, Rust checks, privacy scan all green
- [x] T8.5 Confirm coverage ≥ 90% on `src-tauri/src/forecast/` — **99.06% regiões, 100% funções, 99.38% linhas**

## Parallelization notes

- Phases 2–6 are pure-core and can be developed independently after Phase 1 (each is a separate reducer).
- Phase 7 (shell) depends on the core API being stable (end of Phase 6).
- Phase 8 depends on Phase 7.
- Out of scope (later slices, per spec): account liquidity classes, first-class invoice entity,
  owner/payer/beneficiary/responsible attribution, reimbursement links, what-if scenarios.
