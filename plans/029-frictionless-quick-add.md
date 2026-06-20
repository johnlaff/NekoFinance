# Plan 029: Frictionless daily quick-add (description + type selector + global shortcut)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan in
> `plans/README.md` unless a reviewer told you they maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src/screens/dashboard/DailyCheckinCard.tsx src/screens/NewTransactionForm.tsx src/shell/AppShell.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Category**: feature
- **Depends on**: none
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

The user's stated number-one pain is the friction of logging expenses every day. Currently, the
dashboard quick-add (`DailyCheckinCard`) hardcodes `description: null`, `paymentMethod: 'debit'`,
and `isFixed: false` — so every quick entry is anonymous and locked to the Diário type. To log a
credit charge, a fixed expense, or any described entry the user must navigate away to Lançamentos
and open the full `NewTransactionForm`. There is no keyboard shortcut to start an entry from any
screen; the only global shortcut is Ctrl/⌘-K for search. This makes Neko slower than typing
directly into the spreadsheet. The fix extends the dashboard card with an optional description
field and a compact 5-type selector (reusing the `kindToFields` mapping already proven in
`NewTransactionForm`), adds a global "N" shortcut to focus the amount field from any screen, and
preserves the zero-friction fast path (Diário/debit + today's date by default, amount clears
after submit, type+date retained).

## Current state

### `src/screens/dashboard/DailyCheckinCard.tsx` (verified at `bf92101`)

The component owns its state in three `useState` calls (lines 49–51):

```tsx
// file:DailyCheckinCard.tsx:49-51
const [amount, setAmount] = useState("");
const [busy, setBusy] = useState(false);
const [error, setError] = useState<string | null>(null);
```

The `logSpend` function (lines 62–90) calls `createTransaction` with hardcoded fields — no
description, no type selector:

```tsx
// file:DailyCheckinCard.tsx:70-79
await createTransaction({
  txnType: "expense",
  amountCents: cents,
  description: null,
  date: todayISO(),
  paymentMethod: "debit",
  isFixed: false, // Diário = variável, débito/dinheiro
  tagIds: [],
  recurrence: null,
});
```

The input row (lines 182–201) contains only one `<input>` (amount) and a button — no description
field, no type chips:

```tsx
// file:DailyCheckinCard.tsx:182-201
<div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
  <input
    aria-label="Gasto de hoje no débito, PIX ou dinheiro"
    inputMode="decimal"
    placeholder="Gasto de hoje — débito, PIX ou dinheiro (R$)"
    value={amount}
    onChange={(e) => setAmount(e.target.value)}
    onKeyDown={(e) => {
      if (e.key === "Enter" && canSubmit) void logSpend();
    }}
    style={DAILY_INPUT_STYLE}
  />
  <Button variant="primary" disabled={!canSubmit} onClick={() => void logSpend()}>
    {busy ? "…" : "Registrar"}
  </Button>
</div>
```

The component docstring (line 37) says "O form completo (tipo/tags/Repetir) fica nas Transações"
— this plan partially un-does that constraint by adding a compact type selector and description
directly here.

### `src/screens/NewTransactionForm.tsx` (verified at `bf92101`)

Contains the canonical `kindToFields` mapping (lines 23–41) that converts the 5 movement types
to `{ txnType, isFixed, paymentMethod }`:

```tsx
// file:NewTransactionForm.tsx:23-41
function kindToFields(kind: MovKind): {
  txnType: "income" | "expense" | "transfer";
  isFixed: boolean;
  paymentMethod: string | null;
} {
  switch (kind) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: "debit" };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
    case "economia":
      return { txnType: "transfer", isFixed: false, paymentMethod: null };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: "debit" };
  }
}
```

`FORM_KINDS` (line 20) is the ordered list `["entrada", "saida", "diario", "cartao", "economia"]`.

The form uses a `useReducer` whose `submitSuccess` action (lines 215–223) resets volatile fields
while keeping type and date — this is the sequential-entry reset pattern to mirror:

```tsx
// file:NewTransactionForm.tsx:215-223
case "submitSuccess":
  // Reset dos campos voláteis; mantém tipo e data para lançamentos em sequência.
  return {
    ...s,
    amount: "",
    description: "",
    selectedTags: [],
    repeat: false,
    busy: false,
  };
```

`MovBadge` (imported from `../../design-system/components/MovBadge`) accepts `kind`, `showLabel`,
and `size` props and handles the sr-only name automatically — use it for the type chips.

### `src/shell/AppShell.tsx` (verified at `bf92101`)

The existing global keyboard handler (lines 107–117) registers Ctrl/⌘-K for search:

```tsx
// file:AppShell.tsx:107-117
useEffect(() => {
  const onKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      searchRef.current?.focus();
      searchRef.current?.select();
    }
  };
  window.addEventListener("keydown", onKeyDown);
  return () => window.removeEventListener("keydown", onKeyDown);
}, []);
```

`AppShell` receives `active` (the current screen) and `onNavigate` (to switch screens) but has
no ref or callback for the quick-add amount input — one must be threaded through.

### `src/design-system/components/MovBadge.tsx`

Exports `MovKind = "entrada" | "saida" | "diario" | "economia" | "cartao"` (line 8) and
`MovBadge` (line 51). The `KIND_META` record maps each kind to `{ token, glyph, name }` where
`token` is a CSS variable like `var(--type-diario)`.

### Repo conventions that apply here

- **React Compiler ON**: no manual `memo`/`useMemo`/`useCallback`. Hoist static `CSSProperties`
  objects to module scope (see `DAILY_INPUT_STYLE`, `DAILY_BAR_TRACK` in `DailyCheckinCard.tsx`).
- **Money = positive-magnitude integer cents**. Amount parsing uses `parseBRLToCents` from
  `src/lib/format.ts`.
- **Native `<dialog>` for modals** — the repo uses `showModal()` for the write-back confirm
  dialog (`src/features/sheets/WriteBackPreview.tsx:178`); use the same pattern for the
  global shortcut modal.
- **Test pattern**: `@testing-library/react` + `userEvent` + `vi.mock("@tauri-apps/api/core")` +
  the `mockCommands`/`mockInvoke` helpers from `src/test/commands.ts`. Model new tests after
  `DailyCheckinCard.test.tsx`.
- **Accessible type chips** use `role="radio"` inside a `role="radiogroup"` with `aria-pressed`
  on each button — see the `NewTransactionForm` kind-chip pattern (line 559–568).
- **Error display**: `role="alert"` paragraph, `color: "var(--danger-400)"` — copy the existing
  `DailyCheckinCard` pattern (lines 202–213).

## Commands you will need

| Purpose           | Command              | Expected on success                    |
| ----------------- | -------------------- | -------------------------------------- |
| Type-check        | `npm run typecheck`  | exit 0, zero errors                    |
| Lint              | `npm run lint`       | exit 0                                 |
| Unit tests        | `npm run test:run`   | all pass                               |
| React Doctor      | `npm run doctor`     | 0 new findings vs baseline             |
| Full gate         | `npm run check`      | exit 0                                 |
| E2E visual smoke  | `npm run e2e`        | all Playwright tests pass              |
| Playwright report | `npm run e2e:report` | opens HTML report for screenshot check |

## Suggested executor toolkit

- Use the `neko-finance-design` skill when choosing spacing/color tokens for new UI elements.
- Reference `src/screens/NewTransactionForm.tsx` as the structural exemplar for the type-selector
  chip pattern and the `kindToFields` import.

## Scope

**In scope** (the only files you should modify):

- `src/screens/dashboard/DailyCheckinCard.tsx` — add description input + type selector; extend
  `logSpend` to use `kindToFields`; expose an `amountRef` via the `onAmountRef` callback prop.
- `src/screens/dashboard/DailyCheckinCard.test.tsx` — update existing tests + add new cases.
- `src/shell/AppShell.tsx` — accept `onQuickAddRef` prop; add "N" shortcut handler that focuses
  the amount input (and, if not on the dashboard, navigates there first).
- `src/App.tsx` — wire the `amountRef`/`onQuickAddRef` bridge between `DailyCheckinCard` and
  `AppShell`.

**Out of scope** (do NOT touch, even though they look related):

- `src/screens/NewTransactionForm.tsx` — do not modify; only import from it.
- Any screen other than Dashboard and `AppShell`.
- Tags, RepeatControls, and the Economia `toAccountId` flow — too many fields for a quick-add.
  Defer to the full form in Lançamentos; note this as deferred.
- Batch/SUM parsing ("50+30+20 = …" expression eval in the amount field) — deferred; note in
  maintenance section.
- Mobile companion or any Tauri native UI.

## Git workflow

- Branch: `feat/029-frictionless-quick-add`
- Commit style matches the repo: `feat: <verb in Portuguese imperative> — plano 029` (see recent
  `git log`). Example: `feat: seletor de tipo e descrição no check-in diário — plano 029`
- One commit per logical step, or squash into one before PR — your choice.
- Do NOT push or open a PR unless instructed.

## Steps

### Step 1: Export `kindToFields` from `NewTransactionForm.tsx`

`kindToFields` is currently unexported (`function kindToFields` at line 23). Change the
declaration to `export function kindToFields` so `DailyCheckinCard` can import it without
duplicating the mapping.

**Verify**: `npm run typecheck` → exit 0. `grep -n "export function kindToFields"
src/screens/NewTransactionForm.tsx` returns one match.

---

### Step 2: Extend `DailyCheckinCard` state — add `kind` and `description`

Replace the three separate `useState` calls with a single small state object, or add two new
`useState` calls alongside the existing three — either style is fine as long as the React
Compiler sees no dependency issues. Recommended minimal addition:

```tsx
const [kind, setKind] = useState<MovKind>("diario"); // default = fast path
const [description, setDescription] = useState("");
```

Import `MovKind` from `../../design-system/components/MovBadge` and `kindToFields` from
`../NewTransactionForm`.

**Verify**: `npm run typecheck` → exit 0.

---

### Step 3: Add the compact type selector above the amount row

Insert a `role="radiogroup"` container labeled "Tipo de movimento" above the existing amount
`<input>`. Each of the 5 kinds gets a `<button type="button" role="radio"
aria-checked={kind === k}` chip using `<MovBadge kind={k} showLabel size={14} />` inside it.

Hoist the chip base style object to module scope (React Compiler rule):

```tsx
// module scope — hoist, do not declare inside the component
const QUICK_KIND_BTN_BASE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "var(--space-2)",
  height: "var(--hit-min)",
  padding: "0 var(--space-2)",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "transparent",
};
```

The active chip merges `{ background: "var(--surface-selected)", borderColor: "var(--primary)" }`
inline (one computed object per render per chip — acceptable because it depends on active state).

Keep the `FORM_KINDS` order from `NewTransactionForm`: `["entrada", "saida", "diario", "cartao",
"economia"]`. When `kind === "economia"` add a short hint below the row: "Economia registra uma
transferência para a sua reserva." (mirrors the hint in `NewTransactionForm` at line 571–577).

**Verify**: `npm run typecheck` → exit 0. `npm run doctor` → 0 new findings (no inline styles
with 8+ props; hoisted objects are fine).

---

### Step 4: Add the optional description input

Insert a text `<input>` between the type selector and the amount row. It should be optional
(empty description → `description: null` on submit, matching the existing behavior).

```tsx
<input
  id="qac-desc"
  aria-label="Descrição (opcional)"
  placeholder="Descrição — ex.: mercado, aluguel…"
  value={description}
  onChange={(e) => setDescription(e.target.value)}
  onKeyDown={(e) => {
    if (e.key === "Enter") amountRef.current?.focus();
  }}
  style={DAILY_INPUT_STYLE} // reuse existing style constant
/>
```

Enter in the description field moves focus to the amount field (tab-order shortcut for speed).

**Verify**: `npm run typecheck` → exit 0.

---

### Step 5: Wire `kindToFields` into `logSpend`; update reset behavior

Replace the hardcoded fields in the `createTransaction` call with the mapped values:

```tsx
// inside logSpend, before the try block:
const fields = kindToFields(kind);

// inside createTransaction call:
await createTransaction({
  txnType: fields.txnType,
  amountCents: cents,
  description: description.trim() || null,
  date: todayISO(),
  paymentMethod: fields.paymentMethod,
  isFixed: fields.isFixed,
  tagIds: [],
  recurrence: null,
  toAccountId: null, // economia via quick-add: no account picker, always null
});
```

After a successful submit (currently `setAmount("")`), also clear description but keep kind:

```tsx
setAmount("");
setDescription("");
// kind is intentionally retained so sequential entries share a type
```

Note: `toAccountId: null` means an Economia quick-add creates a bare transfer with no
destination pocket. This is intentional for the quick path — the user can use the full form
for a named-account Economia. Add a comment to that effect.

**Verify**: `npm run typecheck` → exit 0. `npm run test:run` → all existing tests pass.

---

### Step 6: Expose `amountRef` so `AppShell` can focus it

Add an optional `onAmountRef` callback prop to `DailyCheckinCard`:

```tsx
export function DailyCheckinCard({
  summary,
  monthAvgCents = 0,
  onLogged,
  onAmountRef,
}: {
  summary: DashboardSummary;
  monthAvgCents?: number;
  onLogged: () => void;
  /** Called once on mount with a ref to the amount <input>; allows AppShell to focus it. */
  onAmountRef?: (ref: HTMLInputElement | null) => void;
});
```

Inside the component:

```tsx
const amountRef = useRef<HTMLInputElement>(null);

useEffect(() => {
  onAmountRef?.(amountRef.current);
  // intentionally no dep on onAmountRef — we call it once after first mount
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, []);
```

Attach `ref={amountRef}` to the amount `<input>`.

**Verify**: `npm run typecheck` → exit 0.

---

### Step 7: Register the "N" global shortcut in `AppShell`

Add an `onQuickAddRef` prop to `AppShell`:

```tsx
export function AppShell({
  active,
  onNavigate,
  onSearch,
  authStatus,
  children,
  onQuickAddRef, // NEW
}: {
  active: Screen;
  onNavigate: (screen: Screen) => void;
  onSearch: (query: string) => void;
  authStatus: AuthStatus;
  children: React.ReactNode;
  /** Caller sets this to pass a focus-amount callback; called when the user presses N. */
  onQuickAddRef?: (focusFn: (() => void) | null) => void;
});
```

Inside `AppShell`, maintain a `quickAddFocusRef = useRef<(() => void) | null>(null)` and
expose a setter so `App.tsx` can bridge the two:

```tsx
const quickAddFocusRef = useRef<(() => void) | null>(null);

// Let the caller register the focus function
useEffect(() => {
  onQuickAddRef?.((fn) => {
    quickAddFocusRef.current = fn;
  });
}, [onQuickAddRef]);
```

Extend the existing `onKeyDown` handler to also handle "N":

```tsx
useEffect(() => {
  const onKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      searchRef.current?.focus();
      searchRef.current?.select();
      return;
    }
    // "N" = quick-add: only when no modifier, not in an input/textarea/select
    if (
      e.key === "n" &&
      !e.metaKey &&
      !e.ctrlKey &&
      !e.altKey &&
      !(e.target instanceof HTMLInputElement) &&
      !(e.target instanceof HTMLTextAreaElement) &&
      !(e.target instanceof HTMLSelectElement)
    ) {
      e.preventDefault();
      quickAddFocusRef.current?.();
    }
  };
  window.addEventListener("keydown", onKeyDown);
  return () => window.removeEventListener("keydown", onKeyDown);
}, []);
```

The shortcut focuses the amount field. If the user is not on the dashboard, the focus call
silently no-ops (the ref is null) — the executor MAY optionally navigate to the dashboard first
and re-focus after a `requestAnimationFrame`, but do not add navigation as a requirement unless
it is trivial; a no-op is acceptable for the first iteration.

Show the shortcut hint in the header alongside the ⌘K hint. Add a `<kbd>` next to the search
bar: `<kbd className="ak-kbd" aria-hidden="true">N</kbd>` with a label "Novo lançamento (N)".

**Verify**: `npm run typecheck` → exit 0.

---

### Step 8: Wire the bridge in `App.tsx`

Open `src/App.tsx`. Find where `DailyCheckinCard` is rendered and where `AppShell` is rendered.

Add a module-level ref bridge (or a `useRef` at the `App` component level):

```tsx
const quickAddInputRef = useRef<HTMLInputElement | null>(null);
const quickAddFocusFnRef = useRef<((fn: (() => void) | null) => void) | null>(null);
```

Pass to `DailyCheckinCard`:

```tsx
onAmountRef={(el) => {
  quickAddInputRef.current = el;
  // If AppShell already registered its setter, push the focus fn immediately
  quickAddFocusFnRef.current?.(() => quickAddInputRef.current?.focus());
}}
```

Pass to `AppShell`:

```tsx
onQuickAddRef={(setter) => {
  quickAddFocusFnRef.current = setter;
  // If DailyCheckinCard already registered its input, push the fn now
  if (quickAddInputRef.current) {
    setter(() => quickAddInputRef.current?.focus());
  }
}}
```

This two-way handshake is order-independent: whichever mounts first stores its callback; the
second to mount pushes the function through.

**Verify**: `npm run typecheck` → exit 0. `npm run test:run` → all pass.

---

### Step 9: Update and extend the unit tests

Edit `src/screens/dashboard/DailyCheckinCard.test.tsx`:

1. **Update the existing "registra um Diário" test** — the `aria-label` on the amount input will
   change (it no longer says "no débito, PIX ou dinheiro" for all types). Update the query to
   `screen.getByLabelText(/Valor/)` or whatever label the executor chose for the amount field.

2. **Add: description is passed through on submit** — type a description, type an amount, click
   Registrar; assert `call?.[1].description === "mercado"`.

3. **Add: type selector produces the correct kindToFields mapping** — render the card, click the
   "Saída" chip, type an amount, submit; assert `isFixed: true` and `paymentMethod: "debit"`.
   Separately, click "Cartão", submit; assert `paymentMethod: "credit"` and `isFixed: false`.

4. **Add: description and amount reset after submit; kind is retained** — after a successful
   submit, assert the description input value is empty, the amount input value is empty, and the
   "Diário" (or whatever kind was selected) chip is still active.

5. **Add: Enter in description focuses amount** — use `userEvent.keyboard('{Enter}')` after
   focusing the description field; assert the amount input has focus.

Model the test structure and mocking after the existing `DailyCheckinCard.test.tsx` file.

**Verify**: `npm run test:run` → all pass, including the 5 new/updated tests.

---

### Step 10: Run the full gate + E2E

Run `npm run check` (typecheck + lint + tests + rust:check + privacy scan) and `npm run e2e`.

For E2E, inspect the Playwright screenshots/trace to confirm:

- The dashboard quick-add card shows the 5-type chips.
- A description input is visible below the type selector.
- Submitting a transaction with a description persists it (check the Lançamentos screen or
  re-inspect network/DB via the existing E2E helpers).

**Verify**: `npm run check` → exit 0. `npm run e2e` → all tests pass. `npm run doctor` → 0 new
findings.

---

### Step 11: Update `plans/README.md`

Add a row for plan 029 and set its status to DONE.

**Verify**: `grep "029" plans/README.md` returns the new row.

## Test plan

New tests all live in `src/screens/dashboard/DailyCheckinCard.test.tsx`.

| Case                                                      | Coverage                                                    |
| --------------------------------------------------------- | ----------------------------------------------------------- |
| `description is passed through on submit`                 | happy path — description field flows to `createTransaction` |
| `empty description submits null`                          | fast path — field empty → `description: null`               |
| `Saída chip produces isFixed:true paymentMethod:debit`    | type selector → `kindToFields` routing                      |
| `Cartão chip produces paymentMethod:credit isFixed:false` | credit type routing                                         |
| `description + amount reset after submit; kind retained`  | sequential-entry reset behavior                             |
| `Enter in description field focuses amount input`         | keyboard ergonomics                                         |

Structural pattern: `src/screens/dashboard/DailyCheckinCard.test.tsx` (existing file).

Run: `npm run test:run -- DailyCheckinCard` → 9+ tests pass (4 existing + the new ones above).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; `DailyCheckinCard.test.tsx` includes at least 6 tests covering
      description, type selector, and sequential-reset
- [ ] `npm run doctor` → no new findings vs pre-change baseline
- [ ] `npm run check` exits 0
- [ ] `npm run e2e` exits 0
- [ ] `grep -n "description: null" src/screens/dashboard/DailyCheckinCard.tsx` → no match (the
      hardcoded null is gone; it is now derived from the description field)
- [ ] `grep -n "paymentMethod: \"debit\"" src/screens/dashboard/DailyCheckinCard.tsx` → no match
      (hardcoded debit replaced by `kindToFields`)
- [ ] `grep -n "isFixed: false" src/screens/dashboard/DailyCheckinCard.tsx` → no match (same
      reason)
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row for 029 → DONE

## STOP conditions

- The code at the locations described in "Current state" does not match the excerpts — the
  codebase has drifted since this plan was written (drift check at the top found changes).
- `npm run test:run` fails on an existing test after a step and a single fix attempt does not
  restore it.
- The fix to `AppShell` appears to require modifying the `AppShell` prop interface in a way that
  breaks other callers not listed in scope — stop and report the additional callers found.
- `kindToFields` cannot be exported from `NewTransactionForm.tsx` without breaking another
  consumer (e.g. a barrel export collision) — report instead of duplicating.
- `npm run doctor` reports a new finding caused by this change that cannot be fixed by hoisting
  a style object or similar React Compiler–clean pattern.

## Maintenance notes

- **Economia via quick-add creates a transfer with `toAccountId: null`**: the backend must
  tolerate this (an Economia with no destination account). If the backend rejects it, either
  gate the Economia chip (show it grayed out in the quick-add with a tooltip directing the user
  to the full form), or add the `ReserveAccountPicker` to the card. That decision is out of
  scope here — if blocked, gate the chip.
- **Batch SUM parsing** ("50+30+20" in the amount field) was requested as a future accelerator
  and is deliberately deferred. When implemented, the amount field should evaluate the expression
  using a deterministic parser (no LLM) and display the resolved total before submit.
- **Auto-focus on dashboard load**: a simple `autoFocus` on the amount input was considered but
  skipped — it breaks focus management when the user navigates away and back. The "N" shortcut
  covers the same use case without stealing focus.
- **Global shortcut "N" conflicts** with any future text area added to non-dashboard screens that
  does not use an `<input>`/`<textarea>`/`<select>` element. Review the guard condition in
  `AppShell` if new interactive surfaces are added.
- A reviewer should verify in the PR diff that `kindToFields` export in `NewTransactionForm.tsx`
  is the only change to that file (no accidental reformatting or logic changes).
