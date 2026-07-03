# Plan 065: Calendar day cells show the day's movements and click through to the ledger

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 290d538..HEAD -- src/screens/YearGridScreen.tsx`
> If the file changed since this plan was written, compare the "Current state"
> excerpts against the live code before proceeding; on a mismatch, treat it as
> a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW–MED (interaction + a11y changes on an existing grid)
- **Depends on**: none (synergy with plan 063, not a dependency)
- **Category**: direction (view parity with the reference spreadsheet)
- **Planned at**: commit `290d538`, 2026-07-03

## Why this matters

In the reference spreadsheet, each day row shows Entrada, Saída, Diário AND
Saldo side by side — the balance is always read in the context of what moved
that day. The app's calendar (Saldo do dia screen) paints only the balance:
the day's movements are invisible and the cell is inert, so investigating "why
did the balance drop here?" requires manually navigating to the ledger. This
plan adds the day's movement values to the cell tooltip/aria and makes cells
clickable, landing on the ledger at that month.

## Current state

- `src/screens/YearGridScreen.tsx` — cell render (search for `cal-cell`):

```tsx
// src/screens/YearGridScreen.tsx (~line 285-320)
return (
  <div
    key={iso}
    className={
      "cal-cell" +
      (iso === TODAY ? " cal-cell--today" : "") +
      (future ? " cal-cell--future" : "")
    }
    style={{ background: band.fill === "transparent" ? "var(--surface)" : band.fill }}
    title={
      balance != null
        ? `Saldo ${fmtBRL(balance)}`
        : iso > TODAY
          ? "Projeção indisponível"
          : "Sem dados"
    }
  >
    <span className="cal-cell__d">{d}</span>
    {balance != null ? (
      <span className="cal-cell__s" style={{ color: band.text }}>
        {fmtCompact(balance)}
      </span>
    ) : (
      <span className="cal-cell__s" style={{ color: "var(--text-faint)" }}>
        —
      </span>
    )}
  </div>
);
```

- The data already contains the movements: the screen consumes
  `MonthGridDay[]` (via `getMonthGrid` for the "Mês" tab and `yearGridFetcher`
  for "Ano inteiro" — see `YearGridScreen.tsx:85-95`), and each `MonthGridDay`
  has `income_cents`, `fixed_out_cents`, `daily_out_cents` alongside
  `balance_cents`. Confirm what the cell loop currently receives: if it only
  receives a `balanceForDate(iso)` lookup, extend the lookup to the full day
  row (`dayForDate(iso): MonthGridDay | undefined`).
- Navigation: screens are switched by the shell. Find the mechanism with
  `grep -rn "useNekoApp\|NekoAppProvider\|onNavigate" src/shell src/screens | head`.
  Tests wrap screens in `NekoAppProvider` with `{ navigate, openCompose }` —
  so a `navigate("lancamentos")`-style call is available via the app context.
  Confirm the exact screen key for the ledger (search the shell's route map
  for the Lançamentos screen id).
- Vocabulary: use the method's five type names (Entrada, Saída, Diário) — the
  grid day carries income/fixed/daily; label them "Entrada", "Saída fixa",
  "Diário" as in `CONTEXT.md`.

## Commands you will need

| Purpose   | Command                                              | Expected on success                                |
| --------- | ---------------------------------------------------- | -------------------------------------------------- |
| Typecheck | `npm run typecheck`                                  | exit 0                                             |
| Lint      | `npm run lint`                                       | exit 0                                             |
| Unit test | `npx vitest run src/screens/YearGridScreen.test.tsx` | all pass (create the file if absent — check first) |
| Full gate | `npm run check`                                      | exit 0                                             |
| E2E smoke | `npm run e2e`                                        | all pass                                           |

## Scope

**In scope**:

- `src/screens/YearGridScreen.tsx`
- `src/screens/YearGridScreen.test.tsx` (create if it does not exist)
- The calendar CSS (locate with `grep -rn "cal-cell" src/`) — hover/focus
  affordance for the now-interactive cell

**Out of scope**:

- `src-tauri/` — no backend change.
- A day-level filter in the ledger (deep link lands on the MONTH; the day
  anchor is a documented follow-up).
- The thermometer colors/thresholds (`saldoBand`) — untouched.

## Git workflow

- Branch: `feat/065-calendar-day-clickthrough`
- Conventional commits, e.g. `feat(saldo): células do calendário com movimentos e clique para o Livro-razão`

## Steps

### Step 1: Extend the per-day lookup to the full MonthGridDay

Where the cell loop resolves `balanceForDate(iso)`, add/replace with a
`dayForDate(iso)` map built from the same fetched rows. Keep
`balance = day?.balance_cents ?? null` so existing rendering is unchanged.

**Verify**: `npm run typecheck` → exit 0.

### Step 2: Movement-aware tooltip and aria-label

Build the title/aria string from the day row, omitting zero values:

```
Saldo R$ 1.234,56 · Entrada R$ 500,00 · Saída fixa R$ 200,00 · Diário R$ 45,00
```

Fallbacks stay as today ("Projeção indisponível" / "Sem dados"). Set the same
string as `aria-label`.

**Verify**: `npm run lint` → exit 0.

### Step 3: Make the cell an accessible button that navigates to the ledger

Change the cell wrapper from `<div>` to `<button type="button">` (keyboard
focusable, native semantics — repo convention prefers native tags over
role attributes). `onClick` → navigate to the Lançamentos screen (exact
context/prop found in "Current state" recon). Empty cells
(`cal-cell--empty`) stay `<div>`. Add `:focus-visible` and hover styles
consistent with the app's existing interactive cells; ensure the button reset
does not break the grid layout (`display: flex` etc. — copy the current cell
styles onto the button).

**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 4: Tests

- If `YearGridScreen.test.tsx` does not exist, create it modeled on
  `src/screens/AnnualScreen.test.tsx` (frozen clock, `mockCommands`,
  `NekoAppProvider` wrapper — copy the provider usage from
  `TransactionsScreen.test.tsx`).
- Test A: a day with movements exposes them in the accessible name
  (`getByRole("button", { name: /Entrada R\$ 500,00/ })`).
- Test B: clicking a day cell calls `navigate` with the ledger screen key
  (spy via the provider mock).

**Verify**: `npx vitest run src/screens/YearGridScreen.test.tsx` → all pass.

### Step 5: Visual smoke

**Verify**: `npm run e2e` → pass; inspect the calendar screenshot — cells
must look identical at rest (the button must not introduce borders/margins).

## Test plan

- Tests A and B above; plus: empty cell renders no button role
  (`queryAllByRole("button")` count matches days-with-data).
- Pattern: `AnnualScreen.test.tsx` for mocking, `TransactionsScreen.test.tsx`
  for the provider.

## Done criteria

- [ ] `npm run check` exits 0; e2e smoke passes
- [ ] Day cells expose movements in tooltip and aria-label
- [ ] Clicking a data cell navigates to the ledger (test-proven)
- [ ] Visual parity at rest (screenshot inspected)
- [ ] `plans/README.md` status row updated

## STOP conditions

- The navigation context does not exist in `YearGridScreen` (screen not
  wrapped in the provider) — report the actual navigation mechanism used by
  sibling screens instead of inventing a prop.
- The "Ano inteiro" tab's fetcher turns out to return day rows WITHOUT the
  movement fields (only balances) — scope the tooltip to the "Mês" tab and
  report.
- e2e screenshot shows the grid layout broken by the button swap twice in a
  row after fix attempts.

## Maintenance notes

- Follow-up (deferred): day-level anchor in the ledger — when added, the
  `onClick` here should pass the ISO date through.
- Reviewer: keyboard-tab through a month; the focus ring must be visible on
  dark background (WCAG AA) and the aria-labels must not read as noise for
  every empty day.
