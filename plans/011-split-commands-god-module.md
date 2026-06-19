# Plan 011: Split the commands.rs god-module + dedupe the row mapper

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/commands.rs src-tauri/src/lib.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/010-characterization-tests.md
- **Category**: tech-debt
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

`src-tauri/src/commands.rs` is 3,832 lines — roughly 15× the median Rust
file — with 29 `#[tauri::command]` functions, ~38 private async helpers, 2
module-level constants, and 45 tests all in a single file. The row→`CashflowEvent`
SQL query + iterator mapper is copy-pasted verbatim three times
(`load_cashflow_events` lines 707–797, `load_realized_month_events`
lines 830–867, `load_year_events` lines 1157–1185), with identical SQL column
order and identical `filter_map` closure body; they differ only in the `WHERE`
clause. Any bug fix or schema change to the mapping must be applied three
times, and reviewers cannot tell whether divergence is intentional. Splitting
into cohesive submodules (following the repo's existing `google_sheets/*` and
`forecast/mod.rs` patterns) and extracting a single shared mapper restores
maintainability without changing any observable behaviour: all public
`#[tauri::command]` names and signatures are kept identical (they are the
frontend `invoke` contract, wired in `src-tauri/src/lib.rs`).

## Current state

### File sizes and locations

- `src-tauri/src/commands.rs` — 3,832 lines; single module; owns everything.
- `src-tauri/src/lib.rs` — wires all `#[tauri::command]`s in
  `tauri::generate_handler![…]` (lines 21–64); also declares the Rust module
  tree (lines 3–11). This file must be updated to declare the new submodules
  and re-export commands from them.
- `src-tauri/src/forecast/mod.rs` — pure engine; single file; exemplar for a
  domain module with no IO (no `sqlx`, no `tauri::State`).
- `src-tauri/src/google_sheets/` — exemplar multi-file module dir:
  `mod.rs`, `import.rs`, `layout_detect.rs`, `reconcile.rs`, `write_back.rs`.

### Cohesion groupings in commands.rs (verified line numbers)

Each group below is a self-contained concern. The boundary is drawn where the
existing section comments are:

| Proposed submodule | Line range in commands.rs | Public commands included |
|---|---|---|
| `commands/oauth.rs` | 1–91 | `start_oauth_flow`, `check_auth_status`, `disconnect_google` |
| `commands/sheets_import.rs` | 93–413 | `list_sheet_names`, `fetch_sheet_preview`, `import_sheet_data`, `import_local_xlsx`, `import_economia_sheet`, `detect_sheet_layout`, `save_sheet_mapping`, `get_sheet_mappings` |
| `commands/forecast.rs` | 415–1310 | `get_app_info`, `get_forecast`, `get_month_grid`, `get_annual_metrics`, `get_dashboard_summary` |
| `commands/pockets.rs` | 1469–1760 | `get_pockets`, `create_account` |
| `commands/transactions.rs` | 1647–1862 | `get_recent_transactions`, `create_transaction` |
| `commands/write_back.rs` | 1864–2540 | `preview_write_back`, `apply_write_back`, `write_back_enabled`, `preview_economia_write_back`, `apply_economia_write_back`, `get_app_setting`, `set_app_setting`, `backup_database`, `list_user_spreadsheets` |

Note: `get_recent_transactions` starts at line 1647 inside the transactions
section; `get_dashboard_summary` is within forecast (line 1328 onward). Some
helpers straddle sections (e.g. `SAVINGS_TARGET_BPS` at line 482 and
projection-seed helpers at lines 433–570 live in the forecast concern).

### The duplicated row mapper (three copies — verified)

All three share the same SQL column tuple `(String, i64, String, String, i64, i64, String)`
and the same `filter_map` body. Their WHERE clauses differ:

**Copy 1** — `load_cashflow_events` (lines 707–735): future window `date > today AND date <= horizon`

```rust
// commands.rs:707-735
let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
    "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
             COALESCE(a.liquidity,'') \
     FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date > ?1 AND t.date <= ?2",
)
// …then filter_map identical to copies 2 & 3:
let mut all_events: Vec<CashflowEvent> = txn_rows
    .into_iter()
    .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
        let pm = (!pm.is_empty()).then_some(pm);
        let to_liq = (!liq.is_empty()).then_some(liq);
        let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
        Some(CashflowEvent {
            date,
            kind,
            amount_cents: amount.abs(),
            realized: is_proj == 0,
        })
    })
    .collect();
```

**Copy 2** — `load_realized_month_events` (lines 838–866): realized window `date >= start AND date <= today`

```rust
// commands.rs:838-866
let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
    "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
             COALESCE(a.liquidity,'') \
     FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date >= ?1 AND t.date <= ?2",
)
// same filter_map body
```

**Copy 3** — `load_year_events` (lines 1158–1185): whole-year window `substr(date,1,4) = year`

```rust
// commands.rs:1158-1185
let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
    "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
             COALESCE(a.liquidity,'') \
     FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE substr(t.date, 1, 4) = ?1",
)
// same filter_map body
```

The shared mapper to extract (into `commands/cashflow_row.rs` or inline in
`commands/forecast.rs`):

```rust
/// Maps a raw DB row to a `CashflowEvent`. Returns `None` for rows that
/// `forecast::classify` cannot classify (e.g. unrecognised type — filtered
/// silently, consistent with the current behaviour of all three callers).
///
/// Invariant: `amount_cents` is always a POSITIVE MAGNITUDE; the sign is
/// conveyed by `kind` (see `forecast::signed`). `.abs()` guards against
/// a non-canonical negative stored by a buggy writer.
pub fn map_cashflow_row(
    (ttype, amount, date_str, pm, is_fixed, is_proj, liq):
        (String, i64, String, String, i64, i64, String),
) -> Option<CashflowEvent> {
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    let pm = (!pm.is_empty()).then_some(pm);
    let to_liq = (!liq.is_empty()).then_some(liq);
    let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
    Some(CashflowEvent {
        date,
        kind,
        amount_cents: amount.abs(),
        realized: is_proj == 0,
    })
}
```

### Module declarations to add to lib.rs

```rust
// src-tauri/src/lib.rs (current, lines 3-11)
mod commands;
mod conflicts;
mod forecast;
mod google_sheets;
mod http;
mod oauth;
mod recurrence;
mod splits;
mod tags;
```

After this plan: `mod commands;` remains; new submodules live under
`src-tauri/src/commands/` as a module directory. Alternatively keep
`commands.rs` as the top-level re-export file and add inner `mod` declarations
inside it — see Step 1 for the chosen approach.

### Existing exemplar: google_sheets module

`src-tauri/src/google_sheets/mod.rs` is the structural model: it declares
inner modules (`pub mod import; pub mod layout_detect; …`) and re-exports
types that callers need. Match this pattern.

### Frontend invoke contract (do not break)

`src-tauri/src/lib.rs` lines 21–64 list every `#[tauri::command]` function
passed to `tauri::generate_handler![…]`. The Tauri macro uses the Rust
function name as the JS command name. **All 29 public command function names
and signatures in this list must remain identical.** The only allowed change
to `lib.rs` is re-rooting the path (e.g. `commands::forecast::get_forecast`
instead of `commands::get_forecast`) OR keeping `commands` as a flat
re-export module so `commands::get_forecast` still resolves — either approach
is fine; pick the one that minimises the lib.rs diff.

### Domain vocabulary (CONTEXT.md)

Exact terms to use in names, comments, and doc-strings:
`Person`, `Account`, `Transaction`, `income | expense | transfer`,
`payment_method: debit | credit | pix | cash`, `is_fixed`, `Split`,
`owner_person_id`, `Reserve`, `daily_budget`, `EventKind: Income | FixedOut |
Daily | Economia`. Do not invent synonyms.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust compile + clippy | `npm run rust:check` | exit 0, zero warnings |
| Rust unit tests only | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass |
| Full gate | `npm run check` | exit 0 |
| Typecheck (frontend) | `npm run typecheck` | exit 0 |
| Privacy scan | `npm run privacy:scan` | exit 0 |

Run `npm run rust:check` (not the full gate) after **each step** to keep the
build green at every checkpoint.

## Scope

**In scope** (the only files you should create or modify):

- `src-tauri/src/commands.rs` — convert to a thin re-export module (or module
  directory entry point)
- `src-tauri/src/commands/` (new directory) with:
  - `mod.rs` (or keep `commands.rs` as the entry; see Step 1)
  - `oauth.rs`
  - `sheets_import.rs`
  - `forecast.rs` (includes shared `map_cashflow_row`)
  - `pockets.rs`
  - `transactions.rs`
  - `write_back_cmds.rs` (avoid name collision with the existing
    `src-tauri/src/google_sheets/write_back.rs`)
- `src-tauri/src/lib.rs` — update `mod commands` re-exports if paths change

**Out of scope** (do NOT touch, even if related):

- `src-tauri/src/forecast/mod.rs` — pure engine; this plan does not touch it
- `src-tauri/src/google_sheets/*` — already well-structured
- `src-tauri/src/tags.rs`, `recurrence.rs`, `splits.rs`, `conflicts.rs` —
  already in their own files; not part of commands.rs
- Any change to public `#[tauri::command]` function names or parameter types
- Frontend source files (`src/`) — no frontend changes needed
- Database migrations (`src-tauri/migrations/`) — no schema changes
- The pure `forecast` engine logic — this plan moves the shell; the core stays
- Any linting/clippy rule changes

## Git workflow

- Branch: `advisor/011-split-commands`
- Commit style: match the repo's conventional-commits style from `git log`:
  `chore: split commands.rs → commands/* submodules (step N/N)`
- One commit per step (or per logical extraction unit) so the build stays
  green at every SHA and the diff is reviewable in slices.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 0: confirm the plan's prerequisites

Verify that plan 010 (characterization tests) is DONE: the tests it added must
be green before refactoring the module they cover.

```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

**Verify**: exits 0, all tests pass (including any added by plan 010).
If any test fails, STOP — do not proceed until plan 010 is complete.

---

### Step 1: create the commands/ module directory and stub mod.rs

This is the structural foundation. After this step, `commands.rs` becomes the
module directory entry point at `commands/mod.rs` (Rust module system: rename
`commands.rs` → `commands/mod.rs`, or delete `commands.rs` and create the
directory with a `mod.rs` that re-exports everything).

**Option A (recommended, smallest diff to lib.rs)**: Convert `commands.rs` to a
module directory by:

1. Create directory `src-tauri/src/commands/`.
2. Move the current `src-tauri/src/commands.rs` to
   `src-tauri/src/commands/mod.rs` (use `git mv` so history is preserved).
3. The build should pass immediately — `mod commands;` in `lib.rs` resolves to
   `commands/mod.rs` automatically.

```bash
mkdir -p src-tauri/src/commands
git mv src-tauri/src/commands.rs src-tauri/src/commands/mod.rs
```

**Verify**: `npm run rust:check` → exit 0. No behaviour changes.

---

### Step 2: extract the shared row mapper

Add a small private function `map_cashflow_row` at the top of
`commands/mod.rs` (or in a new `commands/cashflow_row.rs` declared with
`mod cashflow_row;` at the top of `mod.rs`). The exact shape must match the
three copies verified in "Current state":

```rust
// In commands/mod.rs (or commands/cashflow_row.rs)
fn map_cashflow_row(
    (ttype, amount, date_str, pm, is_fixed, is_proj, liq):
        (String, i64, String, String, i64, i64, String),
) -> Option<crate::forecast::CashflowEvent> {
    use chrono::NaiveDate;
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    let pm = (!pm.is_empty()).then_some(pm);
    let to_liq = (!liq.is_empty()).then_some(liq);
    let kind = crate::forecast::classify(
        &ttype,
        is_fixed != 0,
        pm.as_deref(),
        to_liq.as_deref(),
    )?;
    Some(crate::forecast::CashflowEvent {
        date,
        kind,
        amount_cents: amount.abs(),
        realized: is_proj == 0,
    })
}
```

Replace all three identical `filter_map` closures in `load_cashflow_events`
(line ~720), `load_realized_month_events` (line ~852), and `load_year_events`
(line ~1170) with `.filter_map(map_cashflow_row).collect()`.

After replacing, delete the three old closure bodies. The surrounding SQL
query and `.bind` calls remain untouched.

**Verify**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Expected: all 45 tests pass. If any test that previously tested cashflow
classification fails, the mapper has drifted — STOP and compare against the
"Current state" excerpts.

**Also verify** the deduplication is complete:

```bash
grep -c "amount_cents: amount.abs()" src-tauri/src/commands/mod.rs
```

Expected: exactly `1` (the mapper) — not 3.

---

### Step 3: extract `commands/oauth.rs`

Move the OAuth group (lines 1–91 in the original; after Step 1 they are in
`commands/mod.rs` at the same lines). The group is:

- `fn quote_sheet(name)` — private helper; keep in `mod.rs` if used outside
  OAuth, else move alongside
- `pub async fn start_oauth_flow(…)`
- `pub async fn check_auth_status(…)`
- `pub async fn disconnect_google(…)`
- `fn bind_loopback_listener(…)` — private helper used only by `start_oauth_flow`

**What `quote_sheet` is used by**: it is called in `fetch_sheet_preview`,
`import_sheet_data`, `detect_sheet_layout`, `import_economia_sheet`, and
`list_sheet_names` — all in the sheets-import group. Move it to
`commands/sheets_import.rs` in Step 4, or keep it in `mod.rs` as a shared
helper for now. Keep `mod.rs` as the only public re-export surface.

Create `src-tauri/src/commands/oauth.rs`. Add `pub(super) mod oauth;` (or
`mod oauth;` with explicit `pub use`) at the top of `commands/mod.rs`. Move
the four functions and the private helper. Replace their bodies in `mod.rs`
with `pub use oauth::{start_oauth_flow, check_auth_status, disconnect_google};`.

Imports needed in `oauth.rs`:
```rust
use crate::oauth::{self, AppDataDir, OAuthStateStore};
```

**Verify**:
```
npm run rust:check
```
Expected: exit 0. All tests pass.

---

### Step 4: extract `commands/sheets_import.rs`

Group (from `commands/mod.rs` after prior steps):

- `pub struct SheetInfo` and `pub async fn list_sheet_names`
- `pub struct SheetPreview` and `pub async fn fetch_sheet_preview`
- `pub async fn import_sheet_data` (long — lines ~168–295 original)
- `pub async fn import_local_xlsx` (lines ~297–413 original; includes
  `fn xlsx_cell_to_string`, `fn validate_local_xlsx_path` private helpers)
- `pub async fn import_economia_sheet` (lines ~2404–2426 original)
- `pub async fn detect_sheet_layout` (lines ~1864–1910 original)
- `pub async fn save_sheet_mapping` (lines ~2428–2443 original)
- `pub async fn get_sheet_mappings` (lines ~2458–2489 original)
- `fn quote_sheet` — move here (used exclusively by this group)

Create `src-tauri/src/commands/sheets_import.rs`. Declare with
`pub(super) mod sheets_import;` in `mod.rs` and re-export the public commands.

The tests for `xlsx_float_cells_parse_to_correct_cents` (originally at
~line 2570), `local_xlsx_path_validation_rejects_non_xlsx` (~2604), and
`local_xlsx_path_validation_accepts_regular_xlsx_file` (~2614) should move
alongside `import_local_xlsx` into `sheets_import.rs` under a `#[cfg(test)]`
block at the bottom of the file. They use `xlsx_cell_to_string` and
`validate_local_xlsx_path`, which live in this module.

**Verify**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: all tests pass (total count unchanged or higher if sub-module tests
resolve previously shadowed ones).

```
npm run rust:check
```
Expected: exit 0.

---

### Step 5: extract `commands/forecast.rs`

This is the largest group. Move from `commands/mod.rs`:

- `pub async fn get_app_info` and private `fn app_info_for_dir` (the test
  `app_info_exposes_version_and_db_path` goes here too)
- `pub struct AppInfo` (or `AppInfo` and its fields)
- `const SAVINGS_TARGET_BPS: i64 = 2500;`
- `const COVERAGE_COMPLETE_BPS: i64 = 6_000;`
- All private async helpers: `liquid_seed`, `projection_seed`,
  `realized_annual_savings`, `realized_annual_economia`,
  `projected_annual_savings`, `realized_monthly_baseline`,
  `effective_daily_ceiling`, `reserve_floor`, `forecast_horizon_end`,
  `load_cashflow_events`, `load_forecast_events`,
  `load_realized_month_events`, `load_metric_events`, `load_year_events`,
  `forecast_dto`, `annual_metrics`, `month_grid`, `dashboard_summary`
- All public DTOs: `ForecastDayDto`, `DayPointDto`, `MonthEndDto`,
  `MonthMetricDto`, `AnnualSavingsDto`, `MonthCoverageDto`, `ForecastDto`,
  `AnnualMetricsDto`, `MonthGridDayDto`, `DashboardSummary`
- All public commands: `get_forecast`, `get_month_grid`, `get_annual_metrics`,
  `get_dashboard_summary`
- The shared `map_cashflow_row` function (if it currently lives in `mod.rs`,
  move it into this file — it is used only by the three helpers in this group)
- Forecast-related tests (the bulk of the 45 tests): all tests that call
  `forecast_dto`, `dashboard_summary`, `annual_metrics`, `month_grid`,
  `projection_seed`, `liquid_seed`, `realized_annual_savings`, and their
  fixture helpers (`fixture_pool`, `insert_liquid_account`,
  `insert_projection`, `insert_realized`, `insert_sheet_balance`,
  `insert_reserve_account`).

Note: `get_app_info` is in this group for convenience (its line range puts it
here), but it has no dependency on forecast; if a cleaner factoring puts it in
`mod.rs`, that is fine too.

Create `src-tauri/src/commands/forecast.rs`. Add `pub(super) mod forecast;` in
`mod.rs`.

**Verify**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: all tests pass.
```
npm run rust:check
```
Expected: exit 0.

---

### Step 6: extract `commands/pockets.rs`

Group:
- Private types `PocketAccount`, `Pockets`, `fn liquidity_for_type`,
  `fn aggregate_pockets`
- `pub async fn get_pockets`, `async fn pockets`
- `pub async fn create_account`, `async fn create_account_inner`
  (includes `fn liquidity_for_type` dependency — both move together)
- Tests: `liquidity_is_derived_deterministically_per_type`,
  `savings_no_longer_inflates_the_projection_seed`,
  `create_account_derives_liquidity_and_default_person`,
  `pockets_groups_and_net_worth_follow_the_contract`,
  `migration_trigger_backfills_liquidity_on_plain_inserts`

Note: `create_account_inner` calls `liquidity_for_type` — keep both in the
same file.

Create `src-tauri/src/commands/pockets.rs`. Add `pub(super) mod pockets;` in
`mod.rs`.

The tests for `savings_no_longer_inflates_the_projection_seed` call
`liquid_seed` which now lives in `forecast.rs`. Either: (a) make `liquid_seed`
`pub(super)` and import it, or (b) move that test into `forecast.rs` instead.
Option (b) is simpler — that test covers forecast seed behaviour, not pockets.

**Verify**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: all tests pass.
```
npm run rust:check
```
Expected: exit 0.

---

### Step 7: extract `commands/transactions.rs`

Group:
- `pub struct TransactionRow`, `pub struct TagOnRow`, `struct RecentRow`,
  `pub struct RecurrenceInput`
- `pub async fn get_recent_transactions`, `async fn recent_transactions`
- `pub async fn create_transaction`, `async fn create_transaction_inner`
- Tests: `recent_transactions_carry_distinct_owners`,
  `recent_transactions_carry_attached_tags`,
  `create_transaction_inserts_realized_with_tags`,
  `create_transaction_with_recurrence_builds_tagged_series`,
  `create_transaction_rejects_bad_input`

Create `src-tauri/src/commands/transactions.rs`. Add `pub(super) mod transactions;`
in `mod.rs`.

**Verify**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: all tests pass.
```
npm run rust:check
```
Expected: exit 0.

---

### Step 8: extract `commands/write_back_cmds.rs`

Name this file `write_back_cmds.rs` (not `write_back.rs`) to avoid shadowing
the existing `src-tauri/src/google_sheets/write_back.rs` module that
`commands/mod.rs` already re-imports via `use crate::google_sheets::write_back`.

Group:
- `pub struct UserSpreadsheet`
- Private helpers: `load_write_back_txns`, `build_write_back_plan`,
  `load_economia_by_month`, `build_economia_plan`, `ensure_reserve_account`,
  `store_economia_entries`
- Public commands: `preview_write_back`, `apply_write_back`,
  `write_back_enabled`, `preview_economia_write_back`,
  `apply_economia_write_back`, `get_app_setting`, `set_app_setting`,
  `backup_database`, `list_user_spreadsheets`
- Private helpers: `app_setting_get`, `app_setting_set`, `backup_db`
- Tests: `app_setting_roundtrip`, `backup_database_writes_valid_sqlite_file`,
  `economia_import_zero_removes_stale_month`,
  `economia_mixed_entries_commit_in_one_transaction`

Create `src-tauri/src/commands/write_back_cmds.rs`. Add
`pub(super) mod write_back_cmds;` in `mod.rs`.

**Verify**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: all tests pass.
```
npm run rust:check
```
Expected: exit 0.

---

### Step 9: slim down commands/mod.rs and update lib.rs if needed

After Steps 3–8, `commands/mod.rs` should contain only:
- `use` imports consumed by multiple submodules (or none — each subfile owns
  its own imports)
- `pub(crate) use` / `pub use` re-exports of all public commands, so that
  `lib.rs` still resolves `commands::get_forecast`, `commands::get_pockets`,
  etc. without changing the `tauri::generate_handler![…]` paths

If the re-export approach keeps all paths identical, `lib.rs` needs zero
changes. If it is cleaner to update `lib.rs` to use full paths like
`commands::forecast::get_forecast`, do so — either is correct as long as the
Tauri command **names** (what the frontend calls) are identical.

Verify `mod.rs` is now short (target: under 80 lines of actual code, mostly
`pub use` re-exports and `mod` declarations).

**Verify**:
```
npm run rust:check
```
Expected: exit 0.

```
wc -l src-tauri/src/commands/mod.rs
```
Expected: under 100 lines.

---

### Step 10: full gate

Run the complete quality gate and confirm nothing regressed:

```
npm run check
```

Expected: exit 0 (typecheck + lint + tests + rust:check + privacy scan all
pass). If any step fails, diagnose with the narrower command:
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml --locked`
- Frontend: `npm run typecheck && npm run lint`
- Privacy: `npm run privacy:scan`

---

### Step 11: confirm deduplication is complete

```bash
grep -rn "amount_cents: amount.abs()" src-tauri/src/commands/
```

Expected: exactly **1 match** (the single `map_cashflow_row` function). If 2
or 3 matches appear, at least one copy was not replaced — STOP and fix.

```bash
grep -rn "filter_map(|(ttype, amount" src-tauri/src/commands/
```

Expected: **0 matches** — no inline mapper closures remain.

---

## Test plan

This is a pure structural refactor: no new logic is added. The test strategy
is **all existing tests continue to pass at every step**. No new tests are
required because:

- The 45 tests in `commands.rs` cover the behaviour being moved; they travel
  with their code (Steps 3–8).
- The shared mapper (`map_cashflow_row`) is exercised by the same three async
  helpers it replaces, which are in turn covered by the forecast/dashboard
  integration tests.

The single new behavioural addition (the mapper function) is a mechanical
extraction of three identical closures — if the tests pass after Step 2, the
extraction is correct.

**Structural model**: tests in each subfile follow the pattern of the existing
`#[cfg(test)] mod tests { use super::*; … }` block in `commands.rs` — copy
that block structure into each subfile's bottom.

**Verification**:
```
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
Expected: exit 0, **45 tests pass** (same count as before — all tests moved,
none added, none deleted).

---

## Done criteria

ALL of the following must hold before marking this plan DONE:

- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0;
  **45 tests pass** (same count as pre-refactor — verify with
  `cargo test … 2>&1 | grep "test result"`)
- [ ] `npm run check` exits 0 (full gate)
- [ ] `wc -l src-tauri/src/commands/mod.rs` reports under 100 lines
- [ ] `grep -rn "amount_cents: amount.abs()" src-tauri/src/commands/` returns
  exactly **1 match**
- [ ] `grep -rn "filter_map(|(ttype, amount" src-tauri/src/commands/` returns
  **0 matches**
- [ ] `git diff d183bbf..HEAD -- src/` is empty (no frontend files changed)
- [ ] All `#[tauri::command]` function names in
  `src-tauri/src/lib.rs` lines 21–64 are identical to before (verify with
  `git diff HEAD src-tauri/src/lib.rs | grep "commands::"`)
- [ ] No files outside the in-scope list are modified (`git status` shows only
  files under `src-tauri/src/commands/` and `src-tauri/src/lib.rs`)
- [ ] `plans/README.md` status row for plan 011 updated to DONE

---

## STOP conditions

Stop immediately and report (do not improvise) if:

1. **Code at the verified line numbers does not match the excerpts** — the
   file has drifted since this plan was written. Run the drift check in the
   header and report the diff.

2. **Any public `#[tauri::command]` name or signature would have to change**
   to complete an extraction — this breaks the frontend `invoke` contract.
   The plan is written to avoid this; if it comes up, the plan has a bug.
   STOP and report.

3. **Test count drops below 45** after any step — a test was accidentally
   deleted during the move instead of being migrated with its code. Fix by
   locating the missing test in the git diff and restoring it in the correct
   subfile.

4. **`npm run rust:check` fails with a clippy or rustfmt warning that was
   not present before this plan** — the extraction introduced new code that
   violates clippy rules. Fix the warning before proceeding to the next step.

5. **A step's verification fails twice after a reasonable fix attempt** —
   report the exact error output and stop.

6. **The extraction of a group would require touching `src-tauri/src/google_sheets/*`
   or `src-tauri/src/forecast/mod.rs`** — those are out of scope. Adjust
   the grouping to keep the dependency flow correct (commands depend on
   forecast/google_sheets, never the reverse).

---

## Maintenance notes

- **After this plan lands**, future feature work adds new commands to the
  appropriate subfile, not to `commands/mod.rs`. The reviewer should verify
  the PR's new command goes into the right subfile and that `mod.rs` is not
  growing again.

- **Multi-card aggregation (future slice)**: `load_cashflow_events` currently
  uses `credit_cards[0]` for the closing/due cycle. When multi-card is added,
  the aggregation logic will live in `commands/forecast.rs` (the moved
  function). The shared `map_cashflow_row` is not affected.

- **Plan 010 characterization tests**: if plan 010 added new tests inside
  `commands.rs`, they must be migrated to the correct subfile in this plan.
  Check `git log` for any commits after `d183bbf` that added tests to
  `commands.rs` before starting Step 0.

- **`write_back_cmds.rs` naming**: the name avoids a collision with
  `google_sheets::write_back`. If the `google_sheets` module is ever promoted
  to a flat sibling (removing the directory), the naming collision risk
  disappears and `write_back_cmds.rs` could be renamed to `write_back.rs`.
  No action needed now.

- **`get_app_info` grouping**: placed in `forecast.rs` by line proximity only.
  If a future `app_info` submodule is created (e.g. for version checking or
  update logic), move it there at that time.
