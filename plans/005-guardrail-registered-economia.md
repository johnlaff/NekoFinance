# Plan 005: Safe-to-spend guardrail uses registered Economia, not net-surplus proxy

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/commands.rs src-tauri/src/forecast/mod.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The "pode gastar até X" guardrail has two limbs: a cash limb (balance floor)
and a savings limb (Economizado%). The savings limb is supposed to enforce
the method's rule that Economizado% = Economia ÷ Entradas must stay ≥ 20–30%
on an annual average. Instead, it feeds `net income − expense` (a proxy) into
that limb. A user who earns R$ 10 000 and spends R$ 6 000 has a net surplus of
R$ 4 000 and sees a healthy Economizado%; but if they made no formal transfer
to their reserve account, the method says their Economizado is R$ 0. The proxy
flatters the result and lets the guardrail permit spending that the method would
block. Fixing this makes the guardrail match the method: only registered
Economia transfers (type `transfer` to an account with `liquidity IN
('reserve','illiquid')`) count toward the savings limb. The cash/colchão limb
is unchanged.

## Current state

### Files in scope

- `src-tauri/src/commands.rs` — the `forecast_dto` async function and its
  helper functions `realized_annual_savings` (lines 503–522) and
  `realized_annual_economia` (lines 528–546). The call site (lines 1003–1010)
  passes the net-surplus result of `realized_annual_savings` to
  `forecast::safe_to_spend_today` as the savings argument.
- `src-tauri/src/forecast/mod.rs` — the pure `safe_to_spend_today` function
  (lines 136–166) and its doc-comment (lines 123–135). Unit tests for the
  guardrail live in the `#[cfg(test)]` module at the bottom of this file
  (lines ~676–791). **Do not touch the function body or tests in this file** —
  the pure function is already correct; only the call site in `commands.rs`
  feeds it wrong data.

### Key excerpts (verify against live code before changing anything)

**`commands.rs` lines 489–494** — stale doc-comment marks this as "proxy conservador (review P2)":

```
/// `transfer` é IGNORADO (não há linha Economia explícita ainda) — a poupança real virá do saldo
/// da reserva quando o slice de Economia existir; até lá o net é um proxy conservador (review P2).
```

**`commands.rs` lines 924–928** — `AnnualSavingsDto` doc-comment says the guardrail uses the net:

```
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" do Neko; `registered_economia_cents` é a Economia REGISTRADA do
/// método (transfers→reserva), numerador do Economizado%. O guardrail usa o net (colchão); o
/// Economizado mensal usa a Economia registrada.
```

**`commands.rs` lines 1001–1010** — the call site fetches only `realized_annual_savings` before calling `safe_to_spend_today`, then fetches `realized_annual_economia` separately afterwards (line 1028):

```rust
let reserve_floor_cents = reserve_floor(pool).await?;
// Poupança ANUAL realizada (não o mês isolado, não o ano projetado-incompleto).
let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
let sts = forecast::safe_to_spend_today(
    &fc,
    annual_income,
    annual_savings_amt,          // ← BUG: net surplus, not registered Economia
    SAVINGS_TARGET_BPS,
    reserve_floor_cents,
);
```

And at **line 1028**:

```rust
let annual_economia = realized_annual_economia(pool, today_naive).await?;
```

**`commands.rs` lines 528–546** — the correct data source, already exists:

```rust
async fn realized_annual_economia(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND substr(t.date,1,7) < ?2 \
           AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid')",
    )
    .bind(&year_start)
    .bind(&cur_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized economia: {e}"))?;
    Ok(row.0)
}
```

**`forecast/mod.rs` lines 136–148** — the pure function's parameter `annual_savings_cents` is the savings limb numerator; it is generic (the caller decides what to pass):

```rust
pub fn safe_to_spend_today(
    fc: &Forecast,
    annual_income_cents: i64,
    annual_savings_cents: i64,   // ← caller's responsibility
    savings_target_bps: i64,
    reserve_floor_cents: i64,
) -> SafeToSpend {
    // ...
    let savings_headroom_cents = (annual_income_cents > 0)
        .then(|| annual_savings_cents - savings_target_bps * annual_income_cents / 10_000);
```

### Existing test to reason about

`forecast_dual_guardrail_savings_binds_for_owner` (commands.rs ~line 3473) inserts income +
expense for Jan–May with **no reserve transfers**. After the fix, `annual_economia` = 0 for those
months, so `savings_headroom_cents` = `0 − 25% × 1_000_000` = `−250_000` (still negative), the
binding guardrail stays "savings", and `safe_to_spend_today_cents` stays 0. The only assertion
that changes value is `assert!(fc.savings_headroom_cents.unwrap() < 0)` — it stays true. The test
passes without modification.

### Repo conventions

- Money is **integer cents** (`i64`); amounts are positive magnitude. Do not introduce floating
  point.
- Functional-core / imperative-shell: `forecast/mod.rs` is the pure core — no IO changes there.
- All DB helpers follow the `async fn name(pool: &SqlitePool, …) -> Result<T, String>` pattern.
- Tests in `commands.rs` use `fixture_pool()`, `insert_realized()`, `insert_projection()`, and
  ad-hoc `sqlx::query` for reserve accounts. Follow the same structure (see lines ~3090–3131 for
  the `annual_registered_economia_counts_only_reserve_transfers` test as the closest template).
- Commit style from `git log`: `fix: <short description>` (conventional commits, imperative mood,
  no period).

## Commands you will need

| Purpose                          | Command                                                             | Expected on success |
| -------------------------------- | ------------------------------------------------------------------- | ------------------- |
| Rust check (fmt + clippy + test) | `npm run rust:check`                                                | exit 0              |
| Rust tests only                  | `cargo test --manifest-path src-tauri/Cargo.toml --locked`          | all pass            |
| Filter to forecast tests         | `cargo test --manifest-path src-tauri/Cargo.toml --locked forecast` | all pass            |
| Full gate                        | `npm run check`                                                     | exit 0              |
| Typecheck (frontend)             | `npm run typecheck`                                                 | exit 0, no errors   |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands.rs` — call site wiring + comments

**Out of scope** (do NOT touch):

- `src-tauri/src/forecast/mod.rs` — the pure function signature and body are already correct;
  its unit tests already exercise the generic `annual_savings_cents` parameter correctly and
  do not need updating. Touching this file risks changing semantics tested independently.
- `src/` (React frontend) — the Economizado% display already reads
  `registered_economia_cents` from the DTO (not the savings limb result), so it is already
  correct per the advisor's audit. No frontend change is needed.
- The `AnnualSavingsDto` struct definition and its fields — only its doc-comment changes.
- Any migration in `src-tauri/migrations/` — no schema change is required.

## Git workflow

- Branch: `advisor/005-guardrail-registered-economia`
- Create with: `git switch -c advisor/005-guardrail-registered-economia`
- One logical commit covering all changes: `fix: guardrail savings limb uses registered Economia instead of net surplus`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Fetch `realized_annual_economia` before `safe_to_spend_today`, and pass it as the savings argument

Open `src-tauri/src/commands.rs`. In `forecast_dto`, locate the block starting at line ~1001.
Currently it looks like this:

```rust
let reserve_floor_cents = reserve_floor(pool).await?;
// Poupança ANUAL realizada (não o mês isolado, não o ano projetado-incompleto).
let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
let sts = forecast::safe_to_spend_today(
    &fc,
    annual_income,
    annual_savings_amt,
    SAVINGS_TARGET_BPS,
    reserve_floor_cents,
);
```

And at line ~1028 (after `rate_bps` closure and after `sts`/`binding_guardrail`):

```rust
let annual_economia = realized_annual_economia(pool, today_naive).await?;
```

Make these two changes **together** (the second change uses the variable introduced by the first):

1. Fetch `annual_economia` immediately after `realized_annual_savings`, before calling
   `safe_to_spend_today`:

   ```rust
   let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
   let annual_economia = realized_annual_economia(pool, today_naive).await?;
   ```

2. Pass `annual_economia` (not `annual_savings_amt`) to `safe_to_spend_today` as the savings
   argument:

   ```rust
   let sts = forecast::safe_to_spend_today(
       &fc,
       annual_income,
       annual_economia,          // ← registered Economia, not net surplus
       SAVINGS_TARGET_BPS,
       reserve_floor_cents,
   );
   ```

3. Remove the now-duplicated `let annual_economia = …` that previously appeared at line ~1028
   (after the `rate_bps` closure). Keep the `AnnualSavingsDto` construction that uses it — only
   the standalone fetch line is removed.

The final ordering in `forecast_dto` must be:

```
realized_annual_savings → realized_annual_economia → safe_to_spend_today
  → projected_annual_savings → rate_bps closure → AnnualSavingsDto { … annual_economia … }
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked forecast` → all existing
tests pass (zero failures).

### Step 2: Update the stale comment on `realized_annual_savings`

In `src-tauri/src/commands.rs`, update the doc-comment on `realized_annual_savings`
(lines ~489–502). The two stale sentences starting with `` `transfer` é IGNORADO`` must be
removed (they describe the old proxy behaviour as a temporary workaround; that workaround is now
resolved). Keep the rest of the comment intact.

Remove exactly these two sentences from the comment block:

```
/// `transfer` é IGNORADO (não há linha Economia explícita ainda) — a poupança real virá do saldo
/// da reserva quando o slice de Economia existir; até lá o net é um proxy conservador (review P2).
```

Replace them with a sentence explaining the current role of this function:

```
/// Retorna `(renda, net)` — o `net` superávit alimenta `AnnualSavingsDto.realized_savings_cents`
/// (o "colchão" exibido); a Economia registrada para o guardrail vem de `realized_annual_economia`.
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked` → exit 0 (compilation
confirms the comment change introduced no syntax error).

### Step 3: Update the stale comment on `AnnualSavingsDto`

In `src-tauri/src/commands.rs`, update the doc-comment on `AnnualSavingsDto`
(lines ~924–928). The last sentence says "O guardrail usa o net (colchão)"; that is now
false. Replace the two-sentence "ATENÇÃO" block:

Old text:

```
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" do Neko; `registered_economia_cents` é a Economia REGISTRADA do
/// método (transfers→reserva), numerador do Economizado%. O guardrail usa o net (colchão); o
/// Economizado mensal usa a Economia registrada.
```

New text (the final sentence is corrected):

```
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" exibido no Neko; `registered_economia_cents` é a Economia
/// REGISTRADA do método (transfers→reserva), numerador do Economizado%. O guardrail de poupança
/// usa a Economia registrada; o net só aparece como exibição do colchão.
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked` → exit 0.

### Step 4: Add a regression test for the corrected guardrail behaviour

In `src-tauri/src/commands.rs`, inside the `#[cfg(test)]` module, add a new `#[tokio::test]`
function after `annual_registered_economia_counts_only_reserve_transfers` (around line ~3131).

The test must cover the specific regression: a user with large net surplus (income >> expense)
but **zero** formal Economia transfers should have `binding_guardrail = "savings"` and
`safe_to_spend_today_cents = 0`, proving that net surplus no longer satisfies the savings limb.

Use `insert_realized` and ad-hoc `sqlx::query` for the reserve account, following the structure
of `annual_registered_economia_counts_only_reserve_transfers` (lines ~3097–3130) as the template.

Scenario:

- Cash seed: R$ 10 000 (1_000_000 cents) in a liquid account.
- Past complete month (e.g. `2026-03-*`): income R$ 5 000 (500_000), expense R$ 1 000 (100_000).
  Net surplus = R$ 4 000 (400_000). No transfers to any reserve account.
- `today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap()` (June = current month, March is
  complete).

Expected results after the fix:

- `fc.annual_savings.realized_income_cents` = 500_000 (income from March)
- `fc.annual_savings.registered_economia_cents` = 0 (no reserve transfers)
- `fc.savings_headroom_cents` = `Some(0 - 25% × 500_000)` = `Some(-125_000)` (negative)
- `fc.binding_guardrail` = `"savings"` (savings limb bites despite big net surplus)
- `fc.safe_to_spend_today_cents` = 0 (clamped, cannot spend)

Contrast assertion to document the difference from old behaviour: add a comment explaining that
under the old proxy, `savings_headroom_cents` would have been `Some(400_000 - 125_000) = Some(275_000)` and `binding_guardrail` would have been `"cash"`.

Name the test: `guardrail_savings_uses_registered_economia_not_net_surplus`.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked guardrail_savings_uses_registered_economia_not_net_surplus` → 1 test passed.

### Step 5: Run the full Rust gate

**Verify**: `npm run rust:check` → exit 0 (fmt + clippy + all tests green).

### Step 6: Run the full project gate

**Verify**: `npm run check` → exit 0 (typecheck + lint + frontend tests + rust:check + privacy
scan all green).

### Step 7: Update `plans/README.md`

Change the status cell for plan 005 from `TODO` to `DONE`.

**Verify**: `grep '| 005 ' plans/README.md` → output contains `DONE`.

## Test plan

**New test** (Step 4): `guardrail_savings_uses_registered_economia_not_net_surplus` in
`src-tauri/src/commands.rs` `#[cfg(test)]` block.

- Happy path / regression: net surplus R$ 4 000 with zero Economia transfers → guardrail bites
  ("savings"), `safe_to_spend_today_cents = 0`. This is the exact scenario that was broken before
  the fix.
- The existing tests that indirectly cover this path:
  - `forecast_dual_guardrail_savings_binds_for_owner` — inserts income + expense for Jan–May
    with no reserve transfers. After the fix, `annual_economia` = 0 for those months, so
    `savings_headroom_cents = 0 − 25% × 1_000_000 = −250_000` (still negative). All assertions
    still hold without modification.
  - `annual_registered_economia_counts_only_reserve_transfers` — verifies that
    `registered_economia_cents` correctly captures only reserve transfers. Unaffected.

Structural template: `annual_registered_economia_counts_only_reserve_transfers` at ~line 3097.

**Verification command**: `cargo test --manifest-path src-tauri/Cargo.toml --locked forecast` →
all pass, including the new test.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (fmt + clippy + all Rust tests)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked guardrail_savings_uses_registered_economia_not_net_surplus` exits 0; 1 test passed
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked forecast_dual_guardrail_savings_binds_for_owner` exits 0; 1 test passed (no regression)
- [ ] `npm run check` exits 0
- [ ] `grep "proxy conservador (review P2)" src-tauri/src/commands.rs` returns no matches (old stale comment removed)
- [ ] `grep "O guardrail usa o net (colchão)" src-tauri/src/commands.rs` returns no matches (old stale comment updated)
- [ ] `grep "annual_economia" src-tauri/src/commands.rs` shows the variable is used as the savings argument to `safe_to_spend_today` (not `annual_savings_amt`)
- [ ] `git diff --name-only` lists only `src-tauri/src/commands.rs` and `plans/README.md` (no other files modified)
- [ ] `grep '| 005 ' plans/README.md` output contains `DONE`

## STOP conditions

Stop and report back (do not improvise) if:

- The excerpts in "Current state" do not match the live code at the cited locations (line numbers
  drift > 5 lines, or the text differs materially — the codebase has changed since this plan was
  written).
- `realized_annual_economia` does not exist in `commands.rs` (it was renamed or deleted by a
  parallel plan).
- The `safe_to_spend_today` signature in `forecast/mod.rs` has changed (the parameter order or
  names differ from the excerpt above) — the call-site change in Step 1 would silently pass the
  wrong value.
- `forecast_dual_guardrail_savings_binds_for_owner` fails after Step 1 for a reason other than
  a stale assertion value (e.g. a compile error or a logic panic). Investigate; do not
  delete or weaken the test.
- The new test in Step 4 fails even after a reasonable fix attempt (two tries), suggesting
  the scenario assumptions are wrong or the DB fixture setup is broken.
- Fixing this bug appears to require touching `src-tauri/src/forecast/mod.rs` source code
  (beyond reading it for reference) — stop, the plan's analysis was wrong.
- `npm run check` fails on the frontend (typecheck or lint) with an error unrelated to this
  change — report the pre-existing failure rather than fixing it in this branch.

## Maintenance notes

- **Future: Economia import (plan 001/003)** — once the Economia tab is imported and transfers
  are created automatically by the importer, `realized_annual_economia` will start returning
  non-zero values naturally. The guardrail will tighten for users who previously had informal
  Economia (counted as net surplus) but no formal reserve transfers. Communicate this to users
  as a feature (the guardrail is now honest), not a regression.
- **Reviewer focus** — the PR diff should show exactly three logical changes: (a) the two-line
  call-site reorder in `forecast_dto`, (b) two comment updates, and (c) one new test. If the
  diff is larger, something went out of scope.
- **`realized_annual_savings` is still needed** — it provides `realized_savings_cents` (the
  "colchão" net surplus) for `AnnualSavingsDto` display. Do not delete it.
- **`annual_savings_amt` variable** — after the fix it is only used in `AnnualSavingsDto
{ realized_savings_cents: annual_savings_amt, … }`. If a later refactor removes that DTO
  field, `realized_annual_savings` can be removed or reduced to return just `annual_income`.
