# Plan 043: Edit/delete + UX integrity — imported-row edits, items on series, provenance, dead-ends

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
>   src-tauri/src/commands/transactions.rs \
>   src/screens/TransactionsScreen.tsx \
>   src/screens/NewTransactionForm.tsx \
>   src/screens/dashboard/DailyCheckinCard.tsx \
>   src/design-system/components/ProvBadge.tsx
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on a
> mismatch treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Package**: E
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

The ledger shows Editar/Apagar on every row, but the backend silently rejects
those actions for all imported rows, leaving the user stranded with no
explanation. Imported rows are the majority of the data (they come from the
spreadsheet) — so the action panel is effectively broken for most entries.
A related inconsistency: `update_transaction_items_cmd` already lets the user
edit line items on imported rows (this was deliberate in plan 036), but
`update_transaction_cmd` and `delete_transaction_cmd` still block everything
else. Additionally, a future occurrence entered manually shows "Previsto … que
o app criou", describing a user action as automatic; the "conciliado"
provenance exists in the frontend type system but is never emitted by the
backend; and the Economia chip in the check-in card is disabled with no
navigation path. Fixing all five issues removes dead-ends and establishes
honest, coherent UX for the edit/delete flow.

## Current state

### File roles

- `src-tauri/src/commands/transactions.rs` — Tauri commands for
  list/create/update/delete transactions; contains the guard that blocks
  editing and deleting imported rows (lines 452–517) and the provenance
  derivation (lines 197–203).
- `src/screens/TransactionsScreen.tsx` — the Livro-razão table; renders the
  three-dot action panel (`ActionPanelRow`, lines 366–403) with Editar/Apagar
  on every row regardless of provenance.
- `src/screens/NewTransactionForm.tsx` — inline edit/create form; the
  `LineItemEditor` guard at line 675 hides line-item editing for series items
  but not for imported rows.
- `src/screens/dashboard/DailyCheckinCard.tsx` — quick-add card; `KindSelector`
  disables the Economia chip (lines 214–245) with a tooltip but no link.
- `src/design-system/components/ProvBadge.tsx` — renders provenance badges;
  "conciliado" is absent from the `PROV` map (type on line 11 is
  `"importado" | "manual" | "projetado"`) and the backend never emits it.

### Key excerpts (verified at planned SHA d3922d2)

**`transactions.rs` — guard blocks ALL imported rows for edit (lines 492–516)**

```rust
// transactions.rs:492-516
let affected = sqlx::query(
    r#"UPDATE "transaction"
       SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
           is_fixed = ?6, date = ?7, updated_at = ?8
       WHERE id = ?1 AND source_amount IS NULL"#,
)
...
if affected == 0 {
    return Err(
        "lançamento não encontrado ou importado da planilha (não pode ser editado pelo app)"
            .into(),
    );
}
```

**`transactions.rs` — same guard blocks delete (lines 457–469)**

```rust
// transactions.rs:455-469
pub async fn delete_transaction_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let affected =
        sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL"#)
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| format!("delete: {e}"))?
            .rows_affected();
    if affected == 0 {
        return Err(
            "lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)"
                .into(),
        );
    }
```

**`transactions.rs` — provenance derived as binary (lines 197–203)**

```rust
// transactions.rs:197-203
provenance: if r.is_projection != 0 {
    "projetado".to_string()
} else if r.has_source != 0 {
    "importado".to_string()
} else {
    "manual".to_string()
},
```

There is no "conciliado" branch. The comment above `ProvBadge.tsx` (line 8)
confirms: `"conciliado/Conferido" foi removido até existir persistência de
reconciliação`. Until a `is_reconciled` column exists, this value cannot be
emitted correctly — see finding 3 in Scope.

**`TransactionsScreen.tsx` — ActionPanelRow renders Editar/Apagar unconditionally (lines 366–403)**

```tsx
// TransactionsScreen.tsx:366-403
function ActionPanelRow({ t, actionError, onEdit, onDeleteOne, onDeleteSeries }) {
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        ...
        <div style={ACTION_PANEL_STYLE}>
          <Button size="sm" variant="ghost" onClick={onEdit}>
            Editar
          </Button>
          {recurrenceIdOf(t.id) ? (
            <Button size="sm" variant="ghost" onClick={onDeleteSeries}>
              Apagar da série
            </Button>
          ) : (
            <Button size="sm" variant="ghost" onClick={onDeleteOne}>
              Apagar
            </Button>
          )}
        </div>
      </td>
    </tr>
  );
}
```

**`NewTransactionForm.tsx` — LineItemEditor hidden for series items but shown for imported rows (line 675)**

```tsx
// NewTransactionForm.tsx:673-681
{
  /* Detalhamento em partes (plano 036): só fora da Economia e fora de séries recorrentes
    (itens são por-instância). Vale para novo, passado e previsto. */
}
{
  itemsEnabled && !initialValues?.recurrence_id && (
    <LineItemEditor
      items={items}
      onChange={(next) => dispatch({ type: "setItems", items: next })}
      disabled={busy}
    />
  );
}
```

The comment says "passado e previsto" — but the guard is only `!recurrence_id`,
not `source_amount IS NULL`. So an imported-row edit CAN reach `LineItemEditor`
and succeeds (via `update_transaction_items_cmd` which explicitly lacks the
`source_amount` guard). The inconsistency is that imported rows _can_ edit
items but _cannot_ edit the scalar fields — same edit form, split behaviour.

**`ProvBadge.tsx` — "conciliado" absent from type and PROV map (lines 11–38)**

```tsx
// ProvBadge.tsx:11-38
// Nota: "conciliado/Conferido" foi removido até existir persistência de reconciliação.
// ...
type Prov = "importado" | "manual" | "projetado";

const PROV: Record<Prov, { label: string; dot: string; entry: GlossaryEntry }> = {
  importado: { ... },
  manual: { ... },
  projetado: {
    label: "Previsto",
    dot: "var(--secondary)",
    entry: {
      title: "Previsto",
      body: "Ainda não aconteceu. É uma previsão que o app criou para completar o futuro. ...",
    },
  },
};
```

The "projetado" body says "que o app criou" — correct for projection series
created by the recurrence engine. But a user-entered manual future transaction
(single, no `recurrence_id`, date in the future) also gets `is_projection=1`
from `create_transaction_inner` (line 329: `let is_projection = start >
chrono::Local::now().date_naive()`) and therefore inherits the "app criou"
copy, which is inaccurate for a user-entered future obligation.

**`DailyCheckinCard.tsx` — Economia chip disabled, no navigation (lines 214–245)**

```tsx
// DailyCheckinCard.tsx:214-245 (KindSelector)
const economiaDisabled = k === "economia";
...
<button
  ...
  disabled={economiaDisabled}
  title={
    economiaDisabled
      ? "Economia precisa de uma conta-destino — registre em Lançamentos."
      : undefined
  }
  ...
>
```

The tooltip text says "registre em Lançamentos" but provides no affordance to
navigate there. The user must know on their own where to go.

### Schema context

`source_amount` (column in `transaction`): `NULL` = manually created in the
app, never seen on the spreadsheet. `NOT NULL` = the row was present in the
last import (the value is the raw imported amount, used as the merge base).
There is no `is_reconciled` / `reconciled_at` flag in the schema (confirmed
by checking all migrations through `20260620000002`). Adding one would be the
prerequisite for emitting "conciliado" — see finding 3, deferred below.

### Repo conventions that apply

- Rust error strings are user-visible; match the existing Portuguese style.
- React Compiler ON: no manual `useMemo`/`useCallback`; static style objects
  hoisted to module scope (see `ACTION_PANEL_STYLE` etc. in `TransactionsScreen.tsx`).
- Money = positive-magnitude integer cents; direction from type.
- `functional-core/imperative-shell`: the `source_amount IS NULL` removal must
  land in the SQL layer; no logic in the UI.
- Test pattern for Rust: see `mod tests` in `transactions.rs` (lines 519–686)
  — in-memory pool, `sqlx::migrate!`, `insert_txn` helper.
- Test pattern for React: see `NewTransactionForm.test.tsx` — `@testing-library/react`
  - `userEvent`, `vi.mock("@tauri-apps/api/core")`, `mockCommands`.

## Commands you will need

| Purpose      | Command              | Expected on success       |
| ------------ | -------------------- | ------------------------- |
| Rust checks  | `npm run rust:check` | exit 0, no warnings       |
| Typecheck    | `npm run typecheck`  | exit 0, no errors         |
| Lint         | `npm run lint`       | exit 0                    |
| Unit tests   | `npm run test:run`   | all pass                  |
| React Doctor | `npm run doctor`     | 0 issues reported         |
| Full gate    | `npm run check`      | exit 0                    |
| E2E smoke    | `npm run e2e`        | all pass / screenshots ok |

## Suggested executor toolkit

- Invoke `impeccable` or `neko-finance-design` if you need to adjust the
  disabled-state styling of the Economia chip or the inline notice on the
  action panel to match the design system tokens.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/transactions.rs`
- `src/screens/TransactionsScreen.tsx`
- `src/screens/NewTransactionForm.tsx`
- `src/screens/dashboard/DailyCheckinCard.tsx`
- `src/design-system/components/ProvBadge.tsx`
- `src/screens/TransactionsScreen.test.tsx` (create if absent, or extend)
- Rust `mod tests` in `src-tauri/src/commands/transactions.rs`

**Out of scope** (do NOT touch):

- `src-tauri/src/google_sheets/import.rs` — the import engine handles
  `source_amount`; this plan only lifts the edit/delete guard.
- `src-tauri/src/google_sheets/write_back.rs` — write-back is a separate
  concern; re-import reconciliation happens there already.
- `src-tauri/migrations/` — finding 3 (conciliado) is explicitly deferred;
  no schema change in this plan.
- Any other screen, component, or Rust module not listed above.
- Pushing or opening a PR.

## Git workflow

- Branch: `advisor/043-edit-delete-ux-integrity`
- Commit style: `fix: <description> (plano 043)` — match the repo's
  conventional-commits with Portuguese body style (see recent log:
  `fix: correções P1 do Pacote D — projeção de Saldo, audit do lump de cartão, 1º-jan (plano 037)`).
- One commit per logical step is fine; squash into one before done if
  preferred — just make sure all checks pass on the final commit.
- Do NOT push or open a PR unless the operator says so.

## Steps

### Step 1: Allow edit/delete on imported rows (Rust)

**Goal**: remove `AND source_amount IS NULL` from `update_transaction_cmd` and
`delete_transaction_cmd`. The three-way merge on re-import will reconcile any
divergence; deleting an imported row will re-create it on the next import (this
is the expected behaviour — document it in the error path only if the
transaction truly cannot be found).

In `src-tauri/src/commands/transactions.rs`:

1a. In `delete_transaction_cmd` (line 457): change the SQL from

```rust
r#"DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL"#
```

to

```rust
r#"DELETE FROM "transaction" WHERE id = ?1"#
```

Update the error string at line 464 from the current
`"lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)"`
to simply
`"lançamento não encontrado"`.

1b. In `update_transaction_cmd` (line 492): change the WHERE clause from

```rust
WHERE id = ?1 AND source_amount IS NULL"#
```

to

```rust
WHERE id = ?1"#
```

Update the error string at line 511 similarly: remove the "ou importado"
clause.

**Important**: do NOT touch `update_transaction_items_cmd` — it already lacks
the guard intentionally (see plan 036 commentary at line 372–380).

**Verify**: `npm run rust:check` → exit 0, no warnings.

### Step 2: Add Rust regression tests for edit/delete on imported rows

In the `#[cfg(test)] mod tests` block in `transactions.rs`, add two new tests
after the existing ones (after line 686):

**Test 1** — `delete_imported_row_succeeds`: insert a transaction with a
`source_amount` value (simulating an imported row), call the delete SQL
directly, assert `rows_affected = 1` and the row is gone.

**Test 2** — `update_imported_row_succeeds`: insert a transaction with
`source_amount = 5000`, call the update SQL directly with `amount = 9900`,
assert `rows_affected = 1` and the new amount is `9900`.

Model these after `insert_txn` helper at line 533 — use `sqlx::query` to
insert the `source_amount` column explicitly (it is not in the current
`insert_txn` helper, so either add a new `insert_imported_txn` helper or
inline it in the test).

**Verify**: `npm run test:run` → all pass, including the 2 new Rust tests.

### Step 3: Pre-screen the action panel — show context for imported rows

**Goal**: replace the silent backend rejection with honest UX. When a row's
`provenance` is `"importado"`, the action panel should still show Editar and
Apagar (since Step 1 now allows them), but add a brief inline note explaining
re-import behaviour so the user understands what will happen.

`provenance` is already on `TransactionRow` (field `provenance: string`) and
arrives in `ActionPanelRow` via prop `t: TransactionRow`.

In `TransactionsScreen.tsx`, update `ActionPanelRow` to add a notice when
`t.provenance === "importado"`:

```tsx
// After the actionError paragraph and before the button row:
{
  t.provenance === "importado" && (
    <p style={IMPORTED_NOTICE_STYLE}>
      Linha importada da planilha — edições ficam no app; um re-import pode sobrescrever
      o valor se a planilha mudou. Apagar aqui não apaga da planilha; o próximo import
      restaura a linha.
    </p>
  );
}
```

Add `IMPORTED_NOTICE_STYLE` as a hoisted static constant (module scope,
alongside `ACTION_PANEL_STYLE`):

```tsx
const IMPORTED_NOTICE_STYLE: React.CSSProperties = {
  margin: "0 0 var(--space-2)",
  fontSize: "var(--fs-micro)",
  color: "var(--text-faint)",
};
```

**Verify**: `npm run typecheck` → exit 0. `npm run doctor` → 0 issues.

### Step 4: Allow line-item editing on recurring-series items

**Goal**: fix the visible-but-not-editable inconsistency for series items.

In `NewTransactionForm.tsx`, the guard at line 675 is:

```tsx
{itemsEnabled && !initialValues?.recurrence_id && (
  <LineItemEditor ... />
)}
```

Change it to always show `LineItemEditor` when `itemsEnabled` is true
(regardless of `recurrence_id`), but add a note for the series case explaining
that edits apply only to this occurrence:

```tsx
{
  itemsEnabled && (
    <>
      {initialValues?.recurrence_id && (
        <p style={HINT_TEXT}>
          Partes detalhadas se aplicam somente a esta ocorrência da série.
        </p>
      )}
      <LineItemEditor
        items={items}
        onChange={(next) => dispatch({ type: "setItems", items: next })}
        disabled={busy}
      />
    </>
  );
}
```

`HINT_TEXT` is already defined at line 109 in `NewTransactionForm.tsx` —
reuse it (do not add a new constant).

The submit path in `submit()` (lines 534–568) for recurring series (the `recId`
branch) currently does NOT call `updateTransactionItems` — that is intentional
per plan 036 ("Itens são por-instância → fora do escopo de série"). Since we
are now showing the editor for series items, we need to persist items for the
`updateSeriesFrom` / `updateSeriesAll` paths as well.

In the `if (recId)` branch (lines 534–568), after `updateSeriesAll` or
`updateSeriesFrom` resolves, add the same items call that exists in the
non-series path:

```tsx
// After the if(all)/else updateSeries calls, and before dispatch('submitSuccess'):
if (itemsActive) {
  await updateTransactionItems(initialValues.id, items);
}
```

**Verify**: `npm run typecheck` → exit 0. `npm run doctor` → 0 issues.

### Step 5: Fix "Previsto" copy for user-entered future obligations

**Goal**: `ProvBadge` currently shows "É uma previsão que o app criou para
completar o futuro" for ALL projetado rows, including ones manually entered by
the user. A manually entered future bill is not "app created".

The backend uses `is_projection = start > today` for ALL single transactions
(including manual ones) — there is no separate flag distinguishing
"user-created future" from "engine-created projection series". Introducing a
new DB column is out of scope.

The pragmatic fix is to soften the copy to be accurate for both cases:

In `src/design-system/components/ProvBadge.tsx`, change the `projetado` entry
body (line ~34) from:

```ts
body: "Ainda não aconteceu. É uma previsão que o app criou para completar o futuro. Vira real quando o lançamento de verdade chega.",
```

to:

```ts
body: "Ainda não aconteceu. Pode ser um compromisso que você registrou ou uma projeção automática. Vira real quando o lançamento de verdade chega.",
```

This wording is accurate for both user-entered future obligations and
engine-generated projection series, without requiring a schema change.

**Verify**: `npm run typecheck` → exit 0.

### Step 6: Add navigation affordance to the disabled Economia chip

**Goal**: the Economia chip tooltip says "registre em Lançamentos" but there
is no link. Add a navigation callback prop to `DailyCheckinCard` so the
tooltip becomes actionable.

`DailyCheckinCard` already receives an `onLogged` callback. Add a new optional
prop `onGoToTransactions?: () => void` alongside it.

In `DailyCheckinCard.tsx`:

6a. Add `onGoToTransactions?: () => void` to the props of `DailyCheckinCard`
and pass it into `KindSelector`.

6b. `KindSelector` currently takes `{ kind, onSelect }`. Add
`onGoToLancamentos?: () => void` to `KindSelector`'s props.

6c. In `KindSelector`, when the Economia button is clicked while disabled
(replace the dead `disabled` click-block with an `onClick` that calls
`onGoToLancamentos?.()` if provided), OR keep `disabled` and instead render
an anchor/button link below the chip row:

Preferred pattern (keep the chip disabled for clarity, add a small link below
the chip row when `onGoToLancamentos` is provided):

```tsx
// Below the chips div, inside KindSelector, only when onGoToLancamentos is set:
{
  onGoToLancamentos && (
    <button type="button" onClick={onGoToLancamentos} style={ECONOMIA_LINK_STYLE}>
      Registrar Economia → Lançamentos
    </button>
  );
}
```

Add `ECONOMIA_LINK_STYLE` as a hoisted constant in `DailyCheckinCard.tsx`:

```tsx
const ECONOMIA_LINK_STYLE: CSSProperties = {
  marginTop: "var(--space-1)",
  padding: 0,
  border: 0,
  background: "transparent",
  color: "var(--primary)",
  cursor: "pointer",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-micro)",
  textDecoration: "underline",
};
```

6d. In the parent that renders `DailyCheckinCard`, locate where the prop
`onLogged` is passed and add `onGoToTransactions`. The dashboard screen renders
`DailyCheckinCard` — find the call site (grep for `DailyCheckinCard` in
`src/screens/dashboard/`) and add the prop, wiring it to whatever
tab-navigation mechanism the shell uses (look for the `onGoToSettings` pattern
in `TransactionsScreen`).

**If the navigation callback cannot be cleanly threaded** without modifying an
out-of-scope file, use the fallback: change the disabled chip `title` tooltip
to a richer string (e.g. `"Economia precisa de uma conta-destino — abra a aba
Lançamentos e use o form completo."`) and document in a TODO comment that a
proper `onGoToTransactions` prop should be added when the shell navigation API
is available. This avoids touching an out-of-scope shell file.

**Verify**: `npm run typecheck` → exit 0. `npm run doctor` → 0 issues.

### Step 7: Write frontend tests for the action-panel behaviour

Create or extend `src/screens/TransactionsScreen.test.tsx`:

- **Test A** — action panel shows notice for imported row: render the
  `ActionPanelRow` in isolation (or render `TransactionsScreen` with mocked
  data containing an imported row), open the action panel, assert the
  "Linha importada" notice text is visible.
- **Test B** — Editar button calls the edit callback even for an imported row
  (i.e. the button is not disabled): click Editar on an imported row, assert
  `onEdit` was called.
- **Test C** — action panel does NOT show the imported notice for a manual row.

Model test structure after `NewTransactionForm.test.tsx`
(`vi.mock("@tauri-apps/api/core")`, `mockCommands`, `@testing-library/react`).

**Verify**: `npm run test:run` → all pass, including 3 new frontend tests.

### Step 8: Full gate

Run the complete quality gate:

```
npm run check
```

Expected: exit 0. If `npm run e2e` is available in your environment, run it
too and inspect the screenshot artifacts for regressions in the Livro-razão
view.

## Test plan

### Rust tests (in `transactions.rs` mod tests)

| Test                                      | What it covers                                                      |
| ----------------------------------------- | ------------------------------------------------------------------- |
| `delete_imported_row_succeeds`            | regression: delete no longer blocked by `source_amount IS NOT NULL` |
| `update_imported_row_succeeds`            | regression: update no longer blocked by `source_amount IS NOT NULL` |
| Existing `update_transaction_items_cmd_*` | must still pass (no regression on the items path)                   |

### Frontend tests (in `TransactionsScreen.test.tsx`)

| Test                                 | What it covers                                      |
| ------------------------------------ | --------------------------------------------------- |
| `action panel shows imported notice` | the new IMPORTED_NOTICE_STYLE paragraph is rendered |
| `Editar fires for imported row`      | button is enabled and calls edit handler            |
| `no imported notice for manual row`  | the notice is conditional                           |

Structural pattern: `NewTransactionForm.test.tsx`.

**Verification command**: `npm run test:run` → all pass.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run test:run` exits 0; 2 new Rust tests and 3 new frontend tests exist and pass
- [ ] `npm run lint` exits 0
- [ ] `npm run doctor` reports 0 issues
- [ ] `npm run check` exits 0
- [ ] `grep -n "source_amount IS NULL" src-tauri/src/commands/transactions.rs` returns no matches
      (the guard is gone from both `delete_transaction_cmd` and `update_transaction_cmd`)
- [ ] `grep -n "que o app criou" src/design-system/components/ProvBadge.tsx` returns no matches
      (copy softened)
- [ ] `grep -n "registre em Lançamentos" src/screens/dashboard/DailyCheckinCard.tsx`
      either returns no matches (link added in its place) OR shows a richer tooltip
      (fallback path from Step 6d)
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to DONE (or IN PROGRESS if review pending)

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" does not match the excerpts
  (the codebase has drifted since this plan was written).
- `npm run rust:check` fails after Step 1 with errors unrelated to the guard
  removal (suggests a schema or import change that affects these commands).
- The `delete_transaction_cmd` or `update_transaction_cmd` functions have
  already been moved to a different file (plan 011 split the god-module — verify
  the functions are still in `src-tauri/src/commands/transactions.rs`).
- Step 6d requires modifying a shell/nav file that is not in the in-scope list
  and you cannot use the fallback tooltip approach cleanly.
- Any step's verification fails twice after a reasonable fix attempt.
- The fix for any finding appears to require a schema migration (a new column)
  — that would be a separate plan; stop and report instead of adding a migration.

## Maintenance notes

- **Re-import behaviour after this plan**: deleting an imported row in the app
  will cause it to be re-created on the next import (because the spreadsheet
  still has the row). This is by design — the spreadsheet is the source of
  truth. The inline notice in `ActionPanelRow` explains this to the user.
- **"conciliado" provenance (finding 3) deferred**: adding "conciliado" to
  `ProvBadge` requires a schema column (`is_reconciled` or `reconciled_at` on
  the `transaction` table) and a write path that sets it after write-back
  approval. The `ProvBadge.tsx` comment at line 8 documents this exactly.
  Do not add "conciliado" to the PROV map until the backend can emit it
  accurately; a stale or always-false badge is worse than no badge.
- **Series + line items (Step 4)**: `updateTransactionItems` called after
  `updateSeriesFrom`/`updateSeriesAll` uses `initialValues.id` (the specific
  occurrence id) — not the series id (`recId`). This is intentional: line
  items are per-occurrence. If a future plan introduces series-level item
  templates, this call site must be revisited.
- **`onGoToTransactions` prop (Step 6)**: if the shell navigation API changes
  (e.g. a new `useNav` hook), the prop-threading in `DailyCheckinCard` may
  need updating.
- A reviewer should scrutinize:
  - The Rust WHERE clause changes in Step 1 (confirm no other callers relied
    on the guard for safety).
  - The items call added inside the `if (recId)` branch in Step 4 — it must
    use `initialValues.id`, not `recId`.
  - The `IMPORTED_NOTICE_STYLE` style constant is hoisted (not inline) to
    satisfy the React Doctor / React Compiler convention.
