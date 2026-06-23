# Plan 056: Lançamentos opens on "Por mês" and lists it first

> **Executor instructions**: Follow this plan step by step. Run every verification
> command and confirm the expected result before moving on. If a "STOP condition"
> occurs, stop and report — do not improvise. When done, update this plan's row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src/screens/TransactionsScreen.tsx`
> If the file changed since this plan was written, compare the "Current state"
> excerpts to the live code before proceeding; on mismatch, treat as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `da2d3e9`, 2026-06-22
- **Completed**: 2026-06-23 via PR #88 (`24aaa8453dd15c09578973d8940655e322814707`)

## Why this matters

The "Lançamentos" (ledger) screen has two views — **Por mês** (grouped by month) and
**Linha do tempo** (flat timeline). The user's primary mental model is by-month, and they
expect it to be the **first option** in the toggle and the **default on open**. Today the
screen opens in timeline mode with "Linha do tempo" on the left, which is the reverse of
what the user wants.

## Current state

- `src/screens/TransactionsScreen.tsx` — the ledger screen. Two independent defects:
  - The `SegmentedControl` options list "Linha do tempo" first (lines 524–525):
    ```tsx
    { value: "anchor", label: "Linha do tempo" },
    { value: "monthOnly", label: "Por mês" },
    ```
  - The `useReducer` initial state defaults the view to `"anchor"` (line 832):
    ```tsx
    view: "anchor",
    ```
- The view union type is `"anchor" | "monthOnly"` (a `ViewMode`); no other change is
  needed — the rendering branch already handles both.
- Repo conventions: TS strict mode; components in `src/screens/`; tests are Vitest +
  Testing Library, colocated as `*.test.tsx`. The screen test (if present) is
  `src/screens/TransactionsScreen.test.tsx`.

## Commands you will need

| Purpose   | Command             | Expected on success     |
| --------- | ------------------- | ----------------------- |
| Typecheck | `npm run typecheck` | exit 0, no errors       |
| Lint      | `npm run lint`      | exit 0                  |
| Unit test | `npm run test:run`  | all pass                |
| E2E       | `npm run e2e`       | all pass (14 currently) |

## Scope

**In scope:**

- `src/screens/TransactionsScreen.tsx`
- `src/screens/TransactionsScreen.test.tsx` (create if absent)

**Out of scope (do NOT touch):**

- The `ViewMode` type, the reducer actions, the by-month / timeline rendering branches —
  they already work; only the order + default change.
- Any other screen.

## Git workflow

- Branch: `advisor/056-lancamentos-default-month-view`
- Conventional-commit message, e.g. `fix(lancamentos): default to "Por mês" view + list it first`.
- Do NOT push or open a PR unless instructed.

## Steps

### Step 1: Make "Por mês" the default view

In `src/screens/TransactionsScreen.tsx` line ~832, change the `useReducer` initial state:

```tsx
view: "monthOnly",
```

**Verify**: `grep -n 'view: "monthOnly"' src/screens/TransactionsScreen.tsx` → one match in the initializer.

### Step 2: List "Por mês" first in the toggle

In `src/screens/TransactionsScreen.tsx` lines ~524–525, swap the two option objects so
"Por mês" renders first:

```tsx
{ value: "monthOnly", label: "Por mês" },
{ value: "anchor", label: "Linha do tempo" },
```

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0.

### Step 3: Lock the default with a test

In `src/screens/TransactionsScreen.test.tsx`, add a test that renders the screen (with the
existing `mockCommands` helper from `src/test/commands.ts` and wrapped in `NekoAppProvider`
— follow the existing tests in this file or `src/screens/DashboardScreen.test.tsx` as the
structural pattern) and asserts that on first render the "Por mês" segment is the selected
one (e.g. it has `aria-checked="true"` / the active class) and appears before "Linha do tempo"
in the DOM.

**Verify**: `npm run test:run` → all pass, including the new test.

## Test plan

- New/updated test in `src/screens/TransactionsScreen.test.tsx`: "opens in Por mês view by
  default" — asserts the monthOnly segment is active on mount and is the first option.
- Pattern to follow: existing tests in the same file (or `DashboardScreen.test.tsx`) for the
  render+provider setup.
- Also confirm the e2e still passes (the Lançamentos e2e in `tests/e2e/app-shell.spec.ts`
  switches to Lançamentos and asserts content — it should be unaffected).

## Done criteria

- [x] `grep -n 'view: "monthOnly"' src/screens/TransactionsScreen.tsx` → matches the initializer
- [x] "Por mês" option object precedes "Linha do tempo" in the SegmentedControl options array
- [x] `npm run typecheck` exits 0
- [x] `npm run lint` exits 0
- [x] `npm run test:run` exits 0; the new default-view test exists and passes
- [x] `npm run e2e` exits 0
- [x] No files outside the in-scope list modified (`git status`)
- [x] `plans/README.md` status row updated

## STOP conditions

- The excerpts at lines ~524–525 / ~832 don't match (file drifted) → STOP and report.
- The reducer/type turns out to use different view identifiers than `"anchor"`/`"monthOnly"`.

## Maintenance notes

- If a third view is ever added, keep "Por mês" first and the default.
- Reviewer should confirm the default change didn't break the timeline view's own tests.
