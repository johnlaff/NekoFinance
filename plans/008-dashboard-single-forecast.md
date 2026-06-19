# Plan 008: Dashboard: single forecast source + unified cache key

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat d183bbf..HEAD -- src/screens/DashboardScreen.tsx src/lib/useCommand.ts src-tauri/src/commands.rs src/lib/api.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: perf
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Every `DashboardScreen` load fires two Tauri commands — `get_dashboard_summary` and `get_forecast` — each of which independently runs the full forecast pipeline (seed, horizon, event load, projection, `effective_daily_ceiling`). That doubles CPU and SQLite I/O on the most-visited screen. Additionally, the cache keys embed `reloadKey` (e.g. `get_dashboard_summary:0`, `get_forecast:0`) while every other screen uses the plain keys `get_dashboard_summary` and `get_forecast`; this means navigating to the Dashboard always triggers a network-equivalent fetch even when the data is fresh in the shared cache. Finally, `effective_daily_ceiling` is computed twice per `get_dashboard_summary` call (once inside `dashboard_summary` at line 1361, once inside `load_forecast_events` at line 811). Eliminating the duplicate pipeline and aligning the cache key makes the dashboard load from cache on return visits and halves server-side work on cold loads.

## Current state

### Relevant files and their roles

- `src/screens/DashboardScreen.tsx` — Dashboard UI; currently issues two `useCommand` calls with reloadKey-embedded cache keys (lines 22–26).
- `src/lib/useCommand.ts` — SWR-lite cache; one entry per cache-key string; a plain key like `"get_forecast"` is shared across screens; a key like `"get_forecast:0"` is private to this component instance (lines 11–76).
- `src/lib/api.ts` — Tauri invoke wrappers; `getDashboardSummary` (line 252) and `getForecast` (line 284) are the two fetcher functions.
- `src-tauri/src/commands.rs` — Rust command handlers; `get_forecast` (line 987), `get_dashboard_summary` (line 1329), `dashboard_summary` inner (line 1337), `forecast_dto` inner (line 993), `effective_daily_ceiling` helper (line 612), `load_forecast_events` (line 803).

### Code excerpts (verified at commit d183bbf)

**`src/screens/DashboardScreen.tsx` lines 20–30** — the two commands with divergent cache keys:
```tsx
export function DashboardScreen({ onAskMia }: { onAskMia: () => void }) {
  const [reloadKey, setReloadKey] = useState(0);
  const summaryQ = useCommand(
    `get_dashboard_summary:${reloadKey}`,
    getDashboardSummary,
  );
  const forecastQ = useCommand(`get_forecast:${reloadKey}`, getForecast);
  const summary = summaryQ.data ?? null;
  const forecast = forecastQ.data ?? null;
  const loading = summaryQ.loading || forecastQ.loading;
  const error = summaryQ.error ?? forecastQ.error;
```

**`src/lib/useCommand.ts` lines 11–35** — cache semantics (plain string key = shared slot):
```ts
const cache = new Map<string, unknown>();
// ...
export function useCommand<T>(cmd: string, fetcher: () => Promise<T>) {
  const [state, setState] = useState<CommandState<T>>(() => stateFor<T>(cmd));
  const visible = state.cmd === cmd ? state : stateFor<T>(cmd);
  useEffect(() => {
    if (!isTauri) return;
    // ...
    fetcher()
      .then((fresh) => {
        cache.set(cmd, fresh);
        // ...
      })
    // ...
  }, [cmd]);
```
The cache key is the first argument to `useCommand`. Using `"get_forecast"` (no suffix) means `TotaisScreen`, `HorizonteScreen`, and `CopilotScreen` all share one slot; `DashboardScreen` with `:${reloadKey}` uses a separate slot and forces a cold fetch every navigation.

**Other screens use plain keys** (`src/screens/TotaisScreen.tsx:151`, `src/screens/HorizonteScreen.tsx:49`, `src/screens/CopilotScreen.tsx:55`):
```ts
const forecastQ = useCommand("get_forecast", getForecast);
```

**`src-tauri/src/commands.rs` lines 800–823** — `load_forecast_events` calls `effective_daily_ceiling` (first invocation when used by `dashboard_summary`):
```rust
async fn load_forecast_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let mut events = load_cashflow_events(pool, today_naive, horizon_end).await?;
    let daily_ceiling = effective_daily_ceiling(pool, today_naive).await?;   // <── call 1
    // ...
}
```

**`src-tauri/src/commands.rs` lines 1359–1361** — `dashboard_summary` calls `effective_daily_ceiling` a second time:
```rust
    // Teto do diário exibido no tile "Diário de hoje" (`de R$X`):
    let daily_budget = effective_daily_ceiling(pool, today_naive).await?;    // <── call 2
```

**`src-tauri/src/commands.rs` lines 987–989** — `get_forecast` delegates to `forecast_dto`:
```rust
#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String> {
    forecast_dto(pool.inner(), chrono::Local::now().date_naive()).await
}
```

**`src-tauri/src/commands.rs` lines 1329–1332** — `get_dashboard_summary` delegates to `dashboard_summary`:
```rust
pub async fn get_dashboard_summary(
    pool: State<'_, SqlitePool>,
) -> Result<DashboardSummary, String> {
    dashboard_summary(pool.inner(), chrono::Local::now().date_naive()).await
}
```

**`src-tauri/src/commands.rs` `DashboardSummary` struct lines 1315–1326**:
```rust
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub credit_spend_month: i64,
    pub has_credit: bool,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
}
```

The `DashboardSummary.balance` field (projected end-of-month balance) is already derivable from `ForecastDto.month_end[current_month].balance_cents`, and `DashboardSummary.daily_budget` is derivable from `ForecastDto` data (`effective_daily_ceiling` is already called in `load_forecast_events` which `forecast_dto` also uses). However, the four remaining dashboard-specific fields (`daily_spend_today`, `credit_spend_month`, `has_credit`, `reserve_months`, `reserve_trend`, `transaction_count`) require their own queries and are NOT in `ForecastDto`. The cleanest minimum-risk fix is therefore:

1. **Rust side**: refactor `dashboard_summary` to call `forecast_dto` internally and re-use its already-computed `daily_ceiling` + projected balance, eliminating the second independent forecast pipeline call and the second `effective_daily_ceiling` invocation.
2. **Frontend side**: align the dashboard's two cache keys to the plain shared keys (`"get_dashboard_summary"` and `"get_forecast"`) and replace the `reloadKey`-based invalidation with `invalidateCommands()` alone (which already clears the entire cache map, as it calls `cache.clear()`).

### Repo conventions that apply

- **Functional-core / imperative-shell**: keep pure finance math in `src-tauri/src/forecast/`; I/O (SQL queries) stays in `commands.rs`.
- **Money is integer cents**: all `i64` amounts are cents; no floats for money. The existing `DashboardSummary.balance: i64` is correct.
- **React Compiler is enabled**: do NOT add `useMemo`, `useCallback`, or `React.memo` anywhere. The compiler handles memoization automatically.
- **`invalidateCommands()` clears the whole cache**: calling it is sufficient to force a re-fetch on the next render; no `reloadKey` bump is needed after that call, since the next `useEffect` in `useCommand` will see a cache miss on the same plain key.
- **Existing test pattern**: `#[tokio::test]` with `sqlite::memory:` + `sqlx::migrate!("./migrations")` — see `dashboard_balance_is_projected_not_raw_sum` at `commands.rs:2626` and `fixture_pool()` at `commands.rs:2822`.
- **CONTEXT.md vocabulary** (do not rename these in code): `EventKind` (`Income | FixedOut | Daily | Economia`), `Transaction` type (`income | expense | transfer`), `payment_method` (`debit | credit | pix | cash`), `is_fixed`, `Reserve`, `daily_budget`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Typecheck (frontend) | `npm run typecheck` | exit 0, no errors |
| Lint | `npm run lint` | exit 0 |
| Frontend unit tests | `npm run test:run` | all pass |
| Rust check (fmt + clippy + test) | `npm run rust:check` | exit 0 |
| Rust tests only | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass |
| Full gate | `npm run check` | exit 0 |
| Privacy scan | `npm run privacy:scan` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/screens/DashboardScreen.tsx`
- `src-tauri/src/commands.rs`

**Out of scope** (do NOT touch, even though they look related):
- `src/lib/useCommand.ts` — the cache implementation is correct; only the call-site keys need fixing.
- `src/lib/api.ts` — the Tauri invoke wrappers (`getDashboardSummary`, `getForecast`) stay unchanged; the Rust command names and signatures must not change.
- `src-tauri/src/forecast/` — no changes to the forecast engine or its math.
- Any other screen (`TotaisScreen`, `HorizonteScreen`, `CopilotScreen`, etc.) — they already use the correct plain keys.
- The `DashboardSummary` struct's public fields — the response shape visible to TypeScript must not change (the TS type in `src/lib/api.ts:28–39` must remain satisfied).
- Visual layout of `DashboardScreen` — plan 016 handles dashboard UI restructuring.
- The forecast math or `safe_to_spend_today_cents` logic.

## Git workflow

- Branch: `advisor/008-dashboard-single-forecast`
- Commit style: conventional commits, matching the repo style. Example from log: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`. For this plan use: `perf: dashboard single forecast pipeline + shared cache key`
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Fix the Rust double-pipeline — refactor `dashboard_summary` to reuse `forecast_dto`

Open `src-tauri/src/commands.rs`.

Find the `dashboard_summary` async function at line ~1337. Currently it independently calls `projection_seed`, `forecast_horizon_end`, `load_forecast_events` (which internally calls `effective_daily_ceiling`), then `forecast::project`, and then calls `effective_daily_ceiling` a SECOND time for `daily_budget`.

Replace the body of `dashboard_summary` so it:

1. Calls `forecast_dto(pool, today_naive).await?` once to obtain the full `ForecastDto`.
2. Derives `projected_balance` from `fc.month_end` (find the entry matching `today_naive.year()` and `today_naive.month()`; fall back to the last `fc.daily` entry's `balance_cents`; default to 0 — same logic as before).
3. Derives `daily_budget` from `effective_daily_ceiling(pool, today_naive).await?` — but only ONCE, not twice. Note: `forecast_dto` already calls `effective_daily_ceiling` internally via `load_forecast_events`. To avoid calling it a second time, extract the value from the forecast DTO instead. The `ForecastDto` does NOT currently expose `daily_budget` directly. The cleanest approach with minimal risk is to add a `daily_budget_cents: i64` field to `ForecastDto` / `ForecastDtoLocal` and populate it from the single `effective_daily_ceiling` call inside `forecast_dto`. Alternatively, keep the single call in `dashboard_summary` by NOT calling `load_forecast_events` separately anymore — since `forecast_dto` is now called, `effective_daily_ceiling` runs once inside it, and `dashboard_summary` calls it once more for `daily_budget`. That is still one call per `dashboard_summary` invocation (down from two), and `dashboard_summary` no longer runs `projection_seed` / `forecast_horizon_end` / `load_forecast_events` / `forecast::project` independently. This is acceptable. Choose this simpler approach: call `forecast_dto` for the projection, then call `effective_daily_ceiling` once for `daily_budget`.
4. Keep all four query blocks unchanged: `daily_spend`, `credit_spend`, `has_credit`, `reserve_balance`/`reserve_months`/`reserve_trend`, `count`. These are dashboard-specific and not in `ForecastDto`.

The resulting `dashboard_summary` function should look structurally like:

```rust
async fn dashboard_summary(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<DashboardSummary, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    // Single forecast pipeline — reuses the same engine path as get_forecast.
    let fc = forecast_dto(pool, today_naive).await?;
    let projected_balance = fc
        .month_end
        .iter()
        .find(|m| m.year == today_naive.year() as i32 && m.month == today_naive.month() as i32)
        .map(|m| m.balance_cents)
        .or_else(|| fc.daily.last().map(|p| p.balance_cents))
        .unwrap_or(0);

    // effective_daily_ceiling: called once here (forecast_dto also calls it internally via
    // load_forecast_events, but that result is not exposed on ForecastDto). Net: one SQL
    // round-trip for daily_budget, down from two in the previous implementation.
    let daily_budget = effective_daily_ceiling(pool, today_naive).await?;

    // ... existing daily_spend, credit_spend, has_credit, reserve, count queries unchanged ...

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        // ... rest unchanged
    })
}
```

Check the types carefully: `ForecastDto.month_end` is `Vec<MonthEndDto>`. Find `MonthEndDto` in `commands.rs` and confirm its field names (likely `year: i32`, `month: i32`, `balance_cents: i64`). Adjust the `.find` predicate to match the actual types.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -5` → all existing tests pass, including `dashboard_balance_is_projected_not_raw_sum`.

### Step 2: Add a regression test — `dashboard_summary` and `forecast_dto` return the same projected balance

In the `#[cfg(test)]` block in `src-tauri/src/commands.rs` (around line 2820), add a new `#[tokio::test]` function named `dashboard_balance_matches_forecast_month_end`. Use the `fixture_pool()` and `insert_liquid_account` helpers already present at lines ~2822–2852.

The test should:
1. Create a pool with `fixture_pool()`.
2. Insert a liquid account with a known balance (e.g. 200_000 cents).
3. Insert a future fixed expense (projection) dated later in the same test month.
4. Call both `dashboard_summary(&pool, today)` and `forecast_dto(&pool, today)` with the same injected `today`.
5. Assert that `summary.balance == forecast.month_end[0].balance_cents` (or whichever index matches the test month).

This is the regression for the "duplicate pipeline" bug: if `dashboard_summary` ever diverges from `forecast_dto` on the projected balance, this test will catch it.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked dashboard_balance_matches_forecast_month_end 2>&1` → 1 test passed.

### Step 3: Align the frontend cache keys — remove `reloadKey` from forecast/summary keys

Open `src/screens/DashboardScreen.tsx`.

**Current code at lines 21–26**:
```tsx
const [reloadKey, setReloadKey] = useState(0);
const summaryQ = useCommand(
  `get_dashboard_summary:${reloadKey}`,
  getDashboardSummary,
);
const forecastQ = useCommand(`get_forecast:${reloadKey}`, getForecast);
```

**Change to**:
```tsx
const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
const forecastQ = useCommand("get_forecast", getForecast);
```

Remove the `const [reloadKey, setReloadKey] = useState(0);` line (line 21).

`reloadKey` is currently used in three places:
1. The two `useCommand` keys (just fixed above).
2. `MonthLedgerCard` at line 233: `<MonthLedgerCard today={forecast.today} reloadKey={reloadKey} />`. This prop is intentional — `MonthLedgerCard` uses `reloadKey` in its own `useCommand` key (`month_grid:${ym}:${reloadKey}`) to force a re-fetch of the ledger grid on manual reload. Do NOT remove this usage; keep it working.

Since `MonthLedgerCard` still needs a reload trigger, introduce a separate state variable for it:
```tsx
const [ledgerKey, setLedgerKey] = useState(0);
```

Update `handleLogged` and `handleReload` to increment `ledgerKey` (not `reloadKey`):
```tsx
function handleLogged() {
  invalidateCommands();
  setLedgerKey((k) => k + 1);
}

function handleReload() {
  invalidateCommands();
  setLedgerKey((k) => k + 1);
}
```

Update line 233 to pass `ledgerKey`:
```tsx
<MonthLedgerCard today={forecast.today} reloadKey={ledgerKey} />
```

With this change: `invalidateCommands()` clears the entire cache (including `"get_dashboard_summary"` and `"get_forecast"`), so the next render triggers a background re-fetch via `useCommand`'s `useEffect`. The `ledgerKey` increment forces `MonthLedgerCard`'s grid to re-fetch its specific month. This matches the behavior of other screens that call `invalidateCommands()` without a key bump for shared commands.

**Verify**: `npm run typecheck` → exit 0, no errors.

### Step 4: Confirm no reloadKey references remain on the shared command keys

```
grep -n "reloadKey" src/screens/DashboardScreen.tsx
```

Expected output: lines mentioning `ledgerKey` / `setLedgerKey` and `reloadKey={ledgerKey}` on `MonthLedgerCard`. There must be NO line containing `` `get_dashboard_summary:${reloadKey}` `` or `` `get_forecast:${reloadKey}` ``.

**Verify**: `grep -n "get_dashboard_summary:\|get_forecast:" src/screens/DashboardScreen.tsx` → no output (zero matches). Then `grep -n "reloadKey" src/screens/DashboardScreen.tsx` → only shows the prop passed to `MonthLedgerCard` as `reloadKey={ledgerKey}`.

### Step 5: Lint and typecheck the full frontend

**Verify**: `npm run lint` → exit 0. Then `npm run typecheck` → exit 0.

If lint reports unused variable warnings for `reloadKey` or `setReloadKey`, double-check step 3 removed the `useState(0)` declaration entirely.

### Step 6: Run the full Rust check

**Verify**: `npm run rust:check` → exit 0 (fmt + clippy + tests all pass).

If clippy warns about an unused import or variable introduced/left by the refactor, fix it before continuing.

### Step 7: Run the frontend unit tests

**Verify**: `npm run test:run` → all pass. Note the test count; it should not decrease.

### Step 8: Run the privacy scan

**Verify**: `npm run privacy:scan` → exit 0 (no new findings).

### Step 9: Run the full gate

**Verify**: `npm run check` → exit 0.

If any check fails in this step but not in earlier steps, diagnose the failure; do not proceed to commit.

### Step 10: Update `plans/README.md`

Set the status of plan 008 to `DONE` in the table.

**Verify**: `grep "008" plans/README.md` → shows `DONE`.

## Test plan

### New Rust test (step 2)

**File**: `src-tauri/src/commands.rs` — in the existing `#[cfg(test)]` block.

**Test name**: `dashboard_balance_matches_forecast_month_end`

**Cases to cover**:
- Happy path: a liquid account + one future projected expense in the same month → `summary.balance` equals the month-end balance from `forecast_dto` for the same `today`.
- Regression for the duplicate-pipeline bug: if the two pipelines ever diverge, this assertion fails.

**Structural pattern**: model after `dashboard_balance_is_projected_not_raw_sum` at `commands.rs:2626` and use the `fixture_pool()` + `insert_liquid_account` helpers at lines ~2822–2852.

**Run**: `cargo test --manifest-path src-tauri/Cargo.toml --locked dashboard_balance_matches_forecast_month_end` → 1 test passed.

### Existing tests that must remain green

- `dashboard_balance_is_projected_not_raw_sum` (`commands.rs:2627`) — exercises `dashboard_summary`; must still pass after the refactor.
- All `get_forecast`-related tests in the `-- 005 get_forecast (TDD) --` block (around `commands.rs:2820`).
- All Vitest frontend tests (`npm run test:run`).

## Done criteria

All of the following must hold simultaneously before this plan is considered complete:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0 and test count did not decrease
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; test `dashboard_balance_matches_forecast_month_end` exists and passes
- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests)
- [ ] `npm run privacy:scan` exits 0
- [ ] `npm run check` exits 0
- [ ] `grep -n "get_dashboard_summary:\|get_forecast:" src/screens/DashboardScreen.tsx` returns zero matches (no reloadKey-embedded cache keys on the two shared commands)
- [ ] `grep -n "reloadKey" src/screens/DashboardScreen.tsx` returns at most one match (the prop passed to `MonthLedgerCard` as `reloadKey={ledgerKey}`)
- [ ] The `DashboardSummary` Rust struct fields are unchanged (same field names and types at `commands.rs` around line 1315)
- [ ] The TypeScript `DashboardSummary` interface in `src/lib/api.ts:28–39` is unchanged
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD` shows only `src/screens/DashboardScreen.tsx`, `src-tauri/src/commands.rs`, and `plans/README.md`)
- [ ] `plans/README.md` row for plan 008 shows `DONE`

## STOP conditions

Stop and report (do not improvise) if:

- The code at any cited location does not match the excerpts in "Current state" — the file has drifted; re-verify the line numbers and report before making changes.
- `dashboard_summary` at `commands.rs:1337` already delegates to `forecast_dto`; that would mean the Rust side is already fixed and only the frontend cache key alignment remains — report the finding and proceed only with the frontend step.
- The `ForecastDto` struct's `month_end` field has a different type or field name than assumed (e.g. `balance_cents` is named differently) — stop; the type mapping in step 1 must be corrected before proceeding.
- After step 1, `dashboard_balance_is_projected_not_raw_sum` fails — the refactor broke the existing projection logic; undo and diagnose before continuing.
- Step 3 reveals that `reloadKey` is used in more places than described (e.g. additional child components receive it) — stop and report; do not guess which uses are safe to change.
- A `cargo test` run produces a compile error involving `forecast_dto` type signatures — the inner function may not be callable from the test module due to visibility; expose it with `pub(crate)` or restructure accordingly, then report.
- `npm run check` fails on a check that passed individually — there may be a cross-check interaction; report rather than patching around it.

## Maintenance notes

- **Future: expose `daily_budget_cents` on `ForecastDto`**: currently `effective_daily_ceiling` is called once inside `load_forecast_events` (consumed by `forecast_dto`) and once in `dashboard_summary`. If a future screen needs the daily ceiling from the forecast DTO directly, add `daily_budget_cents: i64` to `ForecastDto` / `ForecastDtoLocal` and remove the standalone `effective_daily_ceiling` call from `dashboard_summary` — this would reduce it to zero extra calls. That is a one-step follow-up with minimal risk once there is a consumer.
- **Plan 016 (navigation/IA restructure)**: that plan may change which fields `DashboardScreen` displays and may merge or split the dashboard summary card. After plan 016 lands, revisit whether `get_dashboard_summary` is still needed or whether all its fields can be derived directly from `ForecastDto`.
- **Plan 011 (split commands.rs)**: when `commands.rs` is split, `dashboard_summary` and its `forecast_dto` dependency will need to be in the same module or use `pub(crate)` visibility to call across module boundaries.
- **Reviewer focus**: confirm that `projected_balance` derivation from `ForecastDto.month_end` matches the original logic in `dashboard_summary` (the `find` + fallback chain); any divergence here would cause the `balance` tile to show a different value than before.
