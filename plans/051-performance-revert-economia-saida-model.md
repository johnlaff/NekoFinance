# Plan 051: Performance = income − cost_of_living (economia=Saída model) — supersedes 046, no double-count

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
> git diff --stat 2132297..HEAD -- \
>   src-tauri/src/forecast/mod.rs \
>   src/design-system/components/InfoPopover.tsx \
>   specs/011-engine-five-types/spec.md \
>   plans/README.md
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Package**: G
- **Planned at**: commit `2132297`, 2026-06-21

## Why this matters

Plan 046 changed the Performance formula to `income − cost_of_living − economia`, reasoning that
the savings transfer appears as a Saída in the month grid and must therefore be subtracted.
However, the owner decision of 2026-06-21 (FINAL) clarifies the causal chain more precisely:
**because savings is logged as a Saída (expense row) in the grid**, it is already captured in
`cost_of_living` (via `EventKind::FixedOut` or `EventKind::Daily` in the engine). The
`EventKind::Economia` events come from the Economia-tab import (`store_economia_entries`), which
stores them as `type='transfer'` rows — a **separate** record from the grid Saída row. Keeping
`− economia` on top of `cost_of_living` (which already contains the grid Saída) would
double-subtract the savings amount: once via the expense row in `cost_of_living`, and once via
the transfer row in `economia`. The correct formula is therefore:

```
performance = income − cost_of_living         # plan 040's formula — RESTORED
```

The `− economia` term added by plan 046 is the bug. Today, while the Economia tab is empty
(`economia = 0`), the formulas give identical results — so no user-visible regression exists
yet. But when the user starts recording savings in the Economia tab, the displayed Performance
would be silently wrong (over-subtracted by the savings amount). This plan restores correctness
before that situation arises.

## Current state

### Files and their roles

- `src-tauri/src/forecast/mod.rs` — pure engine; `month_metrics` (private fn) computes
  `performance_cents` at **line 380** (the formula introduced by plan 046); `MonthMetric` struct
  at lines 52–75; tests at lines 793–1099.
- `src/design-system/components/InfoPopover.tsx` — glossary; `performance` entry at **lines
  47–50** (updated by plan 046 to mention Economia — must revert).
- `specs/011-engine-five-types/spec.md` — "Revisão da fórmula de Performance" section at
  **lines 51–71** (updated by plan 046 — must be replaced with the 2026-06-21 final decision).
- `plans/README.md` — "Findings considered and rejected" entry at **lines 112–116** (must be
  updated to reflect the FINAL decision and supersession chain 040→046→051).

### Critical data-flow that explains the double-count

The engine routes DB rows through `map_cashflow_row` (`src-tauri/src/commands/mod.rs:45–66`)
which calls `classify()` (`src-tauri/src/forecast/mod.rs:238–262`):

- An expense row in the month grid (savings logged as a Saída):
  `type='expense'`, `is_fixed=1` → `EventKind::FixedOut` → accumulates into `fixed_out`
  → **inside `cost_of_living_cents`**.
- The Economia-tab import (`store_economia_entries`, `src-tauri/src/commands/write_back_cmds.rs:1009`):
  writes `type='transfer'`, `to_account_id = reserve_account_id`.
  `classify("transfer", …, Some("reserve"))` → `EventKind::Economia` → accumulates into `economia`
  → **separate from `cost_of_living_cents`**.

Therefore: when the user has **both** a grid Saída row for the savings amount **and** an
Economia-tab entry for the same amount, the money appears in BOTH `cost_of_living_cents` (via
the expense row) and `economia` (via the transfer row). Subtracting both = double-count.
The correct model: the expense row in `cost_of_living` IS the performance deduction; the
Economia-tab transfer is a **savings-rate annotation** (feeds `savings_rate_bps` = Economizado%),
NOT a second money movement for Performance.

### Relevant code excerpts (live at commit 2132297)

**`src-tauri/src/forecast/mod.rs`, lines 373–380** — formula block (CURRENT, plan 046 — WRONG):

```rust
// Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
let cost_of_living_cents = fixed_out + daily_realized;
// Performance = Entradas − (Saídas + Diário + Economia) — fórmula fiel da planilha.
// DECISÃO DO DONO (2026-06-21): a Economia é lançada como Saída no grid mensal, portanto
// a planilha já a desconta da Performance (Saída Total inclui o lançamento de economia).
// `daily_projected` NÃO é descontado (a planilha usa o realizado; a projeção serve só ao
// saldo de caixa e não tem correspondência na linha de Performance da planilha).
let performance_cents = income - cost_of_living_cents - economia;
```

**`src-tauri/src/forecast/mod.rs`, lines 67–69** — `MonthMetric` doc comment (CURRENT, plan 046):

```rust
/// Economia lançada no mês (numerador do Economizado%). Desconta a Performance porque a
/// planilha a registra como Saída no grid mensal (fiel ao método — 2026-06-21).
pub economia_cents: i64,
```

**`src/design-system/components/InfoPopover.tsx`, lines 47–50** — performance entry (CURRENT, plan 046):

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: Entradas menos Saídas (incluem fixas, fatura do cartão e o que você guardou como Economia) e Diário. É o mesmo cálculo da sua planilha.",
},
```

**`specs/011-engine-five-types/spec.md`, lines 51–71** — performance decision note (CURRENT, plan 046):

```
## Revisão da fórmula de Performance (2026-06-21)

**Decisão do dono (2026-06-20, corrigida em 2026-06-21)**: a Performance exibida deve ser fiel
à planilha. A clarificação de 2026-06-21 revelou que a Economia é lançada como Saída no grid
mensal — portanto, a planilha já desconta a Economia na Performance (Saída Total inclui o
lançamento de economia). A fórmula correta é:

performance = income − cost_of_living − economia   # = Entradas − (Saídas + Diário + Economia)

O termo `− daily_projected` continua EXCLUÍDO (a projeção de diário afeta o saldo de caixa
mas não tem linha correspondente na Performance da planilha — só o realizado aparece lá).

Economia continua sendo o numerador do Economizado% (savings_rate_bps) e continua alimentando
o guardrail de poupança anual via `realized_annual_economia` (independente de `performance_cents`).

Esta seção substitui a nota de 2026-06-20 do plano 040, que erroneamente excluía a Economia da
Performance. A fórmula do plano 040 (`income − cost_of_living`) era incorreta — divergia da
planilha pelo valor da economia guardada. O plano 046 corrige essa divergência.
```

**Tests in `src-tauri/src/forecast/mod.rs` that assert `performance_cents` with a non-zero `Economia` event — must be updated:**

| Line | Test name                                              | Current `performance_cents` assertion         | Correct value after this plan                          |
| ---- | ------------------------------------------------------ | --------------------------------------------- | ------------------------------------------------------ |
| 839  | `real_daily_avg_and_savings`                           | `550_000` (1_000_000 − 200_000 − 250_000)     | `800_000` (1_000_000 − 200_000)                        |
| 1069 | `performance_excludes_only_projected`                  | `450_000` (1_000_000 − 350_000 − 200_000)     | `650_000` (1_000_000 − 350_000)                        |
| 1096 | `performance_economia_subtracted_once_no_double_count` | `2_700_000` (5_000_000 − 1_500_000 − 800_000) | Remove this test entirely (it asserts the WRONG model) |

**Tests that do NOT include an `Economia` event — unaffected, must still pass:**

- `month_performance_and_cost` (line 798): `income=1_000_000`, `FixedOut=400_000`, `Daily=200_000`;
  no Economia; `performance=400_000` unchanged.
- `cash_differs_from_performance` (line 812): no Economia; unchanged.
- `performance_excludes_daily_projected_ceiling` (line 1029): Economia absent; `performance=1_000_000`; unchanged.
- `economia_reduces_spending_balance` (line ~975): tests the Saldo chain, not `performance_cents`; unchanged.
- Integration tests in `commands/mod.rs` (lines 273, 1117, 2376): none have `Economia` events
  alongside `performance_cents` assertions; unchanged.

### Repo conventions

- Functional-core/imperative-shell: pure logic in `forecast/mod.rs`, IO at command adapters.
- React Compiler is ON — no manual `memo` or `useMemo` in TypeScript files.
- Conventional commits style (e.g. `fix: …`, `docs: …`).
- Money: positive-magnitude integer cents; `performance_cents` may be negative.
- `safe_to_spend_today` (`forecast/mod.rs:132–162`) uses `annual_savings_cents` passed by value;
  it does NOT read `performance_cents`. Changing this formula has no effect on the guardrail.
- `savings_rate_bps` computation at lines 397–402 uses only `economia` and `income` — it does NOT
  read `performance_cents`. It is unaffected by this change and must not be touched.

## Commands you will need

| Purpose                    | Command                                                          | Expected on success    |
| -------------------------- | ---------------------------------------------------------------- | ---------------------- |
| Narrowed Rust tests        | `cargo test --manifest-path src-tauri/Cargo.toml performance`    | all pass               |
| Narrowed Rust tests        | `cargo test --manifest-path src-tauri/Cargo.toml real_daily_avg` | all pass               |
| Rust typecheck + all tests | `npm run rust:check`                                             | exit 0, all tests pass |
| Frontend typecheck         | `npm run typecheck`                                              | exit 0, no errors      |
| Lint                       | `npm run lint`                                                   | exit 0                 |
| Full gate                  | `npm run check`                                                  | exit 0                 |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/forecast/mod.rs` — formula at line 380 + two doc comments + three test updates
- `src/design-system/components/InfoPopover.tsx` — `performance` glossary body (lines 47–50)
- `specs/011-engine-five-types/spec.md` — replace the performance decision section (lines 51–71)
- `plans/README.md` — update "Findings considered and rejected" + add this plan's row

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/commands/forecast_cmds.rs` — `performance_cents` is a pass-through field copy;
  the corrected value propagates automatically with no change here.
- `src-tauri/src/commands/write_back_cmds.rs` — `store_economia_entries` and the Economia-tab
  import remain as-is; they feed `savings_rate_bps`, not Performance.
- `src-tauri/src/commands/mod.rs` — `map_cashflow_row` and `classify()` are unchanged.
- `safe_to_spend_today` and `realized_annual_economia` — confirmed independent of
  `performance_cents`; do not touch.
- `savings_rate_bps` accumulation and the `economia_cents` field in `MonthMetric` — keep as-is
  (economia is still the numerator of Economizado%).
- `EventKind::Economia`, the signed-balance logic, or any Saldo chain mechanics.
- Any frontend React file other than `InfoPopover.tsx`.

## Git workflow

- Branch: `advisor/051-performance-revert-economia-saida-model`
- Conventional commit style, e.g.:
  `fix: revert economia double-count — Performance = income − cost_of_living (plan 051)`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 0: drift check

Run the drift check from the header. Then confirm the key landmarks:

- `src-tauri/src/forecast/mod.rs:380` must read:
  `let performance_cents = income - cost_of_living_cents - economia;`
- `src-tauri/src/forecast/mod.rs:67` must read:
  `/// Economia lançada no mês (numerador do Economizado%). Desconta a Performance porque a`
- `src/design-system/components/InfoPopover.tsx:49` must read:
  `body: "A foto do mês: Entradas menos Saídas (incluem fixas, fatura do cartão e o que você guardou como Economia) e Diário. É o mesmo cálculo da sua planilha.",`

If any excerpt does not match, stop and report — do not attempt to adapt the steps.

**Verify**: `git diff --stat 2132297..HEAD -- src-tauri/src/forecast/mod.rs src/design-system/components/InfoPopover.tsx specs/011-engine-five-types/spec.md` → empty OR verify that any live differences do not affect the lines cited in "Current state" excerpts above.

### Step 1: confirm no hidden consumers of performance_cents

Before changing anything, run:

```
grep -rn "performance_cents" src-tauri/src/ src/
```

Every match must be one of: field declaration in `forecast/mod.rs`; the formula line (line 380);
the struct literal at line 408; a DTO struct field or pass-through in `forecast_cmds.rs`; test
assertions in `forecast/mod.rs` and `commands/mod.rs`.

**STOP if** `performance_cents` appears as an operand in any formula feeding cash-flow logic or
`safe_to_spend_today`. Report the extra consumer — do not proceed.

**Verify**: `grep -n "performance_cents" src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs` → matches only in struct definition, formula line, struct literal, DTO mapping, and tests. Zero matches in `safe_to_spend_today` or any arithmetic that derives a new value.

### Step 2: restore the formula and update the doc comments in forecast/mod.rs

Open `src-tauri/src/forecast/mod.rs`. Locate lines 373–380 (the formula block inside
`month_metrics`). Replace the block with:

```rust
// Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
let cost_of_living_cents = fixed_out + daily_realized;
// Performance = Entradas − (Saídas + Diário) — fórmula fiel à planilha.
// DECISÃO DO DONO (2026-06-21, FINAL): a economia é lançada como Saída (expense) no grid
// mensal → torna-se FixedOut/Daily → já está em cost_of_living. A aba Economia importa um
// transfer separado (EventKind::Economia) que alimenta savings_rate_bps (Economizado%), mas
// NÃO é deduzido da Performance de novo — subtrair `economia` aqui seria dupla contagem.
// `daily_projected` NÃO é descontado (a planilha usa o realizado; a projeção serve só ao
// saldo de caixa e não tem correspondência na linha de Performance da planilha).
let performance_cents = income - cost_of_living_cents;
```

Also update the `economia_cents` field doc comment in `MonthMetric` (lines 67–69) from:

```rust
/// Economia lançada no mês (numerador do Economizado%). Desconta a Performance porque a
/// planilha a registra como Saída no grid mensal (fiel ao método — 2026-06-21).
pub economia_cents: i64,
```

to:

```rust
/// Economia lançada no mês — numerador do Economizado% (savings_rate_bps). NÃO desconta a
/// Performance diretamente: a poupança já entra em cost_of_living como Saída no grid (expense
/// row → FixedOut/Daily). Esta linha é o transfer da aba Economia = anotação de taxa, não
/// duplo movimento. DECISÃO DO DONO 2026-06-21 final — plano 051 reverte o plano 046.
pub economia_cents: i64,
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance` → fails on the tests
with non-zero economia (expected — fixed in Step 3). No NEW compilation errors beyond assertion mismatches.

### Step 3: update the two tests whose economia is non-zero

**Test `real_daily_avg_and_savings`** (around line 823):

Fixture: `Income=1_000_000`, `Daily realized=4×50_000=200_000`, `Economia=250_000`.
`cost_of_living = 200_000`. Correct `performance = 1_000_000 − 200_000 = 800_000`.

Locate the assertion and comment around line 837–839:

```rust
        // Performance = renda (1000) − custo de vida (diário 200) − economia (250) = 550
        // (a economia é lançada como Saída na planilha, portanto desconta a Performance).
        assert_eq!(m.performance_cents, 550_000);
```

Replace with:

```rust
        // Performance = renda (1000) − custo de vida (diário 200) = 800
        // (economia não desconta Performance de novo — já está em cost_of_living como Saída
        // no grid; o transfer da aba Economia alimenta savings_rate_bps, não Performance).
        assert_eq!(m.performance_cents, 800_000);
```

**Test `performance_excludes_only_projected`** (around line 1046):

Fixture: `Income=1_000_000`, `FixedOut=300_000`, `Daily realized=50_000`, `Economia=200_000`,
`Daily projected=30_000` (realized=false).
`cost_of_living = 300_000 + 50_000 = 350_000`. Correct `performance = 1_000_000 − 350_000 = 650_000`.

Locate the assertion and comment around lines 1065–1069:

```rust
        // cost_of_living = fixed_out(300) + daily_realized(50) = 350_000
        assert_eq!(m.cost_of_living_cents, 350_000);
        // performance = income(1_000) − cost_of_living(350) − economia(200) = 450_000
        // (economia desconta Performance; daily_projected NÃO desconta — não está na planilha)
        assert_eq!(m.performance_cents, 450_000);
```

Replace with:

```rust
        // cost_of_living = fixed_out(300) + daily_realized(50) = 350_000
        assert_eq!(m.cost_of_living_cents, 350_000);
        // performance = income(1_000) − cost_of_living(350) = 650_000
        // (economia NÃO desconta Performance — já em cost_of_living como Saída no grid;
        // daily_projected NÃO desconta — só afeta o saldo de caixa)
        assert_eq!(m.performance_cents, 650_000);
```

Also update the test-header comment above the test (around line 1046):

```rust
    // Regressão: economia desconta Performance; previsão de diário NÃO desconta (planilha-parity
    // 2026-06-21 — economia é lançada como Saída no grid; daily_projected não tem linha na planilha).
```

Replace with:

```rust
    // Regressão: economia NÃO desconta Performance diretamente (já está em cost_of_living como
    // Saída no grid — evita dupla contagem plano 051). Previsão de diário NÃO desconta (não tem
    // linha na Performance da planilha). DECISÃO FINAL 2026-06-21.
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance real_daily_avg` → all pass.

### Step 4: remove the double-count guard test (it asserts the WRONG model)

The test `performance_economia_subtracted_once_no_double_count` (around lines 1075–1099) was
added by plan 046 to verify the `− economia` behavior. That behavior is now wrong. Delete the
entire test (the comment header + the `#[test]` fn block):

```rust
    // Guarda dupla-contagem: um mês com Economia lançada tem Performance = renda − custo_de_vida
    // − economia; a Economia NÃO está dentro do custo_de_vida (FixedOut/Daily são disjuntos de
    // EventKind::Economia na classificação — cada evento só cai em um braço do match).
    #[test]
    fn performance_economia_subtracted_once_no_double_count() {
        ...
    }
```

Exact text to identify the block start (around line 1075):

```
    // Guarda dupla-contagem: um mês com Economia lançada tem Performance = renda − custo_de_vida
```

After deletion, the closing `}` of the `mod tests` block (currently line 1100) and the file
end (currently line 1101) must remain intact.

**Verify**: `grep -n "performance_economia_subtracted_once_no_double_count" src-tauri/src/forecast/mod.rs` → no matches.

### Step 5: add a correct regression test

In place of the removed test, add a new test that documents the CORRECT model — economia is NOT
in performance (already in cost_of_living as a Saída):

Add immediately before the closing `}` of `mod tests`:

```rust
    // Regressão dupla-contagem (plano 051): economia NÃO é subtraída da Performance — ela
    // já está em cost_of_living como Saída no grid (expense → FixedOut/Daily). O transfer da
    // aba Economia é só anotação de taxa (savings_rate_bps). DECISÃO FINAL 2026-06-21.
    #[test]
    fn performance_economia_not_double_counted() {
        // Arrange: renda 5_000_000, Saída fixa 1_000_000, Diário realizado 500_000.
        // Economia 800_000 representa o transfer da aba Economia (anotação de taxa).
        // A poupança real já está no custo de vida como expense row (FixedOut ou Daily).
        let events = [
            ev("2026-05-01", EventKind::Income, 5_000_000),
            ev("2026-05-10", EventKind::FixedOut, 1_000_000),
            ev("2026-05-15", EventKind::Daily, 500_000),    // realized
            ev("2026-05-20", EventKind::Economia, 800_000), // savings-rate annotation
        ];
        let f = project(0, d("2026-05-01"), &events, d("2026-05-31"));
        let m = f.months.iter().find(|m| m.month == 5).unwrap();

        // cost_of_living = FixedOut(1_000) + Daily(500) = 1_500_000 (Economia NOT in here)
        assert_eq!(m.cost_of_living_cents, 1_500_000);
        // economia_cents is reported separately (feeds savings_rate_bps only)
        assert_eq!(m.economia_cents, 800_000);
        // performance = income(5_000) − cost_of_living(1_500) = 3_500_000
        // (NOT 2_700_000 — that was the double-count introduced by plan 046)
        assert_eq!(m.performance_cents, 3_500_000);
        // savings_rate_bps = 800_000 / 5_000_000 = 1600 bps (16%) — unaffected
        assert_eq!(m.savings_rate_bps, 1_600);
    }
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance_economia_not_double_counted` → 1 test passes.

### Step 6: update the InfoPopover glossary

Open `src/design-system/components/InfoPopover.tsx`. Find the `performance` entry at lines 47–50:

```ts
  performance: {
    title: "Performance",
    body: "A foto do mês: Entradas menos Saídas (incluem fixas, fatura do cartão e o que você guardou como Economia) e Diário. É o mesmo cálculo da sua planilha.",
  },
```

Replace the body — remove the Economia mention (it is not a separate deduction from Performance):

```ts
  performance: {
    title: "Performance",
    body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha — igual ao que você já conhece.",
  },
```

**Verify**: `npm run typecheck` → exit 0.

### Step 7: update spec 011

Open `specs/011-engine-five-types/spec.md`. Find the "Revisão da fórmula de Performance
(2026-06-21)" section at lines 51–71. Replace it entirely:

```markdown
## Revisão da fórmula de Performance — decisão FINAL (2026-06-21)

**Decisão do dono (2026-06-21, FINAL — supersede planos 040 e 046)**:
`performance = income − cost_of_living` = Entradas − (Saídas + Diário).

Por que **não** subtrair `− economia` separadamente:

- A poupança é lançada como Saída (expense) no grid mensal do método → ao ser importada como
  `type='expense'`, torna-se `EventKind::FixedOut` ou `EventKind::Daily` → **já está em
  `cost_of_living_cents`**.
- A aba Economia importa o mesmo valor como `type='transfer'` para a conta reserva →
  `EventKind::Economia` → alimenta `savings_rate_bps` (Economizado%). Este é um registro de
  **taxa de poupança**, não um segundo movimento de dinheiro.
- Subtrair `economia` além de `cost_of_living` seria dupla contagem (plano 046 cometeu esse erro).

O termo `− daily_projected` continua EXCLUÍDO: a projeção de diário afeta o saldo de caixa
mas não tem linha correspondente na Performance da planilha (só o realizado aparece lá).

`economia_cents` permanece em `MonthMetric` como numerador do Economizado% e é reportado no DTO.
`realized_annual_economia` (`forecast_cmds.rs`) alimenta o guardrail de poupança —
completamente independente de `performance_cents`.
```

**Verify**: file is valid markdown; inspect visually that no fenced code block is left unclosed.

### Step 8: full gate

```
npm run check
```

Expected: exit 0. If `rust:check` fails, revisit Steps 2–5. If `typecheck` fails, revisit Step 6.

### Step 9: update plans/README.md

**Add a new row** for plan 051 (after the plan 048 row):

```
| 051  | Performance = income − cost_of_living (economia=Saída model) — supersedes 046, no double-count | P1 | M | — | DONE |
```

**Update the "Findings considered and rejected" entry** for the Performance formula (lines 112–116).

Current text:

```
- **Performance formula in `forecast/mod.rs`**: corrected by plans 040 + 046. The formula is
  `income − cost_of_living − economia` (spreadsheet-parity, 2026-06-21): economia is recorded
  as a Saída in the month grid and is therefore subtracted by the sheet. `daily_projected` is
  excluded (the sheet's Performance row uses realized figures only). The savings guardrail and
  Economizado% are computed independently and are unaffected.
```

Replace with:

```
- **Performance formula in `forecast/mod.rs`**: FINAL decision via plans 040 → 046 → 051.
  Formula: `income − cost_of_living` (Entradas − (Saídas + Diário)). Economia is NOT a
  separate deduction: the savings expense row is already in `cost_of_living` (expense →
  FixedOut/Daily); the Economia-tab transfer is a savings-rate annotation that feeds
  `savings_rate_bps` only, not Performance. Subtracting `economia` again (plan 046) was a
  double-count — plan 051 reverts it. `daily_projected` stays excluded. Do NOT re-add
  `− economia` to the formula; the decision is final and documented in spec 011.
```

**Verify**: `git diff --name-only` lists only the four in-scope files.

## Test plan

### Tests that must be updated (Step 3)

| Test                                  | File              | Line  | Old `performance_cents` | New `performance_cents` | Reason                       |
| ------------------------------------- | ----------------- | ----- | ----------------------- | ----------------------- | ---------------------------- |
| `real_daily_avg_and_savings`          | `forecast/mod.rs` | ~839  | `550_000`               | `800_000`               | Remove `− economia(250_000)` |
| `performance_excludes_only_projected` | `forecast/mod.rs` | ~1069 | `450_000`               | `650_000`               | Remove `− economia(200_000)` |

### Test that must be removed (Step 4)

`performance_economia_subtracted_once_no_double_count` — entire `#[test]` block (~lines 1075–1099).
It was added by plan 046 to guard the now-wrong `− economia` behavior. It must be deleted,
not commented out.

### New regression test to add (Step 5)

`performance_economia_not_double_counted` — in `src-tauri/src/forecast/mod.rs`, immediately
before the closing `}` of `mod tests`. Verifies:

1. `performance = income − cost_of_living` (no `− economia`).
2. `cost_of_living_cents` does NOT include the Economia amount.
3. `economia_cents` is reported separately (feeds `savings_rate_bps` only).
4. `savings_rate_bps` is unaffected.

Model the fixture after the removed test (same helpers `ev()`, `d()`, `project()`).

### Tests that must pass unchanged

- `month_performance_and_cost` — no Economia event; `performance=400_000` unchanged.
- `cash_differs_from_performance` — no Economia; unchanged.
- `performance_excludes_daily_projected_ceiling` — no Economia; `performance=1_000_000`; unchanged.
- `economia_reduces_spending_balance` (~line 975) — tests Saldo chain, not `performance_cents`.
- `classify_transfer_to_reserve_is_economia` — classify() unchanged.
- All tests in `commands/mod.rs` — none have an `Economia` event paired with a `performance_cents` assertion.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml` → all pass, including the 1 new test and 0 of the removed tests.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (includes `cargo test`)
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run check` exits 0
- [ ] `grep -n "performance_cents = income - cost_of_living_cents - economia" src-tauri/src/forecast/mod.rs` → no matches (plan 046 formula gone)
- [ ] `grep -n "performance_cents = income - cost_of_living_cents;" src-tauri/src/forecast/mod.rs` → one match (restored formula)
- [ ] `grep -n "performance_economia_subtracted_once_no_double_count" src-tauri/src/forecast/mod.rs` → no matches (wrong-model test removed)
- [ ] `grep -n "performance_economia_not_double_counted" src-tauri/src/forecast/mod.rs` → one match (correct-model test added)
- [ ] `grep -n "800_000" src-tauri/src/forecast/mod.rs` → match in `real_daily_avg_and_savings` assertion
- [ ] `grep -n "650_000" src-tauri/src/forecast/mod.rs` → match in `performance_excludes_only_projected` assertion
- [ ] `grep -n "Economia" src/design-system/components/InfoPopover.tsx` → no match in the `performance` entry body (mention removed)
- [ ] `git diff --name-only` lists only the four in-scope files
- [ ] `plans/README.md` has a row for plan 051 with status DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The formula at `src-tauri/src/forecast/mod.rs:380` does not read
  `let performance_cents = income - cost_of_living_cents - economia;` — the codebase drifted
  since this plan was written (plan 046 may have been reverted or plan 051 partially applied).
- Step 1 finds `performance_cents` used as an operand in any formula other than display or
  test comparison. Introduce a separate field and report before proceeding.
- After removing `− economia`, `npm run rust:check` fails for any reason other than the two
  test assertion mismatches documented in Step 3.
- The test `performance_economia_subtracted_once_no_double_count` is not found at ~line 1075;
  search the whole file before reporting — it may have moved.
- Any step's verification fails twice after a reasonable fix attempt.
- `git diff --name-only` includes a file outside the in-scope list (indicates the change
  unexpectedly propagates elsewhere).

## Maintenance notes

- The formula `income − cost_of_living_cents` is the FINAL spreadsheet-faithful form as of
  2026-06-21. Plans 040 and 046 both arrived at this point; 051 is the definitive fix that
  closes the oscillation. Do NOT re-add `− economia` without a new explicit owner decision.
- The key invariant: the savings money enters the engine EITHER as an expense row in
  `cost_of_living` (grid Saída → FixedOut/Daily) OR as a transfer in `EventKind::Economia`
  (Economia-tab annotation), but NOT both. `economy_cents` is a savings-rate annotation, not
  a second deduction. Any future feature that changes how savings is recorded (e.g., a unified
  "save" transaction type) must preserve this invariant or explicitly reconsider the formula.
- `safe_to_spend_today` uses `annual_savings_cents` (from `realized_annual_economia`) passed
  by value; it does NOT read `performance_cents`. This independence must be preserved.
- Part 2 of the original brief (whether the Economia-tab import could simultaneously create a
  transfer that double-counts the grid Saída in the Saldo chain) is deferred: today the Saldo
  chain uses `signed()` on all events including `EventKind::Economia`, so if the user has
  BOTH a grid Saída (expense, hits Saldo) and an Economia-tab transfer (transfer, hits Saldo),
  the savings amount would leave the Saldo twice. Investigate and fix in a follow-up plan if
  the user begins recording both simultaneously. The Performance formula fix here (plan 051)
  does not make this Saldo double-count worse or better — it is a pre-existing orthogonal risk.
- A reviewer should verify: (a) the new test asserts `performance = income − cost_of_living`
  with a non-zero Economia fixture; (b) spec 011 unambiguously says `− economia` is WRONG and
  this decision is final; (c) the README "rejected" entry warns future auditors not to re-add
  `− economia`.
