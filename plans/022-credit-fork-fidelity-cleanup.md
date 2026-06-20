# Plan 022: Remove the credit-accumulator fork residue (stay faithful: credit is a lump)

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
> git diff --stat 51afe33..HEAD -- \
>   src/screens/MethodologyScreen.tsx \
>   src/lib/api.ts \
>   src/test/commands.ts \
>   src/screens/dashboard/DailyCheckinCard.test.tsx \
>   src/design-system/components/CardChip.tsx \
>   src/design-system/components/CardChip.test.tsx \
>   src/design-system/tokens/states.css \
>   src-tauri/src/forecast/mod.rs \
>   src-tauri/src/commands/forecast_cmds.rs
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `51afe33`, 2026-06-20

## Why this matters

The method treats credit as a single fatura lump on the due date — the
spreadsheet and the documented method behavior are consistent on this.
Neko's engine (`classify()` in `forecast_cmds.rs`) is already faithful to
this: it folds credit spend into a `FixedOut` lump at the card due day.
However, an earlier "credit accumulates daily" fork was partially abandoned,
leaving residue in four places: a MethodologyScreen card that actively
teaches the wrong model to users, a ghost API field pair (`credit_spend_month`
/ `has_credit`) that is computed but read by zero production components, jargon
("Régua 2") in comments and test names that implies a dual-tracking mechanism
that was never part of the method, and a `CardChip` component with "fatura
acumulada" framing that is imported by no production screen. Removing these
brings the codebase into line with the method, eliminates dead code paths that
generate confusion for every future contributor, and removes one user-facing
misinformation about how Neko tracks credit.

## Current state

### Files and their roles

- `src/screens/MethodologyScreen.tsx` — static card grid teaching the method;
  contains one card with misleading copy (lines 34–36).
- `src/lib/api.ts` — TypeScript types for Tauri IPC; declares
  `credit_spend_month` and `has_credit` on `DashboardSummary` (lines 33–35).
- `src-tauri/src/commands/forecast_cmds.rs` — Tauri command implementations;
  contains the `DashboardSummary` Rust struct (lines 882–893), the
  `credit_spend_month` query (lines 953–978), the `has_credit` query (lines
  980–990), and "Régua 2" jargon in comments (lines 300, 369, 817, 953).
- `src-tauri/src/forecast/mod.rs` — pure forecast engine; "Régua 2" jargon in
  a doc comment (line 20), a test comment (line 852), and a test function name
  `regua2_credit_lump_at_due_day` (line 855).
- `src/test/commands.ts` — test fixture factory; `SUMMARY` and `EMPTY_SUMMARY`
  both carry `credit_spend_month` and `has_credit` fields (lines 37–38, 48–49).
- `src/screens/dashboard/DailyCheckinCard.test.tsx` — unit test with a local
  `SUMMARY` fixture that carries `credit_spend_month` and `has_credit` (lines
  14–15).
- `src/design-system/components/CardChip.tsx` — card-chip UI component; JSDoc
  says "Régua 2 / fatura acumulada" (line 5–7); zero production imports.
- `src/design-system/components/CardChip.test.tsx` — unit test for CardChip
  only (5 cases; delete with the component).
- `src/design-system/tokens/states.css` — CSS custom properties; line 53 has a
  `/* ---- Pressão de fatura / crédito (Régua 2) ---- */` section comment.
- `tests/e2e/app-shell.spec.ts` — Playwright smoke; line 71 asserts
  `"Débito e crédito: dois ritmos"` text (title unchanged); line 70 asserts
  the body text `"Previsibilidade primeiro"` is visible — only the card _body_
  text changes in step 1, not the card _title_, so this assertion survives as-is.

### Key excerpts (verified at commit 51afe33)

**MethodologyScreen.tsx lines 34–36 — misleading body copy (to be reworded):**

```tsx
  {
    icon: Ruler,
    title: "Débito e crédito: dois ritmos",
    body: "Débito, PIX e dinheiro afetam o caixa no mesmo dia. O crédito acumula na fatura e só pesa no vencimento. O Neko acompanha os dois de forma independente: isso evita o autoengano de um diário \"zerado\" enquanto a fatura cresce em silêncio.",
  },
```

The phrase "O Neko acompanha os dois de forma independente: isso evita o
autoengano de um diário 'zerado' enquanto a fatura cresce em silêncio" implies
a live independent credit tracker that does not exist in the method or the
engine. It must be replaced.

**api.ts lines 28–39 — DashboardSummary TS type (ghost fields to remove):**

```ts
export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents (forecast engine, spec 003). */
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  credit_spend_month: number;
  /** Há rastreio de crédito (cartão ou gasto). `false` → mostrar "—" no tile, não R$0. */
  has_credit: boolean;
  reserve_months: number;
  reserve_trend: string;
  transaction_count: number;
}
```

**forecast_cmds.rs lines 882–893 — Rust DashboardSummary struct (fields to remove):**

```rust
#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub credit_spend_month: i64,
    /// Há rastreio de crédito (cartão ou gasto de crédito). `false` → a UI mostra "—" no tile,
    /// não um R$0 estrutural.
    pub has_credit: bool,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
}
```

**forecast_cmds.rs lines 953–978 — credit_spend query (to remove):**

```rust
    // Crédito no mês (Régua 2) como MAGNITUDE positiva, mesma regra do Diário: ...
    let month_start = format!("{}-01", today_naive.format("%Y-%m"));
    let month_end = forecast::last_day_of_month(today_naive.year(), today_naive.month())
        .format("%Y-%m-%d")
        .to_string();
    let credit_spend: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM \"transaction\" \
                                 WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                                   AND date >= ?1 AND date <= ?2) \
                     THEN ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                                        WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                                          AND date >= ?1 AND date <= ?2), 0)) \
                     ELSE COALESCE((SELECT SUM(credit_spend) FROM daily_checkin \
                                    WHERE date >= ?1 AND date <= ?2), 0) \
                END",
    )
    .bind(&month_start)
    .bind(&month_end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query credit spend: {e}"))?;
```

**forecast_cmds.rs lines 980–990 — has_credit query (to remove):**

```rust
    let has_credit: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM account WHERE type='credit_card') \
                  OR EXISTS(SELECT 1 FROM \"transaction\" WHERE payment_method='credit') \
                  OR COALESCE((SELECT SUM(credit_spend) FROM daily_checkin), 0) > 0 \
                THEN 1 ELSE 0 END",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query has_credit: {e}"))?;
```

**forecast_cmds.rs lines 1024–1034 — Ok(DashboardSummary { … }) (fields to remove):**

```rust
    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        daily_spend_today: daily_spend.0,
        credit_spend_month: credit_spend.0,   // remove
        has_credit: has_credit.0 != 0,         // remove
        reserve_months,
        reserve_trend: reserve_trend.0,
        transaction_count: count.0,
    })
```

**forecast_cmds.rs line 300 — "Régua 2" jargon in function doc:**

```rust
/// plus credit-cycle lumps aggregated from `daily_checkin` at the card due date (Régua 2).
```

**forecast_cmds.rs line 369 — "Régua 2" jargon in inline comment:**

```rust
                // Credit spend (Régua 2) → aggregate by due_date
```

**forecast_cmds.rs line 817 — "Régua 2" jargon in SQL query comment:**

```rust
        // Crédito (régua 2) entra em Saída como a fatura, não em Diário — espelha forecast::classify.
```

**forecast/mod.rs line 20 — "Régua 2" jargon in EventKind doc:**

```rust
    /// Saída — fixed outflow: fixed bills + the credit-invoice lump at the card due day (Régua 2).
```

**forecast/mod.rs lines 852–858 — "Régua 2" in test comment and function name:**

```rust
    // T6.2 / T6.3 — a credit lump (Régua 2) lands as one FixedOut on the due day, depressing the
    // future month, while débito daily (Régua 1) only touches its own day.
    #[test]
    fn regua2_credit_lump_at_due_day() {
        let events = [
            ev("2026-01-10", EventKind::Daily, 20000), // débito daily (Régua 1)
            ev("2026-02-15", EventKind::FixedOut, 600000), // invoice lump at due day (Régua 2)
```

**states.css line 53 — "Régua 2" in section comment:**

```css
/* ---- Pressão de fatura / crédito (Régua 2) ---- */
```

**CardChip.tsx lines 4–7 — "Régua 2 / fatura acumulada" JSDoc:**

```tsx
/**
 * CardChip — cartão de crédito (o 5º tipo: gasto com cartão / Régua 2). Face com gradiente da
 * marca, apelido, final, e a fatura acumulada. Portado do novo DS em inline-style (puro). `brand`
 * default = token do tipo Cartão.
 */
```

**test/commands.ts lines 37–38, 48–49 — ghost fields in fixtures:**

```ts
export const SUMMARY: DashboardSummary = {
  // ...
  credit_spend_month: 120000,
  has_credit: true,
  // ...
};

export const EMPTY_SUMMARY: DashboardSummary = {
  // ...
  credit_spend_month: 0,
  has_credit: false,
  // ...
};
```

**DailyCheckinCard.test.tsx lines 14–15 — ghost fields in local fixture:**

```ts
const SUMMARY: DashboardSummary = {
  // ...
  credit_spend_month: 0,
  has_credit: false,
  // ...
};
```

### Conventions to honor

- Money amounts are positive-magnitude integer cents; sign comes from the
  transaction type. `amount.abs()` is defense-in-depth only.
- React Compiler is ON: no manual `useMemo`/`useCallback`; hoist static styles
  to module-level `const`.
- Functional-core/imperative-shell: pure engine (`forecast/mod.rs`) is
  side-effect free; IO only in the shell (`forecast_cmds.rs`).
- Language in public-facing comments: method-neutral. Refer only to "the method"
  and "the spreadsheet"; do not reference any third-party product, course, or
  external analysis.
- The engine's `classify()` logic (credit spend → `FixedOut` lump at due date)
  is **faithful and must not be touched**. Only dead-code fields and misleading
  copy are in scope.

## Commands you will need

| Purpose            | Command              | Expected on success         |
| ------------------ | -------------------- | --------------------------- |
| Typecheck (TS)     | `npm run typecheck`  | exit 0, no errors           |
| Lint               | `npm run lint`       | exit 0                      |
| Unit tests         | `npm run test:run`   | all pass                    |
| Rust check + tests | `npm run rust:check` | exit 0                      |
| React Doctor scan  | `npm run doctor`     | no new findings vs baseline |
| Full gate          | `npm run check`      | exit 0                      |
| E2E visual smoke   | `npm run e2e`        | all tests pass              |

All commands run from the repo root.

## Scope

**In scope** (the only files you should modify):

- `src/screens/MethodologyScreen.tsx`
- `src/lib/api.ts`
- `src/test/commands.ts`
- `src/screens/dashboard/DailyCheckinCard.test.tsx`
- `src/design-system/components/CardChip.tsx` (delete)
- `src/design-system/components/CardChip.test.tsx` (delete)
- `src/design-system/tokens/states.css`
- `src-tauri/src/forecast/mod.rs`
- `src-tauri/src/commands/forecast_cmds.rs`

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/commands/forecast_cmds.rs` — the `classify()` / `map_cashflow_row` credit→`FixedOut` logic. That logic is faithful (credit lump lands as `FixedOut` at due date) and must not change.
- `src-tauri/src/commands/forecast_cmds.rs` — the `load_cashflow_events` credit aggregation loop (lines 330–385). That loop produces the faithful lump projection. Only the comment labels change (step 3), not the logic.
- `src/screens/dashboard/DailyCheckinCard.tsx` — production component; does not read the ghost fields.
- The `daily_checkin` table and its schema — that is plan 024.
- The 5th movement type "cartao" in the transaction form — it is faithful (a method movement type); keep it.
- `tests/e2e/app-shell.spec.ts` line 71 `"Débito e crédito: dois ritmos"` — that assertion matches the card _title_, which does not change. It does not need updating.

## Git workflow

- Branch: `advisor/022-credit-fork-fidelity`
- One commit per logical unit (copy fix, ghost API removal, jargon rename, dead
  component removal). Message style follows the repo's conventional commits,
  e.g. `fix: reword credit card body copy to the faithful lump framing`.
- Do NOT push or open a PR unless instructed.

## Steps

### Step 0: Pre-flight grep (safety check)

Before touching any file, confirm no production `.tsx` or `.ts` file (outside
tests and fixtures) imports `CardChip` or reads `credit_spend_month` /
`has_credit`:

```bash
grep -rn "credit_spend_month\|has_credit" src/ --include="*.tsx" --include="*.ts" \
  | grep -v "test\|\.test\.\|commands.ts\|api.ts"
```

Expected: **no output** (zero matches). If you see any production component
reading these fields, STOP and report — do not remove the fields.

```bash
grep -rn "CardChip" src/ --include="*.tsx" --include="*.ts" \
  | grep -v "CardChip\.tsx\|CardChip\.test\.tsx"
```

Expected: one match in `src/lib/format.ts` (a JSDoc comment listing `CardChip`
as an example user — not an import). Any `import.*CardChip` line is a STOP
condition.

**Verify**: both greps return zero `import` lines → proceed.

### Step 1: Reword the misleading method copy

File: `src/screens/MethodologyScreen.tsx`, the `PRINCIPLES` array entry at
lines 34–36 (the Ruler icon card).

Replace the `body` string of the "Débito e crédito: dois ritmos" card:

Old body (exact):

```
"Débito, PIX e dinheiro afetam o caixa no mesmo dia. O crédito acumula na fatura e só pesa no vencimento. O Neko acompanha os dois de forma independente: isso evita o autoengano de um diário \"zerado\" enquanto a fatura cresce em silêncio."
```

New body (method-faithful framing):

```
"Débito, PIX e dinheiro afetam o caixa no mesmo dia. O crédito é diferente: cada compra vai para a fatura e o Neko lança esse total como uma Saída única no vencimento — o cartão sequestra o salário futuro. Por isso a fatura aparece nas Saídas, não no Diário."
```

The card _title_ ("Débito e crédito: dois ritmos") does not change; the e2e
assertion at `tests/e2e/app-shell.spec.ts:71` matches the title and will
continue to pass.

**Verify**: `npm run typecheck` → exit 0

### Step 2: Remove the ghost API fields from the TypeScript type

File: `src/lib/api.ts`, lines 33–35.

Remove the two fields `credit_spend_month` and `has_credit` from the
`DashboardSummary` interface. The interface should go from:

```ts
export interface DashboardSummary {
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  credit_spend_month: number;
  /** Há rastreio de crédito (cartão ou gasto). `false` → mostrar "—" no tile, não R$0. */
  has_credit: boolean;
  reserve_months: number;
  reserve_trend: string;
  transaction_count: number;
}
```

To:

```ts
export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents (forecast engine, spec 003). */
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  reserve_months: number;
  reserve_trend: string;
  transaction_count: number;
}
```

**Verify**: `npm run typecheck` → will likely fail with errors in test fixtures
(expected, proceed to step 3 before re-running).

### Step 3: Remove ghost fields from test fixtures

**File A**: `src/test/commands.ts`, lines 37–38 and 48–49.

Remove `credit_spend_month` and `has_credit` from both `SUMMARY` and
`EMPTY_SUMMARY`:

```ts
export const SUMMARY: DashboardSummary = {
  balance: 842000,
  daily_budget: 4300,
  daily_spend_today: 3800,
  reserve_months: 4.5,
  reserve_trend: "down",
  transaction_count: 42,
};

export const EMPTY_SUMMARY: DashboardSummary = {
  balance: 0,
  daily_budget: 0,
  daily_spend_today: 0,
  reserve_months: 0,
  reserve_trend: "flat",
  transaction_count: 0,
};
```

**File B**: `src/screens/dashboard/DailyCheckinCard.test.tsx`, lines 14–15.

Remove `credit_spend_month` and `has_credit` from the local `SUMMARY` const:

```ts
const SUMMARY: DashboardSummary = {
  balance: 500000,
  daily_budget: 5000,
  daily_spend_today: 2000,
  reserve_months: 3,
  reserve_trend: "flat",
  transaction_count: 10,
};
```

**Verify**: `npm run typecheck` → exit 0, `npm run test:run` → all pass.

### Step 4: Remove ghost fields from the Rust struct and their computing queries

File: `src-tauri/src/commands/forecast_cmds.rs`.

**4a. Remove the two fields from the `DashboardSummary` struct (lines 882–893).**

The struct becomes:

```rust
#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
}
```

**4b. Remove the `credit_spend_month` query block (lines 953–978).**

Delete from the `// Crédito no mês (Régua 2)…` comment through the
`.map_err(|e| format!("query credit spend: {e}"))?;` line, inclusive. Also
delete the `month_start` and `month_end` bindings immediately above (lines
959–962) — those variables are only used by this query. Confirm they are
not used anywhere else in `dashboard_summary` before deleting.

**4c. Remove the `has_credit` query block (lines 980–990).**

Delete from `let has_credit: (i64,) = sqlx::query_as(` through
`.map_err(|e| format!("query has_credit: {e}"))?;` inclusive.

**4d. Remove the two fields from the `Ok(DashboardSummary { … })` return
(lines 1024–1034).**

The return becomes:

```rust
    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        daily_spend_today: daily_spend.0,
        reserve_months,
        reserve_trend: reserve_trend.0,
        transaction_count: count.0,
    })
```

**Verify**: `npm run rust:check` → exit 0.

### Step 5: Rename "Régua 2" jargon in Rust comments (no behavior change)

These are comment-only changes — no logic changes.

**File: `src-tauri/src/commands/forecast_cmds.rs`**

| Line | Old comment text                                                                                 | Replacement                                                                                                       |
| ---- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| 300  | `/// plus credit-cycle lumps aggregated from `daily_checkin` at the card due date (Régua 2).`    | `/// plus credit-cycle lumps aggregated from `daily_checkin` and folded into a fatura lump at the card due date.` |
| 369  | `// Credit spend (Régua 2) → aggregate by due_date`                                              | `// Credit spend → aggregate into a fatura lump at due_date`                                                      |
| 817  | `// Crédito (régua 2) entra em Saída como a fatura, não em Diário — espelha forecast::classify.` | `// Crédito entra em Saída como a fatura (lump no vencimento), não em Diário — espelha forecast::classify.`       |
| 937  | `// Mesma regra no crédito abaixo e no forecast (`load_cashflow_events`).`                       | (no change needed here; this comment does not contain "Régua 2")                                                  |
| 953  | entire block removed in step 4                                                                   | (already gone)                                                                                                    |

**File: `src-tauri/src/forecast/mod.rs`**

| Location                          | Old text                                                                                          | Replacement                                                                                                                     |
| --------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Line 20 (EventKind::FixedOut doc) | `/// Saída — fixed outflow: fixed bills + the credit-invoice lump at the card due day (Régua 2).` | `/// Saída — fixed outflow: fixed bills + the fatura lump at the card due date (credit settles as one lump, not per-purchase).` |
| Line 852 (test comment)           | `// T6.2 / T6.3 — a credit lump (Régua 2) lands as one FixedOut on the due day, depressing the`   | `// T6.2 / T6.3 — a fatura lump lands as one FixedOut on the card due day, depressing the`                                      |
| Line 853                          | `// future month, while débito daily (Régua 1) only touches its own day.`                         | `// future month, while débito/PIX daily spend only touches its own day.`                                                       |
| Line 855 (test fn name)           | `fn regua2_credit_lump_at_due_day()`                                                              | `fn credit_fatura_lump_lands_at_due_day()`                                                                                      |
| Line 857 (comment in test)        | `ev("2026-01-10", EventKind::Daily, 20000), // débito daily (Régua 1)`                            | `ev("2026-01-10", EventKind::Daily, 20000), // débito/PIX daily spend`                                                          |
| Line 858 (comment in test)        | `ev("2026-02-15", EventKind::FixedOut, 600000), // invoice lump at due day (Régua 2)`             | `ev("2026-02-15", EventKind::FixedOut, 600000), // fatura lump at card due date`                                                |

**Verify**: `npm run rust:check` → exit 0, `npm run test:run` → all pass (the
renamed test function still exists; it was not deleted).

### Step 6: Remove "Régua 2" section comment from states.css

File: `src/design-system/tokens/states.css`, line 53.

Change:

```css
/* ---- Pressão de fatura / crédito (Régua 2) ---- */
```

To:

```css
/* ---- Pressão de fatura / crédito ---- */
```

The three `--pressure-low/mid/high` tokens below the comment are **kept** — they
are valid design tokens expressing fatura payment pressure; only the jargon
label changes.

**Verify**: `npm run lint` → exit 0.

### Step 7: Delete CardChip component and its test

Before deleting, confirm zero production imports one more time:

```bash
grep -rn "import.*CardChip\|from.*CardChip" src/ --include="*.tsx" --include="*.ts" \
  | grep -v "CardChip\.tsx\|CardChip\.test\.tsx"
```

Expected: no output. If any match appears, STOP and report.

Delete both files:

```bash
rm src/design-system/components/CardChip.tsx
rm src/design-system/components/CardChip.test.tsx
```

If the design-system has a barrel export that re-exports `CardChip`, remove
that export line too. Check:

```bash
grep -rn "CardChip" src/design-system/index.ts 2>/dev/null || true
```

Remove any matching export line.

**Verify**: `npm run typecheck` → exit 0, `npm run test:run` → all pass (5
fewer test cases from the deleted CardChip suite; no new failures).

### Step 8: Full gate

```bash
npm run check
```

Expected: exit 0.

### Step 9: E2E smoke

```bash
npm run e2e
```

Expected: all tests pass. Pay attention to:

- `sidebar navigation switches screens and marks the current item` — this test
  navigates to "Ajuda" and asserts `page.getByText("Débito e crédito: dois
ritmos")` (the card _title_). The title is unchanged; this should continue to
  pass. If it fails, check that the `title` field in `PRINCIPLES` was not
  accidentally modified.
- No other e2e test asserts the old misleading body copy.

## Test plan

No new test files are required by this plan. The changes are:

1. A copy edit — covered by the existing e2e assertion on the card title.
2. Dead-code removal — TS/Rust type errors are the self-checking mechanism;
   `npm run typecheck` and `npm run rust:check` confirm no consumer was missed.
3. Comment-only jargon rename — no behavioral change; existing tests (`npm run
test:run` and `npm run rust:check`) confirm the renamed test function still
   compiles and passes.
4. Component deletion — covered by the zero-import grep in step 0 and step 7;
   `npm run test:run` confirms the 5 deleted cases are the only casualties.

If a production consumer of `credit_spend_month` / `has_credit` is discovered
during step 0, the executor should write a regression test for whatever tile
replaces it, then remove the old fields. That scenario is a STOP condition.

## Done criteria

All must hold simultaneously:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run rust:check` exits 0
- [ ] `npm run test:run` exits 0 (5 fewer tests than baseline — the CardChip suite)
- [ ] `npm run lint` exits 0
- [ ] `npm run doctor` shows no new findings vs the pre-change baseline
- [ ] `npm run e2e` exits 0; the "Débito e crédito: dois ritmos" title assertion passes
- [ ] `grep -rn "credit_spend_month\|has_credit" src/` returns only `src/lib/api.ts` (zero matches after step 2 removes it from there too)
- [ ] `grep -rn "Régua 2\|regua2\|Regua 2" src/ src-tauri/` returns no matches
- [ ] `grep -rn "CardChip" src/` returns only the JSDoc comment in `src/lib/format.ts` (not an import)
- [ ] `grep -rn "acompanha os dois de forma independente\|cresce em silêncio" src/` returns no matches
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop immediately and report back (do not improvise) if:

- Any in-scope file's content at the locations cited in "Current state" does
  not match the excerpts in this plan (codebase has drifted; re-verify before
  proceeding).
- Step 0's grep finds a production `.tsx` file importing `CardChip` or
  destructuring `credit_spend_month` / `has_credit` from a `DashboardSummary`.
- Step 7's pre-delete grep finds a production import of `CardChip`.
- `npm run rust:check` fails after step 4 for any reason other than the
  expected struct-field removals — investigate before continuing.
- `npm run e2e` fails on the `"Débito e crédito: dois ritmos"` assertion after
  the copy change — this means the card title was accidentally modified.
- Any step's verification fails twice after a reasonable fix attempt.
- The fix appears to require modifying a file outside the in-scope list.
- `month_start` or `month_end` are found to be used elsewhere in
  `dashboard_summary` beyond the two queries being removed (they are only used
  by `credit_spend` and `has_credit`; if another query references them the
  removal in step 4b must be adjusted).

## Maintenance notes

- **Future credit tile**: if a future plan adds a credit summary tile back to
  the dashboard, it should compute the figure from `forecast_dto` (the existing
  faithful FixedOut lump projection), not from a new live-accumulator query.
  The `--pressure-low/mid/high` CSS tokens are still available for that tile.
- **PR reviewer focus**: confirm the MethodologyScreen body copy accurately
  reflects the lump model without introducing new inaccuracies; confirm the
  three Rust query blocks (credit_spend, has_credit, and their let-bindings)
  are fully removed from `dashboard_summary`; confirm no test coverage
  regression.
- **Deferred**: the `daily_checkin` table still has a `credit_spend` column
  that was the data source for the removed query. That column is part of plan
  024, which addresses the `daily_checkin` writer gap holistically. Do not drop
  that column here.
- **Deferred**: the `--pressure-low/mid/high` tokens remain in `states.css`.
  They are retained for future use by a faithful credit-pressure indicator.
  Removing them now would be premature (no consumer, but also no cost to keep).
