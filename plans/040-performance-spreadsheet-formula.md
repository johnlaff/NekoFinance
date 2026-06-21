# Plan 040: Performance = spreadsheet formula (Entradas − (Saídas + Diário)) — owner decision

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
> git diff --stat d3922d2..HEAD -- \
>   src-tauri/src/forecast/mod.rs \
>   src-tauri/src/commands/mod.rs \
>   src-tauri/src/commands/forecast_cmds.rs \
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
- **Package**: E
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

The spreadsheet's Performance row is the authoritative user-visible metric: it simply shows
`Entradas − Saída Total`, where "Saída Total" is `Saídas (fixas + cartão lump) + Diário
realizado`. Neko currently computes a richer formula —
`income − cost_of_living − economia − daily_projected` — that was faithful to the method's
App logic (spec 011) but diverges from what the spreadsheet actually shows. The owner decided
on 2026-06-20 that spreadsheet-parity is the right choice for this specific metric: dropping
`− economia − daily_projected` aligns the displayed number with the source-of-truth document
users compare against. This change is deliberate and must be recorded as such, not silently
reverted by a future reviewer who sees the spec 011 formula.

## Current state

### Files and roles

- `src-tauri/src/forecast/mod.rs` — core engine; `MonthMetric` struct (line 52–75) +
  `month_metrics` (private fn at line 341) + `project_daily_ceiling` + tests.
- `src-tauri/src/commands/forecast_cmds.rs` — Tauri command layer; maps `MonthMetric` →
  `MonthMetricDto` (struct at line 552, mapping at lines 783–794, 843–854).
- `src-tauri/src/commands/mod.rs` — integration tests; three assertions that reference
  `performance_cents` with the old formula.
- `src/design-system/components/InfoPopover.tsx` — glossary; `performance` entry at line 47–50.
- `specs/011-engine-five-types/spec.md` — spec that documented the richer formula.
- `plans/README.md` — must have this plan's row added and the "Findings considered and
  rejected" entry for Performance updated to reflect the owner reversal.

### Relevant code excerpts

**`src-tauri/src/forecast/mod.rs`, lines 52–74** — `MonthMetric` struct (field docs note the
old formula dependency):

```rust
pub struct MonthMetric {
    pub year: i32,
    pub month: u32,
    /// Renda do mês (Entradas). Guardada à parte porque `performance` já desconta economia e a
    /// previsão de diário restante, então não é mais reconstituível só de `performance + custo`.
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub real_daily_avg_cents: i64,
    pub savings_rate_bps: i64,
    /// Economia lançada no mês (numerador do Economizado%). Já descontada da performance.
    pub economia_cents: i64,
    pub total_outflow_cents: i64,
}
```

**`src-tauri/src/forecast/mod.rs`, lines 373–379** — current (old) formula with comments:

```rust
// Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
// NÃO inclui economia nem a previsão de diário restante.
let cost_of_living_cents = fixed_out + daily_realized;
// Performance = Entradas − (custo de vida + economia + previsão de diário restante).
// O termo `daily_projected` faz o mês "nascer no vermelho e esverdeia" conforme o
// diário real fica abaixo do teto.
let performance_cents = income - cost_of_living_cents - economia - daily_projected;
```

**`src-tauri/src/forecast/mod.rs`, lines 396–400 and 403–415** — savings rate + struct literal:

```rust
let savings_rate_bps = if income > 0 {
    economia * 10_000 / income
} else {
    0
};

MonthMetric {
    year,
    month,
    income_cents: income,
    performance_cents,
    cost_of_living_cents,
    fixed_out_cents: fixed_out,
    daily_out_cents: daily_realized,
    real_daily_avg_cents,
    savings_rate_bps,
    economia_cents: economia,
    total_outflow_cents: fixed_out + daily_realized + daily_projected,
}
```

**`src/design-system/components/InfoPopover.tsx`, lines 47–50** — current glossary body (to
be replaced):

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: o que entrou menos tudo que saiu — Saídas (já incluem as fixas e a fatura do cartão), Diário, Economia e a previsão do diário que ainda falta. Por isso o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
},
```

**Tests asserting the old formula** (all in-scope, must be updated):

| File                            | Line      | Old assertion                                                      | Reason old is wrong                                              |
| ------------------------------- | --------- | ------------------------------------------------------------------ | ---------------------------------------------------------------- |
| `src-tauri/src/forecast/mod.rs` | 836       | `performance_cents == 550000` (1000 − 200 daily − 250 economia)    | economia now stays out                                           |
| `src-tauri/src/forecast/mod.rs` | 1038      | `performance_cents == 790000` (1000 − 210 projected)               | daily_projected now stays out                                    |
| `src-tauri/src/commands/mod.rs` | 273       | `performance_cents == 450_000` (700 − 250)                         | already matches new formula — no economia/projected in this test |
| `src-tauri/src/commands/mod.rs` | 1112–1114 | `performance_cents == 1_000_000 - 1_100_000 - projected_daily_jun` | projected_daily_jun term must be removed                         |
| `src-tauri/src/commands/mod.rs` | 2373      | `performance_cents == 400_000` (500 − 100)                         | already matches new formula — no economia in this test           |

**`src-tauri/src/forecast/mod.rs`, lines 795–806** — test `month_performance_and_cost`
(already agrees with new formula, no change needed):

```rust
fn month_performance_and_cost() {
    // income=1_000_000, FixedOut=400_000, Daily=200_000 (realized)
    assert_eq!(m.cost_of_living_cents, 600000); // 400 + 200
    assert_eq!(m.performance_cents, 400000);    // 1000 - 600  ← same under new formula
}
```

**`src-tauri/src/forecast/mod.rs`, lines 822–838** — test `real_daily_avg_and_savings`
(has economia=250_000; performance assertion must change):

```rust
events.push(ev("2026-03-09", EventKind::Economia, 250000));
// Old: assert_eq!(m.performance_cents, 550000);  (1000 - 200 daily - 250 economia)
// New: assert_eq!(m.performance_cents, 800000);  (1000 - 200 daily)
```

**`src-tauri/src/forecast/mod.rs`, lines 1023–1038** — test `performance_includes_remaining_daily_ceiling`
(21 projected events × 10_000; performance assertion must change):

```rust
// Old: assert_eq!(m.performance_cents, 790000);  (1000 − 210 projected)
// New: assert_eq!(m.performance_cents, 1000000); (1000 − 0 cost_of_living, no projected, no economia)
```

The test is still useful — it should become a **negative** assertion confirming that projected
daily does NOT change performance under the new formula.

**`src-tauri/src/commands/mod.rs`, lines 1103–1114** — integration test comment + assertion:

```rust
// Old comment: "Inclui também a PREVISÃO de diário restante..."
// Old assertion: 1_000_000 - 1_100_000 - projected_daily_jun
// New: performance = 1_000_000 - 1_100_000 = -100_000
```

**`specs/011-engine-five-types/spec.md`, lines 43–48** — documented the richer formula:

```
performance      = income − cost_of_living − economia − daily_projected
```

### What the guardrail uses

`safe_to_spend_today` (`forecast/mod.rs`, lines 132–162) computes the savings guardrail from
`annual_economia` (fetched via `realized_annual_economia` in `forecast_cmds.rs`, line 650).
It does NOT read `performance_cents`. This is confirmed by the call site (line 651–657 of
`forecast_cmds.rs`): the arguments are `annual_income`, `annual_economia`, `SAVINGS_TARGET_BPS`,
and `reserve_floor_cents` — no `performance_cents` involved. Removing `economia` and
`daily_projected` from the performance formula therefore does NOT affect the guardrail.
The only consumer of `performance_cents` is display (DTO mapping and frontend).

### Repo conventions

- Functional-core/imperative-shell: pure logic in `forecast/mod.rs`, IO at command layer.
- React Compiler is ON — no manual memo in TypeScript.
- Conventional commits style (e.g. `fix: ...`).
- Money is always a positive-magnitude integer (cents); `performance_cents` can be negative.

## Commands you will need

| Purpose                | Command                                                       | Expected on success    |
| ---------------------- | ------------------------------------------------------------- | ---------------------- |
| Rust typecheck + tests | `npm run rust:check`                                          | exit 0, all tests pass |
| Frontend typecheck     | `npm run typecheck`                                           | exit 0, no errors      |
| Lint                   | `npm run lint`                                                | exit 0                 |
| Full gate              | `npm run check`                                               | exit 0                 |
| Unit tests (Rust only) | `cargo test --manifest-path src-tauri/Cargo.toml`             | exit 0                 |
| Narrowed test filter   | `cargo test --manifest-path src-tauri/Cargo.toml performance` |                        |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/forecast/mod.rs` — formula + doc comments + two tests
- `src-tauri/src/commands/mod.rs` — two integration test assertions
- `src/design-system/components/InfoPopover.tsx` — `performance` glossary body
- `specs/011-engine-five-types/spec.md` — record the deliberate reversal
- `plans/README.md` — add this plan's row; update "Findings considered and rejected"

**Out of scope** (do NOT touch):

- `src-tauri/src/commands/forecast_cmds.rs` — no change needed; `performance_cents` is
  passed through as-is (struct field copy), so the new value propagates automatically.
- `safe_to_spend_today` and the savings guardrail — confirmed independent of `performance_cents`.
- `classify()`, `EventKind`, `savings_rate_bps`, `cost_of_living_cents`, `economia_cents` —
  all unchanged.
- Any frontend React file other than `InfoPopover.tsx`.
- `src-tauri/src/forecast/mod.rs` test `month_performance_and_cost` (lines 795–806) — already
  correct under both formulas; verify it still passes, do not rewrite.
- `src-tauri/src/commands/mod.rs` lines 273 and 2373 — already agree with the new formula
  (no economia/projected in those test fixtures); verify they still pass, do not rewrite.

## Git workflow

- Branch: `advisor/040-performance-spreadsheet-formula`
- Commit messages follow conventional commits (match `git log` style, e.g.
  `fix: performance = Entradas − (Saídas + Diário) — spreadsheet-parity (owner decision)`).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 0: verify drift

Run the drift check from the header. If any file shows changes relative to `d3922d2`, read
the diff and confirm the "Current state" excerpts still match. If they do not, stop.

**Verify**: `git diff --stat d3922d2..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/forecast_cmds.rs src/design-system/components/InfoPopover.tsx specs/011-engine-five-types/spec.md` → empty output (or no relevant content change in the listed excerpts)

### Step 1: confirm performance_cents has no other consumers

Before touching anything, run:

```
grep -rn "performance_cents" src-tauri/src/ src/
```

Expected matches: the struct field in `forecast/mod.rs`, the formula line, the struct literal,
the two DTO mappings in `forecast_cmds.rs` (lines 787, 847), the `MonthMetricDto` struct
field (line 552), and the test assertions. No match should be in a `safe_to_spend` context or
any calculation that feeds back into cash-flow logic.

**Verify**: every match is either a field declaration, a DTO pass-through, or a test assertion
→ zero matches in `safe_to_spend_today`, `realized_annual_economia`, or any arithmetic that
computes a new derived value from `performance_cents`.

**STOP if** you find `performance_cents` used as an operand in any formula other than display
or comparison-for-display. In that case, introduce a separate `display_performance_cents`
field instead of mutating the shared `performance_cents`, then update the DTO mapping to use
the new field and keep the old one for the consuming formula. Report the extra consumer.

### Step 2: update the formula and doc comments in forecast/mod.rs

Open `src-tauri/src/forecast/mod.rs`. Locate lines 373–379 (the formula block inside
`month_metrics`).

Replace the formula and its comments with the new spreadsheet-faithful version:

```rust
// Custo de vida = Saídas fixas + Diário realizado (cartão já entra em fixed_out via lump).
let cost_of_living_cents = fixed_out + daily_realized;
// Performance = Entradas − (Saídas + Diário) — fórmula da planilha (linha Performance).
// DECISÃO DO DONO (2026-06-20): paridade com planilha preferida sobre a fórmula do App.
// Economia e previsão de diário restante NÃO são descontadas aqui (afetam só o guardrail
// de poupança e o forecast de caixa, que têm suas próprias entradas).
let performance_cents = income - cost_of_living_cents;
```

Also update the doc comment on `income_cents` in `MonthMetric` (lines 56–57), which currently
says "Guardada à parte porque `performance` já desconta economia e a previsão de diário
restante, então não é mais reconstituível só de `performance + custo`." — this is now wrong.
Replace with a neutral comment:

```rust
/// Renda do mês (Entradas).
pub income_cents: i64,
```

And update the `economia_cents` field doc (line 68):

```rust
/// Economia lançada no mês (numerador do Economizado%). Não afeta performance (planilha-parity).
pub economia_cents: i64,
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance` → fails on the two
outdated performance assertions (expected — they will be fixed in Step 3). No NEW compilation
errors.

### Step 3: update the two failing tests in forecast/mod.rs

**Test `real_daily_avg_and_savings`** (around line 822, file `src-tauri/src/forecast/mod.rs`):

The fixture has `Income=1_000_000`, `Daily realized=4×50_000=200_000`, `Economia=250_000`.

Old assertion: `assert_eq!(m.performance_cents, 550000);` (1_000_000 − 200_000 − 250_000)

New assertion (spreadsheet formula — economia stays out):

```rust
// Performance = Entradas − (Saídas + Diário realizado) = 1_000_000 − 200_000 = 800_000.
// Economia não entra na Performance (paridade com planilha); afeta só o Economizado%.
assert_eq!(m.performance_cents, 800000);
```

Update the comment on the line above it (line 836 area):

```rust
// Performance = renda (1000) − custo de vida (diário 200) = 800 (economia não desconta).
```

**Test `performance_includes_remaining_daily_ceiling`** (around line 1024, file
`src-tauri/src/forecast/mod.rs`):

The fixture has `Income=1_000_000`, `Daily projected=21 days × 10_000=210_000`, zero realized
daily. Under the new formula, `cost_of_living = fixed_out + daily_realized = 0 + 0 = 0`, so
`performance = 1_000_000 − 0 = 1_000_000`.

The test name claimed "performance includes remaining daily ceiling" — that behavior is now
gone. Rename the test to reflect the new behavior and rewrite it as a confirmation that
projected daily does NOT affect performance:

```rust
// Confirma que a previsão de diário (daily_projected) NÃO desconta a Performance
// (paridade com planilha — DECISÃO DO DONO 2026-06-20).
// Custo de vida = 0 (sem diário realizado); previsão é só para o saldo de caixa.
#[test]
fn performance_excludes_daily_projected_ceiling() {
    let mut events = vec![ev("2026-03-01", EventKind::Income, 1000000)];
    events.extend(project_daily_ceiling(
        10000,
        d("2026-03-10"),
        d("2026-03-31"),
        &HashSet::new(),
    ));
    let f = project(0, d("2026-03-10"), &events, d("2026-03-31"));
    let m = f.months.iter().find(|m| m.month == 3).unwrap();
    assert_eq!(m.cost_of_living_cents, 0);    // previsão NÃO entra no custo de vida
    assert_eq!(m.real_daily_avg_cents, 0);    // previsão NÃO conta como realizado
    // New: performance = income − cost_of_living = 1_000_000 − 0 = 1_000_000
    assert_eq!(m.performance_cents, 1000000); // previsão NÃO desconta performance
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml performance` → all pass.

### Step 4: update the integration test in commands/mod.rs

Open `src-tauri/src/commands/mod.rs`. Find the block around lines 1100–1118.

The old assertion at lines 1111–1114:

```rust
let projected_daily_jun = (220_000 / 31) * 17;
assert_eq!(
    jun.performance_cents,
    1_000_000 - 1_100_000 - projected_daily_jun
); // −220.632
```

The new formula: `performance = income − cost_of_living`. In this fixture, `income=1_000_000`,
`fixed_out(cost_of_living) = 1_100_000`, so `performance = -100_000`.

Replace with:

```rust
// Performance = Entradas − (Saídas + Diário realizado) = 1_000_000 − 1_100_000 = −100_000.
// A previsão de diário restante NÃO desconta a Performance (planilha-parity, 2026-06-20).
assert_eq!(jun.performance_cents, -100_000);
```

Remove the now-unused local variable `projected_daily_jun` if it is only used in this one
assertion. If it is used elsewhere in the same test, keep it.

Also update the comment block around lines 1104–1108 that described the old behavior. Replace
the sentence about "Inclui também a PREVISÃO de diário restante" with a note that projected
daily affects the cash balance but not the performance metric.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml forecast_dto` → passes (or use
the test function name if it differs — search for `fn forecast_dto_` or similar near line 1100).

### Step 5: update the InfoPopover glossary

Open `src/design-system/components/InfoPopover.tsx`. Find the `performance` entry at lines
47–50:

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: o que entrou menos tudo que saiu — Saídas (já incluem as fixas e a fatura do cartão), Diário, Economia e a previsão do diário que ainda falta. Por isso o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
},
```

Replace the body with the spreadsheet formula explanation (method-neutral language):

```ts
performance: {
  title: "Performance",
  body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha — igual ao que você já conhece.",
},
```

(Keep the body concise — the InfoPopover budget is 1–2 sentences.)

**Verify**: `npm run typecheck` → exit 0.

### Step 6: update spec 011

Open `specs/011-engine-five-types/spec.md`. At lines 43–48, the formula block reads:

```
daily_realized   = Σ Daily realizado
daily_projected  = Σ Daily projetado (teto + futuros pré-lançados)
economia         = Σ Economia (mês)
cost_of_living   = fixed_out + daily_realized            (cartão já em fixed_out)
performance      = income − cost_of_living − economia − daily_projected
savings_rate_bps = economia × 10000 / income             (0 se income ≤ 0)
real_daily_avg   = daily_realized / dias_decorridos      (inalterado)
```

Append a note below the formula block (after line 49, before the DoD section):

```markdown
## Revisão da fórmula de Performance (2026-06-20)

**Decisão do dono**: a Performance exibida foi alterada para paridade com a planilha.
```

performance = income − cost_of_living # = Entradas − (Saídas + Diário)

```

Os termos `− economia` e `− daily_projected` foram removidos da fórmula de exibição.
Economia continua sendo o numerador do Economizado% e continua alimentando o guardrail de
poupança anual via `realized_annual_economia` (independente de `performance_cents`).
A previsão de diário restante continua reduzindo o saldo de caixa projetado (correto para o
forecast); ela não afeta mais a Performance.

Esta é uma reversão deliberada em relação à fórmula do spec original. O motivo: o usuário
compara a Performance do Neko com a linha correspondente na sua planilha — qualquer divergência
quebra a confiança. Paridade de planilha prevalece sobre a fidelidade ao comportamento do App
neste métrica específica.
```

**Verify**: file is valid markdown (no broken fences) — visual inspection sufficient.

### Step 7: full gate

Run the complete check suite:

```
npm run check
```

Expected: exit 0. If `rust:check` fails, revisit Steps 2–4. If `typecheck` fails, revisit
Step 5.

### Step 8: update plans/README.md

Add a row for this plan in the table (after plan 039):

```
| 040  | Performance = spreadsheet formula (Entradas − (Saídas + Diário)) — owner decision | P1 | S–M | — | DONE |
```

Also update the "Findings considered and rejected" entry that currently reads
(lines 103–104 of `plans/README.md`):

```
- **Performance formula in `forecast/mod.rs`**: correct — subtracts Economia and projected
  remaining Diário, and `savings_rate_bps` uses registered Economia. No change.
```

Replace with:

```
- **Performance formula in `forecast/mod.rs`**: changed by plan 040 (2026-06-20) to
  `income − cost_of_living` (spreadsheet-parity). The richer formula (`− economia −
  daily_projected`) was removed from the display metric by owner decision; the guardrail and
  Economizado% are unaffected.
```

**Verify**: `git diff --stat` shows only files in the in-scope list.

## Test plan

### Existing tests that must still pass (no modification needed)

- `month_performance_and_cost` — fixture has no economia/projected; already matches new formula.
- `cash_differs_from_performance` — fixture has no economia/projected; unchanged.
- Annual metrics test at `commands/mod.rs:273` — `performance_cents == 450_000` (700 − 250 expense, no economia).
- Tag-exclude test at `commands/mod.rs:2373` — `performance_cents == 400_000` (500 − 100 expense, no economia).

### Tests that must be updated (Step 3 and 4)

- `real_daily_avg_and_savings`: `performance_cents` changes from 550_000 → 800_000.
- `performance_includes_remaining_daily_ceiling` (renamed `performance_excludes_daily_projected_ceiling`):
  assertion changes from 790_000 → 1_000_000.
- Integration test in `commands/mod.rs` around line 1112: assertion changes from
  `1_000_000 − 1_100_000 − projected_daily_jun` → `-100_000`.

### New regression test to add (in Step 3 or as a separate test)

Add one new test in `src-tauri/src/forecast/mod.rs` covering the specific case the owner
cared about: a month with both registered `Economia` and projected daily — performance equals
`income − cost_of_living` only:

```rust
// Regressão: economia e previsão de diário NÃO afetam performance (planilha-parity 2026-06-20).
#[test]
fn performance_excludes_economia_and_projected() {
    let events = [
        ev("2026-04-01", EventKind::Income, 1_000_000),
        ev("2026-04-05", EventKind::FixedOut, 300_000),
        ev("2026-04-08", EventKind::Daily, 50_000),   // realized
        ev("2026-04-09", EventKind::Economia, 200_000),
        // projected daily (realized=false) — normally injected by project_daily_ceiling
        CashflowEvent {
            date: d("2026-04-15"),
            kind: EventKind::Daily,
            amount_cents: 30_000,
            realized: false,
        },
    ];
    let f = project(0, d("2026-04-10"), &events, d("2026-04-30"));
    let m = f.months.iter().find(|m| m.month == 4).unwrap();
    // cost_of_living = fixed_out(300) + daily_realized(50) = 350_000
    assert_eq!(m.cost_of_living_cents, 350_000);
    // performance = income(1_000) − cost_of_living(350) = 650_000 (NOT 420_000 old formula)
    assert_eq!(m.performance_cents, 650_000);
    // economia still feeds savings_rate
    assert_eq!(m.economia_cents, 200_000);
    assert_eq!(m.savings_rate_bps, 2_000); // 200/1000 = 20%
}
```

Place this test after `performance_excludes_daily_projected_ceiling`.

Use `src-tauri/src/forecast/mod.rs` helper `ev()` and `d()` (already defined in the test module)
and the existing `CashflowEvent` struct literal pattern.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml` → all pass, including
`performance_excludes_economia_and_projected`.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (includes `cargo test`)
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run check` exits 0
- [ ] `grep -n "income - cost_of_living_cents - economia - daily_projected" src-tauri/src/forecast/mod.rs` returns no matches
- [ ] `grep -n "performance_excludes_economia_and_projected" src-tauri/src/forecast/mod.rs` returns a match (new regression test exists)
- [ ] The `performance` glossary body in `src/design-system/components/InfoPopover.tsx` no longer mentions "Economia e a previsão do diário que ainda falta"
- [ ] `git diff --name-only` lists only the five in-scope files (plus `plans/README.md`)
- [ ] `plans/README.md` has a row for plan 040 with status DONE
- [ ] `specs/011-engine-five-types/spec.md` contains the "Revisão da fórmula de Performance (2026-06-20)" section

## STOP conditions

Stop and report back (do not improvise) if:

- The formula at `src-tauri/src/forecast/mod.rs:379` does not match the excerpt in "Current
  state" (the codebase drifted since this plan was written).
- Step 1 finds `performance_cents` used as an operand in any non-display formula — this plan
  assumes it is display-only; if not, a separate field is needed.
- Removing `− economia − daily_projected` causes `npm run rust:check` to fail with an error
  other than the expected test assertion mismatches — this would indicate an undiscovered consumer.
- Any step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching `forecast_cmds.rs` beyond a comment change
  (the DTO pass-through should be transparent to the formula change).
- The test fixture in `commands/mod.rs` around line 1100 is more complex than described, and
  the expected `performance_cents` is not simply `1_000_000 − 1_100_000`.

## Maintenance notes

- The richer formula (`income − cost_of_living − economia − daily_projected`) is intentionally
  removed from `performance_cents`. If a future feature needs a "projected surplus" figure that
  includes those deductions, introduce a separate field (e.g. `projected_surplus_cents`) rather
  than reverting this change.
- The savings guardrail in `safe_to_spend_today` (`forecast/mod.rs:132–162`) uses
  `annual_economia` passed by value — it does not read `performance_cents`. This independence
  must be preserved in any future refactor of the guardrail.
- The `daily_projected` term is still computed inside `month_metrics` (it feeds
  `total_outflow_cents`); it must not be removed from the accumulation loop, only from the
  `performance_cents` formula.
- A reviewer should check: (a) the new regression test covers a fixture with non-zero economia
  AND non-zero projected daily (both terms simultaneously); (b) the spec 011 addendum is
  clearly a reversal note, not a rewrite that erases the original intent.
- If debit/daily-first workflow (Diário > 0) is adopted in the future, the formula stays
  the same — `Diário realizado` is already in `cost_of_living_cents`; the only change would
  be that `daily_out_cents` would carry real values instead of being mostly zero.
