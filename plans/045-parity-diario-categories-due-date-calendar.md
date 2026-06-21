# Plan 045: Parity: Diário budget categories + due-date/fatura calendar (+ installment tracking)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
>
> ```
> git diff --stat d3922d2..HEAD -- \
>   src-tauri/migrations/ \
>   src-tauri/src/commands/forecast_cmds.rs \
>   src-tauri/src/commands/transactions.rs \
>   src-tauri/src/recurrence.rs \
>   src/lib/api.ts \
>   src/screens/NewTransactionForm.tsx \
>   src/screens/SettingsScreen.tsx \
>   src/screens/TransactionsScreen.tsx
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: none
- **Category**: feature
- **Package**: E
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

The spreadsheet's variable-spending cell (Diário) can carry a structured
monthly budget note that breaks the Diário total into named category amounts
and derives the per-day rate (e.g. "Category A R$300 / Category B R$200 / ...
Total R$1250 ÷ 31 = R$X/day"). Neko imports this note as a description or
line-items but has no typed model: the category budget view is completely
absent. Without it the user cannot audit whether they are on-track per
category during the month, and the per-day-rate ("teto diário") derived in
`effective_daily_ceiling` has no category breakdown to inform it.

Several spreadsheet Saída notes also act as payment-due reminders (e.g.
"Bill X due 10/07, Utility Y due 11/07"). Neko has no due-date field, so
these reminders are buried in description text and there is no calendar view
of upcoming payments — the user must manually remember them.

Finally, the spreadsheet annotates installment series (e.g. "4/36") in the
note. Neko tracks recurrence count (`repetitions`) but exposes no
"installment N of M" surface in the UI, so the user cannot see how many
payments remain at a glance.

This plan restores all three capabilities so the user loses no function
versus the sheet.

## Current state

### Schema

The `daily_budget` table (migration `20240608000009_daily_budget.sql`,
lines 1–10) stores only a single monthly amount per person — no category
breakdown:

```sql
-- src-tauri/migrations/20240608000009_daily_budget.sql:1-10
CREATE TABLE IF NOT EXISTS daily_budget (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    status TEXT NOT NULL CHECK(status IN ('active','under_review','deprecated')),
    free_income INTEGER,
    calculated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

The `transaction` table (migration `20240608000006_transaction.sql`,
lines 1–14) has no `due_date` or installment fields:

```sql
-- src-tauri/migrations/20240608000006_transaction.sql:1-14
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,
    description TEXT,
    date TEXT NOT NULL,
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

The `recurrence` table (migration `20240612000005_recurrence.sql`,
lines 5–12) stores `repetitions` (total occurrences) but not a label for
how to display "N of M":

```sql
-- src-tauri/migrations/20240612000005_recurrence.sql:5-12
CREATE TABLE IF NOT EXISTS recurrence (
    id TEXT PRIMARY KEY NOT NULL,
    frequency TEXT NOT NULL CHECK(frequency IN ('diaria', 'semanal', 'mensal')),
    infinite INTEGER NOT NULL DEFAULT 0,
    repetitions INTEGER,
    start_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

The `recurrence_id` is embedded in each transaction id as `{rec_id}:{i}`
(0-based index). The current-index and repetitions are already available
in the database; only a Rust/TS query and a UI surface are missing.

### Rust — existing daily-budget write path

`upsert_daily_budget_inner` in `src-tauri/src/commands/forecast_cmds.rs`
(lines 300–340) writes a single `amount` row. There is no equivalent
for category rows, no read command that returns category totals, and no
`upsert_daily_budget` Tauri command that accepts a category list.

### Rust — `TransactionRow` struct

`src-tauri/src/commands/transactions.rs` (lines 47–65) — `TransactionRow`
has no `due_date`, `installment_index`, or `installment_total` fields:

```rust
// src-tauri/src/commands/transactions.rs:47-65
pub struct TransactionRow {
    pub id: String,
    pub r#type: String,
    pub amount: i64,
    pub description: String,
    pub date: String,
    pub payment_method: String,
    pub is_projection: bool,
    pub is_fixed: bool,
    pub owners: Vec<String>,
    pub tags: Vec<TagOnRow>,
    pub provenance: String,
    pub line_items: Vec<LineItemOnRow>,
}
```

### Rust — recurrence series

`src-tauri/src/recurrence.rs` (lines 56–63) — `RecurringTemplate` has no
installment-label or due-date field. The occurrence index is derivable from
the id pattern `{rec_id}:{i}` (see `occurrence_index` at lines 123–127):

```rust
// src-tauri/src/recurrence.rs:123-127
fn occurrence_index(transaction_id: &str) -> Option<i64> {
    transaction_id
        .rsplit_once(':')
        .and_then(|(_, i)| i.parse().ok())
}
```

### TypeScript — `TransactionRow`

`src/lib/api.ts` (lines 57–75) — the frontend `TransactionRow` mirrors the
Rust struct and also lacks `due_date`, `installment_index`, and
`installment_total`.

### TypeScript — `upsertDailyBudget`

`src/lib/api.ts` (lines 561–563) — current API sends only `amountCents`:

```ts
// src/lib/api.ts:561-563
export function upsertDailyBudget(amountCents: number): Promise<void> {
  return invoke("upsert_daily_budget", { amountCents });
}
```

### Seed categories

`src-tauri/migrations/20240608000005_seed_categories.sql` (lines 1–12) has
a generic `category` tree already. This plan introduces `daily_budget_category`
as a dedicated sibling table tied to the budget, not a generic category FK, so
that budget categories can be freely renamed by the user without touching the
generic category tree. Generic example names in code and tests: "Groceries",
"Transport", "Pharmacy", "Leisure", "Shopping".

### Pattern to follow

- Tests: model after `src-tauri/src/commands/transactions.rs` (in-file
  `#[cfg(test)] mod tests` with `sqlx::migrate!("./migrations")` in-memory pool).
- Functional-core / imperative-shell: pure helpers first, then `#[tauri::command]`
  thin wrappers.
- React Compiler conventions: static style objects hoisted as `const` outside the
  component (no inline object literals ≥ 8 props); no manual `memo`/`useMemo`;
  `useReducer` for multi-field forms. See `src/screens/NewTransactionForm.tsx`
  (lines 121–138 for reducer pattern) and `src/screens/SettingsScreen.tsx`
  (lines 44–136 for a settings section with read/write round-trip).
- Method-neutral language: never name the official method app, course, or author
  in plan file, code, or tests. Generic category and bill names only.

## Commands you will need

| Purpose          | Command                                 | Expected on success               |
| ---------------- | --------------------------------------- | --------------------------------- |
| Typecheck        | `npm run typecheck`                     | exit 0, no errors                 |
| Lint             | `npm run lint`                          | exit 0                            |
| Unit tests (all) | `npm run test:run`                      | all pass                          |
| Rust check       | `npm run rust:check`                    | exit 0                            |
| Full gate        | `npm run check`                         | exit 0                            |
| E2E smoke        | `npm run e2e`                           | all pass; inspect screenshots     |
| React Doctor     | `npm run doctor`                        | 0 issues (advisory; gate = check) |
| Rust test filter | `cargo test -p neko-finance-lib <name>` | from `src-tauri/` directory       |

## Suggested executor toolkit

- Use the `neko-finance-design` skill for any new UI component tokens/color choices.
- Use the `tdd` skill for the Diário budget category model — it has deterministic
  pure functions (sum, derive daily rate) that should be test-driven.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/migrations/` — new migration files (add tables/columns; never edit existing ones)
- `src-tauri/src/commands/forecast_cmds.rs` — extend `upsert_daily_budget`; add read command
- `src-tauri/src/commands/transactions.rs` — add `due_date`, `installment_index`,
  `installment_total` to `TransactionRow` and `create_transaction`; expose in query + listing
- `src-tauri/src/recurrence.rs` — (read-only change) derive installment index/total in the
  existing query helpers; no schema change to recurrence itself
- `src-tauri/src/lib.rs` — register any new Tauri commands
- `src/lib/api.ts` — extend `TransactionRow`, `upsertDailyBudget`, new API functions
- `src/screens/NewTransactionForm.tsx` — add optional `due_date` field on Saída/Cartão kinds;
  add installment display in edit mode
- `src/screens/SettingsScreen.tsx` — add Diário budget-category editor section
- `src/screens/TransactionsScreen.tsx` — show due_date chip; show "N/M parcelas" badge
- `src/screens/HorizonteScreen.tsx` — add calendar/list of upcoming bills (due-date view)
- New test files (mirroring existing pattern — see Test plan section)
- `plans/README.md` — update this plan's status row when done

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/migrations/20240608000009_daily_budget.sql` — never edit existing migrations;
  add a new migration file instead.
- `src-tauri/migrations/20240608000006_transaction.sql` — same rule.
- `src-tauri/src/forecast/mod.rs` — the forecast engine's projection of the daily teto
  already reads `daily_budget WHERE status='active' AND amount > 0` via
  `effective_daily_ceiling`; the total-amount row is still written, so the engine continues
  to work unchanged. Category-level drill-down is a UI-only concern.
- `src-tauri/src/google_sheets/import.rs` — no importer changes in this plan; future
  auto-detection of structured notes is a separate follow-up.
- Any write-back path (`write_back_cmds.rs`, `write_back.rs`) — due-dates and budget
  categories are app-managed metadata, not written back to the sheet in this plan.
- `src/screens/DashboardScreen.tsx` — the hero forecast tile is unchanged; the per-category
  breakdown lives in `SettingsScreen` and the upcoming-bills view lives in `HorizonteScreen`.
- Any public API response-shape changes that break existing Tauri command contracts
  (add optional nullable fields, do not remove or rename existing ones).

## Git workflow

- Branch: `advisor/045-parity-diario-categories-due-date-calendar`
- Commit per logical step (match repo style — conventional commits, e.g.
  `feat: add daily_budget_category table + upsert command`).
- Do NOT push or open a PR unless instructed.

## Steps

This plan has three independent features. Implement in order (1 → 2 → 3) to keep
the codebase always-green between steps. Each step ends with a verification command.

---

### Step 1: Diário budget-category model + UI

**Scope**: migration, Rust model + commands, TS bindings, SettingsScreen section.

#### 1a. Migration: `daily_budget_category` table

Create `src-tauri/migrations/20260621000001_daily_budget_category.sql`:

```sql
-- Plan 045: per-category monthly budget breakdown for the variable-spending slot.
-- Each row is one named category with a monthly target amount (positive integer cents).
-- The total of all active rows SHOULD equal the active daily_budget.amount (enforced
-- in the application layer, not the DB — the sum is derived at read time).
-- FK to daily_budget (not person) so that when a budget is deprecated the categories
-- travel with it for historical reference.
CREATE TABLE IF NOT EXISTS daily_budget_category (
    id           TEXT    PRIMARY KEY NOT NULL,
    budget_id    TEXT    NOT NULL REFERENCES daily_budget(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,   -- e.g. "Groceries", "Transport"
    amount_cents INTEGER NOT NULL,   -- monthly target, positive integer
    position     INTEGER NOT NULL DEFAULT 0  -- display order, 0-based
);

CREATE INDEX IF NOT EXISTS idx_daily_budget_category_budget_id
    ON daily_budget_category (budget_id);
```

**Verify**: `npm run rust:check` → exit 0.

#### 1b. Rust: read + write commands

In `src-tauri/src/commands/forecast_cmds.rs`, add:

1. A `DailyBudgetCategoryRow` struct (serde::Serialize, sqlx::FromRow):
   - fields: `id: String`, `name: String`, `amount_cents: i64`, `position: i64`

2. A pure `upsert_daily_budget_with_categories_inner` function:
   - Signature: `async fn upsert_daily_budget_with_categories_inner(pool: &SqlitePool, amount_cents: i64, categories: &[CategoryInput]) -> Result<(), String>`
   - Where `CategoryInput` is `pub struct CategoryInput { pub name: String, pub amount_cents: i64, pub position: i64 }` (serde::Deserialize).
   - Logic:
     1. Call the existing `upsert_daily_budget_inner(pool, amount_cents).await?` to write/deprecate the budget amount row (reuse the existing path).
     2. If `amount_cents > 0` and `categories` is non-empty: fetch the newly inserted budget id (`SELECT id FROM daily_budget WHERE status='active' ORDER BY start_date DESC LIMIT 1`), then DELETE all existing `daily_budget_category` rows for that budget_id and INSERT new rows from `categories`. Run both DELETEs+INSERTs inside a single SQLite transaction.
     3. If `categories` is empty, do nothing for the category table (the total-only budget remains valid).
   - Validate: each `category.amount_cents > 0`; return Err if not.

3. A `get_daily_budget_categories` pure function:
   - Signature: `async fn get_daily_budget_categories_inner(pool: &SqlitePool) -> Result<Vec<DailyBudgetCategoryRow>, String>`
   - Query: `SELECT dbc.id, dbc.name, dbc.amount_cents, dbc.position FROM daily_budget_category dbc JOIN daily_budget db ON db.id = dbc.budget_id WHERE db.status='active' ORDER BY dbc.position`.

4. Two `#[tauri::command]` wrappers:
   - `upsert_daily_budget_with_categories_cmd` (replaces the plain `upsert_daily_budget` in the new UI path — keep the old command for backwards compat).
   - `get_daily_budget_categories_cmd` → `get_daily_budget_categories_inner`.

5. Register both in `src-tauri/src/lib.rs` inside `tauri::generate_handler![…]`.

The daily-rate derivation (total ÷ days in month) is a pure function:

```rust
/// Pure: monthly amount → daily rate for the given month.
pub fn monthly_to_daily_rate(amount_cents: i64, days_in_month: u32) -> i64 {
    if days_in_month == 0 { return 0; }
    amount_cents / days_in_month as i64
}
```

Place it as a `pub(crate)` function in `forecast_cmds.rs` and unit-test it in the
`#[cfg(test)]` block.

**Verify**: `npm run rust:check` → exit 0.

#### 1c. TypeScript bindings

In `src/lib/api.ts`, add:

```ts
export interface DailyBudgetCategory {
  id: string;
  name: string;
  amount_cents: number;
  position: number;
}

export interface DailyBudgetCategoryInput {
  name: string;
  amount_cents: number; // positive cents
  position: number;
}

/** Reads the categories for the currently active daily budget. Empty array = no breakdown set. */
export function getDailyBudgetCategories(): Promise<DailyBudgetCategory[]> {
  return invoke("get_daily_budget_categories_cmd");
}

/**
 * Writes the total Diário teto + an optional per-category breakdown.
 * `categories` may be empty (retains total-only budget, clears any prior breakdown).
 * `amountCents = 0` deactivates the explicit teto (engine falls back to prior-month average).
 */
export function upsertDailyBudgetWithCategories(
  amountCents: number,
  categories: DailyBudgetCategoryInput[],
): Promise<void> {
  return invoke("upsert_daily_budget_with_categories_cmd", { amountCents, categories });
}
```

**Verify**: `npm run typecheck` → exit 0.

#### 1d. SettingsScreen: Diário category editor

In `src/screens/SettingsScreen.tsx`, add a new section `DiarioCategorySection`
(below the existing `DailyReminderSection`, above any remaining sections).

The section pattern to follow is `DailyReminderSection` (lines 44–136): a
`useEffect` loads current data on mount; a local `useReducer` or `useState`
holds draft state; an explicit Save button persists.

The editor should:

1. Show the current active-budget total (read from the existing `getDashboardSummary`
   result already available in the parent, or fetch `getAppSetting` pattern from
   `DailyReminderSection`). If no explicit budget is set, show a placeholder.
2. List editable rows: name (text input) + monthly amount (BRL decimal input),
   using the same `field`/`label` style constants already defined at the top of
   `SettingsScreen.tsx`.
3. "Add category" button appends a blank row.
4. "Remove" button (×) removes a row.
5. Show a derived summary: "Total R$ X,XX — R$ Y,YY/day (N days in current month)".
   The per-day rate is `total ÷ days_in_month` computed in TypeScript (no extra
   round-trip needed).
6. A Save button calls `upsertDailyBudgetWithCategories(totalCents, categories)`.
7. If the sum of categories ≠ the stated total, show a soft warning (not blocking
   — the user can choose to set one without the other).

Use generic category placeholder names in UI copy: "Alimentação", "Transporte",
"Farmácia", "Lazer", "Outros" (these are generic lifestyle categories, not
proprietary method terms).

Static style constants must be hoisted outside the component. No inline object
literals with ≥ 8 props. No manual `memo`.

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0;
`npm run doctor` → 0 issues.

---

### Step 2: Due-date field + upcoming-bills calendar

**Scope**: migration, Rust schema extension, TS extension, `NewTransactionForm`,
`TransactionsScreen`, `HorizonteScreen` calendar/list.

#### 2a. Migration: `due_date` column on `transaction`

Create `src-tauri/migrations/20260621000002_transaction_due_date.sql`:

```sql
-- Plan 045: optional payment due date for fixed recurring bills.
-- NULL = no explicit due date set (default for all existing rows).
-- Format: ISO date text "YYYY-MM-DD", same as the `date` column.
-- The `date` column remains the cash-flow date (when money leaves);
-- `due_date` is the billing/due date shown in calendar reminders.
ALTER TABLE "transaction" ADD COLUMN due_date TEXT;
```

**Verify**: `npm run rust:check` → exit 0.

#### 2b. Rust: expose `due_date` in `TransactionRow`

In `src-tauri/src/commands/transactions.rs`:

1. Add `pub due_date: Option<String>` to `TransactionRow` (after `line_items`).
2. Add `due_date: Option<String>` to `RecentRow` (sqlx::FromRow).
3. Extend the `recent_transactions` SELECT to include `t.due_date`.
4. Map `r.due_date` in the `TransactionRow::from(r)` block.
5. Add `pub due_date: Option<String>` to `create_transaction_inner` parameters
   and the `INSERT INTO "transaction"` query. Wire it through the Tauri command
   `create_transaction`.
6. Add a new Tauri command `get_upcoming_bills_cmd`:

```rust
/// An upcoming bill: a transaction with a due_date in [today, horizon].
#[derive(serde::Serialize)]
pub struct UpcomingBill {
    pub id: String,
    pub description: String,
    pub amount: i64,
    pub due_date: String,
    pub is_projection: bool,
}

/// Returns bills with a due_date in the next `days` calendar days (inclusive of today).
/// Ordered by due_date ASC. Limit 100 to avoid unbounded result sets.
pub async fn get_upcoming_bills_cmd(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> Result<Vec<UpcomingBill>, String> { ... }
```

Implementation: `WHERE due_date >= date('now') AND due_date <= date('now', '+' || ?1 || ' days') ORDER BY due_date ASC LIMIT 100`.

Register `get_upcoming_bills_cmd` in `src-tauri/src/lib.rs`.

**Verify**: `npm run rust:check` → exit 0.

#### 2c. TypeScript: extend `TransactionRow` + add bindings

In `src/lib/api.ts`:

1. Add `due_date: string | null` to `TransactionRow` (after `line_items`).
2. Add `dueDate?: string | null` to `createTransaction` input params; pass to invoke.
3. Add:

```ts
export interface UpcomingBill {
  id: string;
  description: string;
  amount: number;
  due_date: string;
  is_projection: boolean;
}

/** Returns bills with a due_date in the next `days` calendar days. */
export function getUpcomingBills(days: number): Promise<UpcomingBill[]> {
  return invoke("get_upcoming_bills_cmd", { days });
}
```

Also extend `TransactionEditValues` in `NewTransactionForm.tsx` to carry `due_date?: string | null`.

**Verify**: `npm run typecheck` → exit 0.

#### 2d. `NewTransactionForm`: optional due-date field

In `src/screens/NewTransactionForm.tsx`:

1. Add `dueDate: string` to `FormState` (default `""`).
2. Show the due-date input only when `kind === "saida"` (Saída/fixed bill) or
   `kind === "cartao"` (Cartão/credit). Label: "Vencimento (opcional)".
   Use the existing `field` + `label` style constants (lines 59–79).
3. Pass `dueDate: dueDate.trim() || null` to `createTransaction` on submit.
4. Extend `makeInitialForm` to load `due_date` from `TransactionEditValues` when
   editing.

No change is needed for `updateTransaction` (edit path) in this step — due-date
on existing imported rows is set to NULL and can be filled in on next create. A
follow-up can add editing of the due-date for existing transactions.

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0;
`npm run doctor` → 0 issues.

#### 2e. `TransactionsScreen`: due-date chip

In `src/screens/TransactionsScreen.tsx`, within the transaction-row expansion
panel (the block that shows tags, owners, line items), show a due-date chip when
`row.due_date != null`:

```tsx
{
  row.due_date && (
    <span
      style={DUE_DATE_CHIP_STYLE}
      aria-label={`Vencimento: ${fmtDate(row.due_date)}`}
    >
      📅 {fmtDate(row.due_date)}
    </span>
  );
}
```

`DUE_DATE_CHIP_STYLE` must be a hoisted `const` outside the component (React
Compiler convention). Use design-system tokens (e.g. `var(--surface-2)`,
`var(--border)`, `var(--fs-micro)`).

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0.

#### 2f. `HorizonteScreen`: upcoming-bills calendar

`src/screens/HorizonteScreen.tsx` already shows the cashflow horizon chart
(`BalanceTrajectory`). Add an "Upcoming bills" section below the existing
content:

1. `useCommand` hook (pattern: see `src/lib/useCommand.ts` usage in
   `DashboardScreen.tsx`) calling `getUpcomingBills(60)` (60-day window).
2. If the list is empty, show an `EmptyState` message: "Nenhum vencimento nos
   próximos 60 dias."
3. If non-empty, render a compact list sorted by `due_date`:
   - Each row: date chip + description + amount (use the `Money` component from
     `src/design-system/components/Money.tsx`) + projection badge (`ProvBadge`
     when `is_projection` is true).
4. Section title: "Vencimentos próximos" (h2 with design-system heading styles).

No new screen needed — this is additive content on an existing screen.

**Verify**: `npm run typecheck` → exit 0; `npm run e2e` → screenshots include
HorizonteScreen without regressions.

---

### Step 3: Installment tracking (P3 — implement if steps 1 and 2 are green)

**Scope**: Rust query-side derivation + TS + TransactionsScreen display.

No new migration is needed: the installment index is already embedded in the
transaction id (`{rec_id}:{i}`, 0-based), and `repetitions` is already stored in
the `recurrence` table. This step only surfaces that existing data.

#### 3a. Rust: add `installment_index` + `installment_total` to `TransactionRow`

In `src-tauri/src/commands/transactions.rs`:

1. Add fields to `TransactionRow`:

   ```rust
   /// 1-based position in the installment series (1 = first). None for non-recurrent rows.
   pub installment_index: Option<i64>,
   /// Total number of installments in the series. None for non-recurrent rows.
   pub installment_total: Option<i64>,
   ```

2. Add `recurrence_id: Option<String>` to `RecentRow` (already partially available
   via the id format — but `recurrence_id` column exists in the table since
   migration `20240612000005_recurrence.sql`). Extend the SELECT to include
   `t.recurrence_id`.

3. Derive `installment_index` and `installment_total` in the mapping step:
   - `installment_index`: use `occurrence_index(&r.id).map(|i| i + 1)` (convert 0-based to 1-based).
     The `occurrence_index` function is in `src-tauri/src/recurrence.rs` (lines 123–127) — make it
     `pub(crate)`.
   - `installment_total`: if `recurrence_id` is present, join the `recurrence` table in a sub-query
     to get `repetitions`. Efficient approach: collect the distinct `recurrence_id` values from the
     result set and do a single batch query `SELECT id, repetitions FROM recurrence WHERE id IN (...)`,
     then map. Pattern: same as the tag batch query (lines 115–135 of `transactions.rs`).

4. Update the `TransactionRow` construction to set both fields.

#### 3b. TypeScript: extend `TransactionRow`

In `src/lib/api.ts`, add to `TransactionRow`:

```ts
/** 1-based installment index. null = not a recurring series transaction. */
installment_index: number | null;
/** Total installments in the series. null = not a recurring series transaction. */
installment_total: number | null;
```

Also extend the `TXNS` mock in `src/test/commands.ts` with `installment_index: null, installment_total: null` (both fields nullable — existing tests should still pass).

**Verify**: `npm run typecheck` → exit 0; `npm run test:run` → all pass.

#### 3c. `TransactionsScreen`: installment badge

In the transaction-row expansion panel, show the installment badge when
`installment_index != null` and `installment_total != null`:

```tsx
{
  row.installment_index != null && row.installment_total != null && (
    <span style={INSTALLMENT_BADGE_STYLE}>
      {row.installment_index}/{row.installment_total} parcelas
    </span>
  );
}
```

`INSTALLMENT_BADGE_STYLE` must be a hoisted `const`. Style it similarly to the
existing `ProvBadge` chip — use `var(--surface-2)`, `var(--border)`, `var(--fs-micro)`.

**Verify**: `npm run lint` → exit 0; `npm run doctor` → 0 issues.

---

### Final verification

Run the full gate:

```
npm run check
```

Expected: exit 0, all checks green (typecheck + lint + test:run + rust:check +
doctor + e2e).

## Test plan

### New Rust unit tests (in-file `#[cfg(test)] mod tests`)

#### `src-tauri/src/commands/forecast_cmds.rs`

Model after the existing `#[tokio::test]` tests in the same file.

| Test name                                                   | Covers                                                                                       |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `upsert_daily_budget_with_categories_stores_breakdown`      | happy path: budget + 3 categories round-trips; sum is preserved; old categories are replaced |
| `upsert_daily_budget_with_categories_without_cats_ok`       | empty categories list leaves total-only budget; no category rows inserted                    |
| `upsert_daily_budget_with_categories_deprecates_old`        | second call deprecates previous budget and replaces categories                               |
| `upsert_daily_budget_with_categories_rejects_zero_category` | a category with amount_cents=0 returns Err                                                   |
| `get_daily_budget_categories_returns_empty_without_budget`  | no active budget → empty vec                                                                 |
| `monthly_to_daily_rate_divides_correctly`                   | 3100 / 31 = 100; 100 / 0 = 0 (no panic)                                                      |

#### `src-tauri/src/commands/transactions.rs`

| Test name                                          | Covers                                                                                                                          |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `recent_transactions_carry_due_date`               | a transaction with due_date set is returned correctly; one without returns null                                                 |
| `create_transaction_inner_stores_due_date`         | create with dueDate="2026-08-10" persists the value                                                                             |
| `get_upcoming_bills_returns_bills_in_window`       | two bills, one in window (10 days), one outside (90 days) → only the near one returned                                          |
| `installment_index_and_total_populated_for_series` | create a recurring series of 6; fetch via recent_transactions; check installment_index=1 for first, installment_total=6 for all |
| `installment_fields_null_for_single_transaction`   | non-recurrent transaction → both fields null                                                                                    |

### TypeScript unit tests

Model after `src/screens/NewTransactionForm.test.tsx`.

Create `src/screens/SettingsScreen.test.tsx` (if not already present) or extend it:

| Test name                                                          | Covers                                                                                 |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| `DiarioCategorySection renders with empty state`                   | mount with mocked `getDailyBudgetCategories` returning []; shows "Add category" button |
| `DiarioCategorySection shows existing categories`                  | mock returns 2 categories; renders both name+amount                                    |
| `DiarioCategorySection Save calls upsertDailyBudgetWithCategories` | user clicks Save; assert invoke called with correct args                               |
| `DiarioCategorySection derived daily rate`                         | total=31000, 31-day month → shows "R$10,00/dia"                                        |

Create `src/screens/HorizonteScreen.test.tsx` (if not present) or extend it:

| Test name                                         | Covers                                                                         |
| ------------------------------------------------- | ------------------------------------------------------------------------------ |
| `HorizonteScreen shows upcoming bills list`       | mock `getUpcomingBills` with 2 bills; both appear with formatted date + amount |
| `HorizonteScreen shows empty state when no bills` | mock returns []; EmptyState message shown                                      |

Verify: `npm run test:run` → all pass, including the new tests.

## Done criteria

Machine-checkable. ALL must hold before marking this plan DONE:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; new Rust + TS tests listed above exist and pass
- [ ] `npm run rust:check` exits 0
- [ ] `npm run doctor` exits 0 (no new React Doctor issues introduced)
- [ ] `npm run e2e` exits 0 (screenshots show no visual regressions)
- [ ] `npm run check` exits 0 (full gate clean)
- [ ] Migration files are additive only — no existing migration edited
- [ ] No private data, real names, private category labels, or method-app references
      appear in any new file (scan `src/` and `src-tauri/` for the forbidden brand/source terms
      listed in the local `.private-forbidden-patterns` denylist — zero matches; `npm run privacy:scan`)
- [ ] `git diff --name-only HEAD` shows only files from the in-scope list
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The schema at the "Current state" cited locations does not match the live files
  (codebase has drifted since this plan was written).
- `npm run rust:check` fails with a sqlx compile-error related to a migration
  not applied to the offline query cache — run `cargo sqlx prepare` from `src-tauri/`
  and commit the updated `sqlx-data.json` (or `.sqlx/` directory if that is the
  pattern in use), then re-run `rust:check`.
- Adding `due_date` to `TransactionRow` causes TypeScript errors in files outside
  the in-scope list (the field is nullable so existing callers should be unaffected —
  if they are not, the plan's assumption is wrong; report before touching out-of-scope files).
- Any step's verification fails twice after a reasonable fix attempt.
- Step 3 (installment tracking) requires touching a file outside the in-scope list
  beyond `occurrence_index` visibility change (make it `pub(crate)` in `recurrence.rs`
  is in-scope; anything more is a STOP).
- The upcoming-bills query causes a clippy warning about `sqlx::AssertSqlSafe` usage
  without the placeholder pattern — switch to a parameterised query with a computed
  date string in Rust instead of string-interpolating SQL.

## Maintenance notes

- **Engine compatibility**: `effective_daily_ceiling` (lines 264–298 of
  `forecast_cmds.rs`) reads `daily_budget WHERE status='active' AND amount > 0`.
  The new `upsert_daily_budget_with_categories_inner` calls the existing
  `upsert_daily_budget_inner` first, which continues to write the total-amount row.
  The engine is unaffected. If a future plan changes how the engine reads the
  daily budget, verify that `daily_budget_category` rows can be aggregated
  to the correct total as a consistency check.
- **Importer neutrality**: `src-tauri/src/google_sheets/import.rs` is NOT touched.
  If a future plan parses structured budget notes from the spreadsheet automatically,
  it should write to `daily_budget_category` using the same upsert path created here.
- **Due-date vs cash-flow date**: `due_date` is advisory metadata for the calendar
  view. It does NOT change the forecast's `balance_cents` or the Saldo chain (those
  use the `date` column). If this distinction is ever confused in a future change,
  the forecast regression tests in `src-tauri/src/commands/mod.rs` will catch it.
- **Installment display is derived, not stored**: `installment_index` and
  `installment_total` are computed at read time from `recurrence.repetitions` and
  the embedded `{rec_id}:{i}` id pattern. If the id pattern ever changes, the
  derivation must be updated. The tests added in Step 3 provide a regression net.
- **Deferred follow-ups** (explicitly out of this plan):
  - Auto-parsing structured Diário notes from the spreadsheet into `daily_budget_category` rows.
  - Editing `due_date` on existing imported transactions (currently only settable at create time).
  - Push-notification reminder tied to a `due_date` (could integrate with the OS scheduler from plan 039).
  - Write-back of `due_date` or budget categories to the spreadsheet.
