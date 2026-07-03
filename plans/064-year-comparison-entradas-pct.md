# Plan 064: Year comparison shows Entradas and Economizado% (not just absolute Economia)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 290d538..HEAD -- src/screens/AnnualScreen.tsx`
> If the file changed since this plan was written, compare the "Current state"
> excerpts against the live code before proceeding; on a mismatch, treat it as
> a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction (view parity with the reference spreadsheet)
- **Planned at**: commit `290d538`, 2026-07-03

## Why this matters

The reference spreadsheet's savings tab places two years side by side, and for
each month shows **Entradas, Economia and %** (economia ÷ entradas). The
percentage is the method's central savings metric (annual target 20–30%) — the
absolute Economia alone is misleading when income differs between years (R$500
saved on R$2.000 income beats R$800 on R$10.000). The app's "Comparar anos"
tab currently draws only absolute Economia bars, so the one comparison the
method actually teaches (did the savings _rate_ improve?) is missing.

## Current state

- `src/screens/AnnualScreen.tsx` — `AnoCmpSection` renders the comparison:

```tsx
// src/screens/AnnualScreen.tsx (search for "function AnoCmpSection")
const pairs = MES_ABBR.map((_, i) => ({
  a: getEcon(monthsA, i + 1),
  b: getEcon(monthsB, i + 1),
}));
const maxEcon = Math.max(...pairs.map((p) => Math.max(p.a, p.b)), 1);
// ... legend: "Economia guardada por mês"; rows of two bars (chart-3 / primary)
```

- `MonthMetric` (from `src/lib/api.ts`) already carries everything needed per
  month: `income_cents`, `economia_cents`, `savings_rate_bps` (basis points;
  `3000` = 30%). `monthsA`/`monthsB` are `MonthMetric[]` for the two years.
- Weighted-annual convention: the yearly Economizado% is
  `Σ economia / Σ entradas` — **never** the average of monthly rates. The
  existing regression test
  `src/screens/AnnualScreen.test.tsx` ("Economizado% anual é ΣEconomia/ΣEntradas
  (ponderado, não média das taxas)") pins this for the main table; mirror the
  same rule here.
- Money formatting: `fmtBRL`, `fmtCompact` (already imported in the file).
- Tests: `src/screens/AnnualScreen.test.tsx` — frozen clock
  (`vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"))`), `mockCommands`
  helper. Follow it.

## Commands you will need

| Purpose   | Command                                            | Expected on success |
| --------- | -------------------------------------------------- | ------------------- |
| Typecheck | `npm run typecheck`                                | exit 0              |
| Lint      | `npm run lint`                                     | exit 0              |
| Unit test | `npx vitest run src/screens/AnnualScreen.test.tsx` | all pass            |
| Full gate | `npm run check`                                    | exit 0              |

## Scope

**In scope**:

- `src/screens/AnnualScreen.tsx` (only `AnoCmpSection` and its helpers)
- `src/screens/AnnualScreen.test.tsx`
- The annual screen's CSS file (locate with `grep -rn "ano-cmp" src/`) for the
  new row/summary styles

**Out of scope**:

- The "Este ano" table and KPIs — already correct.
- `src-tauri/` — `get_annual_metrics` already returns both years' data.
- Changing the bar chart library/approach — extend the existing hand-rolled bars.

## Git workflow

- Branch: `feat/064-year-cmp-entradas-pct`
- Conventional commits, e.g. `feat(ano): comparar anos com Entradas e Economizado%`

## Steps

### Step 1: Add per-month % to each comparison row

In `AnoCmpSection`, extend `pairs` with the rates:

```tsx
const pairs = MES_ABBR.map((_, i) => {
  const mA = monthsA.find((m) => m.month === i + 1);
  const mB = monthsB.find((m) => m.month === i + 1);
  return {
    a: mA?.economia_cents ?? 0,
    b: mB?.economia_cents ?? 0,
    pctA: mA && mA.income_cents > 0 ? mA.savings_rate_bps / 100 : null,
    pctB: mB && mB.income_cents > 0 ? mB.savings_rate_bps / 100 : null,
  };
});
```

Render the two percentages at the end of each row (`—` when null), colored
`var(--chart-3)` / `var(--primary)` to match the legend. Keep the bars.

**Verify**: `npm run typecheck` → exit 0.

### Step 2: Add the year summary footer (Entradas · Economia · % ponderado)

Below the rows, add one summary line per year:

```
2025  Entradas R$ X · Economia R$ Y · Economizado 15%
2026  Entradas R$ X · Economia R$ Y · Economizado 22%
```

Weighted: `pct = Σ economia_cents / Σ income_cents` over months with data
(guard division by zero → render `—`). For the CURRENT year sum only months
`m.month - 1 <= currentMonthIdx` (mirror how `AnoTable` builds `realized` —
do not count empty future months as zeros in the weighted rate).

**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 3: Regression test

Add to `AnnualScreen.test.tsx` (new test in the existing describe): mock two
years where absolute Economia is HIGHER in year B but the rate is LOWER
(e.g. A: 30k/100k = 30%; B: 40k/400k = 10%). Switch to the "Comparar anos" tab
(find the tab's accessible name in the component — search for `Comparar`),
assert both `30%` and `10%` render, and assert the summary line contains the
weighted values.

**Verify**: `npx vitest run src/screens/AnnualScreen.test.tsx` → all pass.

## Test plan

- New test (step 3): rate renders per month and weighted per year; the
  higher-absolute/lower-rate year shows the lower % (the exact scenario the
  feature exists to expose).
- Edge: month without income → `—` (no division by zero, no `Infinity%`).
- Pattern: existing weighted-% test in the same file.

## Done criteria

- [ ] `npm run check` exits 0
- [ ] "Comparar anos" shows per-month % for both years and a per-year summary
      (Entradas, Economia, weighted %)
- [ ] Weighted rule verified by test (never average-of-rates)
- [ ] No files outside the in-scope list modified
- [ ] `plans/README.md` status row updated

## STOP conditions

- `get_annual_metrics` for the older year returns empty/missing months in a
  shape other than "12 entries or a sparse list keyed by month" — report the
  actual shape.
- The comparison tab is fed by a different command than the main table —
  report before adding any new fetch.
- Any verification fails twice.

## Maintenance notes

- If a third comparison metric is requested later (custo de vida per month),
  the row layout will need a redesign — do not keep appending columns.
- Reviewer: check the row fits at 320px; percentages may need `font-variant-numeric: tabular-nums`.
