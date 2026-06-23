# Plan 057: Compose tags Cartão with the engine's "credit" payment method

> **Executor instructions**: Follow step by step; run every verification command and
> confirm the expected result. If a "STOP condition" occurs, stop and report. When done,
> update this plan's row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src/shell/Compose.tsx src/lib/movement.ts`
> On any change, compare the "Current state" excerpts to the live code before proceeding.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `da2d3e9`, 2026-06-22

## Why this matters

A lançamento created/edited as **Cartão** in the redesigned Compose drawer is silently
**misclassified by the forecast engine** as a *Diário* (daily variable) expense instead of a
card/fixed-out expense. The Compose maps the "cartao" chip to `paymentMethod: "credito"`
(Portuguese), but the rest of the system — `src/lib/movement.ts` and the Rust engine — keys
off the exact string `"credit"` (English). Money is therefore put in the wrong bucket, so
"custo de vida", cartão totals, and the forecast are wrong for anything entered via Compose.

## Current state

- `src/shell/Compose.tsx` line ~36–37, `mapType()` — the bug:
  ```tsx
  case "cartao":
    return { txnType: "expense", isFixed: false, paymentMethod: "credito" };
  ```
- `src/lib/movement.ts` line ~26–27 — the canonical mapping the rest of the app uses:
  ```ts
  case "cartao":
    return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
  ```
  and line ~45: `if (paymentMethod === "credit") return "cartao";`
- Rust engine `src-tauri/src/forecast/mod.rs` `classify()` (line ~249): an expense is routed
  to `FixedOut` (the Cartão bucket) only when `is_fixed || payment_method == Some("credit")`.
  `"credito"` matches neither → the expense falls to `Daily`.
- Convention: this is a pure mapping bug; the fix is to use the same literal `"credit"` that
  `movement.ts` and the engine already agree on.

## Commands you will need

| Purpose   | Command             | Expected on success     |
|-----------|---------------------|-------------------------|
| Typecheck | `npm run typecheck` | exit 0                  |
| Lint      | `npm run lint`      | exit 0                  |
| Unit test | `npm run test:run`  | all pass                |
| E2E       | `npm run e2e`       | all pass                |

## Scope

**In scope:**
- `src/shell/Compose.tsx`
- `src/shell/Compose.test.tsx` (create if absent) OR an existing Compose test file

**Out of scope (do NOT touch):**
- `src/lib/movement.ts` — already correct (`"credit"`); it is the reference, not the bug.
- The Rust engine — already correct.
- Any change to `paymentMethod` for non-cartão types (entrada/saida/diario/economia).

## Git workflow

- Branch: `advisor/057-compose-credit-payment-method`
- Message: `fix(compose): use 'credit' payment method for Cartão (was 'credito', misclassified)`

## Steps

### Step 1: Fix the literal

In `src/shell/Compose.tsx` `mapType()`, change the cartao case to:
```tsx
case "cartao":
  return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
```

**Verify**: `grep -n "credito" src/shell/Compose.tsx` → no matches. `grep -n 'paymentMethod: "credit"' src/shell/Compose.tsx` → one match (the cartao case).

### Step 2: Guard against regression with a test

Add a unit test that calls/exercises the Compose cartao mapping and asserts the produced
`paymentMethod` is `"credit"`. Two acceptable approaches:
- If `mapType` is exported (or can be exported without widening the public surface), test it
  directly: `expect(mapType("cartao").paymentMethod).toBe("credit")`.
- Otherwise, render Compose (with `mockCommands` + `NekoAppProvider`, pattern from
  `src/screens/DashboardScreen.test.tsx`), pick the Cartão chip, fill amount, click "Salvar
  lançamento", and assert the `createTransaction` mock was called with `paymentMethod: "credit"`.
  Prefer the direct `mapType` test if a small export is acceptable; keep the export minimal.

**Verify**: `npm run test:run` → all pass, including the new assertion.

## Test plan

- New test asserting `paymentMethod === "credit"` for the cartao path.
- Optional stronger test: after saving a Cartão via Compose, the created transaction carries
  `payment_method = "credit"` so the engine buckets it as FixedOut.
- Pattern: `src/screens/DashboardScreen.test.tsx` (render + provider + command mocks).

## Done criteria

- [ ] `grep -rn "credito" src/shell/Compose.tsx` → no matches
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; a test asserts the cartao→"credit" mapping
- [ ] `npm run e2e` exits 0
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

- `mapType` no longer exists or uses a different shape than `{ txnType, isFixed, paymentMethod }` → STOP.
- `movement.ts` turns out to also use `"credito"` (i.e. the discrepancy is the other way) → STOP and report; do not change movement.ts without confirmation.

## Maintenance notes

- The canonical payment-method strings live in `src/lib/movement.ts` and the Rust `classify()`.
  Any new movement-type mapping must reuse those literals (`"credit"`, `"debit"`, `null`).
  Consider, as a follow-up (out of this plan), having Compose import the mapping from
  `movement.ts` instead of duplicating `mapType`, to prevent this class of drift.
