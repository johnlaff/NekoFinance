# Plan 006: Calibrate "operate" phase + reserve baseline to the method

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src/screens/dashboard/colchaoPhase.ts src/screens/dashboard/colchaoPhase.test.ts src/screens/dashboard/ColchaoCard.tsx src-tauri/src/commands.rs CONTEXT.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The app currently gates the "Operar" phase — displayed on the ColchaoCard — at
`reserve_months >= 3`. The method's reserve floor is 6 months; 3–5 months is
the at-risk ("amarelo") zone, not the all-clear zone. A user with 4 months of
reserve wrongly sees the Operar badge and stops building their cushion. In
parallel, the monthly cost-of-living denominator used to derive `reserve_months`
aggregates ALL `type='expense'` rows, including transfers that the method does
not count as custo de vida. The reserve coverage is therefore understated (the
denominator is too large), compounding the early "you're fine" signal. Fixing
the gate threshold to 6 and aligning the denominator to `cost_of_living =
FixedOut + Daily` (excluding Economia/transfer rows, which are already
`type='transfer'` and therefore already excluded from the current query —
confirmed below) ensures the badge matches the method and CONTEXT.md stays
accurate.

## Current state

### Files and their roles

- `src/screens/dashboard/colchaoPhase.ts` — pure function that classifies the
  phase from `DashboardSummary` + `Forecast`; contains the buggy gate.
- `src/screens/dashboard/colchaoPhase.test.ts` — vitest unit tests for the
  pure function.
- `src/screens/dashboard/ColchaoCard.tsx` — React component; renders the phase
  badge and the tooltip string that says "≥ 3 meses" verbatim.
- `src-tauri/src/commands.rs` — Rust; contains `realized_monthly_baseline`
  (the denominator function), called from the `dashboard_summary` command to
  derive `reserve_months`.
- `CONTEXT.md` — canonical domain vocabulary; line 103 documents "reserve ≥ 3
  months" in the Phase definition, propagating the wrong threshold into docs.

### Verified code excerpts (re-confirm before editing)

**`src/screens/dashboard/colchaoPhase.ts` lines 1–24** (entire file):

```ts
import type { DashboardSummary, Forecast } from "../../lib/api";
import type { Phase } from "../../design-system/components/PhaseBadge";

/**
 * Fase de adaptação ao método derivada dos dados (não mais fixa em "calibrate"):
 * - "map": ainda mapeando — poucos lançamentos (<30) ou nenhum mês realizado.
 * - "operate": operando — economizado anual ≥ 20% E reserva ≥ 3 meses.
 * - "calibrate": no meio do caminho (o caso comum enquanto se ajusta o diário).
 *
 * Em módulo próprio (não no arquivo do componente) para não quebrar o Fast Refresh
 * (`only-export-components`).
 */
export function colchaoPhase(
  summary: DashboardSummary | null,
  forecast: Forecast,
): Phase {
  const txns = summary?.transaction_count ?? 0;
  const income = forecast.annual_savings.realized_income_cents;
  if (txns < 30 || income === 0) return "map";
  const economia = forecast.annual_savings.registered_economia_cents;
  const rateOk = economia * 10_000 >= income * 2_000;
  const reserveOk = (summary?.reserve_months ?? 0) >= 3; // ← BUG: should be 6
  return rateOk && reserveOk ? "operate" : "calibrate";
}
```

**`src/screens/dashboard/ColchaoCard.tsx` line 31** (tooltip title attribute):

```tsx
        <span title="Fases do método — Mapear: menos de 30 lançamentos. Calibrar: ajustando o diário. Operar: ≥ 20% economizado no ano e ≥ 3 meses de reserva.">
```

**`src-tauri/src/commands.rs` lines 572–602** (`realized_monthly_baseline`):

```rust
/// Gasto típico de um mês = MEDIANA da saída dos meses realizados COMPLETOS (anteriores ao mês
/// corrente), dos **últimos 6 meses** (recentes representam melhor o padrão atual que meses
/// antigos de anos anteriores — review ui-vs-planilha). Mediana para ser robusta a um mês atípico.
async fn realized_monthly_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT SUM(amount) FROM \"transaction\" \
         WHERE type='expense' AND substr(date,1,7) < ?1 \
         GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6",
    )
    ...
```

**`CONTEXT.md` line 103**:

```
**Phase** (adaptação): `map` (mapping — few lançamentos / no realized month) → `calibrate` (tuning the diário) → `operate` (Economizado% ≥ 20% and reserve ≥ 3 months). Derived from summary + forecast (`colchaoPhase`), not stored.
```

### Baseline denominator analysis

The SQL in `realized_monthly_baseline` already filters `WHERE type='expense'`,
which means `type='transfer'` rows (Economia → reserve/illiquid) are already
excluded. The method's `cost_of_living = FixedOut + Daily` maps exactly to
`type='expense'` rows (FixedOut = `is_fixed=1` or `payment_method='credit'`;
Daily = `is_fixed=0` non-credit). So the denominator query is already correct
with respect to excluding Economia transfers. **Part (b) of the finding (wrong
denominator) does not require a SQL change.** The fix is only to the phase gate
threshold and its documentation.

### Repo conventions

- Pure finance logic lives in dedicated modules (`colchaoPhase.ts`), not in
  component files — the existing pattern is already correct.
- React Compiler is enabled; do NOT add `useMemo`, `useCallback`, or `React.memo`.
- Prefer named constants over bare magic numbers — add `RESERVE_MIN_MONTHS`
  as a named export so callers and tests reference the symbol.
- TypeScript strict mode is on. All edits must pass `npm run typecheck` with
  zero errors.
- The tooltip text string in ColchaoCard.tsx at line 31 is user-visible copy;
  update it together with the logic to keep them in sync.
- CONTEXT.md is a checked-in domain vocabulary file. Update line 103 to reflect
  the corrected threshold.

## Commands you will need

| Purpose          | Command                                                                                                                                                                                        | Expected on success                    |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| Drift check      | `git diff --stat d183bbf..HEAD -- src/screens/dashboard/colchaoPhase.ts src/screens/dashboard/colchaoPhase.test.ts src/screens/dashboard/ColchaoCard.tsx src-tauri/src/commands.rs CONTEXT.md` | Only in-scope files changed (or empty) |
| Front unit tests | `npm run test:run`                                                                                                                                                                             | all pass, exit 0                       |
| Filtered tests   | `npx vitest run src/screens/dashboard/colchaoPhase.test.ts`                                                                                                                                    | all pass, exit 0                       |
| Typecheck        | `npm run typecheck`                                                                                                                                                                            | exit 0, no errors                      |
| Lint             | `npm run lint`                                                                                                                                                                                 | exit 0                                 |
| Full gate        | `npm run check`                                                                                                                                                                                | exit 0                                 |

## Scope

**In scope** (the only files you should modify):

- `src/screens/dashboard/colchaoPhase.ts` — raise operate gate; add `RESERVE_MIN_MONTHS` constant.
- `src/screens/dashboard/colchaoPhase.test.ts` — add regression cases for 3, 5, and 6 months.
- `src/screens/dashboard/ColchaoCard.tsx` — update tooltip string at line 31.
- `CONTEXT.md` — update line 103 ("≥ 3 months" → "≥ 6 months").

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/commands.rs` — the `realized_monthly_baseline` SQL is correct
  (already filters `type='expense'`; Economia/transfers are `type='transfer'` and
  therefore excluded). No change needed.
- `src/lib/saldoHeatmap.ts` — the saldo thermometer thresholds are correct
  absolute R$ bands matching the spreadsheet's conditional formatting. Do not touch.
- Any other file not listed above. The bug is fully contained in the four files listed.

## Git workflow

- Branch: `fix/006-phase-reserve-calibration`
- Commit style: conventional commits, matching recent history (e.g.
  `fix: calibrate operate phase gate to 6-month reserve floor`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add `RESERVE_MIN_MONTHS` constant and raise the gate in `colchaoPhase.ts`

Open `src/screens/dashboard/colchaoPhase.ts`.

Replace the jsdoc comment on line 7 that says
`"operate": operando — economizado anual ≥ 20% E reserva ≥ 3 meses.`
with `"operate": operando — economizado anual ≥ 20% E reserva ≥ 6 meses.`

Add a named constant before the function declaration:

```ts
/** Mínimo de meses de reserva para a fase "operate" (método: piso de 6 meses). */
export const RESERVE_MIN_MONTHS = 6;
```

Change line 22 from:

```ts
const reserveOk = (summary?.reserve_months ?? 0) >= 3;
```

to:

```ts
const reserveOk = (summary?.reserve_months ?? 0) >= RESERVE_MIN_MONTHS;
```

The full file after the edit should be:

```ts
import type { DashboardSummary, Forecast } from "../../lib/api";
import type { Phase } from "../../design-system/components/PhaseBadge";

/**
 * Fase de adaptação ao método derivada dos dados (não mais fixa em "calibrate"):
 * - "map": ainda mapeando — poucos lançamentos (<30) ou nenhum mês realizado.
 * - "operate": operando — economizado anual ≥ 20% E reserva ≥ 6 meses.
 * - "calibrate": no meio do caminho (o caso comum enquanto se ajusta o diário).
 *
 * Em módulo próprio (não no arquivo do componente) para não quebrar o Fast Refresh
 * (`only-export-components`).
 */

/** Mínimo de meses de reserva para a fase "operate" (método: piso de 6 meses). */
export const RESERVE_MIN_MONTHS = 6;

export function colchaoPhase(
  summary: DashboardSummary | null,
  forecast: Forecast,
): Phase {
  const txns = summary?.transaction_count ?? 0;
  const income = forecast.annual_savings.realized_income_cents;
  if (txns < 30 || income === 0) return "map";
  const economia = forecast.annual_savings.registered_economia_cents;
  const rateOk = economia * 10_000 >= income * 2_000;
  const reserveOk = (summary?.reserve_months ?? 0) >= RESERVE_MIN_MONTHS;
  return rateOk && reserveOk ? "operate" : "calibrate";
}
```

**Verify**: `npm run typecheck` → exit 0, zero errors.

### Step 2: Update tests in `colchaoPhase.test.ts`

Open `src/screens/dashboard/colchaoPhase.test.ts`.

The existing test fixture uses `reserve_months: 6`. The existing passing test
`"operates when registered Economia reaches 20% and reserve is ready"` already
covers the 6-month boundary correctly (it will still pass after Step 1).

Add three new `it` cases inside the existing `describe("colchaoPhase", ...)` block
that specifically cover the boundaries exposed by the bug:

1. **At 3 months: calibrate (regression)** — Economia meets 20% but 3 months of
   reserve must NOT yield "operate" after the fix.
2. **At 5 months: calibrate** — same, confirming the at-risk zone is still
   "calibrate".
3. **At exactly 6 months: operate** — confirms the floor itself grants "operate"
   when rate is also met (this is a boundary-value confirmation; the existing fixture
   already proves this but the explicit case documents intent).

Pattern to follow: the existing tests build `{ ...forecast, annual_savings: {
...forecast.annual_savings, registered_economia_cents: 200_000 } }` inline. Use
the same style; override `reserve_months` on the `summary` object. Import
`RESERVE_MIN_MONTHS` to keep the numbers tied to the constant:

```ts
import { colchaoPhase, RESERVE_MIN_MONTHS } from "./colchaoPhase";
```

New cases (add after the existing `it("operates when …")` block):

```ts
it("calibrates at 3 months even when rate is met — below method floor", () => {
  expect(
    colchaoPhase(
      { ...summary, reserve_months: 3 },
      {
        ...forecast,
        annual_savings: {
          ...forecast.annual_savings,
          registered_economia_cents: 200_000,
        },
      },
    ),
  ).toBe("calibrate");
});

it("calibrates at 5 months — at-risk zone, not yet at floor", () => {
  expect(
    colchaoPhase(
      { ...summary, reserve_months: 5 },
      {
        ...forecast,
        annual_savings: {
          ...forecast.annual_savings,
          registered_economia_cents: 200_000,
        },
      },
    ),
  ).toBe("calibrate");
});

it("operates at exactly RESERVE_MIN_MONTHS when rate is met", () => {
  expect(
    colchaoPhase(
      { ...summary, reserve_months: RESERVE_MIN_MONTHS },
      {
        ...forecast,
        annual_savings: {
          ...forecast.annual_savings,
          registered_economia_cents: 200_000,
        },
      },
    ),
  ).toBe("operate");
});
```

**Verify**: `npx vitest run src/screens/dashboard/colchaoPhase.test.ts` → all 5
tests pass, exit 0.

### Step 3: Update the tooltip in `ColchaoCard.tsx`

Open `src/screens/dashboard/ColchaoCard.tsx`.

At line 31, change the `title` attribute from:

```tsx
        <span title="Fases do método — Mapear: menos de 30 lançamentos. Calibrar: ajustando o diário. Operar: ≥ 20% economizado no ano e ≥ 3 meses de reserva.">
```

to:

```tsx
        <span title="Fases do método — Mapear: menos de 30 lançamentos. Calibrar: ajustando o diário. Operar: ≥ 20% economizado no ano e ≥ 6 meses de reserva.">
```

No other change in this file.

**Verify**: `grep -n "meses de reserva" src/screens/dashboard/ColchaoCard.tsx`
→ must show exactly one match containing `≥ 6 meses de reserva` and no match
containing `≥ 3 meses de reserva`.

### Step 4: Fix the threshold in `CONTEXT.md`

Open `CONTEXT.md`.

At line 103, change:

```
**Phase** (adaptação): `map` (mapping — few lançamentos / no realized month) → `calibrate` (tuning the diário) → `operate` (Economizado% ≥ 20% and reserve ≥ 3 months). Derived from summary + forecast (`colchaoPhase`), not stored.
```

to:

```
**Phase** (adaptação): `map` (mapping — few lançamentos / no realized month) → `calibrate` (tuning the diário) → `operate` (Economizado% ≥ 20% and reserve ≥ 6 months). Derived from summary + forecast (`colchaoPhase`), not stored.
```

**Verify**: `grep -n "reserve" CONTEXT.md` → line 103 must contain `≥ 6 months`
and must NOT contain `≥ 3 months`.

### Step 5: Run the full verification gate

**Verify**:

1. `npm run test:run` → all tests pass, exit 0 (at least 5 tests in colchaoPhase suite).
2. `npm run typecheck` → exit 0.
3. `npm run lint` → exit 0.
4. `grep -rn ">= 3" src/screens/dashboard/colchaoPhase.ts` → no matches (the magic number is gone).
5. `grep -rn "3 meses de reserva" src/screens/dashboard/ColchaoCard.tsx` → no matches.
6. `grep -n "3 months" CONTEXT.md` → no match on line 103 (the Phase definition line).
7. `git diff --name-only` → only the four in-scope files appear.

### Step 6: Commit

Stage exactly the four modified files and commit:

```
git add src/screens/dashboard/colchaoPhase.ts \
        src/screens/dashboard/colchaoPhase.test.ts \
        src/screens/dashboard/ColchaoCard.tsx \
        CONTEXT.md
git commit -m "fix: calibrate operate phase gate to 6-month reserve floor"
```

Then update the `plans/README.md` status row for plan 006 from `TODO` to `DONE`.

**Verify**: `git log --oneline -1` → shows the new commit message.

## Test plan

New test cases (all in `src/screens/dashboard/colchaoPhase.test.ts`):

| #   | Case                                        | Input                                                                      | Expected      |
| --- | ------------------------------------------- | -------------------------------------------------------------------------- | ------------- |
| 1   | Regression: 3 months + 20% rate             | `reserve_months: 3`, `registered_economia_cents: 200_000`                  | `"calibrate"` |
| 2   | At-risk zone: 5 months + 20% rate           | `reserve_months: 5`, `registered_economia_cents: 200_000`                  | `"calibrate"` |
| 3   | Boundary floor: exactly 6 months + 20% rate | `reserve_months: RESERVE_MIN_MONTHS`, `registered_economia_cents: 200_000` | `"operate"`   |

Existing tests to preserve (must still pass):

- `"does not operate from net surplus when registered Economia is below 20%"` → `"calibrate"` (reserve_months=6, economia=0)
- `"operates when registered Economia reaches 20% and reserve is ready"` → `"operate"` (reserve_months=6, economia=200_000)

Model the structural pattern after the existing tests in the same file: inline
spread of `summary` / `forecast` fixtures, no helper functions needed.

Run with: `npx vitest run src/screens/dashboard/colchaoPhase.test.ts`
Expected: 5 tests pass, 0 fail.

## Done criteria

ALL of the following must hold before marking the plan DONE:

- [ ] `npm run test:run` exits 0; `colchaoPhase.test.ts` contains exactly 5 `it(` blocks.
- [ ] `npm run typecheck` exits 0 with zero errors.
- [ ] `npm run lint` exits 0 with zero warnings/errors.
- [ ] `grep -n ">= 3" src/screens/dashboard/colchaoPhase.ts` returns no matches.
- [ ] `grep -n "RESERVE_MIN_MONTHS" src/screens/dashboard/colchaoPhase.ts` returns at least 2 matches (declaration + use).
- [ ] `grep -n "3 meses de reserva" src/screens/dashboard/ColchaoCard.tsx` returns no matches.
- [ ] `grep -n "3 months" CONTEXT.md` returns no match whose text is the Phase definition (line 103).
- [ ] `git diff --name-only HEAD~1` lists exactly the four in-scope files (and `plans/README.md` if updated in the same commit, otherwise a separate commit is fine).
- [ ] `plans/README.md` status row for plan 006 is `DONE`.

## STOP conditions

Stop and report back (do not improvise) if:

- The code at line 22 of `colchaoPhase.ts` does not read `>= 3` as shown in the
  excerpt — the file has drifted or was already partially fixed.
- The tooltip text in `ColchaoCard.tsx` at line 31 does not match the excerpt
  (the line number may have shifted; search for the string `meses de reserva`
  before editing).
- `npm run test:run` fails on any test BEFORE you make changes (pre-existing
  breakage unrelated to this plan).
- Step 5 verification 7 (`git diff --name-only`) shows files outside the four
  in-scope paths — the edit tool touched something unexpected.
- `npm run typecheck` reports an error related to `RESERVE_MIN_MONTHS` — likely
  because an import in another file already references the symbol and broke.
  (Do a `grep -rn "RESERVE_MIN_MONTHS" src/` before editing to confirm no prior
  references exist.)
- You find that `commands.rs` `realized_monthly_baseline` does NOT filter
  `type='expense'` exclusively (i.e. the SQL has changed to include transfers) —
  this would re-open finding (b) and is out of scope for this plan.

## Maintenance notes

- `RESERVE_MIN_MONTHS = 6` is now the single source of truth for the operate
  gate. If the method's recommendation changes (e.g. a reviewer decides 5 is
  correct for users below a certain income threshold), change only that constant.
- The tooltip in `ColchaoCard.tsx` is a free-form string and will drift from
  the constant if updated manually. A future cleanup (plan 014 or similar) could
  make the tooltip render the constant value dynamically.
- The `CONTEXT.md` entry for Reserve months (line 105) references
  `realized_monthly_baseline` and notes that it has no production writer for
  `reserve.current_months`. This remains correct — do not update line 105.
- Part (b) of the original finding (wrong denominator) is confirmed non-issue:
  the SQL already filters `type='expense'`, which excludes all `type='transfer'`
  rows (Economia). No Rust change is needed. Document this in the PR description
  so reviewers do not re-raise it.
- If a future plan adds an "amarelo/at-risk" visual state to the badge for users
  at 3–5 months, the constant `RESERVE_MIN_MONTHS` here remains the threshold for
  the "all clear" state; the at-risk band would be a separate constant.
