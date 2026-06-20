# Plan 001: Fix side-by-side Economia sheet layout (import + write-back)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/google_sheets/import.rs src-tauri/src/google_sheets/write_back.rs`
> If either file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The reference spreadsheet's `Economia` tab places year blocks **side by side
in the same rows** (e.g., 2025 occupies columns B–E and 2026 occupies columns
G–J, both starting at row 4). The current importer (`parse_economia_sheet`)
finds only the first `Economia` label in the header row — which always belongs
to the leftmost (oldest) year — and advances past the entire row, silently
dropping every subsequent year's data. For a user whose current year is in the
second block, 100 % of their savings data is lost on every import. Symmetrically,
`plan_economia_write_back` also picks the first `Economia` column regardless of
which year is being written, so a write-back for the current year corrupts the
previous year's column. The fix makes both functions iterate over all year blocks
in a header row and scope each block's month-column and economia-column to the
year that owns it.

## Current state

### Files in scope

- `src-tauri/src/google_sheets/import.rs` — importer; contains `parse_economia_sheet`
  (lines 819–861) and its unit test `parse_economia_sheet_reads_blocks_per_year`
  (lines 1246–1279).
- `src-tauri/src/google_sheets/write_back.rs` — write-back planner; contains
  `plan_economia_write_back` (lines 212–274) and its unit tests
  `plans_economia_block_for_target_year_multi_year` (lines 382–431) and
  `economia_zeroed_locally_clears_stale_sheet_cell` (lines 433–463).

### Spreadsheet geometry (confirmed in `docs/example/Finanças.xlsx`, `Economia` tab)

```
col:   B       C           D         E    F   G       H           I         J
row 4: 2025    Entradas    Economia  %        2026    Entradas    Economia  %
row 5: jan     <ent>       <eco>     <pct>    jan     <ent>       <eco>     <pct>
row 6: fev     ...         ...       ...      fev     ...         ...       ...
...
row16: dez     ...         ...       ...      dez     ...         ...       ...
```

- Column B (index 1) holds the year label for block 1 and month names below it.
- Column G (index 6) holds the year label for block 2 and month names below it.
- `Economia` for block 1 is at column D (index 3); for block 2 at column I (index 8).
- Blocks share the **same rows** — there is no row separator between them.

### Bug in `parse_economia_sheet` (import.rs lines 819–841)

```rust
// import.rs:827–841  (BUGGY — picks only the FIRST econ_col in the row)
let econ_col = row
    .iter()
    .position(|c| c.trim().eq_ignore_ascii_case("economia")); // ← position() stops at the first match
let year_cell = row.iter().enumerate().find_map(|(i, c)| {   // ← find_map() stops at the first year
    c.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.fract() == 0.0 && (2000.0..2100.0).contains(n))
        .map(|n| (i, n as i32))
});
let (Some(econ_col), true, Some((month_col, year))) = (econ_col, has_entradas, year_cell)
else {
    r += 1;
    continue;
};
// reads block for that single (year, month_col, econ_col) → rest of header row ignored
// r = rr advances past ALL rows for block 1 only; block 2 (same rows) is never visited
```

When multiple year-blocks share a header row, `r = rr` skips past the data rows
that also carry the second block's months. The second block is never parsed.

### Bug in `plan_economia_write_back` (write_back.rs lines 228–237)

```rust
// write_back.rs:228–237  (BUGGY — position() returns the FIRST "Economia" column globally)
let header = rows.iter().enumerate().find_map(|(r, row)| {
    let has_entradas = row
        .iter()
        .any(|c| c.trim().eq_ignore_ascii_case("entradas"));
    let month_col = row.iter().position(|c| is_year(c))?;  // ← correct: finds target year's col
    let econ_col = row
        .iter()
        .position(|c| c.trim().eq_ignore_ascii_case("economia"))?; // ← BUG: always returns col of block 1
    has_entradas.then_some((r, month_col, econ_col))
});
```

Even when `month_col` correctly points to the second year's column, `econ_col`
always resolves to the first `Economia` label in the row (block 1's column).
Write-back for the current year therefore targets the prior year's Economia
column — silent data corruption.

### Repo conventions relevant to this fix

- **Functional-core style**: both functions are pure (`&[Vec<String>]` in, value
  out). Keep them pure — no IO, no Tauri calls.
- **Money as integer cents**: `i64` centavos throughout; `parse_number` from
  `import.rs` converts sheet strings to cents.
- **Amounts are positive magnitudes**: the `(i32, u32, i64)` tuple returned by
  `parse_economia_sheet` stores year/month/cents where cents ≥ 0 (Economia is
  always a savings amount).
- **`econ_col` must be to the right of its year cell**: in a side-by-side
  layout, block 2's `Economia` column comes after block 2's year column. Pair
  each year with the nearest `Economia` label that appears after it (and before
  the next year cell).
- **React Compiler is enabled** — irrelevant here (Rust-only change).
- **Do not add `memo`/`useMemo`/`useCallback`** — irrelevant here (Rust-only).
- **Test pattern**: model new tests after the existing tests in the same `#[cfg(test)]`
  block in each file (see `parse_economia_sheet_reads_blocks_per_year` at
  import.rs:1246 and `plans_economia_block_for_target_year_multi_year` at
  write_back.rs:382). Helpers (`h`, `m`, closures building grid rows) follow the
  same inline-closure pattern.

## Commands you will need

| Purpose            | Command                                                                                 | Expected on success  |
| ------------------ | --------------------------------------------------------------------------------------- | -------------------- |
| Rust check (full)  | `npm run rust:check`                                                                    | exit 0               |
| Rust tests only    | `cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features` | all pass             |
| Filter to economia | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia`                  | all pass (≥ 4 tests) |
| Full gate          | `npm run check`                                                                         | exit 0               |

Run `cargo test ... -- economia` after each step to keep the feedback loop tight.

## Scope

**In scope** (the only files you may modify):

- `src-tauri/src/google_sheets/import.rs` — fix `parse_economia_sheet` + add regression test.
- `src-tauri/src/google_sheets/write_back.rs` — fix `plan_economia_write_back` + add regression test.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/layout_detect.rs` — month/header detection for year-named
  tabs (2025, 2026). That geometry is different (vertical blocks per month); `detect_sheet_layout`
  and `parse_rows_with_layout` are correct and must not change.
- Any SQL migration, Tauri command, or frontend file — this fix is entirely in the pure
  parsing layer.
- `src-tauri/src/google_sheets/mod.rs` or callers of these two functions — the public
  function signatures do not change; callers need no update.

## Git workflow

- Branch: `fix/001-economia-side-by-side`
- Commit style: match recent repo history (`fix: <description>` conventional commit).
  Example from log: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`.
  Use a compact message: `fix: parse and write-back all side-by-side Economia year blocks`.
- One commit covering both file changes (they fix two halves of the same bug) is preferred
  over two separate commits.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Fix `parse_economia_sheet` to scan ALL year blocks in a header row

**Location**: `src-tauri/src/google_sheets/import.rs`, function `parse_economia_sheet`
(lines 819–861 at plan time).

**What the fix must do**:

Replace the current single-block extraction (one `find_map` for year + one `position`
for `econ_col`) with a loop that collects **all** `(year, month_col, econ_col)` triples
from the same header row before reading any data rows.

Algorithm:

1. Scan the header row to find every cell that parses as a year integer in
   `2000..2100`. Collect them as `(col_index, year)` pairs — **all of them**, not
   just the first.
2. For each year cell at `year_col`, find the nearest `Economia` label to its right
   (i.e., the first cell after `year_col` whose trimmed value is `"economia"`
   case-insensitively). That column is `econ_col` for this block.
3. Require at least one `Entradas` label somewhere in the row (existing guard —
   unchanged) to confirm this is a real Economia header row and not a data row that
   happens to contain a year-looking number.
4. If no valid `(year, econ_col)` pairs are found, advance `r` by 1 and continue
   (same as today).
5. When one or more pairs are found, read the data rows (inner `while rr < rows.len()`
   loop) once. For each data row, for each `(year, month_col, econ_col)` triple,
   read `rows[rr][month_col]` for the month name and `rows[rr][econ_col]` for the
   cents. Push `(year, month, cents)` for every pair. Stop the inner loop when the
   month column of the **first** block no longer yields a month name (i.e., TOTAL or
   empty row) — this is the existing termination condition and remains correct because
   all blocks share the same rows.
6. Advance outer `r = rr` as today.

**Target shape** (not the only correct implementation — what matters is the
semantics above):

```rust
pub fn parse_economia_sheet(rows: &[Vec<String>]) -> Vec<(i32, u32, i64)> {
    let mut out = Vec::new();
    let mut r = 0;
    while r < rows.len() {
        let row = &rows[r];
        let has_entradas = row
            .iter()
            .any(|c| c.trim().eq_ignore_ascii_case("entradas"));

        // Collect ALL (year_col, year, econ_col) triples from this header row.
        // econ_col = first "Economia" cell strictly to the right of the year cell.
        let mut blocks: Vec<(usize, i32, usize)> = Vec::new(); // (month_col, year, econ_col)
        if has_entradas {
            for (i, c) in row.iter().enumerate() {
                if let Some(year) = c
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.fract() == 0.0 && (2000.0..2100.0).contains(n))
                    .map(|n| n as i32)
                {
                    // Find the first "Economia" label to the right of column i.
                    if let Some(econ_col) = row[i + 1..]
                        .iter()
                        .position(|e| e.trim().eq_ignore_ascii_case("economia"))
                        .map(|p| i + 1 + p)
                    {
                        blocks.push((i, year, econ_col));
                    }
                }
            }
        }

        if blocks.is_empty() {
            r += 1;
            continue;
        }

        // month_col for the termination check = first block's year column (same as before).
        let first_month_col = blocks[0].0;
        let mut rr = r + 1;
        while rr < rows.len() {
            // Use the first block's month column to decide if this row is still in the block.
            let Some(month) = rows[rr]
                .get(first_month_col)
                .and_then(|l| month_number_from_name(l))
            else {
                break;
            };
            for &(month_col, year, econ_col) in &blocks {
                // Confirm month name is consistent across blocks (optional — belt+suspenders).
                let cents = rows[rr].get(econ_col).map(|c| parse_number(c)).unwrap_or(0);
                // Only push if this block's own month column also names a month (guards
                // against a ragged row where one block has fewer month rows than the other).
                if rows[rr].get(month_col).and_then(|l| month_number_from_name(l)).is_some() {
                    out.push((year, month, cents));
                }
            }
            rr += 1;
            if month == 12 {
                break;
            }
        }
        r = rr;
    }
    out
}
```

**Important**: the existing test `parse_economia_sheet_reads_blocks_per_year` (import.rs:1246)
uses a VERTICALLY stacked layout (one block below the other, different rows). That
test must continue to pass — do not break the vertical-stacking path.

**Verify**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- parse_economia
```

Expected: all existing `parse_economia*` tests pass (no new test yet).

---

### Step 2: Add a regression test for the side-by-side import layout

**Location**: `src-tauri/src/google_sheets/import.rs`, `#[cfg(test)]` block, after
`parse_economia_sheet_reads_blocks_per_year` (near line 1279 at plan time).

Add a test named `parse_economia_sheet_side_by_side_blocks`. It must:

1. Build a row vector that mirrors the actual spreadsheet geometry: ONE header row
   with 2025 at column 1, Entradas at column 2, Economia at column 3, `%` at column 4,
   a gap column at 5, 2026 at column 6, Entradas at column 7, Economia at column 8,
   `%` at column 9.
2. Add month data rows below (at least 2 months for each year, in the same rows).
3. Assert that `parse_economia_sheet` returns entries for **both** years.
4. Assert the correct `econ_col` values are used (different months of 2025 and 2026
   should map to their respective columns, verifiable via the returned `cents` values
   which differ between blocks).
5. Assert that a zero or blank in one block still appears in the output (existing
   zero-preservation invariant).

Model the test after `parse_economia_sheet_reads_blocks_per_year` — use inline
helper closures `h2` (two-block header row builder) and `m2` (two-block month row
builder) rather than importing anything new.

**Example fixture shape** (adapt as needed):

```rust
#[test]
fn parse_economia_sheet_side_by_side_blocks() {
    // Header row: col0="" | col1=2025 | col2=Entradas | col3=Economia | col4=% | col5="" |
    //             col6=2026 | col7=Entradas | col8=Economia | col9=%
    let header = vec![
        "".into(), "2025".into(), "Entradas".into(), "Economia".into(), "%".into(),
        "".into(),
        "2026".into(), "Entradas".into(), "Economia".into(), "%".into(),
    ];
    // Month row: col1=month_name for 2025, col3=eco_2025, col6=month_name for 2026, col8=eco_2026
    let month_row = |name: &str, eco25: &str, eco26: &str| -> Vec<String> {
        vec![
            "".into(), name.into(), "5000.00".into(), eco25.into(), "0".into(),
            "".into(),
            name.into(), "8000.00".into(), eco26.into(), "0".into(),
        ]
    };

    let rows = vec![
        header,
        month_row("jan", "1000.00", "1500.00"),
        month_row("fev", "0.0000", "2000.00"),
    ];

    let got = parse_economia_sheet(&rows);

    // Both years present.
    let y2025: Vec<_> = got.iter().filter(|&&(y, _, _)| y == 2025).collect();
    let y2026: Vec<_> = got.iter().filter(|&&(y, _, _)| y == 2026).collect();
    assert_eq!(y2025.len(), 2, "2025 deve ter jan e fev");
    assert_eq!(y2026.len(), 2, "2026 deve ter jan e fev");

    // Correct values per year.
    assert_eq!(y2025.iter().find(|&&(_, m, _)| m == 1).unwrap().2, 100_000); // 1000.00
    assert_eq!(y2025.iter().find(|&&(_, m, _)| m == 2).unwrap().2, 0);       // 0.0000 preserved
    assert_eq!(y2026.iter().find(|&&(_, m, _)| m == 1).unwrap().2, 150_000); // 1500.00
    assert_eq!(y2026.iter().find(|&&(_, m, _)| m == 2).unwrap().2, 200_000); // 2000.00
}
```

**Verify**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- parse_economia
```

Expected: all `parse_economia*` tests pass, including the new one (≥ 2 tests named
`parse_economia*`).

---

### Step 3: Fix `plan_economia_write_back` to scope `econ_col` to the target year's block

**Location**: `src-tauri/src/google_sheets/write_back.rs`, function
`plan_economia_write_back` (lines 212–274 at plan time).

**The bug** (write_back.rs:233–235):

```rust
// BUGGY: position() always returns the index of the FIRST "Economia" cell,
// which belongs to block 1 even when the target year is in block 2.
let econ_col = row
    .iter()
    .position(|c| c.trim().eq_ignore_ascii_case("economia"))?;
```

**Fix**: after finding `month_col` (the column where the target year appears),
find `econ_col` as the first `"Economia"` label **strictly to the right of
`month_col`**, not the first in the entire row.

```rust
// FIXED: search only to the right of the year column.
let econ_col = row[month_col + 1..]
    .iter()
    .position(|c| c.trim().eq_ignore_ascii_case("economia"))
    .map(|p| month_col + 1 + p)?;
```

Replace only the `econ_col` assignment inside the `find_map` closure. The rest of
the function (the data-row loop, the `CellWrite` construction, the zero-handling
guard) must remain unchanged.

Also update the doc comment on `plan_economia_write_back` (lines 203–211): replace
the phrase "EMPILHA um bloco por ano" with wording that acknowledges the side-by-side
layout — e.g. "can place multiple year blocks SIDE BY SIDE in the same rows or stack
them vertically".

**Verify**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia
```

Expected: all existing `economia*` tests pass (no new test yet).

---

### Step 4: Add a regression test for side-by-side write-back

**Location**: `src-tauri/src/google_sheets/write_back.rs`, `#[cfg(test)]` block,
after `plans_economia_block_for_target_year_multi_year` (near line 431 at plan time).

Add a test named `plans_economia_write_back_side_by_side_targets_correct_block`. It must:

1. Build a grid with ONE header row containing two side-by-side blocks:
   - Block 1: col 1 = "2025", col 2 = "Entradas", col 3 = "Economia", col 4 = "%"
   - Gap: col 5 = ""
   - Block 2: col 6 = "2026", col 7 = "Entradas", col 8 = "Economia", col 9 = "%"
2. Add at least one month data row below with values in both economia columns.
3. Call `plan_economia_write_back(&grid, 2026, &by)`.
4. Assert that all returned `CellWrite` entries target column index 8 (the second
   `Economia` column, i.e., `col_to_a1(8)` = `"I"`), not column index 3 (`"D"`).
5. Also call with `year = 2025` and assert targets are column index 3 (`"D"`).
6. This confirms neither year's write-back lands in the wrong block.

Model after `plans_economia_block_for_target_year_multi_year` — use the same
inline-closure style for the grid builder.

**Example fixture shape**:

```rust
#[test]
fn plans_economia_write_back_side_by_side_targets_correct_block() {
    // col: 0    1       2           3          4    5    6       7           8          9
    // row: ""   2025    Entradas    Economia   %    ""   2026    Entradas    Economia   %
    let header = vec![
        "".into(), "2025".into(), "Entradas".into(), "Economia".into(), "%".into(),
        "".into(),
        "2026".into(), "Entradas".into(), "Economia".into(), "%".into(),
    ];
    let data_row = |month: &str, eco25: &str, eco26: &str| -> Vec<String> {
        vec![
            "".into(), month.into(), "5000.00".into(), eco25.into(), "0".into(),
            "".into(),
            month.into(), "8000.00".into(), eco26.into(), "0".into(),
        ]
    };
    let grid = vec![
        header,
        data_row("jan", "1000.00", "500.00"),
    ];

    let mut by = [0i64; 12];
    by[0] = 200_000; // jan = 2000,00

    // Write-back for 2026 must target col 8 ("I"), NOT col 3 ("D").
    let plan26 = plan_economia_write_back(&grid, 2026, &by);
    assert!(!plan26.is_empty(), "deve planejar pelo menos jan de 2026");
    assert!(
        plan26.iter().all(|c| c.col == 8),
        "todas as escritas de 2026 devem ir para col 8 (bloco de 2026)"
    );
    assert!(plan26.iter().all(|c| c.a1.starts_with("I")),
        "coluna A1 de 2026 deve ser I (col 8)");

    // Write-back for 2025 must target col 3 ("D"), NOT col 8 ("I").
    let plan25 = plan_economia_write_back(&grid, 2025, &by);
    assert!(!plan25.is_empty(), "deve planejar pelo menos jan de 2025");
    assert!(
        plan25.iter().all(|c| c.col == 3),
        "todas as escritas de 2025 devem ir para col 3 (bloco de 2025)"
    );
    assert!(plan25.iter().all(|c| c.a1.starts_with("D")),
        "coluna A1 de 2025 deve ser D (col 3)");
}
```

**Verify**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia
```

Expected: all `economia*` tests pass, including the new one (≥ 4 tests named
`*economia*` in write_back).

---

### Step 5: Run the full Rust check

```
npm run rust:check
```

Expected: exit 0 (fmt + clippy + all tests green). Fix any clippy warnings before
proceeding.

---

### Step 6: Run the full project gate

```
npm run check
```

Expected: exit 0. If a non-Rust check fails (e.g. TypeScript, lint, Playwright),
investigate — the changes in this plan should not touch any frontend file.

## Test plan

**New tests to write**:

1. `parse_economia_sheet_side_by_side_blocks` (import.rs) — regression for the
   import bug: asserts both years' months are parsed from a single header row with
   two side-by-side blocks, with correct cent values and zero-preservation.

2. `plans_economia_write_back_side_by_side_targets_correct_block` (write_back.rs) —
   regression for the write-back bug: asserts that requesting write-back for the
   second year targets the second year's `Economia` column, and the first year targets
   the first year's column.

**Existing tests that must remain green** (do not alter them):

- `parse_economia_sheet_reads_blocks_per_year` (import.rs:1246) — vertical stacking
  (two blocks in different rows). Must still pass; the fix must support both layouts.
- `plans_economia_block_for_target_year_multi_year` (write_back.rs:382) — vertical
  stacking write-back. Must still pass.
- `economia_zeroed_locally_clears_stale_sheet_cell` (write_back.rs:433) — zero-handling.
  Must still pass.

**Structural pattern to follow**: both new tests should use the same inline helper
closure style as the existing tests in their respective files.

**Verification**:

```
cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia
```

Expected: all pass; count should be ≥ the pre-fix count plus 2.

## Done criteria

All of the following must hold before marking this plan DONE:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- economia` exits 0 with ≥ 2 more passing tests than before this plan started.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- parse_economia_sheet_side_by_side_blocks` exits 0 (new import test exists and passes).
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- plans_economia_write_back_side_by_side_targets_correct_block` exits 0 (new write-back test exists and passes).
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- parse_economia_sheet_reads_blocks_per_year` exits 0 (vertical-stacking test still passes — existing behavior not broken).
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked -- plans_economia_block_for_target_year_multi_year` exits 0 (vertical write-back test still passes).
- [ ] `npm run rust:check` exits 0 (fmt + clippy + all tests).
- [ ] `npm run check` exits 0.
- [ ] `git diff --name-only HEAD` shows only `src-tauri/src/google_sheets/import.rs` and `src-tauri/src/google_sheets/write_back.rs` (no other files modified).
- [ ] `plans/README.md` status row for plan 001 updated to `DONE`.

## STOP conditions

Stop and report back (do not improvise) if:

- The `parse_economia_sheet` function at `import.rs:819` does not match the excerpt
  in "Current state" (the codebase has drifted).
- The `plan_economia_write_back` function at `write_back.rs:212` does not match the
  excerpt in "Current state".
- The vertical-stacking test `parse_economia_sheet_reads_blocks_per_year` fails after
  applying the step 1 fix (the fix broke backward compatibility).
- The vertical-stacking write-back test `plans_economia_block_for_target_year_multi_year`
  fails after applying the step 3 fix.
- `npm run rust:check` fails with clippy errors that require touching files outside the
  in-scope list.
- Any step's verification command fails twice after a reasonable fix attempt.
- Resolving the bug appears to require changing `detect_sheet_layout`,
  `parse_rows_with_layout`, or any caller of these two functions (that would be
  out-of-scope scope creep; report it instead).
- A side-by-side fixture cannot be constructed because the real spreadsheet geometry
  differs from the description in "Why this matters" (e.g. the blocks are actually
  stacked vertically in the live sheet — re-examine `docs/example/Finanças.xlsx`
  directly and report the actual geometry).

## Maintenance notes

- **Future years**: the fix generalises to N side-by-side blocks, not just two. When
  2027 is added to the spreadsheet, no code change is needed — the loop collects all
  year cells from the header row.
- **Vertical stacking**: the fix must continue to handle vertical stacking (the existing
  test `parse_economia_sheet_reads_blocks_per_year` guards this). Do not remove that code path.
- **Reviewer focus for this PR**: scrutinise the `econ_col` scoping to the right of
  the year cell — specifically confirm that `row[month_col + 1..]` correctly offsets
  the `.position()` index back to an absolute column index (`month_col + 1 + p`).
  Off-by-one here would silently write to the wrong column.
- **Plan 021 (real-time sync) depends on this**: any background sync that reads/writes
  the Economia tab must go through the fixed versions of these two functions. Do not
  implement 021 before this plan is DONE.
- **Deferred**: this plan does not address the `%` (savings-rate) column — it is a
  spreadsheet formula and is never written by the app. No change needed there.
- **Deferred**: an integration test that imports a real two-block `docs/example/Finanças.xlsx`
  fixture end-to-end (through the Tauri command layer) would give stronger confidence.
  That belongs in a future characterization-test plan (see plan 010).
