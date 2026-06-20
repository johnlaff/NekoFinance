# Plan 015: Wire built-but-unwired UI: edit/delete transaction, recurrence series, owner totals

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat d183bbf..HEAD -- src/lib/api.ts src/screens/TransactionsScreen.tsx src/screens/NewTransactionForm.tsx src/screens/TotaisScreen.tsx src/design-system/components/OwnerChip.tsx src/design-system/components/TransactionRow.tsx src-tauri/src/recurrence.rs src-tauri/src/splits.rs src-tauri/src/commands.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Six backend Tauri commands that are fully implemented and tested — four recurrence series
mutations and two owner-totals queries — are imported in `src/lib/api.ts` but called by
zero screens. A user can create a recurring series but has no way to edit or delete it.
Multi-owner splits are tracked in the database but never surfaced in totals. These gaps make
the app incomplete relative to the method's spreadsheet view and block real dogfooding of
manual entry with recurrences.

## Current state

**Gap confirmed: no single-transaction delete or update Tauri command exists.** A search of
`src-tauri/src/commands.rs`, `recurrence.rs`, `splits.rs`, and `lib.rs` finds zero
`delete_transaction`, `update_transaction`, or equivalently-named handlers. The registered
handler list in `src-tauri/src/lib.rs` (lines 21-64) does not include one. This is a
backend gap that must be filled as an early step (Step 1).

**`src/lib/api.ts`** — all frontend Tauri bindings. Six functions carry the
`react-doctor-disable-next-line deslop/unused-export -- UI pendente` comment, confirming no
screen calls them:

```
// api.ts:584-595  (splitsForTransaction, ownerTotalsForMonth — spec 017)
// api.ts:601-613  (createRecurringSeries — note: createTransaction already calls this when
//                  recurrence != null; the standalone fn is for future use)
// api.ts:615-650  (deleteSeriesFrom, deleteSeriesAll, updateSeriesFrom, updateSeriesAll)
```

The relevant types are:

```typescript
// api.ts:577-582
export interface OwnerTotal {
  owner_person_id: string;
  owner_name: string;
  total_cents: number;
}

// api.ts:599
export type Frequency = "diaria" | "semanal" | "mensal";

// api.ts:627-632
interface SeriesEdit {
  amount: number;
  description: string | null;
  paymentMethod: string | null;
  isFixed: boolean;
}
```

**`src/screens/TransactionsScreen.tsx`** — the Livro-razão (ledger). The transaction table
is built in a `visible.map(...)` at line 315. Each row renders in a `<Fragment key={t.id}>`
and has no edit or delete affordance. The existing tag-editor pattern (inline expandable row,
lines 393-435) is the structural model to follow for the action menu:

```tsx
// TransactionsScreen.tsx:86-112 — reducer state shape (extend this)
interface TransactionsUiState {
  scope: TransactionScope;
  showForm: boolean;
  reloadKey: number;
  tagEditId: string | null;
  tagSaving: string | null;
  tagError: string | null;
}

// TransactionsScreen.tsx:147-153 — screen props (unchanged)
export function TransactionsScreen({
  query,
  onQueryChange,
}: {
  query: string;
  onQueryChange: (query: string) => void;
});
```

The `TransactionRow` data type does NOT include `recurrence_id`. The id of a recurring
transaction follows the format `"{recurrence_id}:{index}"` (e.g. `"uuid:0"`, `"uuid:1"`).
To recover the `recurrence_id` from a transaction id: `id.includes(':') ? id.slice(0, id.lastIndexOf(':')) : null`.
This avoids a backend round-trip for the common "is this recurring?" check.

**`src/screens/NewTransactionForm.tsx`** — the create form (lines 150-431). It is a
controlled form with a `useReducer` (state type `FormState`, lines 87-98). Its `kindToFields`
helper (lines 12-28) maps a `MovKind` to `(txnType, isFixed, paymentMethod)`. This form
can be reused for editing with an `initialValues` prop seeded from the existing transaction.
Note: the form currently always calls `createTransaction`; the edit path must call
`updateSeriesFrom` or `updateSeriesAll` instead (or the new single-transaction update
command added in Step 1).

**`src/screens/TotaisScreen.tsx`** — monthly totals screen (lines 150-337). Currently uses
only the `getForecast` command. There is no call to `ownerTotalsForMonth`. The `MovTotal`
component (lines 124-148) is the per-metric display primitive. The screen already has a
month navigator (`MonthNav`, line 241) that knows `m.year` / `m.month` — the active month
values to pass to `ownerTotalsForMonth`.

**`src/design-system/components/OwnerChip.tsx`** — reusable chip (lines 64-112). Accepts
`name?: string` (overrides the default `who` label), `avatar?: boolean`, and `bare?:
boolean`. For the owner-totals breakdown, use `avatar` mode for visual distinction.

**`src-tauri/src/recurrence.rs`** — series backend (lines 130-320). Key facts:

- `delete_series_from(pool, transaction_id)` — deletes this occurrence and all with a higher
  index (`:N` suffix). Takes a **transaction id**, not a recurrence id.
- `delete_series_all(pool, recurrence_id)` — deletes all occurrences and the recurrence row.
  Takes the **recurrence id** (the uuid prefix).
- `update_series_from(pool, transaction_id, edit)` — same "from here" semantics.
- `update_series_all(pool, recurrence_id, edit)` — updates all rows in the series.
- A recurrence's transaction ids are formatted `"{recurrence_id}:{index}"` (line 97).

**`src-tauri/src/splits.rs`** — `owner_totals_for_month_cmd` is registered (confirmed in
`src-tauri/src/lib.rs` line 61). It takes `year: i64, month: i64`.

**Repo conventions to match:**

- React Compiler is ENABLED — do NOT add `memo`, `useMemo`, or `useCallback` manually.
- Money: `amount` in DB is magnitude-positive integer cents; sign comes from `type`.
- `useReducer` for UI state with multiple related fields (follow the existing pattern).
- Inline-style for component-level styling (no new CSS classes unless absolutely needed for
  table layout that cannot be expressed inline). Keep `CSSProperties` constants hoisted
  outside the component (not recreated per render) for static parts.
- Error messages use `safeErrorMessage(e, "fallback")` from `src/lib/errors.ts`.
- After mutating data, call `invalidateCommands()` then dispatch a reload action (see
  `handleCreated` in `TransactionsScreen.tsx` lines 192-195 for the pattern).
- Confirmations for destructive actions: a simple `window.confirm()` is acceptable for P1
  scope (no custom modal needed). PRODUCT.md preference: prefer inline/contextual UI over
  modal-as-first-thought.
- All interactive elements must have an `aria-label`; follow the existing tag-button pattern
  at `TransactionsScreen.tsx:373`.
- Commit style: conventional commits with description in Portuguese. Example: `feat: editar e apagar lançamentos recorrentes na tela Livro-razão`.

## Commands you will need

| Purpose            | Command                                  | Expected on success       |
| ------------------ | ---------------------------------------- | ------------------------- |
| Typecheck          | `npm run typecheck`                      | exit 0, no errors         |
| Lint               | `npm run lint`                           | exit 0                    |
| Front tests        | `npm run test:run`                       | all pass                  |
| Rust check         | `npm run rust:check`                     | fmt + clippy + tests pass |
| Full gate          | `npm run check`                          | exit 0                    |
| Run specific tests | `npm run test:run -- TransactionsScreen` | all pass                  |
| Run specific tests | `npm run test:run -- TotaisScreen`       | all pass                  |

## Suggested executor toolkit

- Invoke the `neko-finance-design` skill before writing new UI to verify token names and component
  patterns (especially `OwnerChip` avatar mode and button styles).

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands.rs` — add `delete_transaction_cmd` and `update_transaction_cmd`
- `src-tauri/src/lib.rs` — register the two new commands in the invoke handler
- `src/lib/api.ts` — add `deleteTransaction` and `updateTransaction` bindings; remove
  `react-doctor-disable-next-line` comments from the six wired-up functions once each is
  actually called by a screen
- `src/screens/TransactionsScreen.tsx` — add row action affordance (edit/delete) and the
  series-scope confirmation flow
- `src/screens/NewTransactionForm.tsx` — add `initialValues` prop + edit mode
- `src/screens/TotaisScreen.tsx` — add owner totals breakdown section
- `src/screens/TransactionsScreen.test.tsx` — new tests for delete/edit/series flows
- `src/screens/TotaisScreen.test.tsx` — new test for owner totals section
- `src/test/commands.ts` — add mock data fixtures for owner totals and series transactions
  if needed
- `plans/README.md` — update status row when done

**Out of scope** (do NOT touch, even though they look related):

- `src/design-system/components/TransactionRow.tsx` — this DS component is used in the
  annual/tags screens; do not alter its API for this plan. The ledger in TransactionsScreen
  uses its own `<table>` layout, not this component.
- `src/design-system/components/OwnerChip.tsx` — use as-is; do not modify.
- `src-tauri/src/recurrence.rs` — the series backend is complete and tested; do not touch.
- `src-tauri/src/splits.rs` — the splits backend is complete and tested; do not touch.
- Any screen other than `TransactionsScreen`, `NewTransactionForm`, and `TotaisScreen`.
- The `createRecurringSeries` binding in `api.ts` — `createTransaction` already delegates to
  it when `recurrence != null`; the standalone export stays unused for now (its comment is
  correct and can remain).

## Git workflow

- Branch: `feat/015-wire-unwired-ui`
- Commit per logical step (backend first, then each screen, then tests).
- Message style: conventional commits with Portuguese description.
  Example: `feat: apagar e editar lançamentos (único e série) no Livro-razão`
- Do NOT push or open a PR unless explicitly asked.

## Steps

### Step 1: Add single-transaction delete and update Tauri commands (Rust)

Open `src-tauri/src/commands.rs`. Locate `pub async fn create_transaction` at line 1768.
After the `create_transaction` function (after its closing `}`, roughly around line 1864),
add two new `#[tauri::command]` functions:

**`delete_transaction_cmd`** — deletes one transaction row by id. Guard: only allow
deleting rows with `provenance != 'importado'` (i.e., `source_amount IS NULL`) to avoid
silently removing imported history. Return `Ok(())` on success, `Err(String)` on failure.
SQL: `DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL`.
If the DELETE affects 0 rows, return an error: `"lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)"`.

```rust
#[tauri::command]
pub async fn delete_transaction_cmd(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let affected = sqlx::query(
        r#"DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL"#,
    )
    .bind(&id)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("delete: {e}"))?
    .rows_affected();
    if affected == 0 {
        return Err("lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)".into());
    }
    Ok(())
}
```

**`update_transaction_cmd`** — updates amount, description, payment_method, is_fixed, and
date for a single non-imported transaction. Uses the same guard (`source_amount IS NULL`).

```rust
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_transaction_cmd(
    pool: State<'_, SqlitePool>,
    id: String,
    amount_cents: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
    date: String,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET amount = ?2, description = ?3, payment_method = ?4,
               is_fixed = ?5, date = ?6, updated_at = ?7
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
    .bind(&id)
    .bind(amount_cents)
    .bind(&description)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(&date)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("update: {e}"))?
    .rows_affected();
    if affected == 0 {
        return Err("lançamento não encontrado ou importado da planilha (não pode ser editado pelo app)".into());
    }
    Ok(())
}
```

Then open `src-tauri/src/lib.rs`. In the `tauri::generate_handler![...]` block (lines
21-64), add the two new commands after `commands::create_transaction`:

```rust
commands::delete_transaction_cmd,
commands::update_transaction_cmd,
```

**Verify**: `npm run rust:check` → exit 0, no fmt/clippy/test errors.

### Step 2: Add frontend bindings for delete/update in api.ts

Open `src/lib/api.ts`. After the `createTransaction` function (around line 290), add:

```typescript
/** Apaga um lançamento manual pelo id. Importados da planilha são rejeitados pelo backend. */
export function deleteTransaction(id: string): Promise<void> {
  return invoke("delete_transaction_cmd", { id });
}

/** Edita um lançamento manual (valor, descrição, método, fixo, data). */
export function updateTransaction(
  id: string,
  edit: {
    amountCents: number;
    description: string | null;
    paymentMethod: string | null;
    isFixed: boolean;
    date: string;
  },
): Promise<void> {
  return invoke("update_transaction_cmd", { id, ...edit });
}
```

Also remove the `react-doctor-disable-next-line` comment lines from
`deleteSeriesFrom` (line 616), `deleteSeriesAll` (line 622), `updateSeriesFrom` (line 635),
`updateSeriesAll` (line 644), `ownerTotalsForMonth` (line 589), and `splitsForTransaction`
(line 584) — but ONLY after each corresponding screen actually calls them (Steps 3 and 4
below). Do not remove them prematurely.

**Verify**: `npm run typecheck` → exit 0.

### Step 3: Add edit/delete affordance to TransactionsScreen

This step wires `deleteTransaction`, `deleteSeriesFrom`, `deleteSeriesAll`,
`updateSeriesFrom`, and `updateSeriesAll` into the ledger screen.

**3a — Extend the reducer state and actions.**

Open `src/screens/TransactionsScreen.tsx`. Add to the import from `../lib/api`:
`deleteTransaction`, `deleteSeriesFrom`, `deleteSeriesAll`, `updateSeriesFrom`,
`updateSeriesAll`.

Extend `TransactionsUiState` (currently lines 86-93) with two new fields:

```typescript
interface TransactionsUiState {
  // ... existing fields ...
  actionRowId: string | null; // which row has the action panel open
  actionError: string | null; // last error from a delete/edit action
}
```

Extend `TransactionsUiAction` (currently lines 95-103) with new cases:

```typescript
type TransactionsUiAction =
  // ... existing cases ...
  | { type: "toggleActionRow"; id: string }
  | { type: "actionError"; error: string }
  | { type: "actionClear" };
```

Extend `INITIAL_UI_STATE` (lines 105-112) with:

```typescript
  actionRowId: null,
  actionError: null,
```

Extend `transactionsUiReducer` (lines 114-145) with:

```typescript
case "toggleActionRow":
  return {
    ...state,
    actionRowId: state.actionRowId === action.id ? null : action.id,
    actionError: null,
  };
case "actionError":
  return { ...state, actionError: action.error };
case "actionClear":
  return { ...state, actionRowId: null, actionError: null };
```

**3b — Add action handlers inside the component.**

Inside `TransactionsScreen`, add two async handlers. Use `window.confirm` for destructive
confirmation — no custom modal needed:

```typescript
async function handleDeleteOne(t: TransactionRow) {
  if (
    !window.confirm(
      `Apagar "${t.description || "este lançamento"}"? Esta ação não pode ser desfeita.`,
    )
  )
    return;
  try {
    await deleteTransaction(t.id);
    invalidateCommands();
    dispatchUi({ type: "reload" });
    dispatchUi({ type: "actionClear" });
  } catch (e) {
    dispatchUi({
      type: "actionError",
      error: safeErrorMessage(e, "Não foi possível apagar. Tente novamente."),
    });
  }
}

async function handleDeleteSeries(t: TransactionRow) {
  const recId = t.id.includes(":") ? t.id.slice(0, t.id.lastIndexOf(":")) : null;
  if (!recId) {
    dispatchUi({ type: "actionError", error: "Lançamento não pertence a uma série." });
    return;
  }
  const choice = window.confirm(
    `Série recorrente detectada.\n\nOK = apagar este e todos os futuros da série.\nCancela = apagar somente este.`,
  );
  try {
    if (choice) {
      await deleteSeriesFrom(t.id);
    } else {
      await deleteTransaction(t.id);
    }
    invalidateCommands();
    dispatchUi({ type: "reload" });
    dispatchUi({ type: "actionClear" });
  } catch (e) {
    dispatchUi({
      type: "actionError",
      error: safeErrorMessage(e, "Não foi possível apagar. Tente novamente."),
    });
  }
}
```

For editing: open the form panel pre-seeded with the transaction's values. Reuse
`NewTransactionForm` with an `initialValues` prop (added in Step 4). The action panel
should render `<NewTransactionForm initialValues={t} onSaved={handleSaved} />` where
`handleSaved` closes the panel and reloads.

**3c — Add the action affordance in the row JSX.**

In the `<tr>` for each transaction (around line 328), add a small action button in a new
`<td>` (or inline in the description cell after the tag button). Keep it minimal: a single
button with three dots ("…") or a lucide `MoreHorizontal` icon that toggles `actionRowId`.

Example approach — add to the `<tr>` after the value `<td>` (the 5-column table gains a
6th column, or reuse the description cell):

```tsx
<td style={{ width: 32, textAlign: "right", paddingRight: 8 }}>
  <button
    type="button"
    className="txn-tag-btn"
    aria-label={`Ações para ${t.description || "lançamento"}`}
    aria-expanded={ui.actionRowId === t.id}
    onClick={() => dispatchUi({ type: "toggleActionRow", id: t.id })}
  >
    <MoreHorizontal size={13} strokeWidth={1.75} />
  </button>
</td>
```

Add `MoreHorizontal` to the lucide-react import at line 2.

After the tag-editor row (the existing `{ui.tagEditId === t.id && ...}` block), add a
matching action panel row:

```tsx
{
  ui.actionRowId === t.id && (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        {ui.actionError && (
          <p className="txs-inline-error" role="alert">
            {ui.actionError}
          </p>
        )}
        <div
          style={{
            display: "flex",
            gap: "var(--space-3)",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              /* open edit form: set editingTxn = t */
            }}
          >
            Editar
          </Button>
          {t.id.includes(":") ? (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void handleDeleteSeries(t)}
            >
              Apagar da série
            </Button>
          ) : (
            <Button size="sm" variant="ghost" onClick={() => void handleDeleteOne(t)}>
              Apagar
            </Button>
          )}
        </div>
      </td>
    </tr>
  );
}
```

For the edit flow, add an `editingTxn: TransactionRow | null` field to `TransactionsUiState`
and show `<NewTransactionForm initialValues={ui.editingTxn} onSaved={handleSaved} />` inline
when it is non-null (same container pattern as the "Novo lançamento" form at lines 285-289).

**Verify**: `npm run typecheck` → exit 0. Then `npm run lint` → exit 0.

### Step 4: Add edit-mode (initialValues prop) to NewTransactionForm

Open `src/screens/NewTransactionForm.tsx`. Add an optional prop:

```typescript
export interface TransactionEditValues {
  id: string;
  type: string;
  amount: number;          // cents, positive magnitude
  description: string;
  date: string;
  payment_method: string;
  is_fixed: boolean;
  recurrence_id: string | null;  // derived in Step 3 from the id format
}

export function NewTransactionForm({
  onCreated,
  initialValues,
  onSaved,
}: {
  onCreated?: () => void;
  initialValues?: TransactionEditValues;
  onSaved?: () => void;
}) {
```

When `initialValues` is provided:

- Seed `makeInitialForm()` from `initialValues`: convert `amount` (cents) back to BRL
  display string using `formatBRL` from `src/lib/format.ts` (e.g. `(amount / 100).toFixed(2).replace('.', ',')`)
  or use a helper; set `date`, `description`, `kind` (reverse `kindToFields`).
- On submit, call `updateTransaction` (for a non-recurring row) or show a choice (this
  occurrence vs whole series) for a recurring one, then call `updateSeriesFrom` or
  `updateSeriesAll`.
- After a successful save, call `onSaved?.()` instead of `onCreated?.()`.

To derive `kind` from the existing transaction values, add a helper inverse of `kindToFields`:

```typescript
function fieldsToKind(
  type: string,
  isFixed: boolean,
  paymentMethod: string | null,
): MovKind {
  if (type === "income") return "entrada";
  if (type === "transfer") return "economia"; // economia stays out of form; guard with FORM_KINDS check
  if (isFixed) return "saida";
  if (paymentMethod === "credit") return "cartao";
  return "diario";
}
```

The form submit logic when `initialValues` is provided (pseudo-code):

```typescript
if (initialValues) {
  const recId = initialValues.recurrence_id;
  if (recId) {
    const all = window.confirm(
      "Aplicar a alteração em toda a série?\n\nOK = toda a série\nCancela = este e futuros",
    );
    const edit: SeriesEdit = {
      amount: amountCents,
      description: description.trim() || null,
      paymentMethod: fields.paymentMethod,
      isFixed: fields.isFixed,
    };
    if (all) {
      await updateSeriesAll(recId, edit);
    } else {
      await updateSeriesFrom(initialValues.id, edit);
    }
  } else {
    await updateTransaction(initialValues.id, {
      amountCents,
      description: description.trim() || null,
      paymentMethod: fields.paymentMethod,
      isFixed: fields.isFixed,
      date,
    });
  }
  dispatch({ type: "submitSuccess" });
  onSaved?.();
  return;
}
// ... existing create path ...
```

Import `updateTransaction`, `updateSeriesFrom`, `updateSeriesAll`, `type SeriesEdit` (or
inline the type) from `../lib/api`.

Note: `SeriesEdit` is an interface defined in `api.ts` lines 627-632 but NOT currently
exported. Export it: change `interface SeriesEdit` to `export interface SeriesEdit`.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

### Step 5: Surface owner totals in TotaisScreen

Open `src/screens/TotaisScreen.tsx`. Add to the imports:

```typescript
import { getForecast, ownerTotalsForMonth, type OwnerTotal } from "../lib/api";
import { OwnerChip } from "../design-system/components/OwnerChip";
```

Add a `useCommand` call for owner totals, keyed by the active year/month (so it refreshes on
MonthNav navigation):

```typescript
const ownerTotalsQ = useCommand(`owner_totals_for_month:${m.year}:${m.month}`, () =>
  ownerTotalsForMonth(m.year, m.month),
);
const ownerTotals: OwnerTotal[] = ownerTotalsQ.data ?? [];
```

Place this AFTER `m` is resolved (after the early-return guards at lines 175-181 in the
current file, i.e. after `if (!m) { return ... }`). Because React hooks must not be called
conditionally, restructure the component so that `useCommand` for owner totals is called
before any conditional returns, keyed to `m?.year ?? 0` and `m?.month ?? 0`, and the result
used only when `m` is available.

Alternatively — and simpler — move the `ownerTotalsQ` call to just after the `const m = ...`
line (line 174 in the current file), using `m` values only in the key. Both approaches are
acceptable.

Add a new section at the bottom of the returned JSX, after the "Movimentações do mês"
section (after the `</section>` that closes around line 333), but ONLY when
`ownerTotals.length >= 2`:

```tsx
{
  ownerTotals.length >= 2 && (
    <section aria-label="Por titular" style={{ marginTop: "var(--space-8)" }}>
      <h2
        style={{
          fontSize: "var(--fs-label)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--ls-label)",
          textTransform: "uppercase",
          color: "var(--text-muted)",
          margin: "0 0 var(--space-4)",
        }}
      >
        Por titular
      </h2>
      <div style={{ display: "flex", gap: "var(--space-8)", flexWrap: "wrap" }}>
        {ownerTotals.map((o) => (
          <span
            key={o.owner_person_id}
            style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}
          >
            <OwnerChip name={o.owner_name} avatar />
            <Money cents={o.total_cents} size="md" />
          </span>
        ))}
      </div>
    </section>
  );
}
```

After wiring this, remove the `react-doctor-disable-next-line` comment from
`ownerTotalsForMonth` in `api.ts` (line 589).

**Verify**: `npm run typecheck` → exit 0.

### Step 6: Remove remaining react-doctor-disable comments from now-called functions

After Steps 3–5, all six previously-unused api.ts functions are wired to screens.
Open `src/lib/api.ts` and remove the `react-doctor-disable-next-line` comment lines
(and their inline explanations) from:

- `splitsForTransaction` (line 584) — called indirectly via the edit panel in TransactionsScreen
- `ownerTotalsForMonth` (line 589) — called in TotaisScreen (Step 5)
- `deleteSeriesFrom` (line 616) — called in TransactionsScreen (Step 3)
- `deleteSeriesAll` (line 622) — called in TransactionsScreen (Step 3) for "delete all"
- `updateSeriesFrom` (line 636) — called via NewTransactionForm edit mode (Step 4)
- `updateSeriesAll` (line 645) — called via NewTransactionForm edit mode (Step 4)

Note: if `splitsForTransaction` is NOT yet called by any screen (the per-transaction split
detail panel may be deferred), keep its comment until it is wired. Do not remove a comment
unless the function is actually imported and called somewhere.

**Verify**: `npm run lint` → exit 0 (the doctor rule will catch any lingering unused
export that lost its disable comment).

### Step 7: Write tests

**7a — `src/screens/TransactionsScreen.test.tsx`**: add new `describe` block after the
existing ones. Use `mockCommands` (from `src/test/commands.ts`) to mock
`delete_transaction_cmd`, `delete_series_from_cmd`, `update_series_from_cmd`, etc.
Use the existing `TXNS` fixture; extend it in `src/test/commands.ts` with one entry that
has a recurrence-formatted id:

```typescript
// In src/test/commands.ts, add to TXNS or export a separate RECURRING_TXN:
export const RECURRING_TXN: TransactionRow = {
  id: "rec-uuid-abc:2",
  type: "expense",
  amount: 50000,
  description: "Aluguel recorrente",
  date: "2026-06-01",
  payment_method: "debit",
  is_projection: true,
  is_fixed: true,
  owners: [],
  tags: [],
  provenance: "projetado",
};
```

New tests to add (cover these cases):

1. **Delete single — happy path**: clicking the action button opens the panel; clicking
   "Apagar" calls `window.confirm` and then `delete_transaction_cmd`; after confirm, the row
   is gone and the list reloads.
2. **Delete single — error**: `delete_transaction_cmd` rejects; the inline error alert appears
   and the row stays.
3. **Delete recurring — series flow**: for a row with id `"rec-uuid:2"`, the action panel
   shows "Apagar da série"; confirming calls `delete_series_from_cmd` with `{ transactionId: "rec-uuid:2" }`.
4. **Action panel closes on second click**: toggling the action button a second time closes
   the panel.

Model the test structure after the existing `"keeps the tag editor open..."` test at lines
114-137 of the current `TransactionsScreen.test.tsx`.

**7b — `src/screens/TotaisScreen.test.tsx`**: add one test after the existing ones:

```
it("mostra totais por titular quando há 2+ owners", async () => {
  // mock owner_totals_for_month_cmd to return two owners
  // assert both OwnerChip names are visible and their Money amounts appear
});
```

**Verify**: `npm run test:run -- TransactionsScreen` → all pass including N new tests.
`npm run test:run -- TotaisScreen` → all pass.

### Step 8: Full quality gate

Run `npm run check`. Fix any remaining issues before declaring done.

If the React Doctor reports new findings caused by this plan's code changes (e.g., inline
style objects created inline rather than hoisted), fix them to match the React Doctor
patterns documented in memory at `react-doctor-zero-fix-patterns.md` (hoist static style
objects above the component, or use `const` outside the component body).

**Verify**: `npm run check` → exit 0, all sub-checks pass.

## Test plan

New tests to write:

- **`src/screens/TransactionsScreen.test.tsx`** (add to existing file):
  - Delete single transaction: success path (confirms `delete_transaction_cmd` called).
  - Delete single transaction: error path (inline alert visible, row still present).
  - Delete recurring (series): `delete_series_from_cmd` called for recurring-id-format rows.
  - Action panel: closes on second toggle of the action button.
- **`src/screens/TotaisScreen.test.tsx`** (add to existing file):
  - Owner totals section renders when `owner_totals_for_month_cmd` returns 2+ entries.
  - Owner totals section absent when `owner_totals_for_month_cmd` returns `[]`.
- **Structural model**: follow `src/screens/TransactionsScreen.test.tsx` lines 114-137
  for the pattern (mock, render, userEvent.click, waitFor, assert alert).

**Verification command**: `npm run test:run` → all pass, including at least 6 new tests.

## Done criteria

ALL must hold before marking this plan DONE:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0 (including React Doctor — no new `deslop/unused-export`
      violations for the functions wired in this plan)
- [ ] `npm run test:run` exits 0; at least 6 new test cases added covering delete-single,
      delete-series, action-panel-toggle, owner-totals-shown, owner-totals-hidden
- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests)
- [ ] `grep -n "UI pendente" src/lib/api.ts` returns no lines for `deleteSeriesFrom`,
      `deleteSeriesAll`, `updateSeriesFrom`, `updateSeriesAll`, `ownerTotalsForMonth`
      (the five functions actively called by this plan's UI changes)
- [ ] `git status` shows only files in the in-scope list modified
- [ ] `plans/README.md` status row for plan 015 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- Any excerpt in "Current state" does not match the live file at the cited location
  (the codebase has drifted since this plan was written at `d183bbf`).
- `npm run rust:check` fails with a type error inside the new `delete_transaction_cmd` or
  `update_transaction_cmd` functions after two reasonable attempts to fix it — the `sqlx`
  query API or schema may have changed.
- The `source_amount IS NULL` guard in Step 1 causes the SQL to fail (e.g., the column
  was renamed or removed in a migration). Run `SELECT sql FROM sqlite_master WHERE name='transaction'`
  in a migration test to verify before implementing; if the column name differs, STOP.
- `npm run lint` reports a new `deslop/unused-export` error on `deleteSeriesAll` or
  `splitsForTransaction` after removing their disable comments, meaning no screen actually
  calls them — re-check the import wiring before removing the comments.
- The `useCommand` call for `ownerTotalsForMonth` in TotaisScreen triggers an "invalid hook"
  error because it is placed after a conditional return — restructure and STOP only if the
  restructuring requires touching out-of-scope files.
- Any step's verification fails twice after a reasonable fix attempt.
- Implementing the edit flow in `NewTransactionForm` requires touching the `kindToFields`
  logic in a way that breaks existing `NewTransactionForm` tests — reassess the approach
  and report before proceeding.

## Maintenance notes

- **`recurrence_id` not in `TransactionRow`**: this plan derives it from the id format
  (`"uuid:index"`). If the Rust command is later changed to emit `recurrence_id` as a
  separate field in `TransactionRow`, the frontend derivation logic should be removed.
- **Single-transaction import guard**: the `source_amount IS NULL` guard prevents users from
  deleting imported transactions via the app. If a "delete imported row" feature is added
  later, it needs a separate, clearly-named command with its own confirmation flow.
- **`window.confirm` for confirmations**: acceptable for P1 scope (no custom modal needed
  per PRODUCT.md preference for inline/contextual UI). A future polish pass could replace
  these with a proper inline confirmation component.
- **Owner totals section only appears for 2+ owners**: if the user is solo (no split
  transactions), the section is invisible. This matches the intent — the section is only
  meaningful when multi-owner splits exist.
- **`splitsForTransaction`**: this function (per-transaction split detail) is kept with its
  `react-doctor-disable` comment unless a per-row split breakdown panel is added in this
  plan. It is out of scope and should be wired in a follow-up alongside the split-detail
  panel (spec 017).
- **`createRecurringSeries` standalone export** (`api.ts` line 602): `createTransaction`
  already delegates to this internally when `recurrence != null`. The standalone export stays
  for future direct use. Its disable comment is correct and should NOT be removed by this plan.
- **PR reviewer focus**: verify that the `updateSeriesAll` / `updateSeriesFrom` user-facing
  prompt accurately reflects the "past is unchanged" semantic of `updateSeriesFrom`.
