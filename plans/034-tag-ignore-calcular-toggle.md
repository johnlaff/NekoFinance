# Plan 034: Tag "Ignorar nos cálculos" toggle (exclude tagged movements from aggregations)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src-tauri/migrations/ src-tauri/src/tags.rs src-tauri/src/commands/forecast_cmds.rs src/screens/TagsScreen.tsx src/lib/api.ts`
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: feature
- **Package**: C
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

The method includes a per-tag "Ignorar" toggle: movements tagged with an
"ignored" tag are excluded from Performance, Custo de vida, and Economizado% — used for
internal transfers, reimbursements, or non-spending that would distort the metrics.
Neko's tag data model (`src-tauri/src/tags.rs`) has no such field, and the
`TagsScreen` has no UI for it. Without this toggle, every tagged movement (including
reimbursements and bookkeeping entries) is folded into the method's key metrics,
skewing the owner's real cost picture. Adding the toggle restores fidelity to the
method without touching plan 023's narrower reimbursement mechanism — this is the
general, user-controlled exclusion layer.

## Current state

### Tag data model — `src-tauri/src/tags.rs`

The `Tag` struct and the `create_tag` / `list_tags` functions (lines 9–57) have no
`exclude_from_totals` field:

```rust
// tags.rs:9-16
#[derive(Debug, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
}
```

`create_tag` (line 29–47) inserts five columns — `id, name, color, emoji, is_special`
— and the `INSERT` SQL on line 37 matches:

```rust
// tags.rs:37
sqlx::query("INSERT INTO tag (id, name, color, emoji, is_special) VALUES (?1, ?2, ?3, ?4, ?5)")
```

`list_tags` (line 49–57) selects the same five columns:

```rust
// tags.rs:51-52
sqlx::query_as::<_, Tag>(
    "SELECT id, name, color, emoji, is_special FROM tag \
     ORDER BY is_special DESC, name COLLATE NOCASE",
)
```

`TagTotal` (lines 18–27) similarly lacks the field. The Tauri command wrappers live at
lines 111–143.

### DB schema — `src-tauri/migrations/20240612000003_tag.sql`

```sql
-- tag.sql:7-14
CREATE TABLE IF NOT EXISTS tag (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT 'var(--cat-jade)',
    emoji TEXT,
    is_special INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

No `exclude_from_totals` column exists. The link table is `transaction_tag`
(lines 18–22 of the same file).

### Aggregation queries — `src-tauri/src/commands/forecast_cmds.rs`

The metrics flow through the pure engine in `src-tauri/src/forecast/mod.rs`, fed by
event-loading functions in `forecast_cmds.rs`. The key entry points are:

- `load_cashflow_events` (forecast_cmds.rs line 302–328) — loads forward events via a
  JOIN on `account`; no tag awareness.
- `load_realized_month_events` (forecast_cmds.rs line 360–381) — loads past-month
  events; no tag awareness.
- `load_year_events` (forecast_cmds.rs line 676–693) — loads full-year events for the
  annual view; no tag awareness.

All three functions run the same SQL pattern (shown for `load_cashflow_events`,
lines 312–323):

```rust
let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
    "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
            COALESCE(a.liquidity,'') \
     FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date > ?1 AND t.date <= ?2",
)
```

The caller `map_cashflow_row` (commands/mod.rs lines 43–64) converts that 7-tuple into
a `CashflowEvent`. This is the single choke-point: excluding a tagged transaction here
drops it from ALL derived metrics (Performance, Custo de vida, Economizado%) while
leaving the Saldo chain intact (the Saldo is computed from the seed +
`projection_seed`, which reads the `sheet_daily_balance` table, not live events).

### Dashboard summary — `forecast_cmds.rs` lines 840–922

`dashboard_summary` derives the projected balance from `load_forecast_events` (the
same `load_cashflow_events` + daily ceiling driver), so any event exclusion applied at
the SQL level will propagate automatically.

### Savings rate and annual economia — `forecast_cmds.rs` lines 130–148

`realized_annual_economia` (lines 130–148) sums `transfer→reserve` directly from the
`transaction` table — it does NOT go through `map_cashflow_row` or the event loader.
This function feeds the savings guardrail and Economizado%. It also needs a tag-aware
variant: a reimbursement tagged "ignore" and typed as a transfer should be dropped.

```rust
// forecast_cmds.rs:136-147
let row: (i64,) = sqlx::query_as(
    "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
     LEFT JOIN account a ON a.id = t.to_account_id \
     WHERE t.date >= ?1 AND t.date < ?2 \
       AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid')",
)
```

### Frontend API binding — `src/lib/api.ts` lines 573–632

```typescript
// api.ts:573-584
export interface Tag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
}

export interface TagTotal extends Tag {
  /** Soma (centavos, valor absoluto) dos lançamentos do mês com esta tag. */
  total_cents: number;
}
```

No `exclude_from_totals` field. The existing Tauri invoke calls:

```typescript
// api.ts:615-621
export function createTag(
  name: string,
  color: string,
  emoji: string | null,
  isSpecial: boolean,
): Promise<string> {
  return invoke("create_tag_cmd", { name, color, emoji, isSpecial });
}
```

### TagsScreen — `src/screens/TagsScreen.tsx`

The screen (lines 88–329) renders a tag list (lines 278–313) and a creation form
(lines 187–258). There is no toggle per tag row, no `update_tag` call, and no UI to
flip `exclude_from_totals`.

### Repo conventions applicable here

- Functional-core / imperative-shell: pure engine (`forecast/mod.rs`) stays unmodified;
  SQL changes go in the shell (`forecast_cmds.rs`, `tags.rs`).
- Money is always a positive-magnitude integer in cents; sign comes from `EventKind`.
- Migrations are forward-only, numbered with a timestamp prefix. The latest migration
  is `20240620000001_drop_daily_checkin.sql`.
- Existing test pattern: see the `#[tokio::test]` blocks in `commands/mod.rs` (lines
  147–1188). Use `fixture_pool()` (line 280–289) and the `insert_*` helpers already
  defined there. Tag tests in `tags.rs` (lines 172–218) show the pattern for
  pool-based tests without Tauri State.
- React Compiler ON: no manual `useMemo`/`useCallback`; hoist static style objects
  to module-level constants (see `FORM_PANEL_STYLE` at `TagsScreen.tsx:77–86`).
- Reducer pattern for form state is already used in `TagsScreen` (lines 29–75); extend
  it rather than adding new `useState` calls.

## Commands you will need

| Purpose        | Command              | Expected on success |
| -------------- | -------------------- | ------------------- |
| Rust typecheck | `npm run rust:check` | exit 0, no errors   |
| TS typecheck   | `npm run typecheck`  | exit 0, no errors   |
| Unit tests     | `npm run test:run`   | all pass            |
| Full gate      | `npm run check`      | exit 0              |
| React Doctor   | `npm run doctor`     | 0 issues            |
| Lint           | `npm run lint`       | exit 0              |

## Suggested executor toolkit

- If available, use the `shadcn` skill for any toggle/switch component question.
- Use the `neko-finance-design` skill if uncertain about token names (`--text-muted`,
  `--surface-2`, etc.) or spacing.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/migrations/<new-timestamp>_tag_exclude_from_totals.sql` (create)
- `src-tauri/src/tags.rs`
- `src-tauri/src/commands/forecast_cmds.rs`
- `src/screens/TagsScreen.tsx`
- `src/lib/api.ts`

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/forecast/mod.rs` — pure engine; no SQL, no DB, must stay IO-free.
- `src-tauri/src/commands/mod.rs` — `map_cashflow_row` stays unchanged; the filter
  is applied at the SQL level before the row reaches the mapper.
- Plan 023 (`#reembolso` tag mechanism) — a separate, narrower feature; do not merge
  them.
- The `sheet_daily_balance` table / `projection_seed` — the Saldo chain must stay
  intact; do not touch balance computations.
- Any other screen, migration, or Rust file not listed above.

## Git workflow

- Branch: `feat/034-tag-ignore-calcular`
- Commit per step; follow the repo's conventional-commit style, e.g.:
  `feat: add exclude_from_totals to tag schema and aggregation queries (plan 034)`
  `feat: tag ignore toggle UI in TagsScreen (plan 034)`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Forward migration — add `exclude_from_totals` column to `tag`

Create file `src-tauri/migrations/20240621000001_tag_exclude_from_totals.sql`:

```sql
-- Adds an opt-in exclusion flag to tags (plan 034).
-- When exclude_from_totals = 1, movements carrying this tag are omitted from
-- Performance, Custo de vida, and Economizado% (but not from the Saldo chain).
ALTER TABLE tag ADD COLUMN exclude_from_totals INTEGER NOT NULL DEFAULT 0;
```

Use `ALTER TABLE … ADD COLUMN` with `DEFAULT 0` so existing tags get the safe default
(included) with no data migration needed.

**Verify**: `npm run rust:check` → exit 0 (sqlx macro detects migration at compile time
if the offline query data is stale; run `cargo sqlx prepare` under
`src-tauri/` if needed — see note below).

> Note on sqlx offline mode: this repo uses `cargo sqlx prepare` to bake
> query metadata into `.sqlx/`. After adding the migration you may need to
> regenerate it: `cd src-tauri && cargo sqlx prepare --workspace` (requires a
> live SQLite file or the `DATABASE_URL` env var). If the CI environment
> does not support this, commit the updated `.sqlx/` directory. Check whether
> `.sqlx/` is tracked in git with `git ls-files src-tauri/.sqlx | head` before
> deciding.

### Step 2: Update `Tag`, `TagTotal`, and `create_tag` in `tags.rs`

**2a. Extend the structs** — add `pub exclude_from_totals: bool` to both `Tag`
(line 9) and `TagTotal` (line 18). The `sqlx::FromRow` derive will handle the mapping
automatically once the column exists.

**2b. Update `list_tags` SELECT** (line 51) — add `exclude_from_totals` to the column
list:

```rust
"SELECT id, name, color, emoji, is_special, exclude_from_totals FROM tag \
 ORDER BY is_special DESC, name COLLATE NOCASE"
```

**2c. Update `tag_totals_for_month` SELECT** (line 93) — add `t.exclude_from_totals`
to the column list:

```rust
"SELECT t.id, t.name, t.color, t.emoji, t.is_special, t.exclude_from_totals, \
        COALESCE(SUM(ABS(tr.amount)), 0) AS total_cents \
 FROM tag t \
 LEFT JOIN transaction_tag tt ON tt.tag_id = t.id \
 LEFT JOIN \"transaction\" tr ON tr.id = tt.transaction_id \
        AND substr(tr.date, 1, 7) = ?1 \
        AND tr.type IN ('expense', 'transfer') \
 GROUP BY t.id \
 ORDER BY t.is_special DESC, total_cents DESC, t.name COLLATE NOCASE"
```

**2d. Add `update_tag_exclude` function** — a new pure IO function that updates the
`exclude_from_totals` column for one tag:

```rust
pub async fn update_tag_exclude(
    pool: &SqlitePool,
    tag_id: &str,
    exclude: bool,
) -> Result<(), String> {
    let rows = sqlx::query(
        "UPDATE tag SET exclude_from_totals = ?1 WHERE id = ?2",
    )
    .bind(exclude as i64)
    .bind(tag_id)
    .execute(pool)
    .await
    .map_err(|e| format!("update_tag_exclude: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!("tag not found: {tag_id}"));
    }
    Ok(())
}
```

**2e. Add Tauri command wrapper** after the existing wrappers (around line 136):

```rust
#[tauri::command]
pub async fn update_tag_exclude_cmd(
    pool: State<'_, SqlitePool>,
    tag_id: String,
    exclude: bool,
) -> Result<(), String> {
    update_tag_exclude(pool.inner(), &tag_id, exclude).await
}
```

**2f. Register the new command in `src-tauri/src/lib.rs`** — find the
`tauri::generate_handler![…]` macro call and add `update_tag_exclude_cmd` to it.

**Verify**: `npm run rust:check` → exit 0

### Step 3: Exclude tagged transactions from the event-loading SQL queries

Each of the three event-loading functions in `forecast_cmds.rs` needs a subquery guard
that drops any transaction which carries at least one `exclude_from_totals = 1` tag.

The pattern to use (subquery avoids a new JOIN that would duplicate rows when multiple
tags are attached):

```sql
AND NOT EXISTS (
    SELECT 1 FROM transaction_tag tt2
    JOIN tag tg ON tg.id = tt2.tag_id
    WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1
)
```

**3a. `load_cashflow_events`** (line 312–322) — insert the guard after `WHERE t.date >
?1 AND t.date <= ?2`:

```rust
"SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
        COALESCE(a.liquidity,'') \
 FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
 WHERE t.date > ?1 AND t.date <= ?2 \
   AND NOT EXISTS ( \
       SELECT 1 FROM transaction_tag tt2 \
       JOIN tag tg ON tg.id = tt2.tag_id \
       WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
   )"
```

**3b. `load_realized_month_events`** (line 368–378) — same guard after `WHERE t.date

> = ?1 AND t.date <= ?2`:

```rust
"SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
        COALESCE(a.liquidity,'') \
 FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
 WHERE t.date >= ?1 AND t.date <= ?2 \
   AND NOT EXISTS ( \
       SELECT 1 FROM transaction_tag tt2 \
       JOIN tag tg ON tg.id = tt2.tag_id \
       WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
   )"
```

**3c. `load_year_events`** (line 680–691) — same guard after `WHERE t.date >= ?1 AND
t.date < ?2`:

```rust
"SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
        COALESCE(a.liquidity,'') \
 FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
 WHERE t.date >= ?1 AND t.date < ?2 \
   AND NOT EXISTS ( \
       SELECT 1 FROM transaction_tag tt2 \
       JOIN tag tg ON tg.id = tt2.tag_id \
       WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
   )"
```

**3d. `realized_annual_economia`** (line 136–147) — the savings guardrail's direct SQL
also needs the guard, after `AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid')`:

```rust
"SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
 LEFT JOIN account a ON a.id = t.to_account_id \
 WHERE t.date >= ?1 AND t.date < ?2 \
   AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid') \
   AND NOT EXISTS ( \
       SELECT 1 FROM transaction_tag tt2 \
       JOIN tag tg ON tg.id = tt2.tag_id \
       WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
   )"
```

> Design decision: the Saldo chain (`projection_seed`, `sheet_daily_balance`,
> `dashboard_summary`'s `daily_spend_today`) is intentionally left untouched.
> The exclusion affects only DERIVED METRICS (Performance, Custo de vida,
> Economizado%) — the running balance reflects real cash movement.
> This matches the method: ignored tags still move money; they just don't count as
> "spending" or "saving" in the periodic metrics.

**Verify**: `npm run rust:check` → exit 0

### Step 4: Add tests in `src-tauri/src/commands/mod.rs`

Add two `#[tokio::test]` async tests after the last existing test in the `mod tests`
block (around line 1188). Model them after `annual_registered_economia_counts_only_reserve_transfers`
(lines 554–588) and `forecast_dto_chains_daily_flows_and_safe_to_spend` (lines 447–490).

**Test 1 — excluded tag drops from Performance and Custo de vida**:

```
setup: fixture_pool + income 500_000 in a realized month + expense 100_000 with tag "Reembolso" (exclude=true)
assert: the month's cost_of_living_cents == 0 and income_cents == 500_000
         (the expense is excluded from the metric)
assert: the month's performance_cents == 500_000 (full income, no cost)
```

Use `crate::tags::create_tag` to make the tag, then `crate::tags::update_tag_exclude`
to flip `exclude=true`, then `crate::tags::set_transaction_tags` to attach it.
Call `forecast_dto` (or `annual_metrics`) and assert the metric.

**Test 2 — excluded tag drops from Economizado% (realized_annual_economia)**:

```
setup: fixture_pool + reserve account + transfer 50_000 tagged "Ignorar" (exclude=true)
       + transfer 30_000 untagged (should still count)
assert: realized_annual_economia returns 30_000 (not 80_000)
```

Call `realized_annual_economia` directly with the appropriate `today_naive`.

**Test 3 — regression: Saldo is unaffected**:

```
setup: fixture_pool + sheet_daily_balance seed + expense 20_000 tagged "Ignorar" (exclude=true)
assert: projection_seed returns the sheet seed value unchanged
assert: forecast_dto daily[0].balance_cents == seed value (excluded transaction
        not in future events, so balance chain unchanged)
```

This confirms the design decision that the Saldo chain stays intact.

**Verify**: `npm run test:run` → all pass, including 3 new tests

### Step 5: Add `updateTagExclude` binding in `src/lib/api.ts`

**5a. Extend the `Tag` interface** (line 573):

```typescript
export interface Tag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
  exclude_from_totals: boolean; // ← add
}
```

**5b. Add the invoke function** after `setTransactionTags` (around line 630):

```typescript
export function updateTagExclude(tagId: string, exclude: boolean): Promise<void> {
  return invoke("update_tag_exclude_cmd", { tagId, exclude });
}
```

**Verify**: `npm run typecheck` → exit 0

### Step 6: Add the toggle to `TagsScreen.tsx`

**6a. Import `updateTagExclude`** at the top (add alongside the existing named imports
from `../lib/api`).

**6b. Add a `toggleExclude` handler** inside `TagsScreen` (below the existing `submit`
function, around line 136):

```typescript
async function toggleExclude(tagId: string, currentValue: boolean) {
  try {
    await updateTagExclude(tagId, !currentValue);
    invalidateCommands();
    setReload((r) => r + 1);
  } catch {
    // silent — the toggle is best-effort; a future revision can add inline error state
  }
}
```

**6c. Update the tag list row** (currently lines 278–313) to add the toggle control
after the `<Money>` element.

The toggle must be a native `<button type="button">` (not a checkbox — WAI-ARIA for a
two-state switch is `role="switch" aria-checked={t.exclude_from_totals}`).

Design guidance: use a muted label such as "Ignorar nos cálculos" (aria-label only,
not visible text unless space allows). When `exclude_from_totals` is true, show a
de-emphasised indicator so the row is visually distinct from included tags.

Example structure (adapt token names as needed):

```tsx
<button
  type="button"
  role="switch"
  aria-checked={t.exclude_from_totals}
  aria-label={t.exclude_from_totals ? "Incluir nos cálculos" : "Ignorar nos cálculos"}
  onClick={() => void toggleExclude(t.id, t.exclude_from_totals)}
  style={{
    // Hoist this object to a module-level constant to satisfy React Compiler.
    // See FORM_PANEL_STYLE in this file as the pattern.
    padding: "var(--space-1) var(--space-2)",
    borderRadius: "var(--radius-sm)",
    border: "var(--bw-hair) solid var(--border)",
    background: t.exclude_from_totals ? "var(--surface-2)" : "transparent",
    color: t.exclude_from_totals ? "var(--text-muted)" : "var(--text)",
    fontSize: "var(--fs-xs)",
    cursor: "pointer",
  }}
>
  {t.exclude_from_totals ? "ignorado" : "incluído"}
</button>
```

**Important**: the inline `style` object above is dynamic (depends on `t.exclude_from_totals`);
it cannot be hoisted to a module-level constant because it varies per row. This is
correct and expected by the React Compiler — per-row dynamic styles are fine; only
static objects that never change should be hoisted. Do NOT wrap the button in
`useMemo`; the Compiler handles it.

**6d. Optional visual indicator on the tag name** — when `exclude_from_totals` is true,
apply `color: "var(--text-muted)"` to the name `<span>` so the owner can see at a
glance which tags are excluded.

**Verify**: `npm run typecheck` → exit 0, `npm run lint` → exit 0, `npm run doctor` →
0 issues

### Step 7: Full gate

**Verify**: `npm run check` → exit 0

## Test plan

New tests to write (Step 4), all in `src-tauri/src/commands/mod.rs` `mod tests` block:

1. **`excluded_tag_drops_expense_from_performance`** — happy path: expense with an
   excluded tag is invisible to Performance/Custo de vida. Exercises the
   `load_realized_month_events` + `load_cashflow_events` filter.
2. **`excluded_tag_drops_transfer_from_economizado`** — excluded transfer to reserve
   does not inflate Economizado%. Exercises `realized_annual_economia` filter.
3. **`excluded_tag_does_not_affect_saldo_chain`** — regression guard: balance is
   unaffected by tag exclusion (sheet seed unchanged, `projection_seed` returns same
   value).

Use `crate::tags::{create_tag, update_tag_exclude, set_transaction_tags}` and the
existing `fixture_pool()` / `insert_realized()` / `insert_sheet_balance()` helpers
(already in the test module). Model after `annual_registered_economia_counts_only_reserve_transfers`
(lines 554–588 in `commands/mod.rs`).

**Verification**: `npm run test:run` → all pass; `cargo test -p neko-finance-lib 2>/dev/null || cargo test --manifest-path src-tauri/Cargo.toml` → 3 new tests visible in output.

## Done criteria

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run doctor` exits 0 issues
- [ ] `npm run test:run` exits 0; the 3 new Rust tests exist and pass
- [ ] `npm run check` exits 0
- [ ] `grep -n "exclude_from_totals" src-tauri/src/tags.rs` matches at least 5 lines
      (struct field × 2, SELECT × 2, UPDATE × 1)
- [ ] `grep -n "exclude_from_totals" src/lib/api.ts` returns 1 match (interface field)
- [ ] `grep -rn "update_tag_exclude_cmd" src-tauri/src/` returns matches in `tags.rs`
      AND `lib.rs`
- [ ] No files outside the in-scope list are modified (`git status` shows only migration,
      `tags.rs`, `forecast_cmds.rs`, `TagsScreen.tsx`, `api.ts`, plus any `.sqlx/` refresh)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at any location in "Current state" does not match the excerpts (codebase has
  drifted since this plan was written — the drift check at the top should catch this).
- `npm run rust:check` or `npm run test:run` fails after a step and does not pass after
  one reasonable fix attempt.
- The `realized_annual_savings` function (lines 105–124 in `forecast_cmds.rs`) also
  needs the exclusion filter — this plan deliberately excludes it because that function
  computes the NET superávit (renda − saída), a display-only "colchão" figure that is
  NOT used in the savings guardrail; if you find a product reason to exclude from it,
  stop and report rather than expanding scope.
- Adding the subquery to any of the three event loaders breaks an existing test (it may
  indicate a subtle query interaction — do not work around it by deleting the test).
- The `sqlx` macro complains about offline query data: run `cd src-tauri && cargo sqlx prepare --workspace`
  and commit the updated `.sqlx/` if needed; if the environment has no DATABASE_URL
  and no way to regenerate, stop and report.
- The tag toggle requires touching `forecast/mod.rs` (the pure engine) — it must not;
  if you find yourself needing to, stop.

## Maintenance notes

- **Future: multi-tag edge case** — the `NOT EXISTS` guard is correct for "exclude if
  ANY tag is excluded." If a future revision requires "exclude only if ALL tags are
  excluded," the subquery must be inverted to `NOT EXISTS … AND tg.exclude_from_totals = 0`
  (i.e., "no included tag exists"). Keep the semantics documented.
- **Future: plan 023 (#reembolso)** — if plan 023 lands, its net-zero mechanism
  (pairing reimbursements to cancel each other) may overlap with this toggle. The two
  mechanisms target different use-cases (this plan: owner-decided, per-tag exclusion;
  plan 023: automatic net-zero pairing). Avoid merging them silently.
- **Reviewer should check**: that the `realized_annual_economia` SQL (Step 3d) is
  regenerated in `.sqlx/` if sqlx offline mode is in use; and that the toggle's
  aria-label text follows the method-neutral vocabulary (no reference to the official
  app or course).
- **Index consideration**: the `NOT EXISTS` subquery hits `transaction_tag(transaction_id)`
  which already has an index (`idx_transaction_tag_tag` on `tag_id`). If query plans
  show a full scan on high-volume data, a composite index on
  `(transaction_id, tag_id)` is the primary key of `transaction_tag` — the subquery
  should be covered. No new index needed now.
