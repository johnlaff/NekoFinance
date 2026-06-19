# Plan 020: SPIKE: What-if / Scenario Branching of the Forecast

> **Executor instructions**: This is a **spike (design-only) plan**. Your
> deliverable is a written design document, an open-questions list, and a
> schema sketch — NOT working code. Read every step fully before starting.
> Run only the read-only verification commands listed. Do NOT modify any
> source file other than the spike output document and `plans/README.md`.
> If anything in the "STOP conditions" section occurs, stop and report.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands.rs src-tauri/migrations/ src/lib/api.ts`
> If any of those files changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on
> a mismatch treat it as a STOP condition.

## Status

- **Priority**: direction
- **Effort**: spike
- **Risk**: MED
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The method's "what-if" workflow is explicitly "copy the sheet and label it
(simulação)": the user wants to lay hypothetical obligations — a move, a big
purchase, a trip — on top of the real ledger and immediately see how
month-end balance, "buraco do futuro" (`deepest_deficit`), and Performance
compare against the real baseline. Today the app has no mechanism for this;
the only way to explore a scenario is to actually add transactions to the
live ledger, which pollutes the real data.

The forecast engine (`src-tauri/src/forecast/mod.rs`) is already a pure
function over a `seed` + `&[CashflowEvent]` + `NaiveDate` — it has no IO
and no side effects. Running a scenario branch is therefore cheap: build a
second event slice (real events ∪ hypothetical events) and call `project()`
or `project_with_metrics()` a second time. No engine changes are required
for the spike's first cut; the only question is where to store hypothetical
transactions and how to present the comparison.

This spike defines the data model, the engine call shape, the diff/compare
output structure, and the UI entry point — so that plan 021 (or a successor)
can be implemented as a narrow vertical slice.

## Current state

### Forecast engine public surface (`src-tauri/src/forecast/mod.rs`)

Key types and functions the scenario layer will call directly (line numbers
verified at commit d183bbf):

```rust
// mod.rs:21-33 — EventKind enum
pub enum EventKind { Income, FixedOut, Daily, Economia }

// mod.rs:35-41 — the unit of projection
pub struct CashflowEvent {
    pub date: NaiveDate,
    pub kind: EventKind,
    pub amount_cents: i64,  // always positive magnitude
    pub realized: bool,
}

// mod.rs:81-95 — the full output bundle
pub struct Forecast {
    pub daily: Vec<DayPoint>,
    pub month_end: Vec<MonthEnd>,
    pub deepest_deficit: Option<DayPoint>,
    pub cash_floor_cents: i64,
    pub months: Vec<MonthMetric>,
}

// mod.rs:477-489 — primary entry point (pure, no IO)
pub fn project(seed_cents: i64, today: NaiveDate,
               events: &[CashflowEvent], horizon_end: NaiveDate) -> Forecast

// mod.rs:499-537 — variant that separates chain events from metric events
pub fn project_with_metrics(seed_cents: i64, today: NaiveDate,
    chain_events: &[CashflowEvent],   // forward window only
    metric_events: &[CashflowEvent],  // full month (realized + projected)
    horizon_end: NaiveDate) -> Forecast

// mod.rs:136-166 — safe-to-spend with dual guardrail
pub fn safe_to_spend_today(fc: &Forecast,
    annual_income_cents: i64, annual_savings_cents: i64,
    savings_target_bps: i64, reserve_floor_cents: i64) -> SafeToSpend

// mod.rs:242-266 — row→EventKind classifier (pure)
pub fn classify(txn_type: &str, is_fixed: bool,
    payment_method: Option<&str>, to_liquidity: Option<&str>) -> Option<EventKind>
```

The engine is 100% pure (no IO, no ambient clock, no DB). Running a
scenario branch is a second call to `project_with_metrics` with the same
seed but a different event slice. The existing 30+ unit tests in
`mod.rs:539-1026` cover every branch and can serve as the regression net.

### Imperative shell — command that wraps the engine (`src-tauri/src/commands.rs`)

The real forecast pipeline (verified at d183bbf):

```rust
// commands.rs:663-691 — computes horizon from latest transaction or balance row
async fn forecast_horizon_end(pool, today_naive) -> Result<NaiveDate, String>

// commands.rs:697-798 — loads forward events from DB (transactions + credit lumps)
async fn load_cashflow_events(pool, today_naive, horizon_end) -> Result<Vec<CashflowEvent>, String>
// SQL used:
// "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed,
//  t.is_projection, COALESCE(a.liquidity,'')
//  FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id
//  WHERE t.date > ?1 AND t.date <= ?2"

// commands.rs:803-830 — loads events + daily ceiling for the current-month driver
async fn load_forecast_events(pool, today_naive, horizon_end) -> Result<Vec<CashflowEvent>, String>

// commands.rs:987-988 — public Tauri command
#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String>

// commands.rs:993-1074 — inner: projection_seed + load_forecast_events + project_with_metrics
async fn forecast_dto(pool: &SqlitePool, today_naive: NaiveDate) -> Result<ForecastDto, String>
```

Constants used by the real forecast:

```rust
// commands.rs:482 — savings guardrail: 25% annual (20–30% range, annual average)
const SAVINGS_TARGET_BPS: i64 = 2500;
// commands.rs:487 — future-month completeness threshold: 60% of typical outflow
const COVERAGE_COMPLETE_BPS: i64 = 6_000;
```

### Transaction schema (`src-tauri/migrations/20240608000006_transaction.sql`)

```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,      -- cents, positive magnitude
    description TEXT,
    date TEXT NOT NULL,           -- ISO-8601 YYYY-MM-DD
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

There is currently no `scenario_id` column. `NULL` scenario = real ledger
is the natural sentinel for the first cut.

### Account schema (`src-tauri/migrations/20240608000003_account.sql`)

```sql
CREATE TABLE IF NOT EXISTS account (
    id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('bank','credit_card','wallet','savings','business')),
    owner_person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    ...
    balance INTEGER NOT NULL DEFAULT 0,
    ...
);
```

Accounts are shared across all scenarios. Scenario transactions do NOT
change `account.balance` — balances stay in the real ledger; the scenario
engine runs from the same `projection_seed`.

### Frontend types (`src/lib/api.ts`)

Key DTO interfaces the scenario compare output will extend (lines 114–203):

```ts
// api.ts:135-151 — per-month metrics exposed from Rust
interface MonthMetric {
  year: number; month: number;
  income_cents: number; performance_cents: number;
  cost_of_living_cents: number; fixed_out_cents: number;
  daily_out_cents: number; real_daily_avg_cents: number;
  economia_cents: number; savings_rate_bps: number;
}

// api.ts:178-203 — top-level ForecastDto (the "real" branch already returns this)
interface ForecastDto {
  safe_to_spend_today_cents: number;
  cash_headroom_cents: number;
  deepest_deficit: DayPoint | null;
  daily: ForecastDay[];
  month_end: MonthEnd[];
  months: MonthMetric[];
  // ... (annual_savings, coverage, binding_guardrail, etc.)
}
```

The scenario compare output will be a new DTO, NOT a change to `ForecastDto`.

### Existing navigation (`src/screens/` directory)

Screens at d183bbf: Dashboard, Totais, Anual, Horizonte, Lançamentos, Tags,
Mia, Metodologia, Configurações. No scenario or what-if screen exists yet.
The Horizonte screen (`HorizonteScreen.tsx`) shows the full day-by-day
balance trajectory from `get_forecast`; it is the natural visual anchor for
a "real vs scenario" overlay.

### Domain vocabulary (from `CONTEXT.md`)

Use these exact terms in names and comments:
- **Performance** = `income − (cost_of_living + economia + projected_remaining_daily)`
- **buraco do futuro** = `deepest_deficit` (the minimum projected balance in the horizon)
- **Economizado%** = `economia_cents / income_cents` (the method's savings rate)
- **Economia** = `EventKind::Economia` (transfer to reserve/illiquid account = "guardar")
- **Custo de vida** = `FixedOut + Daily realized`
- **Person** (not "user"); **Account** (real financial instrument)
- **Transaction.type**: `income | expense | transfer`
- **Transaction.payment_method**: `debit | credit | pix | cash`
- `amount_cents` — always positive magnitude; sign implied by `EventKind`

### Architecture note (`docs/architecture.md:58`)

"Evals for diagnoses and safe write behavior; what-if scenarios." is listed
as a next-slice item (item 13 in the MVP Slices table), confirming this
spike is on the roadmap and scoped for future work.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Drift check | `git diff --stat d183bbf..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands.rs src-tauri/migrations/ src/lib/api.ts` | shows changed files or empty |
| Rust engine tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast` | all pass, ~30 tests |
| Full type-check | `npm run typecheck` | exit 0 |
| Full test suite | `npm run test:run` | all pass |
| Full gate | `npm run check` | exit 0 |

This spike does NOT run any gate during authoring (read-only). The executor
who implements the successor plan will run the full gate.

## Scope

**In scope** (the only artifact this spike produces):

- `specs/020-whatif-scenarios/spike.md` — the design document described in
  the Steps below (schema sketch, engine call shape, compare DTO, UI entry
  point, what is in/out for a first cut, open questions).

**Out of scope** (do NOT touch):

- `src-tauri/src/forecast/mod.rs` — no engine changes needed for the spike
  or the first-cut implementation; the pure function already handles it.
- `src-tauri/src/commands.rs` — read for context only; no changes here.
- `src-tauri/migrations/` — the migration for `scenario` will live in the
  implementation plan that follows this spike.
- Any React screen or component — UI shape is described in the spike doc,
  not built.
- `plans/019-spike-invoice-entity.md` — unrelated spike.

## Git workflow

- Branch: `advisor/020-spike-whatif`
- One commit for the spike doc, one for the README update.
- Commit message style (matches repo log):
  `docs: spike — what-if scenario branching design (plan 020)`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Confirm engine purity and call shape

Open `src-tauri/src/forecast/mod.rs` and read lines 477–537. Confirm that
`project()` and `project_with_metrics()` take only value arguments (no DB,
no clock). Copy the exact signatures into the spike doc. This establishes
the core claim: a scenario branch is a second call to `project_with_metrics`
with the same `seed_cents` and `today` but with hypothetical events appended
to the real event slice.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast`
→ all pass (confirms the engine the spike describes is green and stable).

### Step 2: Read the real forecast pipeline entry point

Open `src-tauri/src/commands.rs` lines 663–1074. Identify the three private
helpers (`forecast_horizon_end`, `load_cashflow_events`, `load_forecast_events`)
and the public Tauri command `get_forecast` / inner `forecast_dto`. Note the
SQL in `load_cashflow_events` (around line 707-711) that queries
`"transaction" WHERE t.date > ?1 AND t.date <= ?2`.

The spike doc must explain: adding a `scenario_id` filter to that SQL is the
minimal change needed to isolate scenario transactions from real ones.

**Verify**: read-only; no command to run. Confirm line numbers match the
excerpts in "Current state" above. If they don't, treat as a STOP condition.

### Step 3: Read the transaction schema

Open `src-tauri/migrations/20240608000006_transaction.sql`. Confirm the
column list matches the excerpt in "Current state" (no `scenario_id` column
exists). This is the table that receives the new nullable FK.

**Verify**: `git show d183bbf:src-tauri/migrations/20240608000006_transaction.sql`
→ output matches the excerpt in "Current state" (no drift).

### Step 4: Create the spike output directory and document

Create `specs/020-whatif-scenarios/spike.md` with the content specified
below. All sections are required.

**spike.md must contain all of the following sections**:

---

#### 4a. Problem statement (2–4 sentences)

The method's "what-if" workflow is: copy the sheet, label it "simulação",
add hypothetical rows, and compare month-end against the original. The app
must replicate this without polluting the real ledger. The forecast engine
is a pure function and already supports this at zero cost; the gap is
persistence of hypothetical transactions and a comparison DTO.

---

#### 4b. Scenario data model (schema sketch — not a migration)

```sql
-- NEW table: one row per named scenario
CREATE TABLE scenario (
    id   TEXT PRIMARY KEY NOT NULL,             -- uuid
    name TEXT NOT NULL,                         -- e.g. "Mudança para SP"
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ALTERED column on existing table (migration adds this column):
ALTER TABLE "transaction" ADD COLUMN scenario_id TEXT REFERENCES scenario(id) ON DELETE CASCADE;
-- NULL = real ledger (the existing rows stay NULL; no backfill needed)
-- NOT NULL scenario_id = hypothetical transaction belonging to that scenario
```

**Key invariants**:

- Scenario transactions share the same `type`/`amount`/`date`/`payment_method`/
  `is_fixed`/`from_account_id`/`to_account_id` shape as real transactions.
  No separate table is needed.
- Scenario transactions do NOT write to `account.balance`. The projection
  seed (`projection_seed()`) always comes from the real ledger (liquid
  balances or `sheet_daily_balance`).
- Deleting a scenario cascades to its transactions (safe).
- The existing `load_cashflow_events` SQL needs one extra filter:
  `AND t.scenario_id IS NULL` for the real branch; `AND t.scenario_id = ?`
  for the scenario branch.
- `daily_checkin` is not scoped per scenario — credit-cycle lumps (Régua 2)
  always reflect real card spend. This is intentional: you're projecting
  what-if obligations on top of real ongoing spend.

---

#### 4c. Engine call shape for a scenario branch

The new Tauri command `get_scenario_forecast` (or `compare_scenario`) will:

1. Call `projection_seed(pool, today)` — same seed as the real forecast (the
   scenario doesn't change past balances).
2. Call `load_forecast_events(pool, today, horizon_end)` — real events only
   (`WHERE scenario_id IS NULL`, plus daily ceiling and credit lumps).
3. Load scenario-specific events:
   ```sql
   SELECT t.type, t.amount, t.date,
          COALESCE(t.payment_method,''), t.is_fixed, t.is_projection,
          COALESCE(a.liquidity,'')
   FROM "transaction" t
   LEFT JOIN account a ON a.id = t.to_account_id
   WHERE t.scenario_id = ?1
     AND t.date > ?2 AND t.date <= ?3
   ```
   Map rows through `forecast::classify()` (same path as the real loader).
4. Build `scenario_events = real_events + hypothetical_events`.
5. Call `forecast::project_with_metrics(seed, today, &scenario_events, &scenario_metric_events, horizon_end)`.
6. Call `safe_to_spend_today(&scenario_fc, annual_income, annual_savings_amt, SAVINGS_TARGET_BPS, reserve_floor_cents)`.
7. Return a `ScenarioCompareDto` (see §4d below).

The real `forecast_dto` is unchanged. The scenario is always computed on
demand — no caching, no background job for the spike's first cut.

---

#### 4d. Compare output DTO

```rust
// New Rust DTO (not a change to ForecastDto):
pub struct ScenarioCompareDto {
    pub scenario_id: String,
    pub scenario_name: String,
    pub real:     ForecastSummary,   // re-use or subset of ForecastDto
    pub scenario: ForecastSummary,
    pub diff:     ForecastDiff,
}

pub struct ForecastSummary {
    pub month_end:             Vec<MonthEndDto>,   // balance per month-end
    pub deepest_deficit:       Option<DayPointDto>, // "buraco do futuro"
    pub performance_cents:     i64,   // current month's Performance
    pub safe_to_spend_today_cents: i64,
    pub binding_guardrail:     String, // "cash" | "savings"
}

pub struct ForecastDiff {
    /// month_end_cents[i] = scenario.month_end[i].balance - real.month_end[i].balance
    pub month_end_delta_cents: Vec<MonthEndDelta>,
    /// Change in deepest deficit (buraco do futuro): negative = scenario digs deeper hole
    pub deepest_deficit_delta_cents: i64,
    /// Change in current-month Performance
    pub performance_delta_cents: i64,
    /// Change in safe-to-spend-today
    pub safe_to_spend_delta_cents: i64,
}

pub struct MonthEndDelta {
    pub year: i32, pub month: u32,
    pub real_cents: i64,
    pub scenario_cents: i64,
    pub delta_cents: i64, // scenario − real (negative = scenario is worse)
}
```

TypeScript mirror (`src/lib/api.ts` additions — added in the implementation
plan, not here):

```ts
interface ScenarioCompareDto {
  scenario_id: string;
  scenario_name: string;
  real: ForecastSummary;
  scenario: ForecastSummary;
  diff: ForecastDiff;
}
interface ForecastSummary {
  month_end: MonthEnd[];
  deepest_deficit: DayPoint | null;
  performance_cents: number;
  safe_to_spend_today_cents: number;
  binding_guardrail: "cash" | "savings";
}
interface ForecastDiff {
  month_end_delta_cents: MonthEndDelta[];
  deepest_deficit_delta_cents: number;
  performance_delta_cents: number;
  safe_to_spend_delta_cents: number;
}
interface MonthEndDelta {
  year: number; month: number;
  real_cents: number; scenario_cents: number; delta_cents: number;
}
```

---

#### 4e. UI entry point and interaction model

**Anchor screen**: Horizonte (`HorizonteScreen.tsx`). The screen already
renders the day-by-day balance trajectory from `get_forecast`; scenario
branching is a natural overlay here.

**First-cut interaction** (minimal slice, in/out defined in §4f):

1. A "Simular cenário" button in the Horizonte screen (or a floating action
   button in the shell) opens a modal/panel.
2. The panel lets the user select an existing named scenario or create a new
   one (name field only in the first cut).
3. After selecting/creating a scenario, the panel shows a compact entry form
   for hypothetical transactions (same fields as `NewTransactionForm.tsx` but
   tagged `scenario_id`; description is mandatory in scenarios to keep things
   readable).
4. After each hypothetical transaction is added, the app calls
   `get_scenario_forecast(scenario_id)` and renders:
   - A side-by-side or overlay `month_end` chart showing real vs scenario.
   - The "buraco do futuro" (`deepest_deficit`) for both.
   - The Performance and safe-to-spend delta (`diff.*_delta_cents`).
5. The compare view is read-only — no write-back to the real ledger; no
   changes to `account.balance`.

**Design tokens to use**: all from `src/design-system/` (the "Midnight
Ledger" token set). For the scenario overlay, use `--color-text-muted` and
a dashed border vs the real baseline's solid jade line (`--color-jade-*`).
Money amounts are displayed with `<Money>` (never animated).

---

#### 4f. What is in scope for the first-cut implementation (next plan)

IN for first cut:
- `scenario` table + migration (`ALTER TABLE "transaction" ADD COLUMN scenario_id`).
- CRUD commands: `create_scenario`, `list_scenarios`, `delete_scenario`,
  `add_scenario_transaction`, `delete_scenario_transaction`.
- `get_scenario_forecast(scenario_id: String)` returning `ScenarioCompareDto`.
- The real `get_forecast` command and all existing screens are unchanged.
- A modal in Horizonte screen that: lists scenarios, creates a new one by
  name, accepts hypothetical transactions, and renders the compare DTO.
- Unit tests: `forecast::project_with_metrics` called with a combined
  real+hypothetical slice (tests already in `mod.rs` cover the pure engine;
  new tests cover the shell query logic and DTO mapping).

OUT for first cut (deferred):
- Cloning the FULL real ledger into a scenario ("fork the entire month") —
  this is expensive and unclear UX; start with additive hypotheticals only.
- Scenario sharing or persistence outside the local SQLite.
- Multiple simultaneous scenarios shown in a single overlay (start with
  one active scenario at a time in the UI).
- Editing an existing scenario transaction (delete + re-add is sufficient
  for the first cut).
- `daily_checkin` scoping per scenario — credit lumps always reflect real
  card spend (by design in the first cut).
- Scenario-aware write-back to Google Sheets.
- Scenario "performance" badge in the main Dashboard — deferred until the
  compare UX is validated.

---

#### 4g. Open questions (must be answered before the implementation plan)

1. **Cloning vs additive**: the method says "copy the sheet". Should the
   first-cut UI let the user optionally "clone this month's real transactions
   into the scenario" so they get a full what-if baseline? Or is purely
   additive (add only the hypothetical delta) sufficient for the use case?
   **Impact**: cloning requires a bulk insert of all real forward transactions
   into the scenario; additive is cheaper but the UX is different ("add only
   the new things on top of reality").

2. **Scenario transactions' dates**: hypothetical obligations often span
   multiple months (e.g., "monthly rent R$ 4.500 for 6 months starting
   August"). Should `add_scenario_transaction` accept a recurrence rule (reuse
   `recurrence.rs`) or accept a flat list of individual dates? Recurrence
   support would reuse existing infrastructure but adds complexity to the
   first cut.

3. **Horizon extension**: if the scenario adds a transaction dated 6 months
   out (beyond the current `forecast_horizon_end`), should the scenario's
   horizon automatically extend? The real `forecast_horizon_end` today is
   driven by `MAX(date)` in the real transaction table — a scenario
   transaction in a future month beyond the current horizon won't be picked
   up by the real loader unless the horizon logic is scenario-aware.

4. **Seed for the scenario**: the scenario always shares the real
   `projection_seed` (today's liquid balance). Is this correct, or should a
   scenario be able to start from a different assumed seed (e.g., "assume I
   get a bonus of R$ 20k next month")? The current answer is: add the bonus
   as a hypothetical `income` transaction in the scenario rather than
   overriding the seed.

5. **Scenario isolation from credit lumps**: `daily_checkin` rows (Régua 2)
   are not scoped per scenario — the credit-cycle lump at the due date
   reflects real ongoing spend. For a scenario that assumes "I stop using
   credit", should the hypothetical event list be able to suppress real credit
   lumps? This is complex for the first cut and likely rare; note it as a
   deferred option.

6. **Compare granularity**: the `ScenarioCompareDto` proposed above returns
   monthly `month_end` deltas and summary-level metrics. Is a full day-by-day
   `delta_cents` array also needed for the Horizonte overlay, or is monthly
   granularity sufficient to answer "is this scenario safe"?

7. **Scenario lifecycle**: should scenarios be ephemeral (deleted after the
   session / after the user dismisses) or persistent (named, saveable across
   app restarts)? The data model above supports persistence. However, the UX
   default could be "ephemeral unless named" to avoid clutter. Decide before
   the implementation plan.

---

#### 4h. Engine regression protection for the implementation plan

The successor implementation plan must include tests that call
`forecast::project_with_metrics` with a combined
`real_events + hypothetical_events` slice and assert that:

- The scenario month-end balance differs from the real month-end by exactly
  the amount of the hypothetical obligation (determinism check).
- Removing the hypothetical events from the slice restores the real baseline
  exactly (idempotency).
- A hypothetical income transaction increases `performance_cents` and
  `safe_to_spend_today_cents` by the expected delta.
- The engine never panics on an empty hypothetical slice (degenerate case).

Model these tests after the existing pattern in
`src-tauri/src/forecast/mod.rs:539` (the `fn d()` / `fn ev()` helpers and
the direct `project()` / `project_with_metrics()` calls).

---

**Verify**: `npm run typecheck` → exit 0 (the spike doc is a markdown file;
typecheck ensures no accidental changes to `.ts` source files).

### Step 5: Update plans/README.md

Change the status of plan 020 in the table from `TODO` to `IN PROGRESS` (or
`DONE` when the spike doc is complete and reviewed). No other rows change.

**Verify**: `git diff --stat` shows only two files modified:
`specs/020-whatif-scenarios/spike.md` (new) and `plans/README.md` (updated).
No source files appear in the diff.

## Test plan

This is a spike — no test code is written by this plan. The spike document
itself (§4h above) specifies the tests the successor implementation plan
must include. To confirm the engine is stable before the spike is authored:

`cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast`
→ all pass (approximately 30 tests in `forecast::tests`).

## Done criteria

All must hold before marking this plan DONE:

- [ ] `specs/020-whatif-scenarios/spike.md` exists and contains all eight
  sections (4a–4h) above.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- forecast`
  exits 0 (engine unchanged).
- [ ] `npm run typecheck` exits 0.
- [ ] `git diff --stat` shows zero changes to any file under `src/`,
  `src-tauri/src/`, or `src-tauri/migrations/`.
- [ ] `plans/README.md` status row for plan 020 is updated (not `TODO`).
- [ ] The spike doc answers or explicitly defers every question in §4g.

## STOP conditions

Stop and report back (do not improvise) if:

- The `project_with_metrics` signature at `src-tauri/src/forecast/mod.rs:499`
  does not match the excerpt in "Current state" — the engine may have changed
  and the spike's core claim (scenario = second call, same seed) must be
  re-verified.
- The `"transaction"` table at `src-tauri/migrations/20240608000006_transaction.sql`
  already has a `scenario_id` column — someone may have implemented this
  partially; review what exists before proposing another schema.
- The `load_cashflow_events` SQL in `commands.rs` (around lines 707–711)
  already filters on `scenario_id` — same concern as above.
- A step's verification fails twice after a reasonable fix attempt.
- Authoring the spike doc requires touching any source file (`.rs`, `.ts`,
  `.tsx`, `.sql`) — that means scope has expanded into implementation; stop
  and split into a separate implementation plan.
- `cargo test` for the forecast module fails — do not proceed until the
  regression net is green (the spike assumes the engine is stable).

## Maintenance notes

**For the human/agent who picks up this spike after review**:

- The successor implementation plan (plan 021 or a new number) should read
  §4f ("IN for first cut") as its scope and §4g ("Open questions") as its
  prerequisites. Answer every open question before writing the implementation
  plan.
- The engine (`forecast/mod.rs`) needs no changes for the first cut. Do not
  touch it unless open question 6 (day-by-day delta) is answered YES.
- The `load_cashflow_events` function in `commands.rs` is the only place
  where the `scenario_id IS NULL` filter must be added to isolate the real
  branch. The scenario branch gets its own loader (copy the function, add
  `WHERE t.scenario_id = ?1`). Do NOT generalize with a boolean flag — keep
  the two paths separate for readability (see "imperative shell" convention
  in `AGENTS.md`).
- Plan 011 ("split the commands.rs god-module") is independent of this spike
  but will eventually need to include the new scenario commands in the split.
  Coordinate with plan 011's executor if both run concurrently.
- The `SAVINGS_TARGET_BPS = 2500` and `COVERAGE_COMPLETE_BPS = 6_000`
  constants in `commands.rs` apply equally to scenario projections; do not
  introduce separate scenario-specific constants.
- Money amounts are always positive magnitude (`amount_cents: i64`); sign is
  implied by `EventKind`. This invariant must hold for hypothetical
  transactions too.
