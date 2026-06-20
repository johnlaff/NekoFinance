# Plan 033: Adherence: reserve floor = cost-of-living × months; unify Economizado% threshold

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src-tauri/src/commands/forecast_cmds.rs src/screens/totaisStatus.ts src/screens/dashboard/colchaoPhase.ts src/screens/TotaisScreen.test.tsx src-tauri/src/commands/mod.rs src-tauri/src/forecast/mod.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Package**: C
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

Two distinct method-fidelity bugs weaken the "pode gastar hoje" guardrail.

First, `reserve_floor` returns R$0 when no reserve-tagged pocket exists — the
cash guardrail is then ungated, allowing the user to spend down to zero with no
protection. The method's stated reserve target is "custo de vida mensal × N
meses" (the concrete number the method uses is 6 months for the
"operate" phase), so the floor should default to `monthly_cost_of_living ×
target_months` even when no reserve pocket has been configured.

Second, the Economizado% threshold is defined in three places with two
different values (2500 bps for the annual guardrail, 2000 bps for the monthly
badge), with no shared constant. The method's stated band is 20–30% with a
minimum of 20%. The comment in `totaisStatus.ts` calls the split "deliberate"
but the values are spread across at least three files, making the boundary easy
to drift. This plan standardises the canonical minimum (20% = 2000 bps) in one
shared Rust constant and one shared TS constant, while keeping the 25% annual
guardrail explicitly documented as a higher bar.

## Current state

### File roles

- `src-tauri/src/commands/forecast_cmds.rs` — async Tauri commands and inner
  helpers; owns the `SAVINGS_TARGET_BPS` constant (line 86) and
  `reserve_floor` function (lines 255–261); calls `safe_to_spend_today` (line
  524).
- `src-tauri/src/forecast/mod.rs` — pure forecast engine; contains
  `safe_to_spend_today` (lines 132–162) and `MonthMetric.savings_rate_bps`
  (line 67).
- `src/screens/totaisStatus.ts` — frontend monthly-badge logic; owns its own
  `SAVINGS_TARGET_BPS` constant (line 13) and `economizadoStatus` function
  (lines 36–43).
- `src/screens/dashboard/colchaoPhase.ts` — phase detection; owns
  `RESERVE_MIN_MONTHS = 6` (line 15) and uses it to determine "operate" phase.
- `src-tauri/src/commands/mod.rs` — integration tests for the commands above.
- `src/screens/TotaisScreen.test.tsx` — unit tests for `economizadoStatus`.

### Finding 1 — reserve_floor ungated

`src-tauri/src/commands/forecast_cmds.rs`, lines 250–261:

```rust
/// Piso de reserva = colchão intocável que a folga de caixa não pode comer. Por ora = soma dos
/// Bolsos marcados como reserva (spec 007, `liquidity = 'reserve'`); esses NÃO entram na semente
/// líquida, então subtraí-los aqui não dobra. O ideal metodológico (custo de vida × 12) fica
/// para quando a reserva for modelada como meta — ver limitações na spec 010. Hoje, sem reserva
/// configurada, retorna 0 e a régua de poupança é a que morde.
pub(crate) async fn reserve_floor(pool: &SqlitePool) -> Result<i64, String> {
    let floor: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor: {e}"))?;
    Ok(floor.0)
}
```

When no reserve-tagged account exists, `COALESCE(SUM(balance), 0)` returns 0,
so `reserve_floor_cents = 0` and `cash_headroom_cents = deepest_deficit_balance
− 0` — the guardrail is completely ungated (the user can spend down to zero).

The existing cost-of-living baseline is computed by `realized_monthly_baseline`
(lines 177–203), which returns the median of the last 6 complete months'
expenses. The phase constant `RESERVE_MIN_MONTHS = 6` in
`src/screens/dashboard/colchaoPhase.ts` (line 15) mirrors the method's 6-month
reserve target.

The computed floor must be:
`monthly_cost_of_living_cents × RESERVE_MIN_MONTHS`
falling back to `SUM(reserve balance)` when that is higher, and to 0 when
`monthly_cost_of_living_cents = 0` (no history yet — do not block a new user).

### Finding 2 — Economizado% threshold inconsistency

**Annual guardrail** (`src-tauri/src/commands/forecast_cmds.rs`, lines 82–86):

```rust
/// Meta de poupança do método: piso de 25% (faixa 20–30%, MÉDIA ANUAL — o ano todo deve ficar
/// na faixa, os meses variam). Régua do guardrail ANUAL "pode gastar".
/// O badge MENSAL "Dentro do ideal" (src/screens/TotaisScreen.tsx) usa 20% (piso da faixa), por ser
/// leniente a variações de um mês; ambos ficam dentro da faixa canônica 20–30%.
pub(crate) const SAVINGS_TARGET_BPS: i64 = 2500;
```

**Monthly badge** (`src/screens/totaisStatus.ts`, lines 9–13):

```typescript
// Piso 20% para o badge MENSAL "Dentro do ideal" — um mês pode variar dentro da faixa 20–30% do
// método. O guardrail ANUAL "pode gastar hoje" usa 25% (alvo médio da faixa) em
// src-tauri/src/commands.rs (SAVINGS_TARGET_BPS). Divergência deliberada: indicador mensal leniente,
// gate anual mais firme; ambos dentro da faixa canônica 20–30%.
const SAVINGS_TARGET_BPS = 2000;
```

**Annual visual** (`src/screens/AnnualScreen.tsx`, lines 168–175):

```typescript
// 3 estados (mesma lógica do economizadoStatus em Totais): >30% guardando além do ideal
// (jade/steady), 20–30% dentro do ideal (verde), <20% aquém (âmbar).
const savingsColor =
  annualSavingsPct > 30
    ? "var(--primary)"
    : annualSavingsPct >= 20
      ? "var(--success-400)"
      : "var(--warning-400)";
```

`AnnualScreen.tsx` independently hard-codes the same 20% boundary in a local
computed, with no reference to the shared constant.

**colchaoPhase.ts** (line 25) uses `income * 2_000` (20% = 2000 bps in integer
arithmetic) as the `rateOk` gate for "operate" phase — consistent with 20%, but
again a bare literal with no named constant.

Summary of values found:

| Location               | Value       | Purpose                         |
| ---------------------- | ----------- | ------------------------------- |
| `forecast_cmds.rs:86`  | 2500 bps    | Annual "pode gastar" guardrail  |
| `totaisStatus.ts:13`   | 2000 bps    | Monthly "Dentro do ideal" badge |
| `AnnualScreen.tsx:173` | `>= 20` (%) | Annual visual colour band       |
| `colchaoPhase.ts:25`   | `* 2_000`   | Phase "operate" gate            |

Resolution: add a TS constant `SAVINGS_MIN_BPS = 2000` exported from
`totaisStatus.ts`, import it into `AnnualScreen.tsx` and `colchaoPhase.ts`.
The Rust annual guardrail stays at 2500 bps but is explicitly documented as the
"higher bar within the 20–30% band" — it is deliberately stricter; unifying to
2000 bps would loosen the annual gate. No Rust change needed beyond improving
the comment.

### Conventions

- Functional-core / imperative-shell: pure calculations in `src-tauri/src/forecast/mod.rs`; IO in `forecast_cmds.rs`.
- Money = positive-magnitude integer cents (`i64` in Rust, `number` in TS).
- Tests: Rust integration tests live in `src-tauri/src/commands/mod.rs` using `fixture_pool()` and `insert_liquid_account` / `insert_reserve_account` helpers. TS unit tests live in `src/screens/TotaisScreen.test.tsx` for `totaisStatus`.
- React Compiler is ON — no manual memo. Hoist static styles.
- Method-neutral language in comments (public repo): say "the method"; do not reference any third-party product or course.

## Commands you will need

| Purpose                       | Command              | Expected on success |
| ----------------------------- | -------------------- | ------------------- |
| Rust type-check + tests       | `npm run rust:check` | exit 0, no errors   |
| Rust unit + integration tests | `npm run test:run`   | all pass            |
| TS type-check                 | `npm run typecheck`  | exit 0              |
| Lint                          | `npm run lint`       | exit 0              |
| Full gate                     | `npm run check`      | exit 0              |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/forecast_cmds.rs` — fix `reserve_floor`, update doc comment on `SAVINGS_TARGET_BPS`
- `src-tauri/src/commands/mod.rs` — add integration tests for the new `reserve_floor` behaviour
- `src/screens/totaisStatus.ts` — export `SAVINGS_MIN_BPS`, keep `SAVINGS_TARGET_BPS` private (or rename to `SAVINGS_MIN_BPS`)
- `src/screens/AnnualScreen.tsx` — import and use `SAVINGS_MIN_BPS` instead of the bare `>= 20` literal
- `src/screens/dashboard/colchaoPhase.ts` — import and use `SAVINGS_MIN_BPS` instead of the bare `2_000` literal in the `rateOk` line
- `src/screens/TotaisScreen.test.tsx` — add tests asserting the boundary at 2000 bps and confirming the import

**Out of scope** (do NOT touch):

- `src-tauri/src/forecast/mod.rs` — pure engine; `safe_to_spend_today` already receives `reserve_floor_cents` as a parameter; no engine change needed.
- `src/screens/DashboardScreen.tsx` — reads `savings_target_bps` from the DTO (which carries the 25% annual value); display only, no logic change needed.
- `src/screens/dashboard/PrevisibilidadeCard.tsx` — display only ("referência 20 a 30%") hardcoded text, no threshold logic.
- `src/screens/dashboard/ColchaoCard.tsx` — display only ("≥ 20% economizado").
- Any Rust schema/migration files — no DB schema changes.
- The tag Ignorar toggle (planned for plan 034).

## Git workflow

- Branch: `advisor/033-reserve-floor-savings-threshold`
- Commit style: `fix: <description> (plano 033)` (matches repo convention — see recent `fix:` commits in `git log`)
- Commit per logical step (one commit for reserve_floor, one for TS threshold unification).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Add `RESERVE_MIN_MONTHS` to the Rust layer and fix `reserve_floor`

Open `src-tauri/src/commands/forecast_cmds.rs`.

After line 91 (after `COVERAGE_COMPLETE_BPS`), add a new constant:

```rust
/// Meses de reserva mínimos do método (fase "operar"): o mesmo limiar que o frontend usa em
/// `colchaoPhase.ts` (RESERVE_MIN_MONTHS). Mantidos em sync manualmente (a lógica de fase é
/// purely frontend; se mudar, atualizar os dois).
pub(crate) const RESERVE_MIN_MONTHS: i64 = 6;
```

Then replace the body of `reserve_floor` (lines 255–261) so it computes
`max(reserve_balance, monthly_cost_of_living × RESERVE_MIN_MONTHS)`,
falling back to 0 only when there is no cost-of-living history:

```rust
/// Piso de reserva = colchão intocável que a folga de caixa não pode comer.
///
/// Lógica em duas camadas:
/// 1. Saldo dos Bolsos de reserva configurados (`liquidity = 'reserve'`). Esses Bolsos NÃO
///    entram na semente líquida, então subtraí-los aqui não os dobra.
/// 2. Piso mínimo do método: `custo de vida mensal × RESERVE_MIN_MONTHS`. Se não há Bolso de
///    reserva configurado (ou o saldo está abaixo do piso), usa o piso calculado — assim o
///    guardrail não fica completamente desmontado para quem ainda não criou um Bolso.
///
/// Sem histórico de custo de vida (baseline = 0, usuário novo), retorna 0 — não bloqueia quem
/// está começando. Sem Bolso de reserva mas com histórico, retorna o piso calculado.
pub(crate) async fn reserve_floor(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let reserve_balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor (balance): {e}"))?;
    let baseline = realized_monthly_baseline(pool, today_naive).await?;
    let computed_floor = baseline * RESERVE_MIN_MONTHS;
    Ok(reserve_balance.0.max(computed_floor))
}
```

Note: `reserve_floor` now requires `today_naive: NaiveDate`. Update its two
call sites in the same file:

- Line 518 (in `forecast_dto`): `reserve_floor(pool).await?` →
  `reserve_floor(pool, today_naive).await?`
- Line 888 (in `dashboard_summary`): the `reserve_balance` query there is a
  separate inline query used for `reserve_months` (not the floor) — leave that
  untouched.

**Verify**: `npm run rust:check` → exit 0, no errors.

### Step 2: Add integration tests for `reserve_floor`

Open `src-tauri/src/commands/mod.rs`. Add three new tests alongside the
existing `dashboard_reserve_months_*` tests (after line ~1158). Model after the
existing fixtures (`fixture_pool`, `insert_liquid_account`,
`insert_reserve_account`, `insert_realized`).

```rust
// --- reserve_floor tests (plan 033) ---

// Without any reserve account and without cost-of-living history, the floor is 0
// (do not block a new user).
#[tokio::test]
async fn reserve_floor_zero_when_no_history_and_no_reserve_account() {
    let pool = fixture_pool().await;
    let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
    let floor = reserve_floor(&pool, today).await.unwrap();
    assert_eq!(floor, 0);
}

// Without a reserve account but WITH cost-of-living history, the floor is
// monthly_baseline × RESERVE_MIN_MONTHS (the computed minimum kicks in).
#[tokio::test]
async fn reserve_floor_uses_computed_minimum_when_no_reserve_account() {
    let pool = fixture_pool().await;
    // 3 complete months of expense at 100_000 each → median baseline = 100_000.
    for m in [3u32, 4, 5] {
        insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
    }
    let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
    let floor = reserve_floor(&pool, today).await.unwrap();
    // 100_000 × 6 = 600_000
    assert_eq!(floor, 600_000);
}

// When a reserve account exists with a balance above the computed floor,
// the actual balance wins (we use the higher of the two).
#[tokio::test]
async fn reserve_floor_uses_reserve_balance_when_above_computed_minimum() {
    let pool = fixture_pool().await;
    for m in [3u32, 4, 5] {
        insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
    }
    // Reserve balance 900_000 > computed floor 600_000.
    insert_reserve_account(&pool, 900_000).await;
    let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
    let floor = reserve_floor(&pool, today).await.unwrap();
    assert_eq!(floor, 900_000);
}

// When a reserve account exists with a balance below the computed floor,
// the computed floor wins (method target is the stronger constraint).
#[tokio::test]
async fn reserve_floor_uses_computed_minimum_when_reserve_balance_is_low() {
    let pool = fixture_pool().await;
    for m in [3u32, 4, 5] {
        insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
    }
    // Reserve balance 200_000 < computed floor 600_000.
    insert_reserve_account(&pool, 200_000).await;
    let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
    let floor = reserve_floor(&pool, today).await.unwrap();
    assert_eq!(floor, 600_000);
}
```

**Verify**: `npm run test:run` → all tests pass, including the 4 new ones.

### Step 3: Export `SAVINGS_MIN_BPS` from `totaisStatus.ts`

Open `src/screens/totaisStatus.ts`.

The local `const SAVINGS_TARGET_BPS = 2000` (line 13) is used only inside
`economizadoStatus`. Rename it to `SAVINGS_MIN_BPS` and export it so
`AnnualScreen.tsx` and `colchaoPhase.ts` can import it instead of hard-coding
the same value.

Replace lines 9–13:

```typescript
// Piso 20% para o badge MENSAL "Dentro do ideal" e para os estados visuais da visão anual.
// O guardrail ANUAL "pode gastar hoje" usa 25% (alvo médio da faixa 20–30%) em
// src-tauri/src/commands/forecast_cmds.rs (SAVINGS_TARGET_BPS = 2500). Divergência deliberada:
// indicador mensal e visual são lenientes (um mês pode variar); gate anual é mais firme; ambos
// dentro da faixa canônica 20–30% do método.
export const SAVINGS_MIN_BPS = 2000;
```

In `economizadoStatus` (line 41), change `SAVINGS_TARGET_BPS` to
`SAVINGS_MIN_BPS`:

```typescript
if (bps >= SAVINGS_MIN_BPS) return { level: "strong", label: "Dentro do ideal" };
```

**Verify**: `npm run typecheck` → exit 0.

### Step 4: Use `SAVINGS_MIN_BPS` in `AnnualScreen.tsx`

Open `src/screens/AnnualScreen.tsx`.

Add the import at the top of the file (alongside any existing imports from
`totaisStatus`):

```typescript
import { SAVINGS_MIN_BPS } from "./totaisStatus";
```

Replace the hard-coded `>= 20` (lines 170–175):

```typescript
// 3 estados: >30% acima do ideal; ≥ SAVINGS_MIN_BPS (20%) dentro do ideal; <20% aquém.
const savingsColor =
  annualSavingsPct > 30
    ? "var(--primary)"
    : annualSavingsPct >= SAVINGS_MIN_BPS / 100
      ? "var(--success-400)"
      : "var(--warning-400)";
```

**Verify**: `npm run typecheck` → exit 0.

### Step 5: Use `SAVINGS_MIN_BPS` in `colchaoPhase.ts`

Open `src/screens/dashboard/colchaoPhase.ts`.

Add the import:

```typescript
import { SAVINGS_MIN_BPS } from "../totaisStatus";
```

Replace the `rateOk` line (line 25):

```typescript
const rateOk = economia * 10_000 >= income * SAVINGS_MIN_BPS;
```

Remove or adjust any comment that still refers to "20%" as a bare literal.

**Verify**: `npm run typecheck` → exit 0.

### Step 6: Update `TotaisScreen.test.tsx`

Open `src/screens/TotaisScreen.test.tsx`.

The existing tests already cover the 2000 bps boundary (line 25:
`economizadoStatus(2000).label === "Dentro do ideal"`). Add a test that
confirms `SAVINGS_MIN_BPS` is exported and equals 2000, so any future rename
fails loudly:

```typescript
import { SAVINGS_MIN_BPS } from "./totaisStatus";

it("SAVINGS_MIN_BPS is the 20% canonical constant", () => {
  expect(SAVINGS_MIN_BPS).toBe(2000);
});
```

**Verify**: `npm run test:run` → all tests pass, including the new one.

### Step 7: Full gate

**Verify**: `npm run check` → exit 0, no errors, no new lint warnings.

## Test plan

### Rust integration tests (new, in `src-tauri/src/commands/mod.rs`)

Four tests added in Step 2:

1. `reserve_floor_zero_when_no_history_and_no_reserve_account` — new user, no
   history, no reserve pocket → floor = 0 (do not block).
2. `reserve_floor_uses_computed_minimum_when_no_reserve_account` — history
   exists, no reserve pocket → floor = baseline × 6.
3. `reserve_floor_uses_reserve_balance_when_above_computed_minimum` — reserve
   balance > computed floor → floor = reserve balance.
4. `reserve_floor_uses_computed_minimum_when_reserve_balance_is_low` — reserve
   balance < computed floor → floor = computed floor.

Model after `dashboard_reserve_months_derived_from_balance_and_baseline` (line
1138 in `mod.rs`).

### TS unit test (new, in `src/screens/TotaisScreen.test.tsx`)

One test added in Step 6:

- `SAVINGS_MIN_BPS is the 20% canonical constant` — guards the exported value.

Existing tests in `TotaisScreen.test.tsx` (lines 23–26) already cover the
2000 bps boundary; they pass before and after this plan.

**Run**: `npm run test:run` → all pass, 5 new tests total (4 Rust + 1 TS).

## Done criteria

- [ ] `npm run rust:check` exits 0
- [ ] `npm run test:run` exits 0; 4 new Rust tests for `reserve_floor` exist and pass
- [ ] `npm run test:run` exits 0; 1 new TS test for `SAVINGS_MIN_BPS` exists and passes
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `grep -n "reserve_floor(pool)" src-tauri/src/commands/forecast_cmds.rs` shows the updated call with `today_naive` argument
- [ ] `grep -n "SAVINGS_MIN_BPS" src/screens/AnnualScreen.tsx` returns a match
- [ ] `grep -n "SAVINGS_MIN_BPS" src/screens/dashboard/colchaoPhase.ts` returns a match
- [ ] `grep -n "const SAVINGS_TARGET_BPS = 2000" src/screens/totaisStatus.ts` returns no match (renamed)
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The `reserve_floor` function body at lines 255–261 of `forecast_cmds.rs`
  doesn't match the excerpt above (the codebase has drifted since this plan was
  written).
- `SAVINGS_TARGET_BPS` in `forecast_cmds.rs` is not at line 86, or its value
  is not 2500.
- `SAVINGS_TARGET_BPS` in `totaisStatus.ts` is not at line 13, or its value is
  not 2000.
- The `rateOk` line in `colchaoPhase.ts` doesn't contain `* 2_000` (the literal
  has already been extracted or the logic has changed).
- The `AnnualScreen.tsx` `savingsColor` block is not at lines 170–175 or its
  structure has changed.
- Adding `today_naive` to `reserve_floor` causes a compile error in a call site
  not listed in this plan (there may be an additional caller introduced after
  this plan was written).
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- **RESERVE_MIN_MONTHS synchronisation**: the value 6 now lives in both Rust
  (`forecast_cmds.rs:RESERVE_MIN_MONTHS`) and TS (`colchaoPhase.ts:RESERVE_MIN_MONTHS`).
  They must be kept in sync manually. If the method's reserve target changes,
  update both. Consider a future plan to expose the Rust constant via the DTO
  so the frontend can consume it instead of duplicating.
- **Reserve floor precedence**: once a "reserve goal" feature is implemented
  (spec 010, configurable target months), the `reserve_floor` function should
  read `target_months` from the DB instead of the hard-coded `RESERVE_MIN_MONTHS`.
  The function signature already takes `today_naive` so `realized_monthly_baseline`
  can be called; adding a DB read for `target_months` is a local change.
- **Annual guardrail at 25% vs minimum at 20%**: the Rust `SAVINGS_TARGET_BPS = 2500`
  is intentionally higher than the method's 20% minimum. The comment in
  `forecast_cmds.rs` explains the reasoning (annual average target vs monthly
  floor). A reviewer should confirm this delta is still the desired behaviour
  when reviewing the PR.
- **PR review focus**: confirm the `max(reserve_balance, computed_floor)` logic
  in the updated `reserve_floor`, and that the `realized_monthly_baseline` call
  does not cause a double query compared to the `forecast_dto` path (it does
  call it twice — once for the floor, once for baseline in coverage — but both
  are already lightweight indexed queries).
