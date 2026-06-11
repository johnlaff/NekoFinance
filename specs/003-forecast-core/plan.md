# Plan: Forecast Core — Projected Running Balance

## Architecture

```
Frontend (invoke get_dashboard_summary)
  → Tauri command (imperative shell)
      → load seed = Σ balance of liquid cash accounts (today)
      → load realized transactions + projections + daily_checkin (Régua 1)
      → aggregate credit consumption per card cycle → lump at due_day (Régua 2)
      → map rows → Vec<CashflowEvent>
  → core::forecast::project(seed, events, horizon)   [PURE — no IO, no clock, no DB]
  → Forecast { daily[], month_end[], deepest_deficit, safe_to_spend_today, months[] }
  → command returns projected balance + metrics to the dashboard
```

Functional core / imperative shell (Constitution P3, P7). The core is a pure Rust module with **no
IO, no ambient clock, no DB** — every input (seed, events, horizon, "today") is passed in, so it is
trivially testable and deterministic. The shell (`commands.rs`) does all loading/mapping. **No new
migration** — this slice only reads the `001` schema.

## Core Model

```rust
// src-tauri/src/forecast/mod.rs  (pure)
enum EventKind { Income, FixedOut, Daily }      // Entrada / Saída / Diário
struct CashflowEvent { date: NaiveDate, kind: EventKind, amount_cents: i64, realized: bool }

struct DayPoint   { date: NaiveDate, balance_cents: i64 }
struct MonthEnd   { year: i32, month: u32, balance_cents: i64 }
struct MonthMetric { year, month, performance_cents, cost_of_living_cents,
                     real_daily_avg_cents, savings_rate_bps }   // bps = basis points (integer)
struct Forecast {
    daily: Vec<DayPoint>,
    month_end: Vec<MonthEnd>,
    deepest_deficit: DayPoint,        // min projected balance + its date
    safe_to_spend_today_cents: i64,   // max extra outflow today keeping min(future) >= 0
    months: Vec<MonthMetric>,
}
fn project(seed_cents: i64, today: NaiveDate, events: &[CashflowEvent], horizon_end: NaiveDate) -> Forecast
```

Row → event mapping (shell, tested separately from the pure core):

- **Income**: `transaction.type='income'` (+ projections) on its `date`.
- **Daily** (Régua 1): `daily_checkin.daily_spend` per date (variable débito/cash).
- **FixedOut** (Saída): `transaction` expense with `is_fixed=1` on its date **+** the credit lump =
  Σ `credit_spend` per card cycle, placed on the card `due_day` (Régua 2).
- `realized = NOT is_projection`. Metrics: `cost_of_living = fixed_out + daily + card`;
  `performance = income − all_out`; `savings_rate = saved / income`; cash balance and performance
  are reported as **distinct** numbers.

## Risks

1. **Row→event mapping** is where bugs hide (which rows are Income/FixedOut/Daily; aggregating credit
   into one lump per cycle at `due_day`). Mitigate: keep mapping in the shell with its own tests +
   synthetic fixtures; the pure core is mapping-agnostic.
2. **Seed staleness**: the curve shifts if `account.balance` ≠ real cash today. This slice seeds from
   `Σ liquid account.balance` and documents the seam; bank reconciliation is a later slice.
3. **Month/year boundary**: the chain must carry Dec→Jan across years (the sheet does). Explicit
   boundary tests.
4. **Cash vs Performance conflation** (classic methodology bug): assert both independently in tests.
5. **Behavioral change to `balance`**: its meaning flips (current → projected); the frontend label and
   any callers must update in lockstep (US8) to avoid a misleading hero number.

## Dependencies

- Existing `001` schema and the `get_dashboard_summary` command (`src-tauri/src/commands.rs`).
- `chrono` (Rust) for `NaiveDate` month/day arithmetic (add if not already a dep).
- `uuid` (already present). No new external services, no cloud, no network.

## Data Boundaries

- Pure core touches **no** files, secrets, tokens, or network. All data arrives as function arguments.
- The shell reads only the local `neko-finance.db`. Fixtures are **synthetic and privacy-clean**.
- Spec, plan, tasks, and tests contain **no** personal financial data (Constitution P1); the personal
  mapping stays in the local methodology pack only.

## Testing Strategy

- **TDD mandatory** (Constitution NFR; AGENTS finance-math rule): write the failing test, then the code.
- **Pure-core unit tests** (`cargo test`): chaining formula; month/year boundary continuity; deepest
  deficit; safe-to-spend (cross-checked against the deficit); month metrics; cash≠performance; the
  credit-lump-at-due-date case; determinism (same input → same output).
- **Shell mapping tests**: rows → `CashflowEvent[]` for each transaction type, projections, and credit
  cycle aggregation.
- **Integration test**: `get_dashboard_summary` on a fixture DB returns the projected (not current)
  balance.
- **Coverage target**: ≥ 90% on `src-tauri/src/forecast/`. Do not lower coverage to pass.

## Release Implications

- **No migration, no data migration** — pure computation + a command swap; fully reversible.
- **Behavioral change**: `get_dashboard_summary.balance` flips from current sum to projected. Frontend
  must update the hero label/semantics (Saldo → Saldo projetado de fim de mês) in the same change.
- The engine becomes the **authoritative financial calculator** (Constitution P3); later decision tools
  (Mia "posso comprar?/financiar?", scenarios, invoice/reembolso) build on it — keep its API stable.
- Gate: `npm run check` (typecheck, lint, test, build, Rust checks, privacy scan) green before done.
