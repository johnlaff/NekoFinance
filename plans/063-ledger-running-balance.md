# Plan 063: Show the chained day balance (Saldo) in the ledger's day headers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 290d538..HEAD -- src/screens/TransactionsScreen.tsx src/lib/api.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction (view parity with the reference spreadsheet)
- **Planned at**: commit `290d538`, 2026-07-03

## Why this matters

The reference spreadsheet shows a chained **Saldo** column next to every day —
you always see where the balance stands after each day's movements. The app's
ledger (Livro-razão, "Por mês" view) groups transactions by day but each day
header shows only the day's **net sum**, so the user cannot answer "what was my
balance after the 12th?" without leaving the screen. The engine already
computes the chained balance per day (`get_month_grid`); this plan surfaces it
in the day headers. This is a long-standing roadmap item confirmed by the
latest parity audit.

## Current state

- `src/screens/TransactionsScreen.tsx` — the ledger screen. The month view
  groups rows by day and renders a `GroupHeader` per day:

```tsx
// src/screens/TransactionsScreen.tsx:506-521
/** A sticky day-group header with net sum. */
function GroupHeader({
  title,
  today,
  sum,
}: {
  title: string;
  today: boolean;
  sum: number;
}) {
  return (
    <div className={"lc-gh" + (today ? " lc-gh--today" : "")}>
      <span className="lc-gh__t">{title}</span>
      <span className="lc-gh__sum">{fmtSigned(sum)}</span>
    </div>
  );
}
```

`Group` (lines 524-561) computes `sum` and renders `GroupHeader`. Find the
month-view grouping code by searching for `GroupHeader` call sites.

- `src/lib/api.ts:741` — the data source already exists:

```ts
export function getMonthGrid(year: number, month: number): Promise<MonthGridDay[]> {
```

`MonthGridDay` carries `date` (ISO `YYYY-MM-DD`), `day`, `income_cents`,
`fixed_out_cents`, `daily_out_cents`, `daily_projected_cents` and
`balance_cents` (the chained end-of-day balance, `null` when unknown). This
is the same command AnnualScreen and YearGridScreen already consume.

- Balance coloring convention: `saldoBand(cents)` from `src/lib/nkFormat.ts`
  returns `{ key, fill, text, label }` with the canonical ABSOLUTE thresholds
  (they must never be made relative). Use `band.text` for the balance text
  color — see `src/screens/AnnualScreen.tsx` ("Saldo fim" column) as the
  exemplar.
- Data fetching convention: `useCommand("<cmd>:<key>", fetcher)` from
  `src/lib/useCommand.ts`; screens re-fetch via `invalidateCommands()`. See the
  top of `TransactionsScreen.tsx` for existing `useCommand` calls to copy.
- Tests for this screen live in `src/screens/TransactionsScreen.test.tsx` and
  **freeze the clock** (`vi.useFakeTimers` + `vi.setSystemTime`) because the
  month view defaults to the current month. Follow that pattern exactly.

## Commands you will need

| Purpose   | Command                                                  | Expected on success |
| --------- | -------------------------------------------------------- | ------------------- |
| Install   | `npm ci`                                                 | exit 0              |
| Typecheck | `npm run typecheck`                                      | exit 0              |
| Lint      | `npm run lint`                                           | exit 0              |
| Unit test | `npx vitest run src/screens/TransactionsScreen.test.tsx` | all pass            |
| Full gate | `npm run check`                                          | exit 0              |

## Scope

**In scope** (the only files you should modify):

- `src/screens/TransactionsScreen.tsx`
- `src/screens/TransactionsScreen.test.tsx`
- `src/screens/lancamentos.css` (or wherever the `lc-gh` styles live — locate
  with `grep -rn "lc-gh" src/`) for a small balance style

**Out of scope** (do NOT touch):

- `src-tauri/` — no backend change; `get_month_grid` already returns the data.
- The "Linha do tempo" (timeline) view — balance is month-scoped; only the
  "Por mês" view gets it.
- `src/lib/nkFormat.ts` / `saldoHeatmap.ts` — thresholds are canonical.

## Git workflow

- Branch: `feat/063-ledger-running-balance`
- Conventional commits, e.g. `feat(lancamentos): show chained day balance in group headers`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Fetch the month grid in the month view

In `TransactionsScreen.tsx`, where the month view derives its year/month
(search for the state that drives "Por mês"), add:

```tsx
const gridQ = useCommand(`month_grid:${year}-${month}`, () =>
  getMonthGrid(year, month),
);
const balanceByDate = new Map((gridQ.data ?? []).map((d) => [d.date, d.balance_cents]));
```

Only fetch when the month view is active (mirror how the existing month-view
query is gated). Import `getMonthGrid` and the `MonthGridDay` type from
`../lib/api`.

**Verify**: `npm run typecheck` → exit 0.

### Step 2: Pass the day balance into Group/GroupHeader

Extend `Group` and `GroupHeader` with an optional `balance?: number | null`
prop. In the month-view render, look up `balanceByDate.get(isoDateOfGroup)`.
The day groups are keyed by date — confirm the group key format before wiring
(if the group title is not an ISO date, find where the ISO date is available).

In `GroupHeader`, render after the net sum:

```tsx
{
  balance !== undefined && balance !== null ? (
    <span className="lc-gh__saldo" style={{ color: saldoBand(balance).text }}>
      Saldo {fmtBRL(balance)}
    </span>
  ) : null;
}
```

Add a modest `.lc-gh__saldo` style next to the existing `lc-gh` styles
(smaller font, `margin-left: auto` alignment consistent with the current
header layout).

**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 3: Regression test

In `TransactionsScreen.test.tsx`, add a test in the existing month-view
describe block: mock `get_month_grid` to return one day matching a fixture
transaction's date with `balance_cents: 123_456`, render, and assert
`screen.getByText(/Saldo/)` and `screen.getByText(/1\.234,56/)` are in the
document. Mock every command the screen calls (copy the `mockCommands` set
from the nearest existing test).

**Verify**: `npx vitest run src/screens/TransactionsScreen.test.tsx` → all
pass, including the new test.

### Step 4: Full gate

**Verify**: `npm run check` → exit 0. Also run
`npm run e2e` if the environment has Playwright browsers; inspect the ledger
screenshot for layout overflow in the day header.

## Test plan

- New unit test (step 3): day header shows `Saldo R$ 1.234,56` colored by band.
- Edge case: a day with `balance_cents: null` renders no Saldo span (assert
  via `queryByText(/Saldo/)` absence with a null-balance mock).
- Pattern: model after the existing month-view tests in
  `TransactionsScreen.test.tsx` (frozen clock, `mockCommands`).

## Done criteria

- [ ] `npm run check` exits 0
- [ ] New tests pass; day headers in "Por mês" show the chained Saldo with
      saldoBand coloring; null balance renders nothing
- [ ] Timeline view unchanged (`git diff` shows no timeline-specific edits)
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

- The month-view day groups turn out not to have an ISO date available for the
  lookup (title-only grouping) — report the actual group key shape instead of
  guessing a date parse.
- `get_month_grid` returns balances that disagree with the AnnualScreen
  "Saldo fim" for the same month-end — that would be an engine bug; report it,
  do not "fix" it here.
- Any verification fails twice after a reasonable fix attempt.

## Maintenance notes

- If a day-level deep link is later added to the calendar (plan 065), the
  ledger's day groups are the natural target — keep group keys date-addressable.
- Reviewer: check the sticky header still fits at 320px width (the sum and
  Saldo may need to wrap or truncate).
