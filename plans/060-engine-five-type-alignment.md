# Plan 060: Align the engine to the 5-type method (Cartão + Economia + Patrimônio buckets)

> **Executor instructions**: This plan changes finance math under an explicit owner decision.
> Follow step by step; run every verification command. The regression tests in Step 5 are the
> safety net — if any of them fail, STOP. When done, update `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs src-tauri/src/google_sheets/import.rs`
> Compare excerpts to live code before proceeding.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH (finance math; reopens a locked decision under owner authorization)
- **Depends on**: plans/059-line-item-classification-core.md
- **Category**: methodology / engine
- **Planned at**: commit `da2d3e9`, 2026-06-22

## Why this matters

Neko's engine has 4 `EventKind`s (Income, FixedOut, Daily, Economia) and folds **Cartão** into
FixedOut, and (per the locked 051/052 model) keeps **Economia logged as a Saída** inside
cost-of-living. The owner confirmed against the canonical method (per the methodology reference
material) that this is wrong: there are **5 types**, **custo de vida excludes economia and
patrimônio**, **Cartão is its own bucket**, and **Economia% = economia ÷ entradas**. This plan
changes the
engine accordingly, driven by the per-item classifier from plan 059 so a single Saída lump is
split into its true types.

## 🔓 Owner decision (reopens locked 051/052)

The README "considered and rejected" note and `specs/011-engine-five-types` locked
`Performance = income − cost_of_living` with economia _inside_ cost_of_living, and warned only an
explicit owner decision may reopen it. **That decision has been made** (recorded in the plan 059
spec). Implement the canonical model below.

### Target model (must match the worked example)

For a month with Entradas 10.000, fixed Saídas 3.000, Diário 2.000, Cartão 1.500, Economia 1.000:
| Metric | Target value |
|---|---|
| Custo de vida | **6.500** = Saídas(3.000) + Diário(2.000) + Cartão(1.500) — economia/patrimônio EXCLUDED |
| Gastos com cartão | **1.500** (own bucket) |
| Economia % | **10%** = economia(1.000) ÷ entradas(10.000) |
| **Performance** | **2.500** = entradas − total-outflow(7.500) — **UNCHANGED vs today** |
| **Saldo** | **UNCHANGED vs today** |

**Hard invariant**: Saldo and Performance are identical to the pre-plan values for every
existing fixture (the money leaves the account in both models). Only custo-de-vida (no longer
inflated by economia), Economia%, and the Cartão/Patrimônio buckets change.

## Current state

- `src-tauri/src/forecast/mod.rs`:
  - `enum EventKind { Income, FixedOut, Daily, Economia }` (line ~17).
  - `classify(txn_type, is_fixed, payment_method, to_account) -> Option<EventKind>` (line ~240):
    expense + (`is_fixed` || `payment_method=="credit"`) → FixedOut; else Daily; transfer to
    `reserve`/`illiquid` → Economia.
  - The metric loop (lines ~360–377) sums `income`, `fixed_out`, `daily`, `economia` per event;
    `cost_of_living` and `performance` are derived from these (grep `cost_of_living_cents`,
    `performance_cents`, `savings_rate_bps` in this file and `month_metrics*`).
  - `project_with_metrics` produces `MonthMetric`; `month_metrics_for` is used by `annual_metrics`.
- `src-tauri/src/google_sheets/import.rs`: `line_item` rows carry `section` (plan 059 adds
  `classify_line_item` → `ItemKind`). A Saída lump's note items are in `line_item`.
- Neko pockets already distinguish `reserve` (liquid) vs `illiquid` accounts — map **Patrimônio**
  to the illiquid concept.
- DTOs (`MonthMetricDto` in `forecast_cmds.rs`) currently expose `income/performance/
cost_of_living/fixed_out/daily_out/economia/savings_rate_bps` — you will ADD `cartao_cents`
  and `patrimonio_cents` (additive; don't remove fields).

## Commands you will need

| Purpose         | Command                                                    | Expected                          |
| --------------- | ---------------------------------------------------------- | --------------------------------- |
| Rust check+test | `npm run rust:check`                                       | exit 0                            |
| Engine tests    | `cargo test --manifest-path src-tauri/Cargo.toml forecast` | pass                              |
| Frontend types  | `npm run typecheck`                                        | exit 0 (if you touch the TS DTOs) |

## Scope

**In scope:**

- `src-tauri/src/forecast/mod.rs` — add `EventKind::Cartao` + `EventKind::Patrimonio`; make the
  metric loop attribute **per line-item** (via `classify_line_item`) when a transaction has
  classified items, else fall back to the transaction-level `classify()`; recompute
  `cost_of_living = fixed_out + daily + cartao` (EXCLUDING economia + patrimonio);
  `savings_rate_bps = economia / income`; keep `performance = income − total_outflow` unchanged.
- `src-tauri/src/commands/forecast_cmds.rs` — add `cartao_cents` + `patrimonio_cents` to the
  month DTOs (additive).
- `src/lib/api.ts` — extend the TS `MonthMetric` type with the new fields (additive).
- Tests in the Rust files.

**Out of scope:**

- The classifier itself (plan 059). The Economia-tab write-back (plan 062). UI (plan 061).
- Removing/renaming existing DTO fields (additive only — don't break current screens).
- Changing the Saldo chain (`sheet_daily_balance`) — it is the sheet's truth; do not recompute.

## Git workflow

- Branch: `advisor/060-engine-five-type-alignment`
- Message: `feat(engine): 5-type model — cartão/economia/patrimônio buckets, custo de vida excludes savings`

## Steps

### Step 1: Add the two EventKinds

Add `Cartao` and `Patrimonio` to `EventKind`. Update `signed_amount`/any exhaustive match: Cartao
and Patrimonio are outflows (negative), like FixedOut. **Verify**: `cargo check ...` → exit 0.

### Step 2: Per-line-item attribution

In the metric loop, for a transaction that HAS line_items (from plan 059), attribute each item's
`amount_cents` to the bucket for `classify_line_item(section, description)`:
`Saida/Ajuste → FixedOut`, `Cartao → Cartao`, `Diario → Daily`, `Economia → Economia`,
`Patrimonio → Patrimonio`. For a transaction WITHOUT line_items, keep the existing
transaction-level `classify()` (so plain entries still work; map a `credit` payment_method →
Cartao now). **Guard against double-counting**: when items are used, do NOT also count the parent
amount. **Verify**: `cargo check ...` → exit 0.

### Step 3: Recompute the metrics

- `cost_of_living_cents = fixed_out + daily + cartao` (EXCLUDE economia + patrimonio).
- `performance_cents = income − (fixed_out + daily + cartao + economia + patrimonio)` — i.e.
  income minus ALL outflows; this must equal the OLD performance value (since old performance
  already subtracted everything that left the account). Confirm numerically in tests.
- `savings_rate_bps = round(economia / income)` (basis points), income>0 else 0.
- Expose `cartao_cents`, `patrimonio_cents`.
  **Verify**: `cargo test ... forecast` → existing tests adjusted + compile.

### Step 4: DTOs + TS type

Add `cartao_cents` + `patrimonio_cents` to `MonthMetricDto` (and `MonthMetric` in `api.ts`).
**Verify**: `npm run rust:check` + `npm run typecheck` → exit 0.

### Step 5: Regression + new tests (CRITICAL)

- **Regression (the invariant)**: for every existing engine fixture, `performance_cents` and the
  Saldo chain are **byte-identical** to before this plan. Add an explicit test using the worked
  example numbers asserting performance == income − total_outflow and that it equals the
  pre-change value.
- **New behavior**: with a Saída lump whose items are `CONTAS` + `CARTÕES` + `ECONOMIA` +
  `INVESTIMENTO`, assert: cost_of_living excludes the economia + patrimônio portions; cartao_cents
  == the CARTÕES sum; economia feeds savings_rate; patrimonio_cents == the INVESTIMENTO sum.
- Use the exact worked-example month and assert every row of the target table.
  **Verify**: `cargo test ... forecast` → all pass, including regression + the worked-example test.

## Test plan

- Worked-example month → the target table values exactly.
- Regression: Performance + Saldo unchanged for pre-existing fixtures.
- Plain (no-items) transactions still classify (credit→Cartao, reserve→Economia, illiquid→Patrimonio).
- No double-count (parent vs items).
- Pattern: existing `#[cfg(test)]` tests in `forecast/mod.rs` + `forecast_cmds.rs`.

## Done criteria

- [ ] `EventKind` has Cartao + Patrimonio; metric loop attributes per line-item without double-count
- [ ] `cost_of_living = fixed_out + daily + cartao` (economia + patrimônio excluded)
- [ ] `performance` + Saldo unchanged vs pre-plan (regression test proves it)
- [ ] `savings_rate_bps = economia / income`; `cartao_cents` + `patrimonio_cents` exposed
- [ ] `npm run rust:check` + `npm run typecheck` exit 0
- [ ] worked-example test asserts the full target table
- [ ] `plans/README.md` updated

## STOP conditions

- Any regression test shows Performance or Saldo changed for an existing fixture → STOP (the
  reclassification leaked into the wrong place).
- You cannot attribute per-item without double-counting the parent → STOP and report the design issue.
- The locked-decision docs (`specs/011`) lack the owner-reopen note from plan 059's spec → STOP
  (do not reopen locked math without the recorded decision).

## Maintenance notes

- This supersedes the economia=Saída framing of 051/052; update `specs/011-engine-five-types` to
  reference the new spec from plan 059.
- Reviewer: the single most important check is the Performance/Saldo regression — those must not move.
- Follow-up (plan 062): the now-automatic Economia must be written back to the spreadsheet
  Economia tab so the app and sheet stay in sync.
