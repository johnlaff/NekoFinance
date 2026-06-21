# Plan 046: Performance correction: subtract economia (it lives in the Saída column) — refines plan 040

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
> git diff --stat 26ea4c9..HEAD -- \
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
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Package**: F
- **Planned at**: commit `26ea4c9`, 2026-06-21

## Why this matters

Plan 040 changed `performance_cents` to `income − cost_of_living_cents` for spreadsheet
parity. However, owner clarification on 2026-06-21 revealed that the real spreadsheet records
a savings transfer as a SAÍDA in the month grid — and the Economia tab records the same amount.
This means the spreadsheet's `Performance = Entradas − Saída Total` ALREADY subtracts the
economia, because the economia transfer appears in the Saída column. Plan 040's formula
(`income − cost_of_living_cents`, which omits economia) therefore diverges from the sheet by
the savings amount once the user actually saves. The faithful formula is
`income − cost_of_living_cents − economia`. This also reconciles the engine with the method's
own app behavior (both subtract economia; they only differed on `daily_projected`, which
remains excluded from Performance). A user who saves R$ 1,000 in a month sees their
spreadsheet's Performance drop by that R$ 1,000, but Neko currently does not reflect that —
breaking the "app matches the spreadsheet" trust contract.

## Current state

### Files and roles

- `src-tauri/src/forecast/mod.rs` — pure engine; `month_metrics` (private fn) computes
  `performance_cents` at **line 378**; `MonthMetric` struct (lines 52–74); tests at
  lines 793–1068.
- `src/design-system/components/InfoPopover.tsx` — glossary; `performance` entry at
  **lines 47–50**.
- `specs/011-engine-five-types/spec.md` — contains the 2026-06-20 decision note at
  **lines 51–68** (to be updated with the 2026-06-21 correction).
- `plans/README.md` — "Findings considered and rejected" entry for plan 040 at
  **lines 109–113** (to be updated).

### Relevant code excerpts

**`src-tauri/src/forecast/mod.rs`, lines 52–74** — `MonthMetric` struct (current state after plan 040):

```rust
pub struct MonthMetric {
    pub year: i32,
    pub month: u32,
    /// Renda do mês (Entradas).
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas FIXAS realizadas (coluna Saída da planilha; cartão entra como lump aqui). Exposto à
    /// parte de `cost_of_living_cents` para o rodapé mensal ENTRADAS | SAÍDAS | DIÁRIO.
    pub fixed_out_cents: i64,
    /// Diário REALIZADO (coluna Diário). `cost_of_living = fixed_out + daily_out`.
    pub daily_out_cents: i64,
    pub real_daily_avg_cents: i64,
    pub savings_rate_bps: i64,
    /// Economia lançada no mês (numerador do Economizado%). Não afeta performance (planilha-parity).
    pub economia_cents: i64,
    /// Saída TOTAL lançada no mês = fixas + diário (realizado + projetado/pré-lançado). …
    pub total_outflow_cents: i64,
}
```

**`src-tauri/src/forecast/mod.rs`, lines 372–378** — current formula (set by plan 040, now WRONG):

```rust
// Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
let cost_of_living_cents = fixed_out + daily_realized;
// Performance = Entradas − (Saídas + Diário) — fórmula da planilha (linha Performance).
// DECISÃO DO DONO (2026-06-20): paridade com planilha preferida sobre a fórmula do App.
// Economia e previsão de diário restante NÃO são descontadas aqui (afetam só o guardrail
// de poupança e o forecast de caixa, que têm suas próprias entradas).
let performance_cents = income - cost_of_living_cents;
```

The `economia` variable is already accumulated in the same loop (line 353, `let mut economia = 0i64;`;
accumulated at line 369, `EventKind::Economia => economia += e.amount_cents`). It is available in
scope at line 378 — no new variable or loop change is required.

**Confirmed: `economia` is disjoint from `cost_of_living_cents`.**
`cost_of_living_cents = fixed_out + daily_realized` (lines 372–373). `fixed_out` accumulates
`EventKind::FixedOut` events; `daily_realized` accumulates `EventKind::Daily` with `realized=true`.
`economia` accumulates `EventKind::Economia` events. These three arms of the `match e.kind` block
(lines 358–370) are mutually exclusive — no event can be both FixedOut/Daily and Economia.
Therefore subtracting `economia` once is correct and introduces no double-count.

**Confirmed: imported economia entries are stored as `type='transfer'`** (see
`src-tauri/src/commands/write_back_cmds.rs:1006`), which `classify()` routes to
`EventKind::Economia` only when `to_liquidity ∈ {reserve, illiquid}` — NOT to `FixedOut` or
`Daily`. A future grid economia-Saída row stored the same way would also become `Economia`, not
an additional `FixedOut`. The double-count risk is therefore structural-zero given the current
classification routing.

**`src/design-system/components/InfoPopover.tsx`, lines 47–50** — current `performance` glossary
entry (does not mention economia, to be updated):

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha — igual ao que você já conhece.",
},
```

**`specs/011-engine-five-types/spec.md`, lines 51–68** — current "Revisão da fórmula de
Performance (2026-06-20)" section (to be updated with the 2026-06-21 correction):

```
## Revisão da fórmula de Performance (2026-06-20)

**Decisão do dono**: a Performance exibida foi alterada para paridade com a planilha.

performance = income − cost_of_living          # = Entradas − (Saídas + Diário)

Os termos `− economia` e `− daily_projected` foram removidos da fórmula de exibição.
```

**Tests that assert `performance_cents` with an `Economia` event in scope — must be updated:**

| File                            | Test name                                                 | Current assertion                                                          | Why wrong                                         |
| ------------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------- |
| `src-tauri/src/forecast/mod.rs` | `real_daily_avg_and_savings` (line 823)                   | `performance_cents == 800_000` (income 1_000_000 − daily 200_000)          | economia 250_000 now subtracts → expected 550_000 |
| `src-tauri/src/forecast/mod.rs` | `performance_excludes_economia_and_projected` (line 1045) | `performance_cents == 650_000` (income 1_000_000 − cost_of_living 350_000) | economia 200_000 now subtracts → expected 450_000 |

**Tests that do NOT include an `Economia` event — unaffected, must still pass:**

- `month_performance_and_cost` (line 795): fixture has Income + FixedOut + Daily only; formula
  produces same result.
- `cash_differs_from_performance` (line 809): fixture has Income + FixedOut only; unaffected.
- `performance_excludes_daily_projected_ceiling` (line 1026): fixture has Income + projected
  Daily only; `economia = 0`, so formula unchanged.
- `economia_reduces_spending_balance` (line 975): tests balance chain, not `performance_cents`.
- Integration tests in `commands/mod.rs` at lines 273 and 2376: neither fixture has an
  `Economia` event; unaffected.
- Integration test in `commands/mod.rs` at line 1117 (`jun.performance_cents == -100_000`):
  fixture has Income + FixedOut + projection (`expense/debit`), no economia transfer — unaffected.

### Repo conventions

- Functional-core/imperative-shell: pure logic in `forecast/mod.rs`, IO at command layer.
- React Compiler is ON — no manual memo in TypeScript.
- Conventional commits style (e.g. `fix: …`).
- Money is always a positive-magnitude integer (cents); `performance_cents` can be negative.
- `safe_to_spend_today` (`forecast/mod.rs:132–162`) uses `annual_economia` passed by value; it
  does NOT read `performance_cents`. Changing the formula does NOT affect the guardrail.

## Commands you will need

| Purpose                | Command                                                          | Expected on success    |
| ---------------------- | ---------------------------------------------------------------- | ---------------------- |
| Rust typecheck + tests | `npm run rust:check`                                             | exit 0, all tests pass |
| Narrowed Rust tests    | `cargo test --manifest-path src-tauri/Cargo.toml performance`    | all pass               |
| Narrowed Rust tests    | `cargo test --manifest-path src-tauri/Cargo.toml real_daily_avg` | all pass               |
| Frontend typecheck     | `npm run typecheck`                                              | exit 0, no errors      |
| Lint                   | `npm run lint`                                                   | exit 0                 |
| Full gate              | `npm run check`                                                  | exit 0                 |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/forecast/mod.rs` — formula at line 378 + two doc comments + two tests +
  one new test
- `src/design-system/components/InfoPopover.tsx` — `performance` glossary body (lines 47–50)
- `specs/011-engine-five-types/spec.md` — update the 2026-06-20 decision note (lines 51–68)
- `plans/README.md` — update the "Findings considered and rejected" entry + add this plan's row

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/commands/forecast_cmds.rs` — `performance_cents` is passed through as-is
  (struct field copy); the new value propagates automatically with no change here.
- `safe_to_spend_today` and the savings guardrail — confirmed independent of `performance_cents`;
  the guardrail uses `annual_economia` fetched separately.
- `classify()`, `EventKind`, `savings_rate_bps`, `cost_of_living_cents`, `economia_cents` —
  all unchanged.
- `src-tauri/src/commands/mod.rs` — no assertion in that file involves an `Economia` event
  in the same fixture as `performance_cents`; all integration assertions remain correct.
- Any frontend React file other than `InfoPopover.tsx`.

## Git workflow

- Branch: `advisor/046-performance-include-economia`
- Commit messages follow conventional commits:
  `fix: performance subtracts economia — spreadsheet-parity correction (plan 046)`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 0: verify drift

Run the drift check from the header. Compare the "Current state" excerpts for these key lines:

- `src-tauri/src/forecast/mod.rs:378` must read:
  `let performance_cents = income - cost_of_living_cents;`
- `src-tauri/src/forecast/mod.rs:67` must read:
  `/// Economia lançada no mês (numerador do Economizado%). Não afeta performance (planilha-parity).`
- `src/design-system/components/InfoPopover.tsx:49` must read:
  `body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha — igual ao que você já conhece.",`

If any excerpt does not match, stop.

**Verify**: `git diff --stat 26ea4c9..HEAD -- src-tauri/src/forecast/mod.rs src/design-system/components/InfoPopover.tsx specs/011-engine-five-types/spec.md` → empty (no changes since plan was written) OR differences that match the live code you just read.

### Step 1: confirm performance_cents has no hidden consumers

Before changing anything, run:

```
grep -rn "performance_cents" src-tauri/src/ src/
```

Confirm every match is one of:

- field declaration in `forecast/mod.rs`
- the formula line (line 378)
- the struct literal in `month_metrics` (line 406)
- DTO struct field or pass-through mapping in `forecast_cmds.rs`
- test assertions in `forecast/mod.rs` and `commands/mod.rs`

**STOP if** you find `performance_cents` used as an operand in any formula that feeds back into cash-flow logic or safe-to-spend. In that case, introduce a separate `display_performance_cents` field and report the extra consumer before proceeding.

**Verify**: `grep -n "performance_cents" src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs` → matches only in struct definition, formula, struct literal, DTO mapping, and tests. Zero matches in `safe_to_spend_today` or any arithmetic that derives a new value.

### Step 2: update the formula and doc comments in forecast/mod.rs

Open `src-tauri/src/forecast/mod.rs`. Locate lines 372–378 (the formula block inside
`month_metrics`). Replace the formula and its comments:

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

Also update the `economia_cents` field doc comment in `MonthMetric` (line 67) from:

```rust
/// Economia lançada no mês (numerador do Economizado%). Não afeta performance (planilha-parity).
pub economia_cents: i64,
```

to:

```rust
/// Economia lançada no mês (numerador do Economizado%). Desconta a Performance porque a
/// planilha a registra como Saída no grid mensal (fiel ao método — 2026-06-21).
pub economia_cents: i64,
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance` → fails on the two
tests that have non-zero economia (expected — fixed in Step 3). No NEW compilation errors
beyond test assertion mismatches.

### Step 3: update the two failing tests in forecast/mod.rs

**Test `real_daily_avg_and_savings`** (around line 823):

Fixture: `Income=1_000_000`, `Daily realized = 4×50_000 = 200_000`, `Economia = 250_000`.

`cost_of_living = 200_000`. New `performance = 1_000_000 − 200_000 − 250_000 = 550_000`.

Find this assertion (around line 835–836):

```rust
// Performance = renda (1000) − custo de vida (diário 200) = 800 (economia não desconta).
assert_eq!(m.performance_cents, 800000);
```

Replace with:

```rust
// Performance = renda (1000) − custo de vida (diário 200) − economia (250) = 550
// (a economia é lançada como Saída na planilha, portanto desconta a Performance).
assert_eq!(m.performance_cents, 550_000);
```

**Test `performance_excludes_economia_and_projected`** (around line 1045):

Fixture: `Income=1_000_000`, `FixedOut=300_000`, `Daily realized=50_000`, `Economia=200_000`,
`Daily projected=30_000` (realized=false).

`cost_of_living = 300_000 + 50_000 = 350_000`. New `performance = 1_000_000 − 350_000 − 200_000 = 450_000`.

Find this block (around lines 1063–1064):

```rust
// performance = income(1_000) − cost_of_living(350) = 650_000 (NOT 420_000 old formula)
assert_eq!(m.performance_cents, 650_000);
```

Replace with:

```rust
// performance = income(1_000) − cost_of_living(350) − economia(200) = 450_000
// (economia desconta Performance; daily_projected NÃO desconta — não está na planilha)
assert_eq!(m.performance_cents, 450_000);
```

Also update the test name comment above the test (around line 1043):

```rust
// Regressão: economia e previsão de diário NÃO afetam performance (planilha-parity 2026-06-20).
```

Replace with:

```rust
// Regressão: economia desconta Performance; previsão de diário NÃO desconta (planilha-parity
// 2026-06-21 — economia é lançada como Saída no grid; daily_projected não tem linha na planilha).
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance real_daily_avg` → all pass.

### Step 4: add a double-count guard test in forecast/mod.rs

Add a new test directly after `performance_excludes_economia_and_projected` (after line 1068,
before the closing `}` of the `mod tests` block).

This test verifies:

1. A month with an Economia event has `performance = income − cost_of_living − economia`.
2. The economia event is NOT inside `cost_of_living_cents` (no double-count).

```rust
// Guarda dupla-contagem: um mês com Economia lançada tem Performance = renda − custo_de_vida
// − economia; a Economia NÃO está dentro do custo_de_vida (FixedOut/Daily são disjuntos de
// EventKind::Economia na classificação — cada evento só cai em um braço do match).
#[test]
fn performance_economia_subtracted_once_no_double_count() {
    // Arrange: renda 5_000_000, Saída fixa 1_000_000, Diário realizado 500_000, Economia 800_000.
    let events = [
        ev("2026-05-01", EventKind::Income, 5_000_000),
        ev("2026-05-10", EventKind::FixedOut, 1_000_000),
        ev("2026-05-15", EventKind::Daily, 500_000),    // realized
        ev("2026-05-20", EventKind::Economia, 800_000), // savings transfer
    ];
    let f = project(0, d("2026-05-01"), &events, d("2026-05-31"));
    let m = f.months.iter().find(|m| m.month == 5).unwrap();

    // cost_of_living = FixedOut(1_000) + Daily(500) = 1_500_000 (Economia NOT in here)
    assert_eq!(m.cost_of_living_cents, 1_500_000);
    // economia_cents reported separately
    assert_eq!(m.economia_cents, 800_000);
    // performance = income(5_000) − cost_of_living(1_500) − economia(800) = 2_700_000
    // (would be 3_500_000 if economia were omitted, or 1_900_000 if double-counted)
    assert_eq!(m.performance_cents, 2_700_000);
    // savings_rate_bps = 800_000 / 5_000_000 = 1600 bps (16%)
    assert_eq!(m.savings_rate_bps, 1_600);
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance_economia_subtracted_once` → 1 test passes.

### Step 5: update the InfoPopover glossary

Open `src/design-system/components/InfoPopover.tsx`. Find the `performance` entry at lines 47–50:

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha — igual ao que você já conhece.",
},
```

Replace the body to mention Economia:

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: Entradas menos Saídas (incluem fixas, fatura do cartão e o que você guardou como Economia) e Diário. É o mesmo cálculo da sua planilha.",
},
```

**Verify**: `npm run typecheck` → exit 0.

### Step 6: update spec 011

Open `specs/011-engine-five-types/spec.md`. Find the "Revisão da fórmula de Performance
(2026-06-20)" section (lines 51–68). Replace it entirely:

```markdown
## Revisão da fórmula de Performance (2026-06-21)

**Decisão do dono (2026-06-20, corrigida em 2026-06-21)**: a Performance exibida deve ser fiel
à planilha. A clarificação de 2026-06-21 revelou que a Economia é lançada como Saída no grid
mensal — portanto, a planilha já desconta a Economia na Performance (Saída Total inclui o
lançamento de economia). A fórmula correta é:
```

performance = income − cost_of_living − economia # = Entradas − (Saídas + Diário + Economia)

```

O termo `− daily_projected` continua EXCLUÍDO (a projeção de diário afeta o saldo de caixa
mas não tem linha correspondente na Performance da planilha — só o realizado aparece lá).

Economia continua sendo o numerador do Economizado% (savings_rate_bps) e continua alimentando
o guardrail de poupança anual via `realized_annual_economia` (independente de `performance_cents`).

Esta seção substitui a nota de 2026-06-20 do plano 040, que erroneamente excluía a Economia da
Performance. A fórmula do plano 040 (`income − cost_of_living`) era incorreta — divergia da
planilha pelo valor da economia guardada. O plano 046 corrige essa divergência.
```

(The closing triple-backtick above closes the inline code block for `performance = …` — make sure
the markdown fence for the outer code block is correct: the `performance = …` formula block uses
its own triple-backtick pair, and the outer section body is plain markdown.)

**Verify**: file is valid markdown — no broken fences. Visual inspection sufficient.

### Step 7: full gate

Run:

```
npm run check
```

Expected: exit 0. If `rust:check` fails, revisit Steps 2–4. If `typecheck` fails, revisit Step 5.

### Step 8: update plans/README.md

**Add a new row** after plan 045:

```
| 046  | Performance correction: subtract economia (it lives in the Saída column) — refines plan 040 | P1 | S–M | — | DONE |
```

**Update the "Findings considered and rejected" entry** for plan 040 (lines 109–113):

Current text:

```
- **Performance formula in `forecast/mod.rs`**: changed by plan 040 (2026-06-20) to
  `income − cost_of_living` (spreadsheet-parity). The richer App/method formula (which also
  subtracted Economia and projected remaining Diário) was removed from the display metric by
  owner decision; the savings guardrail and Economizado% are unaffected. This is a deliberate
  reversal of the spec 011 formula — do not "fix" it back to the App/method formula.
```

Replace with:

```
- **Performance formula in `forecast/mod.rs`**: corrected by plans 040 + 046. The formula is
  `income − cost_of_living − economia` (spreadsheet-parity, 2026-06-21): economia is recorded
  as a Saída in the month grid and is therefore subtracted by the sheet. `daily_projected` is
  excluded (the sheet's Performance row uses realized figures only). The savings guardrail and
  Economizado% are computed independently and are unaffected.
```

**Verify**: `git diff --name-only` lists only files in the in-scope list.

## Test plan

### Tests that must be updated (Steps 3 and their expected new values)

| Test                                          | File                   | Old `performance_cents` | New `performance_cents` | Reason                          |
| --------------------------------------------- | ---------------------- | ----------------------- | ----------------------- | ------------------------------- |
| `real_daily_avg_and_savings`                  | `forecast/mod.rs:823`  | 800_000                 | 550_000                 | economia 250_000 now subtracted |
| `performance_excludes_economia_and_projected` | `forecast/mod.rs:1045` | 650_000                 | 450_000                 | economia 200_000 now subtracted |

### New test to add (Step 4)

`performance_economia_subtracted_once_no_double_count` — in `src-tauri/src/forecast/mod.rs`,
after `performance_excludes_economia_and_projected`. Verifies:

1. `performance = income − cost_of_living − economia` (the faithful formula).
2. `cost_of_living_cents` does NOT include the Economia amount (no double-count).
3. `economia_cents` is reported separately (feeds savings_rate_bps independently).

Use the helpers `ev()` and `d()` already defined in the test module. Model the fixture after
`performance_excludes_economia_and_projected` (same helper pattern, different amounts).

### Tests that must pass unchanged

- `month_performance_and_cost` — no Economia in fixture; formula result identical.
- `cash_differs_from_performance` — no Economia; unaffected.
- `performance_excludes_daily_projected_ceiling` — Economia = 0; `income − 0 − 0 = income`, unchanged.
- `economia_reduces_spending_balance` — tests balance chain, not `performance_cents`.
- All tests in `commands/mod.rs` — no `Economia` transfer in any performance-asserting fixture.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml` → all pass, including the 1 new test.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (includes `cargo test`)
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run check` exits 0
- [ ] `grep -n "performance_cents = income - cost_of_living_cents;" src-tauri/src/forecast/mod.rs` returns no matches (old formula gone)
- [ ] `grep -n "performance_cents = income - cost_of_living_cents - economia;" src-tauri/src/forecast/mod.rs` returns a match (new formula present)
- [ ] `grep -n "performance_economia_subtracted_once_no_double_count" src-tauri/src/forecast/mod.rs` returns a match (new guard test exists)
- [ ] `grep -n "550_000" src-tauri/src/forecast/mod.rs` returns a match in `real_daily_avg_and_savings` (updated assertion)
- [ ] `grep -n "450_000" src-tauri/src/forecast/mod.rs` returns a match in `performance_excludes_economia_and_projected` (updated assertion)
- [ ] The `performance` glossary body in `src/design-system/components/InfoPopover.tsx` mentions "Economia"
- [ ] `git diff --name-only` lists only the four in-scope files
- [ ] `plans/README.md` has a row for plan 046 with status DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The formula at `src-tauri/src/forecast/mod.rs:378` does not read
  `let performance_cents = income - cost_of_living_cents;` — the codebase drifted (plan 040
  may not have landed or was partially reverted).
- Step 1 finds `performance_cents` used as an operand in any formula other than display or
  test comparison. If so, introduce a separate field and report.
- After adding `− economia` to the formula, `npm run rust:check` fails with a compile error
  other than assertion mismatches (would indicate `economia` is out of scope at line 378 — check
  the accumulation loop).
- The integration test at `commands/mod.rs:1117` (`jun.performance_cents == -100_000`) starts
  failing after Step 2: the June fixture includes a future-projection expense but no
  `Economia` transfer — if it fails, a economia event was added to that fixture since this plan
  was written; read the fixture carefully and report.
- Any step's verification fails twice after a reasonable fix attempt.
- Changing `performance_cents` appears to require touching `forecast_cmds.rs` beyond a comment
  (the DTO pass-through should be transparent).

## Maintenance notes

- The formula `income − cost_of_living_cents − economia` is the spreadsheet-faithful form.
  `daily_projected` stays excluded. If a future feature needs a "projected surplus" that also
  subtracts projected remaining daily, introduce a separate field (e.g. `projected_surplus_cents`)
  rather than re-adding `daily_projected` to `performance_cents`.
- The savings guardrail in `safe_to_spend_today` (`forecast/mod.rs:132–162`) uses
  `annual_economia` passed by value — it does NOT read `performance_cents`. This independence
  must be preserved in any future refactor of the guardrail.
- `economia_cents` in `MonthMetric` is the single source for Economizado% (savings_rate_bps)
  and is reported separately in the DTO. It is both the discount in the performance formula
  AND the numerator in savings_rate_bps — do not remove the field or its accumulation.
- If the user begins recording the savings transfer via the month grid (as a Saída row) AND the
  Economia tab simultaneously, the import must deduplicate to avoid double-counting the economia
  event in the DB. That deduplication is outside the scope of this plan; a reviewer should note
  the future risk in the PR.
- A reviewer should check: (a) the double-count guard test in Step 4 asserts
  `cost_of_living_cents` does NOT contain the Economia amount; (b) the spec 011 update clearly
  marks this as superseding plan 040's decision, not as a second independent decision.
