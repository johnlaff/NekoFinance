# Plan 067 SPIKE (DESIGN-ONLY) — What-if / Scenario Branching of the Forecast

> **Deliverable**: Design document. No source edits.
>
> **Driven by**: `plans/067-spike-whatif-scenarios-refresh.md` §4a–§4i
>
> **Status**: DRAFT (2026-07-05)

---

## 1. Problem Statement

The method's what-if practice comes in two scoped forms. **(1) Quick single decision**: pre-launch the hypothetical row into the live year ledger at its future date, read the forward Saldo, then undo (Ctrl+Z) if unwanted — no separate simulation layer. **(2) Large lifestyle-scale scenario**: duplicate the whole sheet, label the copy `"(simulação X)"`, do all hypothetical lançamentos inside the copy, and never touch the real sheet.

This plan productizes both into one on-demand forecast branch — a **surpassing** convenience beyond the sheet (like liquidity pockets), not a 1:1 replication of a sheet feature. `scenario_id` isolation replaces both "undo" and "duplicate the file" with a saved, named entity; additive hypothetical rows replace the pre-launch primitive. The forecast engine is a pure function (`src-tauri/src/forecast/mod.rs`) over `seed_cents + &[CashflowEvent] + today + horizon_end + annotation` — no IO, no DB, no ambient clock. A scenario branch is therefore a **second call** to `project_with_metrics` with the same seed/today/horizon/annotation but `real_events ∪ hypothetical_events`. The only gaps are (a) persisting hypothetical transactions in isolation, and (b) a comparison DTO.

---

## 2. Scenario Data Model (Schema Sketch — NOT a Migration)

### 2.1 `scenario` table

```sql
CREATE TABLE scenario (
    id        TEXT PRIMARY KEY NOT NULL,  -- uuid
    name      TEXT NOT NULL,              -- "Mudança para SP", "Financ Celular"
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 2.2 `ALTER TABLE "transaction" ADD COLUMN scenario_id`

```sql
ALTER TABLE "transaction" ADD COLUMN scenario_id TEXT REFERENCES scenario(id) ON DELETE CASCADE;
```

- `NULL` = real ledger (existing rows stay NULL; no backfill)
- `NOT NULL` = hypothetical transaction owned by that scenario

**Invariants**:
- Scenario transactions reuse the exact `"transaction"` shape (`type/amount/date/payment_method/is_fixed/from_account_id/to_account_id/is_projection/due_date`). No separate table.
- Scenario transactions **never** write `account.balance`; the seed always comes from the real ledger via `projection_seed` (unchanged — `src-tauri/src/commands/forecast_cmds.rs:46`).
- Deleting a scenario cascades to its transactions.

### 2.3 `scenario_override` table

Plan 069's `obligation` owns the match rule (normalized description + normalized section + kind) and a resolver that returns the concrete `line_item`s it matches. The override never re-implements string matching.

```sql
CREATE TABLE scenario_override (
    id            TEXT PRIMARY KEY NOT NULL,
    scenario_id   TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    op            TEXT NOT NULL CHECK(op IN ('suppress','replace')),
    from_date     TEXT NOT NULL,           -- applies to occurrences on/after this date
    obligation_id TEXT REFERENCES obligation(id) ON DELETE CASCADE,  -- imported case (plan 069)
    recurrence_id TEXT,                     -- app-native series (spec 016)
    CHECK (obligation_id IS NOT NULL OR recurrence_id IS NOT NULL)
);
```

**Target:** For imported rows, `obligation_id` → `obligation` (069, `src-tauri/src/obligations.rs:19-27`), whose resolver returns matched `line_item`s (`obligation_items`, obligations.rs:285). For app-created series, `recurrence_id` (spec 016, migration `20240612000005_recurrence.sql`). Creating an override always previews the resolved rows (the confirm-preview from plan 069 — obligations.rs:196).

**Override types:**
- **Remove** ("cancelar a academia"): one `suppress` override → drops matched rows.
- **Change** ("aluguel +R$ 900 a partir de ago"): one `replace` override **plus** a new hypothetical row at the new amount, presented as one UI action.

### 2.4 `scenario_id IS NULL` — Complete Audit of Reads Over `"transaction"`

A nullable `scenario_id` on the shared table means **every function** that queries `"transaction"` must be audited. The following is the complete enumeration from `src-tauri/src/commands/forecast_cmds.rs` (verified at `b65f0c6`):

| Function | Line | Query touches `"transaction"` | Filter needed |
|---|---|---|---|
| `projection_seed` (gap query) | :70 | `FROM "transaction"` | `AND t.scenario_id IS NULL` |
| `forecast_horizon_end` | :691 | `FROM "transaction"` | `AND t.scenario_id IS NULL` |
| `load_cashflow_events` | :877 | `FROM "transaction" t` | `AND t.scenario_id IS NULL` |
| `load_forecast_events` | :900 | delegates to `load_cashflow_events` | inherited |
| `load_metric_db_events` (shared loader) | :755, :772 | `FROM "transaction" t` + `JOIN "transaction" t` | `AND t.scenario_id IS NULL` (once) |
| `load_metric_events` | :936–949 | delegates to `load_metric_db_events` | inherited |
| `load_realized_month_events` | :922 | delegates to `load_metric_db_events` | inherited |
| `realized_annual_savings` | :146 | `FROM "transaction" t` | `AND t.scenario_id IS NULL` |
| `realized_annual_economia` | :202, :229 | `JOIN "transaction" t` + `FROM "transaction" t` | `AND t.scenario_id IS NULL` (both) |
| `projected_annual_savings` | :331 | `FROM "transaction" t` | `AND t.scenario_id IS NULL` |
| `effective_daily_ceiling` | :426 | `FROM "transaction"` | `AND scenario_id IS NULL` |
| `realized_monthly_baseline` | :361 | delegates to `load_metric_db_events` | inherited |
| `load_year_events` | :1268 | delegates to `load_metric_db_events` | inherited |
| `month_grid` | :1370 | `FROM "transaction"` | `AND scenario_id IS NULL` |
| `dashboard_summary` (daily_spend) | :1487 | `FROM "transaction"` | `AND scenario_id IS NULL` |
| `dashboard_summary` (count) | :1521 | `FROM "transaction"` | `AND scenario_id IS NULL` |
| `dashboard_summary` (last_real_tx) | :1530 | `FROM "transaction"` | `AND scenario_id IS NULL` |

**Also in `src-tauri/src/obligations.rs`:**
- `fetch_candidate_items` (obligations.rs:153–159): already has a doc comment marker — `"Quando scenario_id chegar (plano 067), adicionar AND t.scenario_id IS NULL aqui"`.

### 2.5 Write-Back Safety — Every Write Path

**Invariant**: scenario rows must **never** reach Google Sheets. Every write-back path must filter `scenario_id IS NULL`:

| Path | File:Line | Filter needed |
|---|---|---|
| `load_write_back_txns` (grade diária) | `write_back_cmds.rs:16` | `AND t.scenario_id IS NULL` |
| credit card lump aggregation | `write_back_cmds.rs:70` | `AND t.scenario_id IS NULL` |
| `load_txn_items` (line items for write-back) | `write_back_cmds.rs` | `AND t.scenario_id IS NULL` |
| `build_economia_plan` (Economia write-back) | `write_back_cmds.rs` | `AND t.scenario_id IS NULL` |
| Import notes (`import.rs:547`) | read-only, but backfill-safe | `AND t.scenario_id IS NULL` |
| `update_transaction` | `transactions.rs:564` | `AND scenario_id IS NULL` (reject if scenario row) |
| `delete_transaction` | `transactions.rs:600` | `AND scenario_id IS NULL` (reject if scenario row) |

---

## 3. Engine Call Shape

The scenario branch is a second call to `project_with_metrics` with the same seed/today/horizon/annotation. The engine needs **no changes** for the first cut.

### 3.1 `project_with_metrics` signature (verified, `forecast/mod.rs:542-549`)

```rust
pub fn project_with_metrics(
    seed_cents: i64,
    today: NaiveDate,
    chain_events: &[CashflowEvent],   // forward window only (date > today)
    metric_events: &[CashflowEvent],  // full current month (realized + projected)
    horizon_end: NaiveDate,
    annotation: &std::collections::HashMap<(i32, u32), i64>,
) -> Forecast
```

### 3.2 `get_scenario_forecast(scenario_id: String)` — the new Tauri command

```
real_events = load_forecast_events (with scenario_id IS NULL)
hypothetical_events = scenario loader (superset-select of load_cashflow_events
                      with AND t.scenario_id = ?1)
→ apply overrides at RAW ROW level (see §3.3)
→ map remainder through map_cashflow_row (commands/mod.rs:45)
→ call project_with_metrics TWICE (same seed, today, horizon, annotation)
→ return ScenarioCompareDto (§4)
```

### 3.3 Raw-Row Override — Line-Item-Scoped

`CashflowEvent` (`forecast/mod.rs:34-40`) carries only `{date, kind, amount_cents, realized}` — no `id`/`description`/`recurrence_id`. Therefore the override must be applied at the **raw row level**, before mapping.

The scenario real-events loader is a **superset-select** variant of `load_cashflow_events` that also SELECTs `t.id, t.description, t.recurrence_id`. For each override (`obligation_id` resolved via plan 069's `obligation_items`, or `recurrence_id`) with `date >= from_date`:

- **`replace` (line-item)**: reduce that day's row `amount` by the matched `line_item.amount_cents` (drop the row only if it hits 0), then add the hypothetical row at the new amount. The matched obligation is one `line_item` inside a possibly multi-item Saída cell — sibling items are never touched.
- **`suppress` (whole series)**: drop the matched rows.

This prevents double-counting (current rent + new rent) **without** zeroing sibling line items in the same cell. The matched obligation's resolver returns concrete `line_item`s (69), each with a stable `li:<txn_id>:<position>` id.

**Note grammar**: items are **NEWLINE-separated**, not `/`-separated — `parse_itemized_note` (`import.rs:982`) iterates `note.lines()`. The resolver strips trailing `\d+/\d+` installment counters (`obligations.rs:56-83`) for matching across months.

### 3.4 Horizon Extension

`forecast_horizon_end` (`forecast_cmds.rs:685`) is driven by `MAX(date)` in the real table. The scenario horizon must be `max(real_MAX, scenario_MAX)` so hypothetical rows beyond the real horizon are covered. The scenario command computes its own `horizon_end` that unions both.

### 3.5 Seed Override — Not Needed

Scenario shares the real `projection_seed`. A "bonus" or "initial capital" is modeled as a hypothetical Income row dated at disbursement — no explicit seed override mechanism is needed.

---

## 4. Compare Output DTO (Rust Sketch)

A **new** DTO — not a change to `ForecastDto`. The TS mirror lands in the successor implementation plan.

```rust
/// A single change the user applied to the scenario (UI-facing list).
#[derive(serde::Serialize)]
pub struct ScenarioChange {
    pub op: String,              // "add" | "remove" | "replace"
    pub description: String,     // "Aluguel" / "Financiamento celular"
    pub from_date: String,       // ISO "YYYY-MM-DD"
    pub old_amount_cents: Option<i64>,  // None for "add"
    pub new_amount_cents: Option<i64>,  // None for "remove"
}

/// Loan/financing cost breakdown (deterministic — PRICE table).
#[derive(serde::Serialize)]
pub struct LoanBreakdown {
    pub loan_principal_cents: i64,
    pub loan_installment_cents: i64,
    pub loan_term_months: u32,
    pub loan_monthly_rate_bps: i64,       // juros a.m. × 10,000
    pub loan_total_paid_cents: i64,       // parcela × n
    pub loan_total_cost_cents: i64,       // juros = total − principal (CET spirit)
    pub reserve_months_after_financing: Option<f64>,
}

/// Side-by-side view of one month-end balance point.
#[derive(serde::Serialize)]
pub struct ScenarioMonthEnd {
    pub year: i32,
    pub month: u32,
    pub real_balance_cents: i64,
    pub scenario_balance_cents: i64,
    /// scenario − real (positive = scenario is better)
    pub delta_cents: i64,
}

/// The full compare payload for one scenario.
#[derive(serde::Serialize)]
pub struct ScenarioCompareDto {
    pub scenario_id: String,
    pub scenario_name: String,
    // Baseline (real)
    pub real_today: String,
    pub real_horizon_end: String,
    pub real_month_end: Vec<MonthEndDto>,
    pub real_deepest_deficit: Option<DayPointDto>,
    pub real_performance_cents: i64,
    pub real_safe_to_spend_today_cents: i64,
    pub real_binding_guardrail: String,    // "cash" | "savings"
    // Scenario branch
    pub scenario_month_end: Vec<MonthEndDto>,
    pub scenario_deepest_deficit: Option<DayPointDto>,
    pub scenario_performance_cents: i64,
    pub scenario_safe_to_spend_today_cents: i64,
    pub scenario_binding_guardrail: String,
    // Diffs (scenario − real)
    pub month_end: Vec<ScenarioMonthEnd>,  // per-month delta
    pub deepest_deficit_delta_cents: i64,
    pub performance_delta_cents: i64,
    pub safe_to_spend_delta_cents: i64,
    // Human-readable change list
    pub changes: Vec<ScenarioChange>,
    // Loan breakdown (Some only when scenario has a financing hypothetical)
    pub loan: Option<LoanBreakdown>,
}
```

**`changes` mapping from storage → UI:**
- `suppress` → `"remove"` (a `scenario_override` row, no corresponding hypothetical row)
- `replace` → `"replace"` (a `scenario_override` row + a hypothetical row at the new amount)
- `add` → `"add"` (a hypothetical row with no `scenario_override` row — plain add)

**`LoanBreakdown`** is **deterministic**, computed via PRICE table: `parcela = round(PV·i / (1 − (1+i)^−n))`; `juros = parcela·n − PV` — the CET spirit. No LLM math (engineering rule).

The loan generates **two things** in the hypothetical row set:
1. A single **Entrada (Income)** row for the principal at the disbursement date — **raises** the projected Saldo.
2. `n` installment **Saída/Cartão** rows at their due dates.

Without (1), the loan only adds outflows and makes the projection *worse* — the opposite of "cover the buraco." The loan must cover the `deepest_deficit` **plus** its own parcelas (iterative sizing).

---

## 5. UI Entry Point & Interaction Model

### 5.1 Anchor Screen

`HorizonteScreen.tsx` (`src/screens/HorizonteScreen.tsx:29-33`) — currently calls `getForecast` via `useCommand("get_forecast", getForecast)` and renders `forecast.daily` (BalanceTrajectory), `forecast.month_end` (month-end cards), and a status banner with `deepest_deficit`. This is the natural anchor for a "Simular cenário" entry point.

### 5.2 Side-Sheet (Not a Modal)

A **side-sheet** (Material Design 3 "standard side sheet") — consistent with Neko's product register. Opens from the Horizonte screen with a "Simular cenário" affordance.

### 5.3 Adding Hypothetical Rows

Reuses the `NewTransactionForm` fields (txnType, amount, description, date, paymentMethod, isFixed). Description is **mandatory**. Tagged `scenario_id`. Each add triggers `get_scenario_forecast` and renders the real-vs-scenario comparison.

### 5.4 The Override Model — Change/Remove an Existing Recurring Item

From a real recurring row in the ledger/forecast:
- **"Simular alteração"** → opens a sub-panel:
  - *Alterar valor*: creates a `replace` override + a new hypothetical row at the new amount (the "rent increase" path that avoids double-counting).
  - *Remover deste cenário*: creates a `suppress` override.
- Works on an app series (`recurrence_id`) or a named **obligation** (plan 069) for imported rows.
- **Always previews the affected occurrences** before saving.

### 5.5 Decomposed Loan Control

Exposed as **editable controls** — not a fixed slider: valor (principal), número de parcelas, juros a.m., data da 1ª parcela. Compute the installment via PRICE table (`parcela = PV·i / (1 − (1+i)^−n)`). Show **custo do crédito** and total pago (CET spirit).

The loan generates BOTH:
- (a) a single Entrada row for the principal at disbursement (raises Saldo), and
- (b) `n` installment Saída/Cartão rows at due dates.

Must cover the **buraco do futuro** (`deepest_deficit`) **plus** its own parcelas (iterative sizing). Show **reserve-months-after-financing** against the method's stricter big-purchase reserve gate (not the 6-month floor — `RESERVE_MIN_MONTHS`, `forecast_cmds.rs:101`).

### 5.6 Design Tokens

All from `src/design-system/` ("Midnight Ledger"): jade primary, brass warmth, dark-first.
- **Scenario overlay**: dashed line / muted token vs solid jade real baseline.
- **Money**: rendered via `<Money>` component, **never animated** (project rule).
- **Accessibility**: dual-line chart, KPI delta tiles, and diff strip need color-independent encoding (solid/dashed line styles + ▲▼ + sign). AA contrast ≥4.5:1 for scenario-specific tokens.

### 5.7 Teaching Layer — Neko explains the method, not just applies it

Neko must **teach** the method, not only compute it: a first-time user reading "Buraco do futuro" or "Pode gastar hoje" has no idea what they mean. Every method-significant term on the scenario surface carries an **on-demand explanation**, following the 2026 in-product-education standard (favor inline definition-on-demand over an intrusive tour):

- A discreet info affordance (small `ⓘ` / dotted underline) next to each term. Hover **and** keyboard focus reveal a small popover; it is dismissible and wired with `aria-describedby` (never a bare `title`).
- Terms to cover: **Buraco do futuro** (deepest_deficit — the lowest projected balance ahead; if it goes negative you need a plan before you reach it), **Performance** (this month's income minus ALL outflows — fixed + daily + savings + card + the *projected remaining daily*; savings and that projection count as outflow, so the month "starts red" and greens as real daily stays under the ceiling — verbatim method definition), **Pode gastar hoje** (safe-to-spend today without breaching the month's cash — the lowest projected balance until the next income — nor the 20–30% savings guardrail), **Piso de reserva** (the reserve = cost-of-living × N months; a protected floor the balance should not fall below), and the loan's **Custo de vida** (monthly fixed + daily + card outflow; EXCLUDES savings — a move/purchase changes it, and the reserve sizes on it), and the loan's **Custo do crédito / CET** (total paid − principal).

> Vocabulary note (raw-source verified 2026-07-05): the scenario metric set is the method's canonical vocabulary — **Buraco do futuro, Saldo/fim-de-mês projetado, Custo de vida, Performance, Pode gastar** — each taught verbatim in the method's original teaching material (local-only corpus). "Folga de caixa" was found in **0** of those transcripts and is NOT method vocabulary; it was dropped in favor of **Custo de vida** (taught 162×).
- Copy is **method-neutral** (public repo — explain the concept, never name the source/course). Data-first / chrome-second: the explanation is opt-in and never crowds the primary number.
- This is a cross-cutting affordance (dashboard, Horizonte, scenarios) — the successor plan wires it on the scenario metrics; a shared `<Term>`/tooltip component is the natural home.

---

## 6. Scope for the Successor Implementation Plan

### IN

| Item | Description |
|---|---|
| `scenario` table migration | UUID PK, name, person_id, timestamps |
| `ALTER TABLE "transaction" ADD COLUMN scenario_id` | Nullable FK → scenario |
| `scenario_override` table migration | FK → scenario, FK → obligation (nullable), recurrence_id, op, from_date |
| `scenario_id IS NULL` filter | On **every** enumerated read over `"transaction"` (§2.4) and **every** write-back path (§2.5) |
| Line-item raw-row override | Superset-select loader + adjust-before-map (§3.3) |
| CRUD commands | `create_scenario`, `list_scenarios`, `delete_scenario`, `add_scenario_transaction`, `delete_scenario_transaction`, `set_scenario_override`, `remove_scenario_override` |
| `get_scenario_forecast` command | Returns `ScenarioCompareDto` (incl. `changes` list + loan/CET breakdown) |
| Side-sheet in Horizonte | Not a modal |
| Decomposed loan control | valor/parcelas/juros/data, PRICE + custo do crédito |
| Change/remove recurring obligation | Within a scenario (§5.4) |
| Persisted-by-default | Named scenarios survive restart (§7.8) |
| Accessibility layer | Chart text/table equivalent + ARIA live region on recompute (§7.9) |
| Rust tests | Engine regression net (§8) |
| TS DTO mirror | In `src/lib/api.ts`, matching naming conventions of `ForecastDto`/`MonthMetric` |

### OUT (Deferred)

| Item | Rationale |
|---|---|
| Full month clone | Override model covers change/remove without cloning |
| N-way ≥3 scenario compare | Adds "which is baseline / how deltas combine" complexity the method doesn't need |
| Scenario write-back to Sheets | Scenarios must never reach the sheet (safety invariant) |
| Scenario-scoped credit-cycle suppression | Deferred to v2 |
| SAC-style declining installments | Engine supports constant amount per event; per-installment declining schedule is out of scope |
| Monte-Carlo / probability-of-success | Breaks the deterministic anchor |

---

## 7. Open Questions (Resolved or Explicitly Deferred)

### 7.1 Additive vs Clone — RESOLVED
Additive hypotheticals **plus per-scenario overrides** on existing recurring series (suppress/replace). Covers add/remove/**change** without the cost and drift risk of cloning the whole month. Rationale: PocketSmith's shipped model is exactly additive+subtractive overlays; the method's "edit the cell in the copy" is a `replace`, not a second row.

### 7.2 Recurring Hypotheticals & Date Fidelity — DEFERRED to Implementation
Should `add_scenario_transaction` accept a recurrence rule (reuse `src-tauri/src/recurrence.rs`) or a flat list of dates? The method treats dates as nominal in simulation, but Neko's forecast is date-driven. The implementation plan must decide how the UI generates concrete dates from a "R$ X/mês for N months starting M" intent.

### 7.3 Horizon Extension — RESOLVED
`forecast_horizon_end` is scenario-aware: `MAX(scenario_MAX(date), real_MAX(date))`. The scenario command computes its own horizon that unions both sources.

### 7.4 Seed Override — RESOLVED
Not needed. Model a bonus/disbursement as a hypothetical Income row. The seed is always from the real ledger via `projection_seed` (unchanged).

### 7.5 Write-Back Safety Invariant — RESOLVED
**Hard invariant, not an option.** Every Google Sheets write path must filter `scenario_id IS NULL`. A simulation reaching the sheet is the worst-case bug. Enumerate every path in §2.5 + add a regression test that asserts no scenario transaction reaches `load_write_back_txns` or `build_economia_plan`.

### 7.6 Credit/Annotation Scoping — DEFERRED to v2
Credit-card transactions (`payment_method='credit'`, classified `EventKind::Cartao`) and the Economia-tab annotation (`load_economia_annotation`, `forecast_cmds.rs:294`) reflect real spend. "What if I stop using credit" scenario-wide suppression of Cartão events is deferred.

### 7.7 Compare Granularity — RESOLVED for First Cut
Monthly `month_end` deltas + summary metrics (deepest_deficit, performance, safe_to_spend). Full day-by-day `delta_cents` array for the Horizonte overlay is a v2 enhancement — the engine is ready for it (pure), but the UI wiring and the trajectory overlay need separate design.

### 7.8 Scenario Lifecycle — RESOLVED: Persisted by Default
Every researched peer that ships scenarios (PocketSmith, Monarch, Boldin) persists named scenarios; none is ephemeral-only. A scenario is named and survives restart; "Descartar" deletes it. The method's own copies are saved sheets, not transient. A future "quick throwaway" mode can be added later.

### 7.9 Accessibility Layer — RESOLVED for First Cut
The dual-line chart, KPI delta tiles, and diff strip need:
- Color-independent encoding (solid/dashed line styles + ▲▼ + sign) — already met by the design.
- A screen-reader-navigable equivalent (data table or structured text of the month-end series and the buraco).
- An ARIA live region so the recompute on toggle/slider announces the new buraco/saldo (WCAG 2.1 SC 4.1.3 Status Messages).
- Redraw routed through the project's existing reduced-motion convention (`src/design-system/`).
- AA contrast (≥4.5:1) verified for new scenario-specific tokens (dashed comparison line, deficit-red vs surplus-green).

### 7.10 Loan-Reserve Gate — DEFERRED with Rationale
The loan-sizing example (§5.5) compares `reserve_months_after_financing` against the method's **stricter big-purchase reserve threshold**, not the general `RESERVE_MIN_MONTHS = 6` (forecast_cmds.rs:101) safety net. The exact threshold must be confirmed from the method sources before implementing. `RESERVE_MIN_MONTHS` is the app's current fallback, but the big-purchase gate is deliberately stricter — confirm the value.

---

## 8. Engine Regression Tests (for the Successor Implementation Plan)

Modeled on the existing `forecast::tests` helpers at the bottom of `mod.rs` — the `d()`/`ev()` builders + direct `project_with_metrics` calls. All tests are unit tests, no DB.

### T8.1 Determinism
Scenario month-end differs from real by **exactly** the hypothetical obligation amount.

### T8.2 Idempotency
Removing the hypotheticals restores the real baseline exactly.

### T8.3 Income Impact
A hypothetical Income event raises `performance_cents` and `safe_to_spend_today_cents` by the expected delta.

### T8.4 Degenerate / Empty
Empty hypothetical slice never panics and equals the real branch.

### T8.5 Override — No Double-Count (Rent-Increase Correctness)
A `replace` override on a real recurring FixedOut of 190_000 + a hypothetical FixedOut at 280_000 yields month-end deltas equal to `(280_000 − 190_000) × months`, **not** `280_000 × months`.

### T8.6 Override — Suppress
A `suppress` override alone raises the balance by exactly the removed series total.

### T8.7 Override — Clearing (Restore)
Clearing the override restores the real baseline exactly.

### T8.8 Line-Item Override — Siblings Unaffected (Core Correctness Check)
A `replace` override on ONE line item inside a multi-item Saída cell (e.g., a cell of R$ 500 split into R$ 300 "Aluguel" + R$ 200 "Condomínio") moves the projected balance by exactly `(new − old)` for the matched item and leaves the other item's contribution to every day's balance **unchanged**. Assert that the whole cell is not dropped.

### T8.9 Loan — PRICE Table
`parcela = round(PV·i / (1 − (1+i)^−n))`; `custo do crédito = parcela·n − PV`. Assert a known case: PV 18.000, n 24, i 1.8% a.m. → parcela ≈ R$ 930, custo ≈ R$ 4.326 (CET figure is regression-locked).

### T8.10 Loan — Principal Raises Seed
The principal Entrada row raises the projected Saldo at the disbursement date, and the `n` parcelas lower it thereafter. The net over the horizon matches `principal − Σ parcelas`. Guards against the "parcelas-only" bug where the loan only worsens the projection.

### T8.11 Scenario Isolation
A scenario row with `scenario_id = 's1'` does not appear in any real forecast pipeline function (refutes the "missing filter" bug). Verify that `load_cashflow_events` (with the `scenario_id IS NULL` filter) excludes it.

---

## 9. Market Alignment (2026 — Method-Filtered)

Verified across independent 2026 sources. Recorded for the executor and reviewer to confirm the design was checked against the market, not invented.

| Pattern | Status | Reason |
|---|---|---|
| Named scenarios isolated from real ledger, toggled live | **Keep** | PocketSmith standard |
| Solid=real, dashed=simulation dual-line | **Keep** | Excel/Power BI → Monarch, ProjectionLab |
| Overlay chart + delta KPI tiles | **Keep** | ProjectionLab Compare Mode |
| Live recompute with no "apply" button | **Keep** | Monarch, bank mortgage calculators |
| Deterministic single-path projection | **Keep** | Norm for ledger/calendar cashflow tools (PocketSmith, Quicken Simplifi) |
| Side-sheet (not modal) | **Keep** | Material Design 3 "standard side sheet" |
| First-class lowest-projected-balance KPI | **Keep** | Simplifi users request it — Neko's "buraco do futuro" already has it |
| Local-first / open-source peer | **Differentiator** | Actual Budget has only an open request; YNAB/Copilot don't compete on this axis |
| Additive+subtractive overrides | **Keep** | PocketSmith / ProjectionLab ship exactly this model |
| **Monte-Carlo / probability-of-success** | **Exclude** | Breaks the deterministic anchor |
| **Literal spreadsheet duplication as cloud sync** | **Exclude** | `scenario_id` isolation is strictly better and never writes the sheet |
| **Live credit-bureau-personalized loan rates** | **Exclude** | Needs cloud (not local-first) |
| **SAC-style declining installments** | **Exclude** | Engine supports constant amount per event; per-installment declining schedule is out of scope. (This is a BR-market note, not a claim about what the method mandates.) |
| **CET/PRICE labeling** | **BR-market aid** | Labeled as such; NOT a method rule |

---

*Sources verified at `b65f0c6`: `src-tauri/src/forecast/mod.rs`, `src-tauri/src/commands/forecast_cmds.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/obligations.rs`, `src-tauri/src/commands/write_back_cmds.rs`, `src-tauri/src/google_sheets/write_back.rs`, `src/lib/api.ts`, `src/screens/HorizonteScreen.tsx`, `CONTEXT.md`. No source files were edited.*
