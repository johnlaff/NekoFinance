# Plan 012: Remove dead code + unify duplicate money formatters

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src/lib/useCountUp.ts src/lib/useCountUp.test.ts src/lib/format.ts src/lib/format.test.ts src/features/reconcile/ConflictGate.tsx src/screens/CopilotScreen.tsx src/screens/DashboardScreen.tsx src/screens/HorizonteScreen.tsx src/screens/dashboard/DailyCheckinCard.tsx src/screens/dashboard/PerformanceCard.tsx src/screens/dashboard/PrevisibilidadeCard.tsx src/design-system/components/BalanceTrajectory.tsx src/design-system/components/CardChip.tsx src/design-system/components/Money.tsx src/design-system/components/TransactionRow.tsx src-tauri/src/forecast/mod.rs src-tauri/src/oauth/pkce.rs`
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Three dead-code clusters accumulate compiler suppressors and lint noise without delivering value. `useCountUp.ts` animates money — which the project rules explicitly prohibit — and is never imported outside its own test, so it is unreachable dead code. Two BRL formatter functions with different semantics (`fmtBRL` / `formatBRL`) coexist in the same module and are used interchangeably across the codebase, making it non-obvious which to use in new code. Two Rust dead stubs in `oauth/pkce.rs` and a module-level `#![allow(dead_code)]` in `forecast/mod.rs` mask items that may need real treatment. Removing these restores the repo's "zero suppressor" posture so future `dead_code` warnings are meaningful signal, not noise.

## Current state

### Frontend — `src/lib/useCountUp.ts` (dead hook, 51 lines)

The file exports one function, `useCountUp`, which animates a numeric value toward a target using `requestAnimationFrame`. Its only consumer is its own test file. No component imports it.

```
// src/lib/useCountUp.ts:1-5
import { useEffect, useRef, useState } from "react";
const DURATION_MS = 480; // --dur-deliberate
const easeOut = (t: number) => 1 - Math.pow(1 - t, 4);
const lastShown = new Map<string, number>();
```

```
// src/lib/useCountUp.ts:24
export function useCountUp(target: number, key = "default"): number {
```

Verify: `grep -rn "useCountUp" src/` hits **only** `src/lib/useCountUp.ts` and `src/lib/useCountUp.test.ts`. If it hits any other file, STOP.

### Frontend — `src/lib/useCountUp.test.ts` (test for dead hook, 19 lines)

```
// src/lib/useCountUp.test.ts:1-7
import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useCountUp } from "./useCountUp";

describe("useCountUp", () => {
  it("snaps to the target instantly in environments without matchMedia (jsdom)", () => {
    const { result } = renderHook(() => useCountUp(842000, "test-a"));
```

### Frontend — `src/lib/format.ts` — two BRL formatters

Both functions live in the same file. They have different signatures and different handling of negative values:

```
// src/lib/format.ts:14-21  — fmtBRL (simple, no hideCents option)
/** Formats INTEGER cents as localized BRL currency (e.g. 123456 → "R$ 1.234,56"). */
export function fmtBRL(cents: number): string {
  const reais = cents / 100;
  return reais.toLocaleString("pt-BR", {
    style: "currency",
    currency: "BRL",
  });
}
```

```
// src/lib/format.ts:67-76  — formatBRL (has hideCents param, uses typographic minus U+2212)
export function formatBRL(cents: number, hideCents = false): string {
  const neg = cents < 0;
  const v = Math.abs(cents) / 100;
  const s = v.toLocaleString("pt-BR", {
    minimumFractionDigits: hideCents ? 0 : 2,
    maximumFractionDigits: hideCents ? 0 : 2,
  });
  // Espaco apos R$ e um NBSP (U+00A0), que cola o simbolo ao numero; menos real e U+2212.
  return (neg ? "−R$ " : "R$ ") + s;
}
```

**Key semantic difference**: `fmtBRL` delegates sign to `toLocaleString` (produces a locale hyphen-minus like `-R$ 50,00`). `formatBRL` uses the typographic minus `−` (U+2212) and supports `hideCents`. `formatBRL` is the Design System formatter (used by `<Money>`, `TransactionRow`, `CardChip`, `BalanceTrajectory`) and is the **survivor**. `fmtBRL` is used only in plain-text contexts (aria-labels, text strings) where no `hideCents` is needed and the difference in minus character does not matter semantically.

**`fmtBRL` callers to migrate** (7 files):
- `src/features/reconcile/ConflictGate.tsx:23` — `fmtBRL(Math.abs(n))` in aria text
- `src/screens/CopilotScreen.tsx:43,47` — two template-string aria texts
- `src/screens/DashboardScreen.tsx:113,179,185,186,190` — five `value=` and `sublabel=` string props
- `src/screens/dashboard/DailyCheckinCard.tsx:113,114,178` — three aria/display strings
- `src/screens/dashboard/PerformanceCard.tsx:60` — one aria-label string
- `src/screens/dashboard/PrevisibilidadeCard.tsx:104,114` — two aria/display strings

**`formatBRL` callers** (already using the survivor, no change needed):
- `src/design-system/components/BalanceTrajectory.tsx` — 4 call sites
- `src/design-system/components/CardChip.tsx` — 2 call sites
- `src/design-system/components/Money.tsx` — 2 call sites (the DS primary consumer)
- `src/design-system/components/Money.test.tsx` — 4 call sites (test)
- `src/design-system/components/TransactionRow.tsx` — 2 call sites
- `src/screens/HorizonteScreen.tsx:179` — 1 call site

**`fmtBRL` tests** live in `src/lib/format.test.ts:38-54` — these must be migrated to test `formatBRL` or removed.

### Rust — `src-tauri/src/forecast/mod.rs:14`

```
// src-tauri/src/forecast/mod.rs:12-14
// Public engine API. Some outputs (`deepest_deficit`, `cash_floor_cents`, `months`) are
// consumed by later slices (Mia decision tools, the Totais screen), so allow unread-for-now.
#![allow(dead_code)]
```

All `pub` items in this module ARE already consumed by `src-tauri/src/commands.rs` (see the 30+ `forecast::` references there). The comment is stale; the suppressor is no longer needed. Action: **remove the `#![allow(dead_code)]` line and the stale comment above it**. If `cargo check` then flags any items, address them per the STOP conditions.

The one field to watch is `cash_floor_cents` (line 92 of `forecast/mod.rs`) — it is NOT referenced in `commands.rs`. If Rust flags it after removing the suppressor, add a `#[allow(dead_code)]` on that field only (not the module) with a comment: `// consumed by planned Mia tools (plan 019/020)`.

### Rust — `src-tauri/src/oauth/pkce.rs:111-130`

Two functions are silenced with `#[allow(dead_code)]`:

```
// src-tauri/src/oauth/pkce.rs:111-120
#[allow(dead_code)]
pub fn exchange_code(
    _config: &OAuthConfig,
    _state: &OAuthState,
    code: String,
) -> Result<(String, String), String> {
    // Token exchange requires HTTP client — placeholder for now
    Err(format!(
        "Code received (exchange not yet implemented): {code}"
    ))
}
```

```
// src-tauri/src/oauth/pkce.rs:123-130
#[allow(dead_code)]
pub fn is_valid_code_verifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() >= 43
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~')
}
```

`exchange_code` is an unimplemented HTTP-client stub (returns `Err` immediately). `is_valid_code_verifier` is a well-defined predicate that is called from within `pkce.rs`'s own `#[cfg(test)]` block (`test_generate_verifier_length`, line 139). Neither is called from outside `pkce.rs`.

Action for `exchange_code`: **delete the function**. It is unimplemented and has no callers. The token exchange will be a separate, real implementation when needed.

Action for `is_valid_code_verifier`: **keep the function, remove the `#[allow(dead_code)]` suppressor, move it inside `#[cfg(test)]`** (it is only ever called in tests). This makes its test-only nature explicit and silences the warning legitimately.

### Repo conventions (executor must match)

- Money values are always **integer cents**; amounts are positive magnitude (sign separate).
- Commit style: conventional commits, e.g. `fix: remove useCountUp dead hook and its test` (see `git log --oneline -5`).
- Branch naming: `advisor/<NNN>-<slug>`.
- React Compiler is ENABLED — do **not** add `useMemo`/`useCallback`/`memo` manually.
- The Design System formatter `formatBRL` lives in `src/lib/format.ts` and is the canonical DS formatter (documented in `src/design-system/components/Money.tsx:5` header comment).

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Typecheck (TS) | `npm run typecheck` | exit 0, no errors |
| Lint | `npm run lint` | exit 0 |
| Frontend tests | `npm run test:run` | all pass |
| Rust check (fmt+clippy+test) | `npm run rust:check` | exit 0 |
| Full gate | `npm run check` | exit 0 |
| Grep dead hook | `grep -rn "useCountUp" src/` | no output |
| Grep old formatter | `grep -rn "\bfmtBRL\b" src/` | no output |
| Grep dead_code suppressors | `grep -rn "allow(dead_code)" src-tauri/src/` | no output |

## Scope

**In scope** (the only files you should modify):

Frontend:
- `src/lib/useCountUp.ts` — delete
- `src/lib/useCountUp.test.ts` — delete
- `src/lib/format.ts` — delete `fmtBRL` function (lines 14–21 at planned-at SHA)
- `src/lib/format.test.ts` — migrate `fmtBRL` test suite to `formatBRL`
- `src/features/reconcile/ConflictGate.tsx` — replace `fmtBRL` import+call with `formatBRL`
- `src/screens/CopilotScreen.tsx` — replace `fmtBRL` import+calls with `formatBRL`
- `src/screens/DashboardScreen.tsx` — replace `fmtBRL` import+calls with `formatBRL`
- `src/screens/HorizonteScreen.tsx` — already uses `formatBRL`; no change needed (listed for awareness)
- `src/screens/dashboard/DailyCheckinCard.tsx` — replace `fmtBRL` import+calls with `formatBRL`
- `src/screens/dashboard/PerformanceCard.tsx` — replace `fmtBRL` import+call with `formatBRL`
- `src/screens/dashboard/PrevisibilidadeCard.tsx` — replace `fmtBRL` import+calls with `formatBRL`

Rust:
- `src-tauri/src/forecast/mod.rs` — remove `#![allow(dead_code)]` (line 14) and its stale comment (lines 12–13); conditionally add a field-level suppressor on `cash_floor_cents` if Rust flags it
- `src-tauri/src/oauth/pkce.rs` — delete `exchange_code` (lines 111–121); remove `#[allow(dead_code)]` from `is_valid_code_verifier` and move it inside `#[cfg(test)]`

**Out of scope** (do NOT touch, even if they look related):
- `src/design-system/components/Money.tsx` — already uses `formatBRL`; no change needed
- `src/design-system/components/BalanceTrajectory.tsx` — already uses `formatBRL`; no change needed
- `src/design-system/components/CardChip.tsx` — already uses `formatBRL`; no change needed
- `src/design-system/components/TransactionRow.tsx` — already uses `formatBRL`; no change needed
- `src/design-system/components/Money.test.tsx` — already tests `formatBRL`; no change needed
- Any behavioral change to `formatBRL`'s output — the output contract must stay identical
- Any refactor of `commands.rs` (that is plan 011)
- Any new feature work on OAuth token exchange

## Git workflow

- Branch: `advisor/012-dead-code-dup-formatters`
- One commit per logical unit (frontend dead hook, formatter migration, Rust suppressors)
- Commit message style (conventional commits, matching repo history):
  - `fix: remove useCountUp dead hook and its test`
  - `fix: unify BRL formatters — replace fmtBRL callers with formatBRL, delete fmtBRL`
  - `fix: remove dead_code suppressors in forecast/mod.rs and oauth/pkce.rs`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Confirm useCountUp has no live callers

Run the grep to ensure no component imports the hook:

```
grep -rn "useCountUp" src/
```

Expected output: exactly two lines, both in `src/lib/useCountUp.ts` and `src/lib/useCountUp.test.ts`. If any other file appears, STOP and report — do not proceed with deletion.

**Verify**: `grep -rn "useCountUp" src/` → 2 lines total (definition + test), no component file.

### Step 2: Delete useCountUp source and test files

Delete both files:
- `src/lib/useCountUp.ts`
- `src/lib/useCountUp.test.ts`

**Verify**: `grep -rn "useCountUp" src/` → no output (exit 0 with empty result).

Then: `npm run typecheck` → exit 0, no errors.

### Step 3: Migrate fmtBRL callers in screen/feature files

For each of the 7 files below, do the following mechanical substitution:
1. In the `import` line, replace `fmtBRL` with `formatBRL` (if `formatBRL` is not already imported in that file).
2. In every call site, replace `fmtBRL(` with `formatBRL(`.

> Note on behavior: `fmtBRL` uses locale hyphen-minus (`-`) for negatives; `formatBRL` uses
> typographic minus `−` (U+2212). All the call sites being migrated are in aria-labels and
> display text strings. The typographic minus is the correct DS minus character (see `Money.tsx`
> comment). This is a quality improvement, not a regression. The `fmtBRL(Math.abs(n))` call in
> `ConflictGate.tsx:23` already passes a positive value, so sign handling is unchanged there.

Files to edit (in any order):

**`src/features/reconcile/ConflictGate.tsx`**
- Line 13: `import { fmtBRL } from "../../lib/format";` → `import { formatBRL } from "../../lib/format";`
- Line 23: `fmtBRL(Math.abs(n))` → `formatBRL(Math.abs(n))`

**`src/screens/CopilotScreen.tsx`**
- Line 9: remove `fmtBRL` from import (keep `monthNamePtBR`); add `formatBRL` to same import
- Lines 43, 47: replace `fmtBRL(` with `formatBRL(`

**`src/screens/DashboardScreen.tsx`**
- Line 11: replace `fmtBRL` with `formatBRL` in import (keep `fmtDayMonth`, `monthNamePtBR`)
- Lines 113, 179, 185, 186, 190: replace `fmtBRL(` with `formatBRL(`

**`src/screens/dashboard/DailyCheckinCard.tsx`**
- Line 7: replace `fmtBRL` with `formatBRL` in import (keep `parseBRLToCents`, `todayISO`)
- Lines 113, 114, 178: replace `fmtBRL(` with `formatBRL(`

**`src/screens/dashboard/PerformanceCard.tsx`**
- Line 3: replace `fmtBRL` with `formatBRL` in import (keep `monthNamePtBR`)
- Line 60: replace `fmtBRL(` with `formatBRL(`

**`src/screens/dashboard/PrevisibilidadeCard.tsx`**
- Line 3: replace `fmtBRL` with `formatBRL` in import (keep `monthNamePtBR`)
- Lines 104, 114: replace `fmtBRL(` with `formatBRL(`

**Verify**: `grep -rn "\bfmtBRL\b" src/screens/ src/features/` → no output.

Then: `npm run typecheck` → exit 0.

### Step 4: Delete fmtBRL from format.ts and update format.test.ts

**In `src/lib/format.ts`**: Delete lines 14–21 (the `fmtBRL` function, including the JSDoc comment above it). Verify no blank lines are left that look like artifacts.

**In `src/lib/format.test.ts`**: The test block `describe("fmtBRL", ...)` at lines 38–54 tests locale formatting of cents. Convert it to test `formatBRL` instead:
1. Add `formatBRL` to the import at line 3 (alongside the existing imports).
2. Remove `fmtBRL` from that import.
3. Rename the describe block from `"fmtBRL"` to `"formatBRL"`.
4. Update the test cases to match `formatBRL`'s output contract:
   - `formatBRL(123456)` → contains `"1.234,56"` ✓ (same as before)
   - `formatBRL(0)` → contains `"0,00"` ✓ (same)
   - `formatBRL(-5000)` → contains `"−"` and `"50,00"` (typographic minus, not `-R$`)
   - `formatBRL(7)` → contains `"0,07"` ✓ (same)

   The negative test case changes from `toBe("-R$ 50,00")` to using `toContain("−")` and `toContain("50,00")` to be locale-tolerant, matching the pattern already used in `src/design-system/components/Money.test.tsx:11`:
   ```ts
   // Money.test.tsx:11 — use as pattern:
   const s = formatBRL(-50000);
   expect(s).toContain("−");
   ```

**Verify**:
- `grep -rn "\bfmtBRL\b" src/` → no output (the symbol is fully gone)
- `npm run test:run` → all pass

### Step 5: Remove module-level dead_code suppressor in forecast/mod.rs

Open `src-tauri/src/forecast/mod.rs`. Remove lines 12–14:

```rust
// Public engine API. Some outputs (`deepest_deficit`, `cash_floor_cents`, `months`) are
// consumed by later slices (Mia decision tools, the Totais screen), so allow unread-for-now.
#![allow(dead_code)]
```

Run `npm run rust:check`. If it exits 0 with no warnings about dead code, the suppressor was truly redundant — done.

If Rust flags `cash_floor_cents` (the one field not referenced in `commands.rs`), add a targeted suppressor on that field only:

```rust
// consumed by planned decision tools (plans 019/020); not yet wired to commands.rs
#[allow(dead_code)]
pub cash_floor_cents: i64,
```

Do NOT add any other `#[allow(dead_code)]` attributes. If Rust flags other items, STOP and report.

**Verify**: `npm run rust:check` → exit 0. `grep -n "allow(dead_code)" src-tauri/src/forecast/mod.rs` → no output (or exactly one line for `cash_floor_cents` if that field needed it).

### Step 6: Fix dead stubs in oauth/pkce.rs

Open `src-tauri/src/oauth/pkce.rs`.

**Delete `exchange_code`** (lines 111–121 at planned-at SHA):
```rust
#[allow(dead_code)]
pub fn exchange_code(
    _config: &OAuthConfig,
    _state: &OAuthState,
    code: String,
) -> Result<(String, String), String> {
    // Token exchange requires HTTP client — placeholder for now
    Err(format!(
        "Code received (exchange not yet implemented): {code}"
    ))
}
```
Remove the blank line that preceded it too.

**Move `is_valid_code_verifier` inside `#[cfg(test)]`**: The function (lines 123–130) is only called in the test module. Remove the `#[allow(dead_code)]` attribute from it. Move the function body (without the `#[allow(dead_code)]`) into the `#[cfg(test)]` block that starts at line 132. The function becomes `fn is_valid_code_verifier` (no `pub` needed inside `#[cfg(test)]`). The existing test `test_generate_verifier_length` at line 136 calls `is_valid_code_verifier(verifier.secret())` — this reference will now resolve to the local function.

After the edit, the `#[cfg(test)]` block should open with `use super::*;` followed by the now-local `is_valid_code_verifier` function, then the test functions.

**Verify**: `npm run rust:check` → exit 0. `grep -n "allow(dead_code)" src-tauri/src/oauth/pkce.rs` → no output.

### Step 7: Full gate

Run the complete gate to confirm nothing else was broken:

```
npm run check
```

Expected: exit 0 (typecheck, lint, tests, rust:check, privacy:scan all pass).

Additionally verify the done criteria greps:

```
grep -rn "useCountUp" src/
grep -rn "\bfmtBRL\b" src/
grep -rn "allow(dead_code)" src-tauri/src/
```

All three should return no output.

## Test plan

No new tests are required. The changes are purely structural:

- `useCountUp.test.ts` is **deleted** with its subject.
- The `fmtBRL` describe block in `src/lib/format.test.ts` is **converted** to test `formatBRL` (same 4 cases, one expected-value update for the negative case to match `formatBRL`'s typographic minus).
- Existing `formatBRL` tests in `src/design-system/components/Money.test.tsx` continue to pass unchanged.
- Rust tests in `pkce.rs` continue to pass; `is_valid_code_verifier` remains callable inside the test module.

Model the updated `format.test.ts` negative-amount test after the pattern in `src/design-system/components/Money.test.tsx:11-13`:
```ts
it("formats negative amounts with typographic minus", () => {
  const s = formatBRL(-5000);
  expect(s).toContain("−");
  expect(s).toContain("50,00");
});
```

**Verification**: `npm run test:run` → all pass (count of tests decreases by 2 from removed `useCountUp.test.ts`, one `fmtBRL` test updated in place).

## Done criteria

Machine-checkable. ALL must hold before marking this plan DONE:

- [ ] `grep -rn "useCountUp" src/` → no output
- [ ] `grep -rn "\bfmtBRL\b" src/` → no output
- [ ] `grep -rn "allow(dead_code)" src-tauri/src/` → no output (or exactly one line on `cash_floor_cents` if Rust required it)
- [ ] `npm run typecheck` → exit 0
- [ ] `npm run lint` → exit 0
- [ ] `npm run test:run` → all pass
- [ ] `npm run rust:check` → exit 0
- [ ] `npm run check` → exit 0
- [ ] Files outside the in-scope list are not modified (`git diff --name-only` touches only in-scope files)
- [ ] `plans/README.md` status row for plan 012 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The `grep -rn "useCountUp" src/` in step 1 returns lines in any file OTHER than `useCountUp.ts` and `useCountUp.test.ts` — a component is using it and must be migrated first.
- After removing `#![allow(dead_code)]` from `forecast/mod.rs`, Rust flags ANY item other than `cash_floor_cents` — another pub item may not be wired and needs investigation before silencing.
- After removing `exchange_code` from `pkce.rs`, Rust or a test references it by name outside the deleted block (i.e., the grep earlier missed a caller).
- Any step's `npm run typecheck` or `npm run rust:check` fails TWICE after a reasonable fix attempt.
- The code at ANY location cited in "Current state" doesn't match the excerpts (the codebase has drifted since this plan was written — treat as a STOP and compare the live file to the plan before proceeding).
- You find that `formatBRL` is re-exported, aliased, or re-named elsewhere in the codebase — stop and map the full symbol graph before proceeding.

## Maintenance notes

- After this lands, `format.ts` contains exactly two money-formatting exports: `formatBRL` (full DS formatter with typographic minus and `hideCents`) and `fmtCompactBRL` (chart labels). Any future money display must use one of these two — do not introduce a third.
- The `cash_floor_cents` field on `Forecast` (if it required its own `#[allow(dead_code)]`) is earmarked for Mia decision tools and the scenario-branching spike (plans 019/020). When those features land, remove the field-level suppressor.
- Token exchange (`exchange_code` deleted here) needs a real HTTP-client implementation using Tauri's `http` plugin before the OAuth flow can complete without relying on the pre-existing `commands.rs` path. Track that work in a separate plan or issue; do not re-add the stub.
- A reviewer checking this PR should confirm: (1) no `fmtBRL` symbol remains anywhere in `src/`, (2) `useCountUp` files are absent from the tree, (3) the updated `format.test.ts` negative test uses `toContain("−")` (typographic minus U+2212), not `toBe("-R$...")`.
