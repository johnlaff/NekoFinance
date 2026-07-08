# Plan 067: SPIKE (refresh) — What-if / Scenario Branching of the Forecast

> **Executor instructions**: This is a **spike (design-only) plan**. Your
> deliverable is a written design document (a single markdown file), an
> open-questions list, and a schema sketch — **NOT working code**. Read every
> step fully before starting. Run only the read-only verification commands
> listed. Do NOT modify any source file other than the spike output document
> and `plans/README.md`. If anything in the "STOP conditions" section occurs,
> stop and report — do not improvise.
>
> **This plan SUPERSEDES `plans/020-spike-whatif-scenarios.md`.** Plan 020 was
> written at commit `d183bbf` (2026-06-19) and its "Current state" excerpts are
> now stale: the engine gained two `EventKind` variants (`Cartao`, `Patrimonio`),
> the monolithic `commands.rs` was split into `commands/*.rs`, and
> `project_with_metrics` gained a sixth argument (the Economia-tab annotation).
> Do NOT follow plan 020. Follow this file.
>
> **Drift check (run first)**:
> `git diff --stat b65f0c6..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/mod.rs src-tauri/migrations/ src/lib/api.ts CONTEXT.md`
> If any of those files changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on a
> mismatch treat it as a STOP condition.

## Status

- **Priority**: direction
- **Effort**: spike
- **Risk**: MED
- **Depends on**: none
- **Supersedes**: plans/020-spike-whatif-scenarios.md
- **Category**: direction
- **Planned at**: commit `b65f0c6`, 2026-07-04 · **Reconciled/aligned at `232a2eb`, 2026-07-05**:
  the forecast engine this plan quotes is UNCHANGED — `project_with_metrics` still at
  `forecast/mod.rs:542`, `EventKind` at :17, `forecast_cmds.rs`/`CONTEXT.md` untouched since
  b65f0c6. `src/lib/api.ts` + `migrations/` GREW (068/069/070 merged): the `obligation` table
  and resolver the override model depends on now EXIST (`obligations.rs` — `strip_trailing_installment_counter`
  at :56, `normalize_desc` at :106), and the `.xlsx` path carries line_items (068). The spike
  design doc `plans/067-spike-design.md` is written against this live code. Alignment with the
  real spreadsheet grammar (newline-separated notes, section headers, mutable `N/36` parcela
  counter) and the method (two what-if forms, loan = principal Entrada + parcelas covering the
  buraco, stricter big-purchase reserve gate) holds — see the design doc §3–§5.
- **Teaching layer (added 2026-07-05, product direction)**: Neko must not only APPLY the method
  but TEACH it — method-significant terms ("Buraco do futuro", "Performance", "Pode gastar hoje",
  "Piso de reserva") carry an on-demand explanatory tooltip/glossary, following the 2026 in-product
  education standard (inline definition on hover/focus of a discreet info affordance, dismissible,
  `aria-describedby`; no intrusive tour). Method-neutral copy (public repo). The successor impl and
  the design doc §5 must include this teaching affordance on the scenario metrics.
- **Adherence verified against the RAW sources (2026-07-05)** — the method's original teaching
  transcripts + the owner's live spreadsheet (both local-only, gitignored corpora), not the digests:
  - Method terms are real: **Buraco do futuro** (7×), **Performance** (463×), **Pode gastar** (11×),
    **Custo de vida** (162×) are taught verbatim. **Simulação por cópia da planilha** is taught
    ("cópia da planilha" 5×, "duplica" 8×) — the large-scenario form. **"Folga de caixa" = 0
    occurrences → NOT a method term**; the artifact's card was replaced with **Custo de vida**.
  - **Performance formula confirmed by the source material itself**: *"Se eu não coloco a economia...
    vou ver a performance um pouco positiva"* → economia (and the previsão de diário) count as **saída**.
    **Reserva = custo de vida × N meses** confirmed ("meses de custo de vida", "8/6/7 meses de custo").
  - **Spreadsheet grammar confirmed**: tabs `2025 / 2026 / Economia`; 548 cell notes, **315
    newline-separated** in the `R$ valor - descrição` pattern (385 matches); section headers
    **CONTAS / CARTÕES / ENTRADAS / FATURAS**; and the **tab-separated monthly-budget note** shape
    that plan 070 labels `MonthlyBudgetPlanNote`. Matches §3.3 / plans 069/070.
- **Revised**: 2026-07-04 — added the override model (change/remove an existing
  obligation, no double-count), decomposed loan + CET, the "what changed" list,
  persisted-by-default lifecycle, an accessibility layer, and a verified 2026
  market-alignment section (§4i).

## Adversarial-review corrections (2026-07-04) — integrated below (changelog)

Reviewed against code + the real spreadsheet + the method; the items below are now
**folded into §4a–§4i / Scope / Maintenance** (this list is a changelog, not a separate
source of truth). Summary of what changed:

1. **Override subtracts at the RAW ROW level, not on `CashflowEvent`.** `CashflowEvent`
   (mod.rs:34-40) has only `{date, kind, amount_cents, realized}` — no id/description/
   recurrence_id. The scenario real-events query is a *superset-select* variant of
   `load_cashflow_events` that also SELECTs `t.id, t.description, t.recurrence_id`, applies
   the override there, then maps the remainder through `map_cashflow_row`.
2. **Override is LINE-ITEM-scoped (the majority case), not whole-cell.** A matched
   obligation is one `line_item` inside a possibly-multi-item Saída cell. Do NOT drop the
   day's `CashflowEvent` — replace it with one whose `amount_cents = original −
   matched_line_item.amount_cents` (drop only if it hits 0), leaving sibling items intact.
   Add a §4h test: "a replace override on one item in a multi-item cell leaves the other
   items' contribution to the projected balance unchanged."
3. **The override target is plan 069's `obligation_id`, not a duplicated match rule.** Give
   `scenario_override` a nullable `obligation_id` FK → `obligation` (069); 069's resolver
   returns the matched `line_item`s; the loader subtracts each. Drop `match_desc`/`match_kind`
   from `scenario_override` (they live on `obligation`). `recurrence_id` remains a second
   target kind for app-created series.
4. **`scenario_id IS NULL` must be audited on EVERY read over `"transaction"`.** A nullable
   `scenario_id` on the shared table leaks into `projection_seed`, `forecast_horizon_end`,
   `load_cashflow_events`, `load_forecast_events`, `load_metric_events`,
   `load_realized_month_events`, `realized_annual_savings`, `load_economia_annotation`,
   `reserve_floor` (all `forecast_cmds.rs`) — plus every write-back path. §4f must require an
   explicit enumeration, not "the cash loader gains the filter."
5. **The loan generates TWO things:** (a) a single **Entrada (income) row for the principal**
   at the disbursement date (raises the projected Saldo), and (b) the `n` installment
   **Saída/Cartão** rows at due dates. Without (a) the loan only worsens the projection — the
   opposite of "cover the buraco." (§4c/§4d/§4e/§4h.)
6. **Loan reserve gate = the method's big-purchase threshold, not the 6-month floor.**
   `RESERVE_MIN_MONTHS = 6` (forecast_cmds.rs:101) is the general safety net; a financing
   decision uses a stricter reserve check. The compare shows reserve-months-after-financing
   against that stricter gate (add an open question in §4g).
7. **Note grammar: items are NEWLINE-separated, not `/`-separated** (`parse_itemized_note`,
   import.rs:982, iterates `note.lines()`). Fix every `R$ x - A / R$ y - B` example to
   `R$ x - A` ⏎ `R$ y - B`. A real recurring loan parcela embeds a mutable **`N/36` counter
   inside its description**, so exact `match_desc` fails across months — the resolver (069)
   must strip a trailing `\d+/\d+` counter and normalize the section before comparing, and
   always confirm the matched rows via the mandatory preview.
8. **Reconcile op vocabularies:** §4d `ScenarioChange.op` (add|remove|replace) maps from
   storage as suppress→remove, replace→replace, add→(a hypothetical row, no override).
9. **Factual fixes:** "~30 unit tests" → ~42 (`forecast::tests`); the AGENTS.md "boolean-flag
   rule" citation does not exist — reword to "keeps IO-adapter functions single-purpose, in
   the spirit of AGENTS.md's functional-core/imperative-shell split."
10. **§4i:** reword the SAC exclusion — the engine supports a constant amount per event; SAC's
    declining schedule needs per-installment amounts (out of scope) — rather than "the method
    uses flat PRICE."
11. **Cross-refs:** this is a design-only spike (no code dependency), but its SUCCESSOR
    implementation depends on **plan 068** (recovers `.xlsx` notes → line items) for the
    `.xlsx` path and on **plan 069** (formal `obligation` identity, supersedes any ad-hoc
    match). CET/PRICE stays labeled a BR-market aid, not a method rule.

## Why this matters

The method teaches a what-if practice in **two scoped forms**, chosen by the
size of the decision. Both share the same underlying primitive — pre-launch the
hypothetical obligation at its future date and read the **chained projected
Saldo** forward — and differ only in blast-radius isolation:

1. **Quick single decision** (an economia amount, one installment, "e se eu
   gastar X hoje"): the taught mechanic is to enter the hypothetical row
   directly into the **same** live year ledger at its future date (the ledger
   already carries pre-launched future rows, flagged `is_projection`), read the
   forward Saldo, then **undo** (Ctrl+Z) if unwanted. There is no separate
   simulation layer.
2. **Large lifestyle-scale scenario** (a move, a big purchase, a trip, a salary
   change): the taught mechanic is to **duplicate the whole sheet**, label the
   copy `"(simulação X)"`, do all hypothetical lançamentos inside the copy, and
   never touch the real/original sheet.

In both, the user compares month-end balance, the **buraco do futuro**
(`deepest_deficit`), **Performance**, and **pode-gastar-hoje** against the real
baseline. Today the app has neither: the only way to explore a scenario is to
add real transactions to the live ledger, which pollutes real data and (worse)
gets written back to Google Sheets — with none of the undo/duplicate safety the
method assumes.

A persisted `scenario` entity (a `scenario` table + `transaction.scenario_id`,
CRUD, an automated compare) is **not a literal method artifact** — the method
has no named, saved scenario; it has an undo-able in-place edit and a manual
whole-file copy. What this plan proposes is a faithful **productization** of
that practice: `scenario_id` isolation *is* the "duplicate the file" safety
wrapper, and additive hypothetical rows *are* the pre-launch primitive — unified
into one on-demand branch instead of two manual rituals. (Neko already labels
such beyond-the-sheet conveniences as surpassing, not contradicting, the method
— e.g. liquidity pockets.) Frame it that way in the doc; do not sell it as a 1:1
replication of a sheet feature.

The forecast engine (`src-tauri/src/forecast/mod.rs`) is a **pure function**
over `seed + &[CashflowEvent] + today + horizon + annotation` — no IO, no DB,
no ambient clock. Running a scenario branch is therefore cheap: build a second
event slice (`real_events ∪ hypothetical_events`) and call
`project_with_metrics` a second time with the same seed. **No engine changes
are required for the first cut.** The only real questions are: where to persist
hypothetical transactions, and how to shape the comparison output.

This spike pins down the data model, the engine call shape, the compare DTO,
and the UI entry point, so a successor implementation plan can be a narrow
vertical slice. It is design-only; it writes no Rust, no TypeScript, no SQL.

## Current state

All line numbers verified at commit `b65f0c6`. Open each file yourself and
confirm before quoting it in the spike doc.

### Forecast engine public surface (`src-tauri/src/forecast/mod.rs`)

```rust
// mod.rs:16-32 — EventKind now has SIX variants (was four at d183bbf).
pub enum EventKind {
    Income,     // Entrada
    FixedOut,   // Saída fixa (exclui cartão/economia/patrimônio)
    Daily,      // Diário (débito/dinheiro variável)
    Cartao,     // Cartão de crédito — bucket próprio, dentro do custo de vida
    Economia,   // guardar em reserva acessível — sai do saldo, feeds Economizado%
    Patrimonio, // long-term/illiquid — sai do saldo, fora de custo de vida e Economia% acessível
}

// mod.rs:34-40 — the unit of projection (amount is always positive magnitude)
pub struct CashflowEvent {
    pub date: NaiveDate,
    pub kind: EventKind,
    pub amount_cents: i64, // always positive; sign implied by kind
    pub realized: bool,
}

// mod.rs:247-273 — row→EventKind classifier (PURE). Note the 5/6-type routing:
pub fn classify(
    txn_type: &str,
    is_fixed: bool,
    payment_method: Option<&str>,
    to_liquidity: Option<&str>,
) -> Option<EventKind>
// income→Income; expense+credit→Cartao; expense+fixed→FixedOut; expense→Daily;
// transfer→{reserve:Economia, illiquid:Patrimonio, else:None}

// mod.rs:516-532 — convenience entry point (no annotation)
pub fn project(seed_cents: i64, today: NaiveDate,
               events: &[CashflowEvent], horizon_end: NaiveDate) -> Forecast

// mod.rs:542-549 — the REAL production entry point. SIX args now:
pub fn project_with_metrics(
    seed_cents: i64,
    today: NaiveDate,
    chain_events: &[CashflowEvent],   // forward window only (date > today)
    metric_events: &[CashflowEvent],  // full current month (realized + projected)
    horizon_end: NaiveDate,
    annotation: &std::collections::HashMap<(i32, u32), i64>, // Economia-tab annotation, plan 052
) -> Forecast

// mod.rs:141-171 — dual guardrail (cash × savings), returns SafeToSpend
pub fn safe_to_spend_today(fc: &Forecast,
    annual_income_cents: i64, annual_savings_cents: i64,
    savings_target_bps: i64, reserve_floor_cents: i64) -> SafeToSpend
```

The engine is 100% pure. A scenario branch = a second call to
`project_with_metrics` with the same `seed_cents`, `today`, `horizon_end`, and
`annotation`, but with hypothetical events appended to the real slices. The
~42 unit tests in `forecast::tests` (bottom of `mod.rs`) are the regression net.

### Imperative shell — the forecast pipeline (`src-tauri/src/commands/forecast_cmds.rs`)

The monolithic `commands.rs` referenced by plan 020 no longer exists; plan 011
split it. The forecast pipeline now lives here:

```rust
// forecast_cmds.rs:46  — projection_seed(pool, today) -> i64  (real ledger seed)
// forecast_cmds.rs:685 — forecast_horizon_end(pool, today) -> NaiveDate (MAX(date)-driven)
// forecast_cmds.rs:858 — load_cashflow_events(pool, today, horizon) — the REAL cash chain (SQL below)
// forecast_cmds.rs:895 — load_forecast_events(...) = load_cashflow_events + projected daily ceiling
// forecast_cmds.rs:935 — load_metric_events(...) — full-month metric events
// forecast_cmds.rs:294 — load_economia_annotation(pool, &years) -> HashMap<(i32,u32),i64>
// forecast_cmds.rs:667 — reserve_floor(pool, today) -> i64
// forecast_cmds.rs:115 — realized_annual_savings(pool, today) -> (income_cents, savings_cents)
// forecast_cmds.rs:1075 — #[tauri::command] pub async fn get_forecast(...) -> ForecastDto
// forecast_cmds.rs:1081 — forecast_dto(pool, today_naive) -> ForecastDto (deterministic inner)
```

The exact SQL that isolates real forward events (`forecast_cmds.rs:874-884`) —
**this is the one query a scenario needs to filter/duplicate**:

```rust
sqlx::query_as(
    "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
            COALESCE(a.liquidity,'') \
     FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date > ?1 AND t.date <= ?2",
)
```

The 7-tuple rows are mapped to `CashflowEvent` by the shared helper
`map_cashflow_row` at **`src-tauri/src/commands/mod.rs:45`** (calls
`forecast::classify`). A scenario loader reuses the exact same mapper.

The `forecast_dto` wiring (`forecast_cmds.rs:1081-1104`) shows the full call:

```rust
let horizon_end = forecast_horizon_end(pool, today_naive).await?;
let seed = projection_seed(pool, today_naive).await?;
let events = load_forecast_events(pool, today_naive, horizon_end).await?;
let metric_events = load_metric_events(pool, today_naive, horizon_end).await?;
let years: Vec<i32> = (today_naive.year()..=horizon_end.year()).collect();
let annotation = load_economia_annotation(pool, &years).await?;
let fc = forecast::project_with_metrics(
    seed, today_naive, &events, &metric_events, horizon_end, &annotation,
);
let reserve_floor_cents = reserve_floor(pool, today_naive).await?;
let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
// ... then safe_to_spend_today(&fc, annual_income, annual_savings_amt, SAVINGS_TARGET_BPS, reserve_floor_cents)
```

Constants (`forecast_cmds.rs:91-101`): the scenario branch reuses
`SAVINGS_TARGET_BPS` and `COVERAGE_COMPLETE_BPS` verbatim; **`RESERVE_MIN_MONTHS` is the
general 6-month safety-net floor — the loan-sizing example needs the method's stricter
big-purchase reserve gate instead (§4g.10), not this constant**:

```rust
pub(crate) const SAVINGS_TARGET_BPS: i64 = 2500;  // 25% annual (20–30% range, annual avg)
pub(crate) const COVERAGE_COMPLETE_BPS: i64 = 6_000; // 60% of typical outflow = "month is credible"
pub(crate) const RESERVE_MIN_MONTHS: i64 = 6;     // reserve floor = 6× cost-of-living
```

### Transaction schema (`src-tauri/migrations/20240608000006_transaction.sql`)

```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,            -- cents, positive magnitude
    description TEXT,
    date TEXT NOT NULL,                 -- ISO-8601 YYYY-MM-DD
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

A later migration `20260621000002_transaction_due_date.sql` adds a `due_date`
column. **There is no `scenario_id` column anywhere** (verified:
`grep -rn scenario src-tauri/migrations/` returns nothing). `NULL scenario_id`
= real ledger is the natural sentinel for the first cut.

**Migration naming convention** (for the successor implementation plan, NOT this
spike): `<timestamp>_<name>.sql`, monotonically increasing. Latest existing is
`20260621000004_economia_annotation.sql`. A scenario migration would be e.g.
`20260704000001_scenario.sql`.

### Frontend DTO (`src/lib/api.ts`)

The TS mirror of `ForecastDto` is `interface Forecast` (api.ts:209-231+), and
`interface MonthMetric` (api.ts:161-182) — both carry the new buckets:

```ts
// api.ts:161-182 — MonthMetric now has cartao_cents, daily_projected_cents, patrimonio_cents
export interface MonthMetric {
  year: number; month: number;
  income_cents: number;
  performance_cents: number;
  cost_of_living_cents: number;
  fixed_out_cents: number;
  daily_out_cents: number;
  daily_projected_cents: number;
  cartao_cents: number;         // NEW since plan 020
  real_daily_avg_cents: number;
  economia_cents: number;
  patrimonio_cents: number;     // NEW since plan 020
  savings_rate_bps: number;
}

// api.ts:149-158
export interface DayPoint { date: string; balance_cents: number; }
export interface MonthEnd { year: number; month: number; balance_cents: number; }
```

The scenario compare output must be a **new** DTO, NOT a change to `Forecast`.

### Domain vocabulary (from `CONTEXT.md` — quote these exactly)

The current (5/6-type) model — this is what the spike doc must reflect, NOT
plan 020's stale 4-type formulas:

- **cost_of_living** (`CONTEXT.md:70`) = `FixedOut + Daily(realized) + Cartao`
  — **excludes Economia and Patrimonio**.
- **Performance** (`CONTEXT.md:70`) = `Income − (FixedOut + Daily(realized) +
  Daily(projected/remaining forecast) + Cartao + Economia + Patrimonio)`.
- **buraco do futuro** = `deepest_deficit` (minimum projected balance in horizon).
- **Economizado%** (`CONTEXT.md:94`) = `registered_economia_cents /
  realized_income_cents` (target 20–30% as an ANNUAL average, never monthly
  pass/fail).
- **binding_guardrail** (`CONTEXT.md:98`) = which of the two limits (cash /
  savings) bit — `"cash" | "savings"`.
- **Colchão** = net surplus kept as cash (not a formal Economia transfer).
- **Person** (not "user"); **Account** (real instrument);
  `transaction.type` ∈ `income | expense | transfer`;
  `payment_method` ∈ `debit | credit | pix | cash`;
  `amount_cents` always positive, sign implied by `EventKind`.

### Anchor screen (`src/screens/HorizonteScreen.tsx`)

Still exists and is the natural visual anchor. It calls
`getForecast` via `useCommand("get_forecast", getForecast)` and renders
`forecast.daily` (day-by-day trajectory), `forecast.month_end`, and
`deepest_deficit`. A "real vs scenario" overlay belongs here.

## Commands you will need

| Purpose           | Command                                                                                                                                                            | Expected on success          |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------- |
| Drift check       | `git diff --stat b65f0c6..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/mod.rs src-tauri/migrations/ src/lib/api.ts CONTEXT.md` | shows changed files or empty |
| Rust engine tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast`                                                                                             | all pass (~42 tests)         |
| Type-check        | `npm run typecheck`                                                                                                                                                 | exit 0                       |

This spike is read-only during authoring. The successor implementation plan
runs the full gate (`npm run check`).

## Scope

**In scope** (the only artifacts this spike produces):

- `specs/022-whatif-scenarios/spike.md` — the design document specified in Step 4
  below. (Spec directory `022` is the next free number; `020` and `021` are
  already taken by unrelated specs.)
- `plans/README.md` — status-row update for plan 067 only.

**Out of scope** (do NOT touch — read for context only):

- `src-tauri/src/forecast/mod.rs` — no engine changes for the spike or first cut.
- `src-tauri/src/commands/forecast_cmds.rs`, `src-tauri/src/commands/mod.rs` —
  read for context; no changes.
- `src-tauri/migrations/` — the `scenario` migration belongs to the successor
  implementation plan, not here.
- Any React screen or component — UI shape is described in prose, not built.
- `plans/020-spike-whatif-scenarios.md` — superseded; leave it as-is (this plan
  marks it superseded in the README index).
- `plans/019-spike-invoice-entity.md` — unrelated spike.

## Git workflow

- Branch: `advisor/067-spike-whatif`
- One commit for the spike doc, one for the README update.
- Commit message style matches the repo log (Portuguese descriptive subject,
  no conventional-commits prefix — e.g. `git log --oneline -6` shows
  `Marcador de 'Hoje': chip neutro no lugar do fundo verde (Proposta A) (#111)`).
  Suggested: `docs: spike — ramificação de cenários what-if do forecast (plano 067)`.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Confirm engine purity and the six-arg call shape

Open `src-tauri/src/forecast/mod.rs` and read lines 511–549. Confirm that
`project` and `project_with_metrics` take only value arguments (no `pool`, no
`Local::now`, no DB). Copy the **exact current** `project_with_metrics`
signature (six args, including `annotation: &HashMap<(i32,u32), i64>`) into the
spike doc. This establishes the core claim: a scenario branch is a second call
to `project_with_metrics` with the same seed/today/horizon/annotation and a
combined event slice.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast`
→ all pass (the engine the spike describes is green and stable).

### Step 2: Read the real forecast pipeline

Open `src-tauri/src/commands/forecast_cmds.rs` and locate: `projection_seed`
(46), `forecast_horizon_end` (685), `load_cashflow_events` (858),
`load_forecast_events` (895), `load_metric_events` (935),
`load_economia_annotation` (294), `reserve_floor` (667),
`realized_annual_savings` (115), `get_forecast` (1075), `forecast_dto` (1081).
Read the `load_cashflow_events` SQL (874–884) and confirm it matches the
excerpt in "Current state". Open `src-tauri/src/commands/mod.rs:45` and confirm
`map_cashflow_row` is the shared row→`CashflowEvent` mapper.

The spike doc must explain: **adding `AND t.scenario_id IS NULL` to that SQL is
the minimal change to isolate real from scenario rows**, and a scenario loader
is a copy of `load_cashflow_events` with `AND t.scenario_id = ?` instead —
reusing `map_cashflow_row` unchanged.

**Verify**: read-only. Confirm the function line numbers match "Current state".
If they don't, treat as a STOP condition.

### Step 3: Read the transaction schema and confirm no scenario_id exists

Open `src-tauri/migrations/20240608000006_transaction.sql`. Confirm the column
list matches the excerpt (no `scenario_id`). Run
`grep -rn scenario src-tauri/migrations/` and confirm it returns nothing.

**Verify**: `grep -rn scenario src-tauri/migrations/` → no output. If it prints
anything, someone partially implemented scenarios — STOP and report.

### Step 4: Write the spike document

Create `specs/022-whatif-scenarios/spike.md` with **all** the sections below.
Where a section references code, use the **Current state** excerpts above
(verified against `b65f0c6`), never plan 020's stale ones.

---

#### 4a. Problem statement (2–4 sentences)

The method's what-if practice has two forms: (1) a **quick** in-place edit —
pre-launch the hypothetical row into the live ledger, read the forward Saldo,
undo if unwanted; and (2) a **large-scenario** ritual — duplicate the whole
sheet, label it `(simulação)`, do the hypothetical lançamentos in the copy, and
leave the original untouched. This plan **productizes both into one on-demand
forecast branch**: hypothetical rows tagged with a `scenario_id` (the isolation
that replaces both "undo" and "duplicate the file"), projected without polluting
the real ledger or triggering write-back to Google Sheets. Say explicitly in the
doc that a saved, named scenario entity is a Neko convenience *on top of* the
method's manual practice, not a literal sheet feature. The forecast engine is a
pure function and already supports the computation at zero cost; the gaps are
(a) persisting hypothetical transactions in isolation and (b) a comparison DTO.

#### 4b. Scenario data model (schema sketch — not a migration)

```sql
-- NEW table: one row per named scenario
CREATE TABLE scenario (
    id   TEXT PRIMARY KEY NOT NULL,          -- uuid
    name TEXT NOT NULL,                       -- e.g. "Mudança para SP"
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ALTERED existing table (the migration adds ONE nullable FK):
ALTER TABLE "transaction" ADD COLUMN scenario_id TEXT REFERENCES scenario(id) ON DELETE CASCADE;
-- NULL          = real ledger  (existing rows stay NULL; no backfill)
-- NOT NULL      = hypothetical transaction owned by that scenario
```

**Key invariants** (state each in the doc):

- Scenario transactions reuse the exact `"transaction"` shape
  (`type/amount/date/payment_method/is_fixed/from_account_id/to_account_id/
  is_projection/due_date`). No separate table.
- Scenario transactions **never** write `account.balance`; the seed always comes
  from the real ledger via `projection_seed` (unchanged).
- Deleting a scenario cascades to its transactions.
- **Every read over `"transaction"` must exclude scenario rows** — not just the cash
  loader. A nullable `scenario_id` on the shared table means EVERY function in
  `forecast_cmds.rs` that queries `"transaction"` must add `AND t.scenario_id IS NULL`:
  `projection_seed`, `forecast_horizon_end`, `load_cashflow_events`,
  `load_forecast_events`, `load_metric_events`, `load_realized_month_events`,
  `realized_annual_savings`, `realized_annual_economia`, `projected_annual_savings`,
  `realized_monthly_baseline`, `effective_daily_ceiling`, `load_year_events`,
  `month_grid`, `dashboard_summary`, `load_economia_annotation`, `reserve_floor`, and
  the shared private loader **`load_metric_db_events`** (which issues the raw SQL that
  several of the above delegate to — the filter belongs there once)
  (audit with `grep -n '"transaction"' src-tauri/src/commands/forecast_cmds.rs` and
  filter EVERY hit). Missing one silently corrupts the real Dashboard/Anual/Month-Grid
  figures with scenario rows. The scenario loader uses `AND t.scenario_id = ?`.
- **Write-back is real-ledger only.** Every Google-Sheets write path must filter
  `scenario_id IS NULL` so a simulation can never reach the sheet. Call this out
  explicitly as a safety invariant (see open question 5).
- The Economia-tab annotation (`load_economia_annotation`) and credit-cycle
  behavior reflect **real** figures; the scenario projects hypotheticals *on top
  of* real ongoing spend (see open question 6).

**Modifying or removing an existing obligation — not just adding (the override
model).** Additive-only hypotheticals cannot express the most common what-if:
*changing* a recurring obligation ("rent goes from R$ 1.900 to R$ 2.800"). Adding
a new R$ 2.800 rent on top of the real R$ 1.900 rent **double-counts** (you'd
project R$ 4.700). The method's own practice handles this natively — in the
duplicated sheet you *edit the rent cell*, you do not add a second rent row.

**Identity caveat — read before implementing; it governs the whole feature.** Two
transaction-identity models coexist, and they matter here:

- *App-created recurrences* share one `transaction.recurrence_id` (spec 016,
  migration `20240612000005_recurrence.sql`). Precise, stored, reliable.
- *Imported spreadsheet rows carry `recurrence_id = NULL`* — the importer never sets
  it (verify: `grep -rn recurrence src-tauri/src/google_sheets/import.rs` is empty).
  Their identity is the deterministic `row_id = sha256("txn-v1|"+sheet+"|"+date+"|"
  +kind+"|"+slot)` (spec 012, `google_sheets/import.rs`) — stable per (aba, dia,
  coluna), surviving value/note edits and re-imports. **But the sheet has NO concept
  of a "series":** a monthly rent is just twelve independent Saída cells; nothing
  links them. So for imported obligations there is no `recurrence_id` to key on — and
  the spreadsheet is the system-of-record (spec 012), so this is the *majority* case.

The override targets an **obligation** (plan 069's user-confirmed identity) for
imported rows, or a `recurrence_id` for app-created series — never a raw string match
stored on this table:

```sql
CREATE TABLE scenario_override (
    id            TEXT PRIMARY KEY NOT NULL,
    scenario_id   TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    op            TEXT NOT NULL CHECK(op IN ('suppress','replace')),
    from_date     TEXT NOT NULL,          -- applies to occurrences on/after this date
    obligation_id TEXT REFERENCES obligation(id) ON DELETE CASCADE, -- imported case (plan 069)
    recurrence_id TEXT,                    -- app-native series (spec 016) — the precise case
    CHECK (obligation_id IS NOT NULL OR recurrence_id IS NOT NULL)
);
```

Plan 069's **obligation** owns the match rule (normalized description + normalized
section + kind) and a resolver that returns the concrete `line_item`s it matches — so
the override never re-implements string matching and both features share one
user-confirmed identity. Creating an override always previews the resolved rows.

- **Remove** ("cancelar a academia") = one `suppress` override.
- **Change** ("aluguel +R$ 900 a partir de ago") = one `replace` override **plus** a
  hypothetical row at the new amount, presented as one UI action.
- **User-driven & confirmed:** the user picks the obligation from the ledger/forecast
  (each shown row has a stable `row_id`); the app resolves the target (the obligation's
  resolver, or `recurrence_id`, over future occurrences) and **shows exactly which
  future rows will be affected, for confirmation, before saving.** No silent fuzzy
  matching. The stored override records *what the user confirmed*.
- **Matching must tolerate the real note grammar** (plan 069's resolver): a real
  recurring parcela embeds a mutable `N/36` counter *inside* its description, so the
  resolver strips a trailing `\d+/\d+` token before comparing, and normalizes the
  section (punctuation drifts by year: `CONTAS` in 2025 vs `FATURAS:` in 2026).
- **Granularity = the line item, not the whole cell.** A day's Saída cell often
  aggregates several obligations; the cell note itemizes them **one per line**
  (`R$ x - Aluguel` ⏎ `R$ y - Fatura …` — `parse_itemized_note` iterates
  `note.lines()`, import.rs:982; items are never `/`-separated), parsed into
  `line_item` rows (plan 035, `id = li:<txn_id>:<position>`, with a `section` header).
  So "change the rent" overrides the rent **line item**, and the scenario loader must
  **reduce that day's event amount by the matched line item's amount** (drop only if it
  reaches 0), NOT drop the whole aggregated event (§4c step 5) — otherwise it zeroes out
  the cell's other items. This needs itemized data, which only the **Sheets-API import**
  carries today (the `.xlsx` path drops notes until plan 068 lands). A genuine
  single-item cell degrades to a whole-cell override.
- The scenario loader **adjusts the overridden real events before adding hypotheticals**
  — real rows are never modified; ledger and sheet stay untouched.
- **Precision caveat (identity ladder):** cell/transaction identity is a stable
  deterministic id (spec 012); line-item identity is `li:<txn_id>:<position>` —
  stable per position but positional-within-note; an "obligation/series" has **no
  native identity** in the sheet (recovered only by confirmed description match).
  Plan the override UI around this reality: always preview the resolved rows, never
  match silently.

#### 4c. Engine call shape for a scenario branch

A new Tauri command `get_scenario_forecast(scenario_id: String)` will:

1. `projection_seed(pool, today)` — same seed as the real forecast.
2. `forecast_horizon_end` — but **scenario-aware** (see open question 3): the
   horizon must extend to cover any hypothetical transaction dated beyond the
   real `MAX(date)`.
3. Real events: `load_forecast_events` with `scenario_id IS NULL`.
4. Scenario events: a loader mirroring `load_cashflow_events` with
   `AND t.scenario_id = ?1 AND t.date > ?2 AND t.date <= ?3`, mapped through the
   shared `map_cashflow_row` (`commands/mod.rs:45`).
5. **Apply overrides at the RAW ROW level, before mapping to `CashflowEvent`.**
   `CashflowEvent` (mod.rs:34-40) carries only `{date, kind, amount_cents, realized}`
   — no id/description/recurrence_id — so an override cannot be applied to the mapped
   slice. The scenario real-events loader is therefore a *superset-select* variant of
   `load_cashflow_events` that also selects `t.id, t.description, t.recurrence_id`. For
   each override (resolved via plan 069's obligation resolver → concrete `line_item`s,
   or via `recurrence_id`) with `date >= from_date`:
   - **`replace` (line-item)**: reduce that day's row amount by the matched
     `line_item.amount_cents` (drop the row if it hits 0), then add the hypothetical
     row at the new amount;
   - **`suppress` (whole series)**: drop the matched rows.
   Only then map the remainder through `map_cashflow_row`. Same transform for the
   metric slice. This prevents the "current rent + new rent" double-count **without**
   zeroing out sibling line items in the same cell.
6. `forecast::project_with_metrics(seed, today, &scenario_events,
   &scenario_metric_events, horizon_end, &annotation)` — same `annotation`
   HashMap as the real branch.
7. `safe_to_spend_today(&scenario_fc, annual_income, annual_savings_amt,
   SAVINGS_TARGET_BPS, reserve_floor_cents)` — same constants.
8. Return `ScenarioCompareDto` (§4d).

`forecast_dto` / `get_forecast` are unchanged. The scenario is computed on
demand — no caching, no background job in the first cut.

#### 4d. Compare output DTO

Sketch a **new** Rust DTO (mirrors, don't mutate, `ForecastDto`). It must carry,
for both the real and scenario branch, at least: `month_end: Vec<MonthEnd>`,
`deepest_deficit: Option<DayPoint>`, current-month `performance_cents`,
`safe_to_spend_today_cents`, and `binding_guardrail: "cash"|"savings"`. Plus a
`diff` block with `month_end_delta_cents` (scenario − real, per month),
`deepest_deficit_delta_cents`, `performance_delta_cents`,
`safe_to_spend_delta_cents`. Provide the Rust struct sketch and its TypeScript
mirror (for `src/lib/api.ts`, added in the implementation plan — **not** here).
Follow the current `MonthMetric`/`Forecast` field-naming style (snake_case
`_cents` suffixes; positive magnitude with sign implied).

Also include a **`changes: Vec<ScenarioChange>`** — the human-readable "what changed"
list rendered beside the numeric deltas. Its `op` is UI-facing (`add | remove |
replace`) and **maps from storage** as `suppress`→`remove`, `replace`→`replace`, and
`add`→a plain hypothetical row (no `scenario_override` row at all). Numbers alone don't
tell the user *what* they toggled; every researched peer that ships compare
(ProjectionLab, PocketSmith) pairs the deltas with a line-item change list. When a
scenario carries a financing hypothetical, also expose its deterministic cost
breakdown: `loan_principal_cents`, `loan_installment_cents`, `loan_term_months`,
`loan_total_paid_cents`, `loan_total_cost_cents` (juros = total − principal — the
deterministic spirit of Brazil's **CET / Custo Efetivo Total**), and
`reserve_months_after_financing` (§4e/§4g). All computed by a deterministic finance
tool (engineering rule: no LLM financial math).

#### 4e. UI entry point and interaction model

- **Anchor**: `HorizonteScreen.tsx` (day-by-day trajectory from `get_forecast`).
- First-cut interaction: a "Simular cenário" affordance opens a panel to
  select/create a named scenario, add hypothetical transactions (same fields as
  `NewTransactionForm.tsx`, tagged `scenario_id`, description mandatory), and —
  after each add — call `get_scenario_forecast` and render real-vs-scenario
  `month_end`, both buracos do futuro, and the `diff.*_delta_cents`.
- **Changing or removing an existing obligation** (§4b override model): from a real
  recurring row (in the ledger/forecast), offer "Simular alteração" → *alterar
  valor* (creates a `replace` override + a new hypothetical row at the new amount)
  or *remover deste cenário* (a `suppress` override). This is the "rent increase"
  path — without it, the only way to model a higher rent is adding one on top,
  which double-counts. Offer it on an app series (`recurrence_id`) or a named
  **obligation** (plan 069) for imported rows; always preview the affected occurrences.
- Read-only compare: no write-back, no `account.balance` change.
- **Motivating example to design for** (a canonical method what-if): sizing a
  loan/financing. Expose it as **decomposed, editable controls** — valor, número de
  parcelas, juros a.m., data da 1ª parcela — not one fixed slider (every Brazilian
  financing UX examined — Nubank, C6, Banco Central's Calculadora do Cidadão — exposes
  exactly these). Compute the installment via the **PRICE table**
  (`parcela = PV·i / (1 − (1+i)^−n)`) and show the **custo do crédito** and total paid
  (CET spirit). The loan generates **two things**: (a) a single **Entrada (income) row
  for the principal** at the disbursement date — which RAISES the projected Saldo — and
  (b) the `n` installment **Saída/Cartão** rows at their due dates. Without (a) the loan
  only adds outflows and would make the projection *worse* — the opposite of covering
  the hole. The scenario must make obvious that the loan has to cover the **buraco do
  futuro** (`deepest_deficit`) *plus* the parcelas it adds (iterative: raise the amount,
  re-check the hole) — which is why the compare DTO leads with `deepest_deficit` deltas.
  Because this is a big-purchase decision, show **reserve-months-after-financing**
  against the method's stricter big-purchase reserve gate (§4g), not the 6-month floor.
- **Design tokens**: all from `src/design-system/` ("Midnight Ledger"). Scenario
  overlay = dashed line / muted token vs the solid jade real baseline. Money is
  rendered with the `<Money>` component and **never animated** (project rule).

#### 4f. Scope for the first-cut implementation (successor plan)

**IN**: `scenario` table + `ALTER TABLE "transaction" ADD COLUMN scenario_id` +
**`scenario_override` table** migrations; the `scenario_id IS NULL` filter added to
**every read over `"transaction"`** (the §4b enumeration) **and every write-back path**;
the loader's raw-row override adjustment (§4c step 5); CRUD commands (`create_scenario`,
`list_scenarios`, `delete_scenario`, `add_scenario_transaction`,
`delete_scenario_transaction`, `set_scenario_override`);
`get_scenario_forecast` returning `ScenarioCompareDto` (incl. the `changes` list and
loan/CET breakdown, §4d); a **side-sheet in Horizonte** (not a modal — matches the
product register and the design prototype); **decomposed loan control** (valor/
parcelas/juros/data, PRICE + custo do crédito); **change/remove an existing
recurring obligation** within a scenario; scenarios **persisted by default** (named,
survive restart — §4g.8); the **accessibility layer** (§4g.9: chart text/table
equivalent + ARIA live region on recompute); Rust tests (§4h). The real
`get_forecast` and all existing screens stay unchanged.

**OUT (deferred)**: cloning the *full* real month into an editable scenario copy
(the override model covers change/remove without it); N-way comparison of ≥3 saved
scenarios at once (Boldin-style — adds "which is baseline / how deltas combine"
complexity the method doesn't need); editing a *hypothetical* scenario row
(delete + re-add suffices); overriding one-off real rows without a `recurrence_id`;
scenario-scoped credit-cycle suppression (§4g.6); scenario-aware write-back; a
scenario badge on the Dashboard; **Monte-Carlo / probability-of-success framing**
(SOTA in retirement tools but breaks the deterministic anchor — do not add).

#### 4g. Open questions (answer or explicitly defer before the implementation plan)

1. **Additive vs clone — RESOLVED**: additive hypotheticals **plus per-scenario
   overrides** on existing recurring series (§4b: suppress/replace). This covers
   add / remove / **change** (the "rent increase" case) without the cost and drift
   risk of cloning the whole month. Full-month editable clone stays deferred
   (§4f OUT). Rationale: PocketSmith's shipped model is exactly additive+subtractive
   overlays, and the method's "edit the cell in the copy" is a `replace`, not a
   second row.
2. **Recurring hypotheticals & date fidelity**: should `add_scenario_transaction`
   accept a recurrence rule (reuse `src-tauri/src/recurrence.rs`) for multi-month
   obligations, or a flat list of dates? Note the method treats dates as
   *nominal* in a pure simulation (the point is the steady-state monthly impact,
   not the exact day), but Neko's forecast is **date-driven** (the chained Saldo
   and the horizon depend on real dates) — so hypothetical rows still need
   concrete dates. Decide how the UI generates them from a "R$ X/mês for N
   months starting M" intent.
3. **Horizon extension**: `forecast_horizon_end` is driven by `MAX(date)` in the
   real table. A hypothetical dated beyond the real horizon needs the horizon
   logic to be scenario-aware — confirm the exact rule.
4. **Seed override**: scenario shares the real `projection_seed`. Is "add a
   bonus as a hypothetical income" sufficient, or is an explicit seed override
   ever needed? (Current answer: model it as a hypothetical income row.)
5. **Write-back safety**: enumerate every Google-Sheets write path
   (`src-tauri/src/google_sheets/write_back.rs`,
   `src-tauri/src/commands/write_back_cmds.rs`) and confirm each filters
   `scenario_id IS NULL`. A simulation reaching the sheet is the worst-case bug —
   this is a hard invariant, not an option.
6. **Credit/annotation scoping**: real credit-card transactions
   (`payment_method='credit'`, classified `EventKind::Cartao` and folded into the
   due-date Saída lump by the write-back path) and the Economia-tab annotation
   reflect **real** spend. For "what if I stop using credit", should the scenario
   be able to suppress real Cartão events? Likely deferred.
7. **Compare granularity**: monthly `month_end` deltas + summary metrics, or a
   full day-by-day `delta_cents` array for the Horizonte overlay?
8. **Scenario lifecycle — RESOLVED: persisted by default**. Every researched peer
   that ships scenarios (PocketSmith, Monarch, Boldin) persists named scenarios;
   none is ephemeral-only. So a scenario is named and survives restart; "Descartar"
   deletes it. (A future "quick throwaway" mode can be added later, but
   persisted-by-default is the market- and method-consistent baseline — the method's
   own copies are saved sheets, not transient.)
9. **Accessibility layer**: the dual-line chart, KPI delta tiles, and diff strip
   need color-independent encoding (already met: solid/dashed line styles + ▲▼ +
   sign), **plus** a screen-reader-navigable equivalent (a data table or structured
   text of the month-end series and the buraco) and an **ARIA live region** so the
   recompute on toggle/slider announces the new buraco/saldo (WCAG 2.1 SC 4.1.3
   Status Messages). Confirm the redraw routes through the project's existing
   reduced-motion convention. Verify AA contrast (≥4.5:1) for the new
   scenario-specific tokens (dashed comparison line, deficit-red vs surplus-green).
10. **Loan-reserve gate**: the loan-sizing example (§4e) compares
    reserve-months-after-financing against the method's **stricter big-purchase reserve
    threshold**, not the general `RESERVE_MIN_MONTHS = 6` (forecast_cmds.rs:101) safety
    net. Confirm the exact threshold from the method sources before implementing.

#### 4h. Engine regression protection for the implementation plan

The successor plan must add tests (modeled on the existing `forecast::tests`
helpers at the bottom of `mod.rs` — the `d()`/`ev()` builders + direct
`project_with_metrics` calls) asserting:

- Scenario month-end differs from real by **exactly** the hypothetical
  obligation (determinism).
- Removing the hypotheticals restores the real baseline exactly (idempotency).
- A hypothetical income raises `performance_cents` and
  `safe_to_spend_today_cents` by the expected delta.
- Empty hypothetical slice never panics and equals the real branch (degenerate).
- **Override (no double-count)**: a `replace` override on a real recurring series +
  a hypothetical row at the new amount yields month-end deltas equal to
  `(new − old) × months`, **not** `new × months` (the rent-increase correctness
  check). A `suppress` override alone raises the balance by exactly the removed
  series. Clearing the override restores the real baseline exactly.
- **Loan (PRICE)**: `parcela = round(PV·i / (1 − (1+i)^−n))`; `custo do crédito =
  parcela·n − PV`. Assert a known case (e.g. PV 18.000, n 24, i 1,8% a.m. →
  parcela ≈ R$ 930, custo ≈ R$ 4.326) so the CET figure is regression-locked.
- **Line-item override (siblings unaffected)**: a `replace` override on ONE line item
  inside a multi-item Saída cell moves the projected balance by exactly `(new − old)`
  and leaves the other items' contribution to every day's balance **unchanged** (the
  whole cell is not dropped) — the core correctness check for §4b/§4c step 5.
- **Loan principal**: the principal Entrada row raises the projected Saldo at the
  disbursement date and the `n` parcelas lower it thereafter — the net over the horizon
  matches `principal − Σ parcelas` (guards against the "parcelas-only" bug).

#### 4i. Market alignment (2026 — verified, method-filtered)

Record this so the executor and reviewer know the design was checked against the
market, not invented. Verified across independent 2026 sources:

- **Validated as standard (keep):** named scenarios isolated from the real ledger,
  toggled live (PocketSmith); solid=real / dashed=simulation dual-line (Excel/Power
  BI → Monarch, ProjectionLab); overlay chart + delta KPI tiles (ProjectionLab
  Compare Mode); live recompute with no "apply" button (Monarch, bank mortgage
  calculators); **deterministic single-path** projection as the norm for
  ledger/calendar cashflow tools (PocketSmith, Quicken Simplifi); side-sheet, not
  modal (Material Design 3 "standard side sheet"); a first-class lowest-projected-
  balance KPI (Simplifi users request it — Neko's "buraco do futuro" already has it).
- **Differentiator:** no local-first / open-source peer ships this (Actual Budget has
  only an open request; YNAB/Copilot don't compete on this axis). Local-first is not
  a handicap here.
- **Correctly excluded (do not add — breaks an anchor):** Monte-Carlo /
  probability-of-success (deterministic anchor); literal spreadsheet duplication as
  cloud sync (`scenario_id` isolation is strictly better and never writes the sheet);
  live credit-bureau-personalized loan rates (needs cloud); SAC-style declining
  installments — the engine models a constant amount per event, so SAC's per-installment
  declining schedule is out of scope (not a claim that the method mandates PRICE).

---

**Verify**: `npm run typecheck` → exit 0 (the spike doc is markdown; typecheck
confirms no `.ts` source was accidentally touched).

### Step 5: Update plans/README.md

Set plan 067's status row to `DONE` (or `IN PROGRESS` while authoring). Do not
change any other row's status — plan 020's superseded marker is already written
by this plan's author into the index; leave it.

**Verify**: `git diff --name-only` shows only `specs/022-whatif-scenarios/spike.md`
(new) and `plans/README.md` (modified). No source files appear.

## Test plan

Spike — no test code is written here. §4h specifies the tests the successor
implementation plan must include. To confirm the engine is stable before
authoring:

`cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast` → all pass.

## Done criteria

All must hold:

- [ ] `specs/022-whatif-scenarios/spike.md` exists with all sections 4a–4i
      (incl. the §4b override model and the §4i market-alignment record).
- [ ] The doc reflects the **current** model: 6 `EventKind` variants,
      `project_with_metrics` six-arg signature, pipeline in
      `commands/forecast_cmds.rs`, and the `CONTEXT.md` Performance/cost-of-living
      formulas (NOT plan 020's stale 4-type versions).
- [ ] The doc answers or explicitly defers every open question in §4g,
      including the write-back-safety invariant (§4g.5).
- [ ] `cargo test ... -- forecast` exits 0 (engine unchanged).
- [ ] `npm run typecheck` exits 0.
- [ ] `git diff --name-only` shows zero changes under `src/`, `src-tauri/src/`,
      or `src-tauri/migrations/`.
- [ ] `plans/README.md` row for plan 067 is updated.

## STOP conditions

Stop and report back (do not improvise) if:

- The `project_with_metrics` signature at `mod.rs:542` does not match the
  six-arg excerpt above — the engine changed; the spike's core claim needs
  re-verification.
- `grep -rn scenario src-tauri/migrations/` prints anything, OR
  `load_cashflow_events` in `forecast_cmds.rs` already filters `scenario_id` —
  someone partially implemented this; review what exists first.
- The forecast pipeline functions are no longer in
  `src-tauri/src/commands/forecast_cmds.rs` (e.g. re-split) — find them and
  confirm the call shape before writing the doc.
- Authoring the doc would require editing any source file (`.rs`/`.ts`/`.tsx`/
  `.sql`) — that means scope crept into implementation; stop and split into a
  separate implementation plan.
- `cargo test ... -- forecast` fails — do not proceed until the regression net
  is green.

## Maintenance notes

For whoever picks this up after review:

- The successor implementation plan reads §4f as its scope and §4g as its
  prerequisites. Answer every open question before writing it.
- The engine (`forecast/mod.rs`) needs **no** changes for the first cut unless
  open question 7 (day-by-day delta) is answered YES.
- The `scenario_id IS NULL` filter must be added to **every read over `"transaction"`**
  (the full §4b enumeration — not just the cash loader) **and every** write-back query
  (`google_sheets/write_back.rs`, `commands/write_back_cmds.rs`). Missing a *read* filter
  silently corrupts real Dashboard/Anual/Month-Grid figures with scenario rows; missing a
  *write-back* filter leaks a simulation to the real spreadsheet (the highest-severity
  bug). Keep the real vs scenario loaders as separate functions (in the spirit of the
  functional-core / imperative-shell split in `AGENTS.md`).
- Reuse `SAVINGS_TARGET_BPS` and `COVERAGE_COMPLETE_BPS` verbatim. **Exception:** the
  loan-sizing example needs the method's stricter big-purchase reserve gate, not the
  general `RESERVE_MIN_MONTHS = 6` (§4g.10) — the one scenario-specific threshold.
- `amount_cents` is always positive magnitude; sign implied by `EventKind`. This
  invariant holds for hypothetical rows too.
- Plan 020 is the stale predecessor; if it is ever revived, redirect to this
  plan.
