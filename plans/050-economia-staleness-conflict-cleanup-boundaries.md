# Plan 050: P2: economia write-back staleness + import conflict cleanup + saw_december + thermometer 1000/2000

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
>   src-tauri/src/commands/write_back_cmds.rs \
>   src-tauri/src/google_sheets/import.rs \
>   src/lib/saldoHeatmap.ts \
>   src/lib/saldoHeatmap.test.ts
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
- **Category**: bug
- **Planned at**: commit `2132297`, 2026-06-21

## Why this matters

Four correctness gaps found in the post-Package-F sign-off. One is real and
open (`apply_economia_write_back` staleness bypass). One was already fixed by
plan 047 (diff-delete `import_conflict` cleanup — see Drift notes below).
Two remain open: the `saw_december` premature break can silently truncate the
current year of a partial side-by-side layout, and the R$1000/R$2000
thermometer boundaries need explicit verification and test coverage against
the spreadsheet's inclusive-vs-exclusive `between`/`lessThan`/`greaterThan`
semantics (the −R$500 boundary was fixed by plan 048; the 1000/2000 boundaries
have never had explicit exact-boundary tests).

Landing this plan closes the three open bugs with regression tests so they
cannot reappear silently.

## Drift notes (read before touching any file)

**Bug #2 (import_conflict diff-delete orphan) is already CLOSED.**
At the planned-at commit `2132297`, `import_rows_core` already deletes
`import_conflict` rows in the diff-delete loop (lines 657-662 of
`src-tauri/src/google_sheets/import.rs`):

```rust
// import.rs lines 657–662 (planned-at SHA)
            // Conflitos órfãos somem com a transação removida.
            sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
                .bind(&eid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete removed conflicts: {e}"))?;
```

This was introduced by plan 047. **Do not re-implement it.** Step 2 below
is therefore a test-only step (confirm coverage exists or add a focused test).

**Bug #4 (saldoBand 1000/2000 boundary):** At the planned-at commit, the
current code uses `cents <= t.tight` and `cents <= t.ok`. Tests already verify
`[100_000, "tight"]` and `[200_000, "ok"]`. Plan 048 re-verified all
boundaries and fixed only the `critical` boundary (changed `<=` to `<`),
explicitly leaving `tight` and `ok` as `<=`. If the live code and tests match
the excerpts in "Current state" below, this boundary is already correct —
Step 4 is a verification-and-coverage-only step (add explicit boundary tests at
99_999 / 100_000 / 100_001 and 199_999 / 200_000 / 200_001 if not already
present, but do NOT change the comparison operators).

## Current state

### File roles

- `src-tauri/src/commands/write_back_cmds.rs` — Tauri command handlers for
  write-back; contains `apply_economia_write_back` (line 929) and
  `apply_write_back` (line 482); the staleness helpers `guard_sheet_unchanged`
  (line 191), `staleness_check` (line 206), and `make_authenticated_client`
  (line 237).
- `src-tauri/src/google_sheets/import.rs` — Sheet import engine; contains
  `import_rows_core` with the diff-delete loop (around line 625) and
  `parse_economia_sheet` with the `saw_december` logic (around line 1404).
- `src/lib/saldoHeatmap.ts` — Thermometer classifier `saldoBand` (line 46)
  and thresholds object (line 34).
- `src/lib/saldoHeatmap.test.ts` — Thermometer unit tests (the test suite to
  extend in Step 4).

### Bug #1 — `apply_economia_write_back` staleness bypass

`apply_write_back` (lines 482-595) was hardened in plan 047: it takes an early
snapshot of `modifiedTime` BEFORE reading the sheet values, then compares it to
a post-plan snapshot, so even when `preview_revision=None` the gate fires.

`apply_economia_write_back` (lines 929-977) was **not** given this treatment.
It calls `build_economia_plan` (line 948) which internally creates its own
`SheetsClient` and reads the sheet. After `build_economia_plan` returns, it
calls `guard_sheet_unchanged` (line 959) with `preview_revision.as_deref()`.
`guard_sheet_unchanged` is a no-op when `preview_revision=None` (see line
196-198):

```rust
// write_back_cmds.rs line 191–201 (planned-at SHA)
pub(crate) async fn guard_sheet_unchanged(
    client: &SheetsClient,
    spreadsheet_id: &str,
    preview_revision: Option<&str>,
) -> Result<(), String> {
    let Some(seen) = preview_revision.filter(|s| !s.trim().is_empty()) else {
        return Ok(());   // ← no-op when preview_revision = None
    };
    let current = client.get_file_modified_time(spreadsheet_id).await?;
    staleness_check(seen, &current)
}
```

So on the legacy path (UI sends `preview_revision = None`), the Economia
write-back has NO staleness gate. Contrast with `apply_write_back` (lines
505-529 of the same file):

```rust
// write_back_cmds.rs lines 503–529 (planned-at SHA) — the pattern to replicate
    // Plano 047: foto do `modifiedTime` ANTES de ler os VALORES da aba (mesmo padrão de
    // `preview_write_back_status`). No caminho LEGADO (sem `preview_revision`), esta foto é o "estado
    // que o apply assumiu como base": ...
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let early_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;

    let (client, plan) = build_write_back_plan( ... ).await?;

    // Re-verifica a frescura (Step 4) SEMPRE — nenhum caminho de apply escapa do gate.
    let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;
    match preview_revision.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(seen) => staleness_check(seen, &post_plan_revision)?,
        None => staleness_check(&early_revision, &post_plan_revision)?,
    }
```

**Fix**: replicate the `apply_write_back` always-on pattern in
`apply_economia_write_back`:

1. Before calling `build_economia_plan`, call `make_authenticated_client` and
   snapshot `early_revision = early_client.get_file_modified_time(...)`.
2. After `build_economia_plan` returns the `(client, plan)`, snapshot
   `post_plan_revision = client.get_file_modified_time(...)`.
3. Replace the `guard_sheet_unchanged` call with the same `match` on
   `preview_revision`: rich path uses `seen`; legacy path uses `early_revision`.

The `staleness_check` pure function (line 206-211) is reused as-is — it has a
unit test (`staleness_check_rejects_different_revision`, line 1377) already.

### Bug #3 — `parse_economia_sheet` `saw_december` premature break

`parse_economia_sheet` (line 1404-1472) iterates over rows in `rr` looking for
month names under each header block. The inner `while rr` loop collects entries
from ALL blocks in `blocks` (e.g. 2025 and 2026 side by side). After collecting
each row it checks:

```rust
// import.rs lines 1443–1468 (planned-at SHA)
        let mut rr = r + 1;
        while rr < rows.len() {
            let mut any = false;
            let mut saw_december = false;
            for &(month_col, year, econ_col) in &blocks {
                let Some(month) = rows[rr]
                    .get(month_col)
                    .and_then(|l| month_number_from_name(l))
                else {
                    continue;
                };
                any = true;
                let cents = rows[rr].get(econ_col).map(|c| parse_number(c)).unwrap_or(0);
                out.push((year, month, cents));
                if month == 12 {
                    saw_december = true;
                }
            }
            if !any {
                break;
            }
            rr += 1;
            if saw_december {
                break;   // ← breaks for ALL blocks when ANY block saw month==12
            }
        }
```

In a typical full side-by-side layout (prior year Jan–Dec, current year
Jan–partial), when the prior year (e.g. 2025) reaches its December row, both
blocks are on the same row. If 2025 has Dec and 2026 has Dec too, the break is
correct. But in an **asymmetric** layout where 2025 has all 12 months and 2026
has only, say, 8 months entered so far, the row that carries 2025's December
also carries 2026's month 8 — and then the break fires, truncating 2026's
remaining months (9–12 if partially filled, or rows 9–12 if they exist but
beyond the break point).

Concretely:

```
row r:   [header: 2025 ... Economia ... 2026 ... Economia]
row r+1: jan 2025 ... 1000 ... jan 2026 ... 1500
...
row r+12: dec 2025 ... 800 ... dec 2026 ... 0  ← saw_december fires, breaks
```

If 2026 does NOT yet have December (the year is in progress), there may be
fewer rows. If the layout still has rows r+9 through r+12 with 2025 data and
no 2026 data for those months (because the user hasn't filled them yet), the
correct behavior is to stop when `!any`, which already works. The bug only
manifests when the layout has more than 12 rows below the header AND the
prior-year block reaches December on a row that the current-year block does
NOT have.

**Fix**: break only when the prior-year December row is exhausted AND there are
no more rows with any valid month for any block. The simplest correct fix is to
remove the `saw_december` early break entirely and rely solely on the `!any`
break (which fires when a row has no valid month name in any block column).
This is safe because:

- The `!any` break already stops at TOTAL rows, blank rows, and the next
  header block.
- December is not special in the layout beyond being the last month — the row
  after December either has no months (→ `!any` fires) or belongs to the next
  block header (→ `!any` fires too, since month_number_from_name returns `None`
  for year numbers and "TOTAL").

The fix is: delete the `saw_december` variable, the `if month == 12` update,
and the `if saw_december { break; }` guard (3 small edits; `!any` break stays).

**Note**: an existing test `parse_economia_sheet_side_by_side_blocks`
(line 1959) uses a 2-row layout (jan+fev for both years) without December and
passes today. The regression test to ADD is a 12+ row layout where 2025 has
all 12 months and 2026 has only the first 8.

### Bug #4 — saldoBand 1000/2000 boundary exactness

Current `saldoBand` (line 46-54 of `saldoHeatmap.ts`):

```typescript
// saldoHeatmap.ts lines 46–55 (planned-at SHA)
export function saldoBand(
  cents: number,
  t: SaldoBandThresholds = SALDO_BAND_THRESHOLDS_CENTS,
): SaldoBand {
  if (cents < t.critical) return "critical"; // strict < (fixed by plan 048)
  if (cents < t.positive) return "negative"; // strict <
  if (cents <= t.tight) return "tight"; // inclusive ≤
  if (cents <= t.ok) return "ok"; // inclusive ≤
  return "comfortable";
}
```

Thresholds (line 34-39):

```typescript
const SALDO_BAND_THRESHOLDS_CENTS: SaldoBandThresholds = {
  critical: -50_000, // −R$ 500,00
  positive: 0,
  tight: 100_000, // R$ 1.000,00
  ok: 200_000, // R$ 2.000,00
};
```

Spreadsheet conditional-formatting semantics (as verified by plan 048's comment
in the test file): the sheet uses strict `lessThan` for the `critical`
boundary, making exactly −500,00 fall in `negative`. For the upper positive
bands the sheet uses `between` (inclusive) for "apertado" (0–1000) and "ok"
(1001–2000), which maps to `cents <= 100_000` for tight and `cents <= 200_000`
for ok. This means R$1000 exact → tight and R$2000 exact → ok — which the
current `<=` operators already produce correctly.

The existing test (line 14-31 of `saldoHeatmap.test.ts`) checks:

```typescript
[200_000, "ok"],
[100_001, "ok"],
[100_000, "tight"],
[50_000, "tight"],
```

Missing exact-boundary tests: 99_999 / 100_000 / 100_001 (R$1000 boundary)
and 199_999 / 200_000 / 200_001 (R$2000 boundary). **Step 4 adds these tests**
without changing the comparison operators (they are already correct).

### Repo conventions

- **Rust pattern**: `make_authenticated_client` (write_back_cmds.rs line 237)
  is the shared helper for creating an authenticated client. Always call it, do
  not inline the token logic. The `staleness_check` pure function (line 206) is
  testable without network; use it in any new pure-decision tests.
- **Test pattern (Rust)**: tests in `#[cfg(test)] mod tests` at the bottom of
  the same file. In-memory SQLite pool created by the `pool()` helper already
  at the bottom of `write_back_cmds.rs` (line 1124-1132). Use
  `#[tokio::test]` for async tests.
- **Test pattern (TypeScript)**: `describe`/`it.each` in Vitest; existing
  `saldoHeatmap.test.ts` is the structural model.
- **Functional-core**: the staleness decision (`staleness_check`) and the
  saw_december parse (`parse_economia_sheet`) are pure — test the pure
  function directly, not the Tauri command.
- **Method-neutral language**: do not name any third-party method, app, or
  RE in comments or test strings.

## Commands you will need

| Purpose        | Command                                           | Expected on success   |
| -------------- | ------------------------------------------------- | --------------------- |
| Rust typecheck | `npm run rust:check`                              | exit 0, no errors     |
| Rust tests     | `npm run test:run` (runs vitest + cargo test)     | exit 0, all pass      |
| TS typecheck   | `npm run typecheck`                               | exit 0, no errors     |
| Lint           | `npm run lint`                                    | exit 0                |
| Full gate      | `npm run check`                                   | exit 0                |
| E2E smoke      | `npm run e2e`                                     | exit 0, 13 tests pass |
| Targeted Rust  | `cd src-tauri && cargo test write_back_cmds 2>&1` | specific tests pass   |
| Targeted Rust  | `cd src-tauri && cargo test import::tests 2>&1`   | specific tests pass   |
| Targeted TS    | `npx vitest run src/lib/saldoHeatmap.test.ts`     | all pass              |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/write_back_cmds.rs` — add early snapshot in
  `apply_economia_write_back`; new staleness pure-test for economia path.
- `src-tauri/src/google_sheets/import.rs` — remove `saw_december` logic;
  add regression test for asymmetric two-year layout; add or verify
  diff-delete `import_conflict` test coverage.
- `src/lib/saldoHeatmap.test.ts` — add exact-boundary tests at 100_000 and
  200_000 (neighbouring values too).

**Out of scope** (do NOT touch):

- `src/lib/saldoHeatmap.ts` — the `saldoBand` comparison operators are already
  correct; do not change them.
- `src-tauri/src/google_sheets/write_back.rs` — not touched by this plan.
- The 028-plan gates (flag, conflict, scope) in `apply_economia_write_back` —
  leave them exactly as they are; this plan only adds the staleness gate.
- Any migration files — no schema changes in this plan.
- Any file not listed under "In scope".

## Git workflow

- Branch: `advisor/050-economia-staleness-cleanup`
- One commit per step is fine; alternatively a single commit is acceptable.
  Commit style: `fix: <imperative, lowercase>` matching repo convention
  (example from `git log --oneline`: `fix: limpeza de órfãos no delete (P1) + 4 correções de fluxo (plano 047)`).
- Do NOT push or open a PR unless instructed.

## Steps

### Step 1: Harden `apply_economia_write_back` with always-on staleness gate

**File**: `src-tauri/src/commands/write_back_cmds.rs`

**What to do**: replicate the pattern from `apply_write_back` (lines 503-529)
into `apply_economia_write_back` (starting around line 939).

Before the `build_economia_plan` call, insert:

```rust
    // Always-on staleness gate (mirrors apply_write_back / plan 047 pattern).
    // Snapshot modifiedTime BEFORE reading the sheet values so the token corresponds
    // to a state no newer than the diff. On the legacy path (no preview_revision),
    // early_revision acts as the base; a concurrent edit between the two snapshots
    // advances modifiedTime → gate fires → no stale diff reaches the sheet.
    let early_client =
        make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
    let early_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;
```

After `build_economia_plan` returns `(client, plan)`, snapshot the post-plan
revision and replace the `guard_sheet_unchanged` call with the same `match`:

```rust
    let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;
    match preview_revision.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(seen) => staleness_check(seen, &post_plan_revision)?,
        None => staleness_check(&early_revision, &post_plan_revision)?,
    }
```

Remove the now-unused `guard_sheet_unchanged` call (`guard_sheet_unchanged(&client, &spreadsheet_id, preview_revision.as_deref()).await?;`).

The full modified function body should have this ordering:

1. `ensure_write_back_enabled()`
2. `guard_no_pending_conflicts(pool.inner())`
3. `ensure_write_scope(...)`
4. `make_authenticated_client(...)` → `early_revision`
5. `build_economia_plan(...)` → `(client, plan)`
6. Post-plan snapshot + `staleness_check` match
7. Filter `written` + batch_update_values + audit

All existing gates (flag, conflict, scope) must remain in place and in their
original order before the network calls.

**Verify**: `cd src-tauri && cargo check 2>&1` → exit 0, no errors.

### Step 2: Confirm diff-delete `import_conflict` coverage in import.rs

**File**: `src-tauri/src/google_sheets/import.rs`

First, confirm the cleanup is present by checking that `import_rows_core`'s
diff-delete loop (around line 625) already contains:

```rust
            sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
                .bind(&eid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete removed conflicts: {e}"))?;
```

If this line is **absent**, add it inside the `if !current_ids.contains(&eid)`
block, after the `sync_log` delete and before the closing `}`. If it is
**present**, skip the code change.

Then, check whether there is an existing `#[tokio::test]` that verifies a
diff-deleted row with an open conflict leaves no `import_conflict` orphan.
Search for test names containing "conflict" or "diff_delete" in the `mod tests`
block:

```
grep -n "diff_delete\|conflict" src-tauri/src/google_sheets/import.rs
```

If no such test exists, add one (model after `diff_delete_removes_derived_rows`
near line 2981):

```rust
#[tokio::test]
async fn diff_delete_removes_orphan_import_conflict() {
    // A row imported once, then an open conflict recorded against it, then the
    // row is removed from the sheet (re-import without that row) → the conflict
    // must not survive (it would block write-back).
    let pool = test_pool().await;

    // Import a single row so it gets a sync_log entry.
    let rows_v1 = vec![imported("2026-03-01", -10_000)];
    import_rows(&pool, "2026", &rows_v1, "p1").await.unwrap();

    // Find the transaction id created by the import.
    let (txn_id,): (String,) =
        sqlx::query_as("SELECT id FROM \"transaction\" WHERE date = '2026-03-01'")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Manually insert an open conflict for that transaction.
    let conf_id = format!("conf:{txn_id}:amount");
    sqlx::query(
        "INSERT INTO import_conflict (id, transaction_id, field, base_value, local_value, sheet_value, created_at) \
         VALUES (?1, ?2, 'amount', '10000', '12000', '11000', '2026-03-01T00:00:00Z')",
    )
    .bind(&conf_id)
    .bind(&txn_id)
    .execute(&pool)
    .await
    .unwrap();

    // Confirm conflict exists.
    let (before,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE transaction_id = ?1")
            .bind(&txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, 1, "conflict exists before re-import");

    // Re-import without the row (simulates line removed from sheet).
    let rows_v2 = vec![imported("2026-03-02", -5_000)]; // anchor row keeps import non-empty
    import_rows(&pool, "2026", &rows_v2, "p1").await.unwrap();

    // Orphan conflict must be gone.
    let (after,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM import_conflict WHERE transaction_id = ?1")
            .bind(&txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, 0, "conflict orphan removed by diff-delete");
}
```

**Note**: the `imported` helper is already defined in the test module (it
creates an `ImportedRow` with standard fields — look for it near `test_pool()`
around line 2043). If the test module does not expose `import_rows` directly,
check the existing `diff_delete_removes_derived_rows` test for the correct
import call pattern.

**Verify**: `cd src-tauri && cargo test import::tests::diff_delete 2>&1`
→ at least `diff_delete_removes_derived_rows` and (if added)
`diff_delete_removes_orphan_import_conflict` pass.

### Step 3: Remove the `saw_december` premature break in `parse_economia_sheet`

**File**: `src-tauri/src/google_sheets/import.rs`

In `parse_economia_sheet` (around line 1443-1468), make three targeted edits:

1. Remove the line `let mut saw_december = false;`
2. Remove the two lines `if month == 12 { saw_december = true; }`
3. Remove the two lines `if saw_december { break; }`

The inner `while rr` loop after the change should look like:

```rust
        let mut rr = r + 1;
        while rr < rows.len() {
            let mut any = false;
            for &(month_col, year, econ_col) in &blocks {
                let Some(month) = rows[rr]
                    .get(month_col)
                    .and_then(|l| month_number_from_name(l))
                else {
                    continue;
                };
                any = true;
                let cents = rows[rr].get(econ_col).map(|c| parse_number(c)).unwrap_or(0);
                out.push((year, month, cents));
            }
            if !any {
                break;
            }
            rr += 1;
        }
```

Then add a regression test in the `mod tests` block, after the existing
`parse_economia_sheet_side_by_side_blocks` test (around line 1959). The test
must have a prior-year block with all 12 months and a current-year block with
only the first 8, sharing the same rows:

```rust
#[test]
fn parse_economia_sheet_asymmetric_blocks_no_premature_break() {
    // Prior year (e.g. 2025) has all 12 months; current year (e.g. 2026) has only
    // months 1–8 entered so far. The bug: `saw_december` was set when 2025 reached
    // December, breaking the loop and truncating 2026's months 9–12 if the rows
    // existed (even as blank). Fix: rely only on the `!any` break.
    //
    // Layout: 2025 in col B (idx 1) / Economia col D (idx 3);
    //         2026 in col F (idx 5) / Economia col H (idx 7).
    let header = vec![
        "".to_string(),
        "2025".to_string(),
        "Entradas".to_string(),
        "Economia".to_string(),
        "".to_string(),
        "2026".to_string(),
        "Entradas".to_string(),
        "Economia".to_string(),
    ];
    let month_names = [
        "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
    ];
    let mut rows = vec![header];
    for (i, &name) in month_names.iter().enumerate() {
        let eco26 = if i < 8 {
            format!("{}.00", (i + 1) * 1000) // months 1–8 of 2026 have values
        } else {
            "".to_string() // months 9–12 of 2026 are blank (not yet entered)
        };
        rows.push(vec![
            "".to_string(),
            name.to_string(),
            "5000.00".to_string(),
            format!("{}.00", (i + 1) * 500), // 2025: all 12 months
            "".to_string(),
            name.to_string(),
            "8000.00".to_string(),
            eco26,
        ]);
    }

    let got = parse_economia_sheet(&rows);

    // 2025 must have all 12 months.
    let y2025: Vec<_> = got.iter().filter(|&&(y, _, _)| y == 2025).copied().collect();
    assert_eq!(y2025.len(), 12, "2025 must have all 12 months (no premature break)");

    // 2026 must have all 12 months (months 9–12 are blank → 0 cents, but still present).
    let y2026: Vec<_> = got.iter().filter(|&&(y, _, _)| y == 2026).copied().collect();
    assert_eq!(y2026.len(), 12, "2026 must have all 12 months even when trailing rows are blank");

    // Spot-check a prior-year value and a current-year blank-trailing month.
    assert_eq!(
        y2025.iter().find(|&&(_, mo, _)| mo == 12).unwrap().2,
        600_000, // month 12 * 500 = 6000 (R$) → parse_number("6000.00") = 600_000 cents
        "2025 December present and correct"
    );
    assert_eq!(
        y2026.iter().find(|&&(_, mo, _)| mo == 9).unwrap().2,
        0,
        "2026 September (blank in sheet) is 0 cents, not missing"
    );
}
```

(Note: the test values use generic integers. Double-check the `parse_number`
behavior on blank strings — `parse_number("")` returns 0 based on the existing
test at line 1561.)

**Verify**: `cd src-tauri && cargo test import::tests::parse_economia_sheet 2>&1`
→ all three `parse_economia_sheet_*` tests pass.

### Step 4: Add explicit exact-boundary tests for saldoBand at R$1000 and R$2000

**File**: `src/lib/saldoHeatmap.test.ts`

First, confirm the live code in `saldoHeatmap.ts` still uses `<=` (not `<`) for
`tight` and `ok`:

```typescript
if (cents <= t.tight) return "tight";
if (cents <= t.ok) return "ok";
```

If the operators are `<` instead of `<=`, STOP and report — the boundaries have
been changed and this plan's assumptions are wrong.

If `<=` is confirmed, add explicit three-point boundary tests to the existing
`it.each` table in `saldoHeatmap.test.ts`. Insert these rows in the correct
numeric order (between existing values):

```typescript
    // R$2000 boundary: 200_001 → comfortable; 200_000 → ok (inclusive ≤); 199_999 → ok.
    [200_001, "comfortable"],
    [200_000, "ok"],       // boundary: R$2000 exact → ok (inclusive)
    [199_999, "ok"],
    // R$1000 boundary: 100_001 → ok; 100_000 → tight (inclusive ≤); 99_999 → tight.
    [100_001, "ok"],       // already present — confirm it is there; do not duplicate
    [100_000, "tight"],    // boundary: R$1000 exact → tight (inclusive)
    [99_999, "tight"],
```

(Some of these values may already be present. Do not duplicate — check the
existing table first and add only the values not yet covered.)

**Verify**: `npx vitest run src/lib/saldoHeatmap.test.ts` → exit 0, all pass.

### Step 5: Full gate

Run the full check suite to confirm nothing regressed:

```
npm run check
```

Expected: exit 0. If any step fails, fix it before moving on.

**Verify**: `npm run check` → exit 0.

## Test plan

| Test                                                        | File                            | Kind            | Covers                                                                      |
| ----------------------------------------------------------- | ------------------------------- | --------------- | --------------------------------------------------------------------------- |
| `staleness_check_rejects_different_revision`                | `write_back_cmds.rs` (existing) | Rust unit       | staleness pure decision — already passes                                    |
| `diff_delete_removes_orphan_import_conflict`                | `import.rs`                     | Rust async unit | re-import of sheet without a row removes its open `import_conflict`         |
| `parse_economia_sheet_asymmetric_blocks_no_premature_break` | `import.rs`                     | Rust unit       | 12-row prior-year + 8-row current-year: all 12 months of both years emitted |
| Boundary rows in `it.each` table                            | `saldoHeatmap.test.ts`          | Vitest unit     | R$1000 and R$2000 exact-boundary inclusive semantics                        |

The `staleness_check` pure function already has a test. No new pure test is
needed for Bug #1 because the decision function is unchanged; only the
plumbing in `apply_economia_write_back` changes (network-dependent → test the
pure function, not the Tauri command). If you want an additional integration
anchor, add a comment in the `economia_write_back_audit_realigns_source_amount`
test noting that the staleness gate now runs always.

## Done criteria

Machine-checkable. ALL must hold before marking DONE:

- [ ] `npm run rust:check` exits 0 (no Rust compilation errors)
- [ ] `npm run test:run` exits 0; new tests
      `diff_delete_removes_orphan_import_conflict` and
      `parse_economia_sheet_asymmetric_blocks_no_premature_break` exist and pass
- [ ] `npx vitest run src/lib/saldoHeatmap.test.ts` exits 0; new boundary rows
      for 100_000 and 200_000 exist and pass
- [ ] `npm run check` exits 0 (full gate: typecheck + lint + tests + e2e)
- [ ] `apply_economia_write_back` in `write_back_cmds.rs` no longer calls
      `guard_sheet_unchanged`; instead has `early_revision` snapshot and
      `staleness_check` match
- [ ] `parse_economia_sheet` in `import.rs` contains no reference to
      `saw_december` (confirm with `grep -n saw_december src-tauri/src/google_sheets/import.rs`)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row for plan 050 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The diff-delete loop in `import_rows_core` (around line 625 of `import.rs`)
  does **not** contain a `DELETE FROM import_conflict WHERE transaction_id = ?1`
  statement — this means the codebase has changed from what this plan describes
  and Step 2 needs a different approach.
- `apply_economia_write_back` no longer calls `build_economia_plan` as a single
  call returning `(client, plan)` — the refactoring in Step 1 depends on this
  signature; if it changed, the plumbing is different.
- The `saldoBand` function in `saldoHeatmap.ts` does NOT use `<=` for `tight`
  and `ok` — the boundary semantics have been changed and Step 4's verification
  is voided.
- `parse_economia_sheet` has been substantially rewritten (e.g. the `blocks`
  Vec and the inner `while rr` loop no longer exist) — the `saw_december`
  removal in Step 3 must be adapted to the new structure.
- Any step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching a file not in the in-scope list.

## Maintenance notes

- **Staleness gate uniformity**: `apply_write_back` and
  `apply_economia_write_back` now both use the always-on early-snapshot
  pattern. If a third apply command is added in the future, it must also follow
  this pattern (not `guard_sheet_unchanged`). The docstring on
  `guard_sheet_unchanged` calls it "no-op when `preview_revision=None`" — this
  is now intentional only for preview commands, not apply commands.
- **`parse_economia_sheet` layout assumptions**: removing `saw_december` means
  the function relies entirely on `!any` to stop a row group. If the sheet
  layout ever has a row after December that happens to contain a month name in
  the block column (e.g. a TOTAL/SUBTOTAL row labelled with a month abbreviation),
  it would be mis-parsed. Review the `month_number_from_name` function to
  confirm it rejects strings like "TOTAL", "Totais", and numeric-only cells.
- **Bug #2 confirmed closed**: the diff-delete `import_conflict` cleanup
  (lines 658-662 of `import.rs` at SHA `2132297`) was introduced by plan 047.
  If someone removes it by accident, the regression test added in Step 2 will
  catch it immediately.
- **Bug #4 boundary semantics**: the `<=` operators for `tight` and `ok` match
  the spreadsheet's `between` (inclusive) semantics for those two bands. Only
  the `critical` boundary uses strict `<` (matching the sheet's `lessThan`).
  Do not "normalize" all operators to one form without re-verifying each band
  against the sheet's conditional-formatting rules.
