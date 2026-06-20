# Plan 036: Itemized transactions — EDIT line items (past/future/new) + write-back round-trip

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e62ecb6..HEAD -- src/screens/NewTransactionForm.tsx src/screens/dashboard/DailyCheckinCard.tsx src/lib/api.ts src-tauri/src/commands/transactions.rs src-tauri/src/google_sheets/write_back.rs src-tauri/src/google_sheets/mod.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/035-itemized-transactions-model-view.md (line_item table + read-side view must exist first)
- **Category**: feature
- **Planned at**: commit `e62ecb6`, 2026-06-20

## Why this matters

The user's primary daily friction is that the sheet uses itemized cells — a single cell total
accompanied by a multi-line cell note listing each part (`R$ <valor> - <descrição>`) — but the
app has no way to edit those parts or round-trip them back to the sheet faithfully. Every edit
from the app today either overwrites the total (losing the breakdown) or is blocked entirely
because imported rows are guarded against plain edits. This plan makes the breakdown fully
editable in both the create and edit flows, persists the parts as `line_item` rows (introduced
in plan 035), keeps the parent total as `Σ parts`, and reconstructs the exact sheet format
(`=SUM(…)` formula + per-part note) on write-back — so the app can be the primary logging
surface without degrading the sheet's detail level.

## Background: the itemization grammar (confirmed from primary source)

A single sheet cell holds a **total** that is the SUM of its parts. The cell value is the
numeric total; the cell **note** is the authoritative per-part breakdown. Each line of the note
follows the pattern `R$ <valor> - <descrição>` (comma decimal, optional dot thousands). Rules
the parser must tolerate:

- Optional space around the dash: `R$ 50,00 - Descrição` and `R$ 50,00-Descrição` both valid.
- `R$` with or without a trailing space before the number.
- An optional header line with no `R$` (e.g. `CONTAS:`) — ignored, not an item.
- Budget/projection cells use tab-separated lines like `Mensal⇥R$ 300,00⇥<categoria>` plus a
  `Total = R$ 1.250,00` trailer — treat the `R$ <valor>` columns as items; the trailer line is
  informational (skip it or keep total-only).
- Values use comma decimal + optional dot thousands: `1.200,50` = 120050 cents.
- Items correspond to SUM addends in order, but the count may not always match — be robust:
  if parts can't be parsed, keep the single total + raw note, **never lose data**.
- The `raw_note` field (already persisted per-row by the importer,
  `src-tauri/src/google_sheets/import.rs:92`) is the authoritative source.

Note: this grammar is distinct from the `#reembolso:`/`#dividir:` marker grammar (plan 023).
Items are plain prose lines; markers are opt-in suffixes. The two coexist on the same note;
item parsing runs independently of marker parsing.

This applies to Entrada, Saída, and Diário; to realized (past) and projected (future) cells.

## Prerequisite: plan 035

Plan 036 assumes plan 035 has landed and introduced:

- **SQLite migration**: a `line_item` table keyed on `transaction_id` with columns
  `id TEXT PK`, `transaction_id TEXT FK`, `amount_cents INTEGER`, `description TEXT`, `sort_order INTEGER`.
- **Importer extension**: `import_rows_core` already parses `raw_note` into `line_item` rows
  after the UPSERT; re-import clears+reinserts items for a given txn.
- **Read-side TS type** `LineItem { id: string; amount_cents: number; description: string; sort_order: number; }`
  exported from `src/lib/api.ts`.
- **`get_transaction_items` Tauri command** returning `Vec<LineItemRow>` for a given txn id.

If plan 035 has NOT landed, stop here and execute it first.

## Current state

### Key files and their roles

- `src/screens/NewTransactionForm.tsx` — create + edit form (633 lines). The `FormState`
  interface (line 108) and `makeInitialForm` (line 123) control what the form knows about a
  transaction; `submit()` (line 429) branches on `initialValues` to call `createTransaction` vs
  `updateTransaction`. The `TransactionEditValues` interface (line 21) is what callers pass in
  edit mode.
- `src/screens/dashboard/DailyCheckinCard.tsx` — quick-add card (384 lines). State via
  `checkinReducer` (line 107); calls `createTransaction` directly in `logSpend()` (line 161).
- `src/lib/api.ts` — all Tauri invocations. `createTransaction` (line 261), `updateTransaction`
  (line 283), `deleteTransaction` (line 277). No `line_item` APIs yet.
- `src-tauri/src/commands/transactions.rs` — Rust commands. `create_transaction` (line 157),
  `create_transaction_inner` (line 185), `update_transaction_cmd` (line 321). The UPDATE guard
  (line 341) is `WHERE id = ?1 AND source_amount IS NULL` — imported rows are blocked from plain
  edits (intentional).
- `src-tauri/src/google_sheets/write_back.rs` — write-back planner. `batch_update_values`
  uses `valueInputOption=RAW` (mod.rs line 237) and writes a bare numeric value. No formula or
  cell-note writing exists yet.
- `src-tauri/src/google_sheets/mod.rs` — `SheetsClient`. `batch_update_values` (line 200):
  `valueInputOption=RAW`; `get_sheet_notes` (line 88) reads notes via `spreadsheets.get`
  - `includeGridData`. There is no write-notes method yet.
- `src-tauri/src/google_sheets/import.rs` — `ImportedRow.raw_note` (line 92), `cell_raw_note`
  (line 719), `parse_note_markers` (line 805). The `raw_note` checksum feed (line 129) means
  editing the note in the sheet triggers a re-import update.
- `src-tauri/src/commands/write_back_cmds.rs` — `apply_write_back` (line 406): orchestrates
  conflict guard → staleness guard → `batch_update_values` → `record_write_back_audit`.
  `load_write_back_txns` (line 10) aggregates by `(date, kind)`.

### Relevant excerpts (verified at commit e62ecb6)

**`TransactionEditValues` interface** (`src/screens/NewTransactionForm.tsx:21–31`):

```typescript
export interface TransactionEditValues {
  id: string;
  type: string;
  amount: number; // centavos, magnitude positiva
  description: string;
  date: string;
  payment_method: string | null;
  is_fixed: boolean;
  /** Prefixo uuid da série (derivado do id "uuid:index"); null = lançamento único. */
  recurrence_id: string | null;
}
```

**`FormState` interface** (`src/screens/NewTransactionForm.tsx:108–121`):

```typescript
interface FormState {
  kind: MovKind;
  amount: string;
  description: string;
  date: string;
  selectedTags: string[];
  toAccountId: string;
  repeat: boolean;
  frequency: Frequency;
  repetitions: number;
  busy: boolean;
  error: string | null;
}
```

**`update_transaction_cmd` signature** (`src-tauri/src/commands/transactions.rs:321–331`):

```rust
pub async fn update_transaction_cmd(
    pool: State<'_, SqlitePool>,
    id: String,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
    date: String,
) -> Result<(), String> {
```

Guard at line 341: `WHERE id = ?1 AND source_amount IS NULL` — imported rows cannot be
edited via this command.

**`batch_update_values` in `SheetsClient`** (`src-tauri/src/google_sheets/mod.rs:200–203`):

```rust
pub async fn batch_update_values(
    &self,
    spreadsheet_id: &str,
    updates: &[(String, f64)],
) -> Result<usize, String> {
```

The body at line 237: `let body = serde_json::json!({ "valueInputOption": "RAW", "data": data });`

**Note fetch** (`src-tauri/src/google_sheets/mod.rs:88–96`):

```rust
pub async fn get_sheet_notes(
    &self,
    spreadsheet_id: &str,
    sheet_name: &str,
) -> Result<Vec<Vec<String>>, String> {
```

Uses `spreadsheets.get` with `includeGridData=true&fields=sheets.data.rowData.values.note`.

**`apply_write_back`** (`src-tauri/src/commands/write_back_cmds.rs:406`): conflict guard →
staleness guard → `batch_update_values` → `record_write_back_audit`. The plan is produced by
`plan_write_back` (write_back.rs line 128), which aggregates by `(date, kind)`.

**`ImportedRow.raw_note`** (`src-tauri/src/google_sheets/import.rs:92`):

```rust
pub raw_note: String,
```

Fed into `parse_note_markers` at line 402; enters the row checksum at line 129.

**Transaction schema** (`src-tauri/migrations/20240608000006_transaction.sql`):

```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,
    ...
);
```

`source_amount` added by `20240612000007_advanced_reconciliation.sql`: `NULL` = manual entry.

### Conventions that apply here

- **Functional-core / imperative-shell**: pure Rust functions (no async, no DB) for string
  builders and formula construction; async DB code only in command layer.
- **React Compiler ON**: no manual `useMemo`/`useCallback`; hoist static styles as module-level
  `const` objects (see `NewTransactionForm.tsx` pattern: `field`, `label`, `KIND_BTN_BASE`).
- **Money = positive-magnitude integer cents**. The sign lives in `type` (`income`/`expense`).
  Never store a negative `amount` in the DB; never store a floating-point value (use `i64`).
- **Method-neutral language** in code, comments, and UI copy. Do not name the upstream
  method/course/app/author in any committed file.
- **Error handling**: Rust commands return `Result<T, String>`. TS side uses `safeErrorMessage`
  from `src/lib/errors.ts`.
- **Tests**: Rust tests live in `#[cfg(test)] mod tests` at the bottom of the module file.
  Frontend tests use Vitest + Testing Library; pattern in `src/screens/NewTransactionForm.test.tsx`.
  Mock Tauri with `vi.mock("@tauri-apps/api/core", ...)` + `mockCommands` helper from
  `src/test/commands.ts`.

## Commands you will need

| Purpose               | Command              | Expected on success           |
| --------------------- | -------------------- | ----------------------------- |
| Typecheck (TS)        | `npm run typecheck`  | exit 0, no errors             |
| Lint                  | `npm run lint`       | exit 0                        |
| Unit tests            | `npm run test:run`   | all pass                      |
| Rust checks + tests   | `npm run rust:check` | exit 0                        |
| React Doctor advisory | `npm run doctor`     | 0 issues (advisory, not gate) |
| Full gate             | `npm run check`      | exit 0                        |
| E2E visual smoke      | `npm run e2e`        | screenshots pass              |

Run `npm run rust:check` after every Rust step. Run `npm run typecheck` after every TS step.

## Suggested executor toolkit

Use the `neko-finance-design` skill when adding new UI components to ensure they follow the
"Midnight Ledger" design system tokens (jade primary, brass warmth, dark-first). Itemized
editor fields should use `var(--bg-subtle)` / `var(--border-input)` / `var(--radius-xs)` /
`var(--hit-min)` consistent with the existing `field` style constant in `NewTransactionForm.tsx`.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/migrations/<next_timestamp>_line_item_write.sql` — if plan 035's migration needs
  adjustment (e.g. adding an `is_user_edited` flag); otherwise leave migrations untouched.
- `src-tauri/src/commands/transactions.rs` — extend `update_transaction_cmd` + add
  `update_transaction_items_cmd`.
- `src-tauri/src/google_sheets/write_back.rs` — add `build_itemized_cell_value` (formula
  builder) and `build_itemized_note` (note string builder); extend `WriteBackTxn` to carry
  optional items; extend `plan_write_back` to produce itemized `CellWrite`s.
- `src-tauri/src/google_sheets/mod.rs` — add `batch_update_notes` method to `SheetsClient`.
- `src-tauri/src/commands/write_back_cmds.rs` — extend `apply_write_back` to call
  `batch_update_notes` after `batch_update_values` (phased: attempt note write, surface error
  as non-fatal warning if API call fails, never block the value write on note failure).
- `src-tauri/src/lib.rs` — register `update_transaction_items_cmd`.
- `src/lib/api.ts` — add `updateTransactionItems` and `LineItem` type (if not from plan 035).
- `src/screens/NewTransactionForm.tsx` — add `LineItemEditor` sub-component; extend
  `TransactionEditValues` and `FormState`; wire `updateTransactionItems` in `submit()`.
- `src/screens/NewTransactionForm.test.tsx` — add tests for item add/remove/total.
- `src/screens/dashboard/DailyCheckinCard.tsx` — add `LineItemEditor` to the quick-add flow
  (collapsed by default, revealed by a "Detalhar" toggle button so the fast path stays fast).
- `src-tauri/src/google_sheets/write_back_test.rs` OR inline in `write_back.rs` — add unit
  tests for `build_itemized_cell_value` and `build_itemized_note`.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/import.rs` — the importer already writes `raw_note` into
  `line_item` rows (plan 035). Do not touch.
- `src-tauri/src/google_sheets/reconcile.rs` — reconcile logic is unchanged; items survive
  re-import because the importer clears+reinserts by `transaction_id` (plan 035 scope).
- Any changes to the `split` table or the `#dividir:`/`#reembolso:` marker parser — different
  concern (owner splits, plan 023).
- `src/screens/TransactionsScreen.tsx` and `src/screens/LedgerScreen.tsx` — the item view is
  plan 035's read-side; this plan only adds the edit path.
- The Economia write-back path (`plan_economia_write_back`, `apply_economia_write_back`) — the
  Economia tab has no per-cell note; skip.
- Series recurrence edits (`updateSeriesAll`, `updateSeriesFrom`) — items are per-instance;
  applying items to a whole series is out of scope.

## Git workflow

- Branch: `advisor/036-itemized-edit-writeback`
- Commit style: conventional commits matching recent history. Example from `git log`:
  `fix: correções de fluxo P1/P2 no write-back e sync (plano 032) (#53)` →
  use `feat: itemized transaction edit + write-back round-trip (plano 036)`.
- One commit per step (or per logical unit); the codebase must compile between commits.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Add `update_transaction_items_cmd` Rust command

**What to do:**

In `src-tauri/src/commands/transactions.rs`, add a new Tauri command
`update_transaction_items_cmd` that replaces the `line_item` rows for a given `transaction_id`
transactionally, then updates the parent `transaction.amount` to `Σ parts`.

The command signature:

```rust
#[tauri::command]
pub async fn update_transaction_items_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
    items: Vec<LineItemInput>,
) -> Result<(), String>
```

Where `LineItemInput` is a local struct:

```rust
#[derive(serde::Deserialize)]
pub struct LineItemInput {
    pub amount_cents: i64,
    pub description: String,
    pub sort_order: i64,
}
```

Implementation:

1. Validate: `transaction_id` is non-empty; each `amount_cents > 0`; `items` non-empty (if
   empty, the caller should instead set a plain amount — reject with a clear error message).
2. Compute `total_cents: i64 = items.iter().map(|i| i.amount_cents).sum()`.
3. Open a SQLite transaction. Inside it:
   a. `DELETE FROM line_item WHERE transaction_id = ?1`
   b. For each item (in `sort_order` order): `INSERT INTO line_item (id, transaction_id, amount_cents, description, sort_order) VALUES (?, ?, ?, ?, ?)`
   using `uuid::Uuid::new_v4().to_string()` for the `id`.
   c. `UPDATE "transaction" SET amount = ?2, updated_at = ?3 WHERE id = ?1`
   — update the parent amount to `total_cents`. Do NOT check `source_amount IS NULL` here:
   the purpose is to allow itemized edits on **both** manual and imported rows. Imported rows
   keep their `source_amount` base intact (the total update does not touch `source_amount`),
   which is correct — the local item breakdown is a richer representation of the same cell.
4. Commit the transaction; return `Ok(())`.

**Note on imported rows**: `update_transaction_cmd` (existing, line 341) blocks imported rows
via `source_amount IS NULL`. The new `update_transaction_items_cmd` intentionally does NOT have
that guard — the user must be able to annotate/edit the breakdown of an imported row (that is
the whole point of this feature). The parent amount update stays within the value the user
chooses (which may differ from the sheet value — the 3-way reconcile will handle that on
re-import, surfacing a conflict if the sheet total also changed).

Register the command in `src-tauri/src/lib.rs` alongside `update_transaction_cmd`.

**Verify**: `npm run rust:check` → exit 0.

---

### Step 2: Extend `WriteBackTxn` and write-back planner for itemized cells

**What to do:**

In `src-tauri/src/google_sheets/write_back.rs`:

1. Extend `WriteBackTxn` to optionally carry items:

```rust
pub struct WriteBackTxn {
    pub date: String,
    pub kind: RowKind,
    pub amount_cents: i64,
    /// When Some, the cell should be written as `=SUM(a+b+c)` with a per-part note.
    /// When None, write the plain numeric total (existing RAW behaviour).
    pub items: Option<Vec<TxnLineItem>>,
}

#[derive(Debug, Clone)]
pub struct TxnLineItem {
    pub amount_cents: i64,
    pub description: String,
}
```

2. Add two pure functions (no async, no DB, easily unit-tested):

```rust
/// Builds the SUM formula for an itemized cell.
/// `items` must be non-empty. Values are in reais (cents / 100.0).
/// Example: [5000, 7500] → "=SUM(50+75)" (no fractional part when whole; "=SUM(50,00+75,00)" otherwise).
/// SAFETY: sanitize each value — if a description starts with `=` or `+` it is not used in the
/// formula (formulas are built from numeric values only, never from description strings).
pub fn build_itemized_cell_value(items: &[TxnLineItem]) -> String { ... }

/// Builds the per-part note string to write alongside the formula.
/// Format: one line per item, `R$ <valor> - <descrição>`, comma decimal, dot thousands.
/// Example: [5000, "Conta A"], [7500, "Conta B"] →
///   "R$ 50,00 - Conta A\nR$ 75,00 - Conta B"
/// Sanitize: if description is empty, use "<sem descrição>".
/// This function is the inverse of the itemized-note parser in plan 035's importer extension.
pub fn build_itemized_note(items: &[TxnLineItem]) -> String { ... }
```

Value formatting in the formula: use reais with 2 decimal places (e.g. `50,00`), comma decimal,
matching what the sheet displays. The formula string: `=SUM(50,00+75,00+1200,50)`.

3. Extend `plan_write_back` to produce an `is_itemized` flag (or carry the items) on the
   `CellWrite` struct. Extend `CellWrite`:

```rust
pub struct CellWrite {
    // ... existing fields ...
    /// When Some, the write-back should use USER_ENTERED + formula for the cell value
    /// and also write the cell note. When None, use RAW numeric (existing behaviour).
    pub items: Option<Vec<TxnLineItem>>,
    /// Pre-built formula string (only when items is Some).
    pub formula: Option<String>,
    /// Pre-built note string (only when items is Some).
    pub note_text: Option<String>,
}
```

In `plan_write_back`, when a txn has items, set `formula = build_itemized_cell_value(&items)`,
`note_text = build_itemized_note(&items)`, `proposed = cents_to_ptbr(total)` (unchanged for
the diff display), and `value_cents = total` (unchanged).

For the aggregation step (lines 163–177 in write_back.rs): when a txn carries items, it cannot
be aggregated with another txn on the same `(date, kind)` — two itemized txns for the same cell
is an edge case requiring a conflict; in that situation keep both as separate `CellWrite`s and
mark the second with a `conflict_note` (or simply aggregate the totals and drop the
per-item breakdown, since the SUM formula needs to be consistent). Simplest safe approach: if
multiple txns land on the same `(date, kind)` and any of them has items, aggregate totals as
before (sum `amount_cents`), but set `items = None` (fall back to RAW numeric) and add a note
in `proposed` like `"<multiple items — edit separately>"`. This prevents formula corruption
while not blocking the write.

**Verify**: `npm run rust:check` → exit 0.

---

### Step 3: Add `batch_update_notes` to `SheetsClient`

**What to do:**

In `src-tauri/src/google_sheets/mod.rs`, add a new method to `SheetsClient`:

```rust
/// Writes cell notes via `spreadsheets.batchUpdate` (the `updateCells` request with
/// `fields="note"`). This is SEPARATE from `values:batchUpdate` — notes are metadata,
/// not values, and require the `spreadsheets` endpoint, not `spreadsheets/values`.
///
/// `note_updates`: list of (A1 notation WITH sheet name e.g. `'2026'!E3`, note text).
/// Empty string clears the note. Returns the count of cells updated.
///
/// Required OAuth scope: `spreadsheets` (read-write). The app already requests this
/// scope for write-back (plan 028 Step 1 ensured `spreadsheets` scope on re-consent).
pub async fn batch_update_notes(
    &self,
    spreadsheet_id: &str,
    note_updates: &[(String, String)],
) -> Result<usize, String>
```

Implementation using the `spreadsheets.batchUpdate` API:

```
POST https://sheets.googleapis.com/v4/spreadsheets/{spreadsheetId}:batchUpdate
Body: {
  "requests": [
    {
      "updateCells": {
        "range": { <GridRange parsed from A1 notation> },
        "rows": [{ "values": [{ "note": "<note text>" }] }],
        "fields": "note"
      }
    },
    ...
  ]
}
```

To convert an A1 range like `'2026'!E3` to a `GridRange`, you need:

- `sheetId`: fetch via `spreadsheets.get` (the sheet metadata) — or accept that the caller
  passes both the A1 and the `sheetId`. Simplest approach: add a helper
  `get_sheet_id_by_name(spreadsheet_id, sheet_name) -> Result<i64, String>` that calls
  `spreadsheets.get?fields=sheets.properties` and finds the matching sheet. Cache the result
  in the call (one extra GET, but note-writes are infrequent).
- Row/col: parse from the A1 string (reuse or adapt the existing `col_to_a1` inverse).

**STOP condition for scope issue**: If at runtime the `spreadsheets.batchUpdate` request
returns HTTP 403 with `insufficient permission`, it means the stored token was granted only
`spreadsheets.readonly` (pre-plan-028 tokens). This is non-fatal: surface the error to the
caller as a warning string prefixed with `"NOTE_WRITE_PERMISSION:"` and let the caller decide
to skip (the value write has already succeeded). Document this in the function's doc comment.

**Verify**: `npm run rust:check` → exit 0. (No live API test at this stage.)

---

### Step 4: Wire note-write into `apply_write_back` (phased)

**What to do:**

In `src-tauri/src/commands/write_back_cmds.rs`, extend `apply_write_back` (line 406) after the
`batch_update_values` call:

```rust
// Phase 2: write cell notes for itemized cells (best-effort, non-fatal).
// The value write has already succeeded. Note write failure surfaces as a warning,
// not an error — the user's total is correct; the note is enrichment.
let note_updates: Vec<(String, String)> = changed
    .iter()
    .filter_map(|c| {
        c.note_text.as_ref().map(|note| {
            (format!("{}!{}", quote_sheet(&sheet_name), c.a1), note.clone())
        })
    })
    .collect();

let note_warning: Option<String> = if note_updates.is_empty() {
    None
} else {
    match client.batch_update_notes(&spreadsheet_id, &note_updates).await {
        Ok(_) => None,
        Err(e) if e.starts_with("NOTE_WRITE_PERMISSION:") => {
            Some("Notas de célula não foram atualizadas: consentimento de escrita necessário.".into())
        }
        Err(e) => Some(format!("Notas de célula: {e}")),
    }
};
```

Change the return type of `apply_write_back` from `Result<usize, String>` to
`Result<WriteBackResult, String>` where:

```rust
#[derive(serde::Serialize)]
pub struct WriteBackResult {
    pub written: usize,
    pub note_warning: Option<String>,
}
```

Update the TS type in `src/lib/api.ts` accordingly, and update any UI that currently reads the
`usize` result to read `written` from the new shape. If changing the return type creates too
much ripple (update callers in the UI), an alternative is to log the note warning to the
Tauri logger (`tauri::plugin::log` or `eprintln!` in debug) and keep the return type as
`Result<usize, String>`. Choose whichever approach minimises out-of-scope changes; document the
choice with a code comment.

**Verify**: `npm run rust:check` → exit 0. `npm run typecheck` → exit 0.

---

### Step 5: Add `updateTransactionItems` TS API

**What to do:**

In `src/lib/api.ts`, add (if plan 035 has not already added it):

```typescript
export interface LineItem {
  id?: string; // undefined for new (not yet persisted) items
  amount_cents: number; // positive magnitude, integer cents
  description: string;
  sort_order: number;
}

/** Replaces all line items for a transaction and updates the parent total = Σ parts. */
export function updateTransactionItems(
  transactionId: string,
  items: LineItem[],
): Promise<void> {
  return invoke("update_transaction_items_cmd", { transactionId, items });
}
```

**Verify**: `npm run typecheck` → exit 0.

---

### Step 6: Build `LineItemEditor` component

**What to do:**

Create `src/design-system/components/LineItemEditor.tsx`. This is a pure presentational
component driven by props (no internal fetch, no Tauri calls).

Props:

```typescript
interface LineItemEditorProps {
  items: LineItem[]; // controlled — parent owns the list
  onChange: (items: LineItem[]) => void;
  disabled?: boolean;
}
```

Behaviour:

- Renders a list of rows, each with: a money input (amount, `inputMode="decimal"`, pt-BR
  format), a text input (description), and a remove button (`×`).
- "Adicionar item" button appends a blank row at the end.
- The amount field in the PARENT form becomes **read-only** when `items.length > 0` (the total
  is auto-computed). When `items.length === 0` the parent amount field is editable as before.
- Show a `Total: R$ X,XX` summary line at the bottom of the item list when there are ≥2 items.
- Styling: use the same `field` style constant pattern as `NewTransactionForm.tsx` (define a
  module-level `const ITEM_FIELD: React.CSSProperties = { ... }` mirroring `field`).
- Accessibility: each row has an `aria-label="Item N"` on the remove button; the amount input
  has a `<label>` (visually hidden via `SR_ONLY` from `../../design-system/srOnly`) or an
  `aria-label` referencing the item index.
- React Compiler: hoist all static style objects at module level. Do not use `useMemo`.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0. `npm run doctor` → 0 issues.

---

### Step 7: Wire `LineItemEditor` into `NewTransactionForm`

**What to do:**

In `src/screens/NewTransactionForm.tsx`:

1. Extend `TransactionEditValues` (line 21) with an optional items field:

   ```typescript
   items?: LineItem[];
   ```

   Callers that open the edit form (e.g. `TransactionsScreen`) need to populate this from the
   `get_transaction_items` command (plan 035). If plan 035's read-side is not wired yet, default
   to `[]` — the editor still works (user can add items from scratch).

2. Extend `FormState` (line 108) with:

   ```typescript
   items: LineItem[];
   ```

   Default: `[]`.

3. Extend `makeInitialForm` (line 123) to populate `items` from `initial.items ?? []`.

4. Add a `FormAction` case:

   ```typescript
   | { type: "setItems"; items: LineItem[] }
   ```

   Handler in `formReducer`: `case "setItems": return { ...s, items: a.items };`

5. In the render body, show `<LineItemEditor>` below the description field, always visible
   (not gated by a toggle — the full form has space). When `items.length > 0`, make the amount
   input `readOnly` and set its value to `centsToInput(items.reduce((s,i)=>s+i.amount_cents,0))`.

6. In `submit()` (line 429), after the create/update call succeeds, if `items.length > 0` call
   `await updateTransactionItems(id, items)` where `id` is the transaction id (returned by
   `createTransaction`, or `initialValues.id` in edit mode).

7. On `submitSuccess` reducer case (line 179), reset `items: []`.

**STOP condition**: If `createTransaction` + `updateTransactionItems` is not atomic (items saved
after the parent), a crash between the two leaves an orphan. Acceptable for this plan — the
risk is low (the parent succeeds; items can be re-entered). Document this in a code comment.
A follow-up can wrap both in a server-side atomic command.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

---

### Step 8: Wire `LineItemEditor` into `DailyCheckinCard` (quick-add, collapsed)

**What to do:**

In `src/screens/dashboard/DailyCheckinCard.tsx`:

1. Add `showItems: boolean` to `CheckinState` (line 85). Default `false`.
   Add action `{ type: "toggleItems" }` to `CheckinAction` (line 101).
   Add `items: LineItem[]` and `{ type: "setItems"; items: LineItem[] }`.

2. Add a "Detalhar ▾" toggle button below the amount/description row. When clicked, reveals
   `<LineItemEditor items={items} onChange={(items) => dispatch({ type: "setItems", items })} />`.
   The toggle label flips to "Ocultar ▴" when open.

3. In `logSpend()` (line 161), if `items.length > 0`, after `createTransaction` resolves (which
   returns a string id), call `updateTransactionItems(id, items)`.

4. On `submitSuccess`, reset `items: [], showItems: false`.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

---

### Step 9: Load existing items in edit mode

**What to do:**

Wherever `NewTransactionForm` is opened in edit mode (find via `grep -rn "initialValues" src/`),
the caller needs to pass `items` alongside the other fields. The call site(s) should call
`getTransactionItems(id)` (plan 035 command) before mounting the form, then include the result
in `initialValues.items`.

If the call site is `src/screens/TransactionsScreen.tsx` or a modal opened from
`src/screens/LedgerScreen.tsx`, fetch the items there and pass them down. This step is purely a
prop-plumbing change at the call site; do not modify `NewTransactionForm.tsx` again.

If plan 035's `getTransactionItems` command is not yet available, skip this step and document
it as a follow-up: items will default to `[]` in edit mode (the user can re-add them, but
pre-population won't work).

**Verify**: `npm run typecheck` → exit 0.

---

### Step 10: Extend `load_write_back_txns` to carry items

**What to do:**

In `src-tauri/src/commands/write_back_cmds.rs`, extend `load_write_back_txns` (line 10) to
join `line_item` rows for each transaction and populate `WriteBackTxn.items`.

SQL addition (after the existing income/expense query):

```sql
SELECT li.amount_cents, li.description
FROM line_item li
WHERE li.transaction_id = ?1
ORDER BY li.sort_order
```

For each transaction in the result, run this query to fetch its items. If `items.is_empty()`,
leave `WriteBackTxn.items = None` (plain numeric write, existing behaviour). If non-empty, set
`WriteBackTxn.items = Some(items_vec)`.

Performance note: this is one additional query per transaction per write-back call. Write-back
is user-initiated and infrequent; N+1 is acceptable here. If the number of transactions per
year grows large, a follow-up can use a single join query.

**Verify**: `npm run rust:check` → exit 0.

---

### Step 11: Unit tests — Rust string builders

**What to do:**

In `src-tauri/src/google_sheets/write_back.rs` (inside the existing `#[cfg(test)] mod tests`
at line 319), add:

```rust
#[test]
fn build_itemized_cell_value_two_items() {
    let items = vec![
        TxnLineItem { amount_cents: 5000, description: "Conta A".into() },
        TxnLineItem { amount_cents: 7500, description: "Conta B".into() },
    ];
    assert_eq!(build_itemized_cell_value(&items), "=SUM(50,00+75,00)");
}

#[test]
fn build_itemized_cell_value_single_item_no_sum() {
    // A single item produces a plain value, not =SUM(x) — no need for a formula.
    let items = vec![TxnLineItem { amount_cents: 12050, description: "Item".into() }];
    assert_eq!(build_itemized_cell_value(&items), "=SUM(120,50)");
    // (Or "120,50" if you decide single-item = no formula. Document the choice.)
}

#[test]
fn build_itemized_note_two_items() {
    let items = vec![
        TxnLineItem { amount_cents: 5000, description: "Conta A".into() },
        TxnLineItem { amount_cents: 7500, description: "Conta B".into() },
    ];
    assert_eq!(
        build_itemized_note(&items),
        "R$ 50,00 - Conta A\nR$ 75,00 - Conta B",
    );
}

#[test]
fn build_itemized_note_empty_description_fallback() {
    let items = vec![TxnLineItem { amount_cents: 1000, description: "".into() }];
    assert_eq!(build_itemized_note(&items), "R$ 10,00 - <sem descrição>");
}

#[test]
fn build_itemized_cell_value_sanitizes_formula_injection() {
    // A description starting with '=' or '+' must NOT appear in the formula string.
    // The formula is built from numeric values only.
    let items = vec![
        TxnLineItem { amount_cents: 100, description: "=HYPERLINK(...)".into() },
        TxnLineItem { amount_cents: 200, description: "+malicious".into() },
    ];
    let formula = build_itemized_cell_value(&items);
    assert!(formula.starts_with("=SUM("));
    assert!(!formula.contains("HYPERLINK"));
    assert!(!formula.contains("malicious"));
}

#[test]
fn build_itemized_note_round_trips_to_parse() {
    // build_itemized_note output must be parseable by the plan-035 note parser
    // (the same grammar: R$ <valor> - <descrição>). This is a contract test.
    let items = vec![
        TxnLineItem { amount_cents: 5000, description: "Conta A".into() },
        TxnLineItem { amount_cents: 120050, description: "Parcela carro".into() },
    ];
    let note = build_itemized_note(&items);
    // Basic check: each line starts with "R$ " and contains " - "
    for line in note.lines() {
        assert!(line.starts_with("R$ "), "line: {line}");
        assert!(line.contains(" - "), "line: {line}");
    }
}
```

**Verify**: `npm run rust:check` → exit 0, including the new tests.

---

### Step 12: Frontend tests

**What to do:**

In `src/screens/NewTransactionForm.test.tsx`, add a `describe("line items")` block:

```typescript
describe("line items", () => {
  it("adding two items makes amount field read-only and shows total", async () => {
    const user = userEvent.setup();
    mockCommands({ list_tags_cmd: [], create_transaction: "new-id",
                   update_transaction_items_cmd: undefined });
    render(<NewTransactionForm onCreated={vi.fn()} />);
    // ... click "Adicionar item" twice, fill amounts/descriptions, verify amount is readOnly
    // and shows correct total
  });

  it("removing the last item re-enables the amount field", async () => { ... });

  it("submitting with items calls updateTransactionItems after createTransaction", async () => {
    // verify mockInvoke was called with "update_transaction_items_cmd" after "create_transaction"
  });

  it("total = sum of item amounts", async () => {
    // add items [5000, 7500] → total displayed = "125,00"
  });
});
```

Model the test structure after the existing tests in `NewTransactionForm.test.tsx` (vitest +
`userEvent` + `mockCommands`).

**Verify**: `npm run test:run` → all pass, including 4+ new tests.

---

### Step 13: Full gate

Run the complete check suite:

```
npm run check
```

This runs typecheck + lint + test:run + rust:check + doctor + build in sequence. All must exit 0. Then run:

```
npm run e2e
```

Inspect screenshots for any visual regression in the transaction form and daily checkin card.

**Verify**: `npm run check` → exit 0. `npm run e2e` → no new failures.

---

## Test plan

### New Rust unit tests (in `src-tauri/src/google_sheets/write_back.rs`)

| Test                                                    | Case                                                   |
| ------------------------------------------------------- | ------------------------------------------------------ |
| `build_itemized_cell_value_two_items`                   | `[5000, 7500]` → `"=SUM(50,00+75,00)"`                 |
| `build_itemized_cell_value_single_item_no_sum`          | single item → formula or plain value (document choice) |
| `build_itemized_note_two_items`                         | two items → correct multi-line note                    |
| `build_itemized_note_empty_description_fallback`        | empty description → `<sem descrição>`                  |
| `build_itemized_cell_value_sanitizes_formula_injection` | `=HYPERLINK` in description → absent from formula      |
| `build_itemized_note_round_trips_to_parse`              | output is parseable by the plan-035 note parser        |

### New Rust integration test (in `src-tauri/src/commands/transactions.rs`)

- `update_transaction_items_cmd_sets_total`: create a transaction with amount 1000, update items
  to `[{500, "A"}, {750, "B"}]`, assert parent `amount = 1250`.
- `update_transaction_items_cmd_rejects_empty_list`: passing `items = []` returns `Err`.

### New frontend tests (in `src/screens/NewTransactionForm.test.tsx`)

- Adding two items makes amount field read-only and shows total.
- Removing the last item re-enables the amount field.
- Submitting with items calls `update_transaction_items_cmd` after `create_transaction`.
- Total = sum of item amounts (arithmetic, not string).

Pattern: model after `src/screens/NewTransactionForm.test.tsx` lines 34–71.

---

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0; `build_itemized_cell_value` + `build_itemized_note` tests
      pass; `update_transaction_items_cmd` integration tests pass.
- [ ] `npm run typecheck` exits 0.
- [ ] `npm run test:run` exits 0; 6+ new tests exist (4 frontend, 2 Rust integration).
- [ ] `npm run lint` exits 0.
- [ ] `npm run doctor` exits 0 (advisory: no new React anti-patterns introduced).
- [ ] `npm run e2e` passes; transaction form and daily checkin card screenshots show no regression.
- [ ] `grep -rn "useMemo\|useCallback\|React.memo" src/screens/NewTransactionForm.tsx` → no new
      occurrences (React Compiler convention).
- [ ] `grep -rn "=HYPERLINK\|formula injection" src-tauri/src/` → formula sanitization test
      exists and passes.
- [ ] `npm run check` exits 0.
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD`).
- [ ] `plans/README.md` status row for plan 036 updated to DONE (or IN PROGRESS if partial).

---

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the excerpts (the codebase has
  drifted since this plan was written — run the drift check at the top first).
- Plan 035 has NOT landed: the `line_item` table does not exist, `get_transaction_items` is not
  registered in `src-tauri/src/lib.rs`, and/or `LineItem` is not exported from `src/lib/api.ts`.
  Stop and execute plan 035 first.
- The Sheets `spreadsheets.batchUpdate` note-write returns a non-403 error (unexpected API
  shape change). The note-write is non-fatal for the value write, but an unexpected error
  means the API contract has changed — stop and report rather than silently dropping notes.
- Extending `WriteBackTxn.items` in `write_back.rs` causes the aggregation logic in
  `plan_write_back` to fail compilation in a way that cannot be resolved without touching the
  out-of-scope Economia path — stop and report (do NOT touch `plan_economia_write_back`).
- Any step's `npm run rust:check` or `npm run typecheck` fails twice after a reasonable fix
  attempt.
- The `update_transaction_items_cmd` Rust command requires touching `import_rows_core` or
  `reconcile.rs` to maintain idempotence — stop and report (the plan assumes plan 035 already
  handles re-import idempotence).

---

## Maintenance notes

- **Re-import idempotence**: the importer (plan 035) clears and reinserts `line_item` rows for
  each txn on every re-import. If the user edited items in the app and the sheet note ALSO
  changed, the re-import wins (the note is authoritative). This is consistent with the 3-way
  reconcile: `source_amount` tracks the base; local item edits have no `source_*` analogue yet.
  A future plan can add `source_items` snapshots for a 3-way merge of the breakdown. For now,
  document in a code comment that local item edits are overwritten by re-import if the sheet
  note changes — users should do the sheet edit first, then re-import, then optionally refine
  in the app.
- **Note-write scope**: the note-write path (`batch_update_notes`) requires the `spreadsheets`
  OAuth scope (read-write). Tokens granted before plan 028 may not have it. The phased approach
  (non-fatal warning on 403) handles this gracefully. A future plan can prompt re-consent when
  the note-write warning appears.
- **Formula and note format are a contract**: `build_itemized_note` output must be parseable by
  the plan-035 importer's note parser. The `build_itemized_note_round_trips_to_parse` test
  enforces this. If the importer grammar changes in a future plan, update the builder to match
  and add a regression test.
- **Reviewer focus areas in the PR**:
  - `update_transaction_items_cmd`: verify the SQLite transaction wraps all three operations
    atomically (DELETE + INSERT + UPDATE parent).
  - `build_itemized_cell_value`: verify formula injection sanitization is airtight (no user
    string interpolated into the `=SUM(...)` string, only numeric values).
  - `batch_update_notes`: verify the 403 path is non-fatal and the value write proceeds.
  - `LineItemEditor`: verify the amount field is correctly disabled when items exist, and
    that `onChange` is called on every add/remove/edit (no stale closure).
- **Deferred**: atomic create + item insert in a single Tauri command (currently two round-trips
  with a gap). Acceptable for the initial implementation; follow up with a combined command if
  the gap causes observable issues.
- **Deferred**: 3-way reconcile for item breakdowns (`source_items` base). Currently local item
  edits are silently overwritten by re-import when the sheet note changes. Low risk in practice
  (the user controls both surfaces), but worth revisiting if dogfooding reveals friction.
