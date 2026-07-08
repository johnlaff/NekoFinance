# Plan 068: Recover cell notes on the local `.xlsx` import (stop dropping itemization)

> **Executor instructions**: Follow this plan step by step. Run every verification
> command and confirm the expected result before the next step. Honor the STOP
> conditions. When done, update this plan's row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat b65f0c6..HEAD -- src-tauri/src/commands/sheets_import.rs src-tauri/src/google_sheets/import.rs`
> If either changed, compare the "Current state" excerpts below against the live
> code before proceeding; on a mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `b65f0c6`, 2026-07-04

## Adversarial-review corrections (2026-07-04) — integrated below (changelog)

The items below are now folded into Scope / Steps / Test plan; this list is a changelog.

1. **Align comment (row,col) to calamine's used-range origin.** `calamine 0.35`'s
   `worksheet_range()` starts at the first non-empty cell (`HeaderRow::FirstNonEmptyRow`),
   not A1. Read `range.start()` from the same call and **subtract it** from every decoded
   comment (row,col) before inserting into the notes grid (drop/clamp comments above/left of
   the range). The notes grid must be indexed identically to `rows`.
2. **The note string must be BYTE-IDENTICAL to `get_sheet_notes`'s output.** `036`/re-import
   compare the whole note string (`note_changed`, import.rs:561) and it feeds the checksum.
   If `read_xlsx_comments` flattens `<text>` runs differently than the API path, it spuriously
   trips `note_changed` / a new checksum. Add a test asserting byte-equality for the same note.
3. **Make the existing warning conditional.** `sheets_import.rs:440-450` already appends an
   unconditional "arquivos .xlsx não carregam notas" disclaimer — gate it on whether any
   sheet's notes were actually empty/unreadable (incl. the Step-4 fallback); drop it on success.
4. **Dependencies:** `zip` and `quick-xml` are transitive-only. Add `zip` as a direct dep;
   for XML, hand-parse or add `quick-xml` **≥0.41** (NOT the transitive 0.39.4, under
   RUSTSEC-2026-0194/0195) as a direct dep, and update
   `docs/version-matrix.md`.
5. **A1 decoder must handle multi-letter columns** (base-26, no cap at Z — the real workbook
   has refs like AZ16/BL32/BP32). Include a two-letter-column comment in the test fixture.
6. **Test at the pool level, not the command.** `import_local_xlsx` takes `tauri::State`,
   which tests can't construct. The automated test stops at `read_xlsx_comments` +
   `parse_rows_with_layout` + the pool-level `import_rows` (mirror
   `line_items_stored_when_note_sums_match_total`). Add a "reimport an `.xlsx` with comments
   twice → same checksum / no-op" test at that level.
7. **Helper location:** keep the new helper in `sheets_import.rs` or under `commands/` (not a
   new `google_sheets/xlsx_notes.rs`, which would need a `mod.rs` declaration and break the
   "no change to `mod.rs`" done criterion) — or relax that done criterion to "no behavioral
   change to `get_sheet_notes`."
8. **Coordinate with plan 070:** once this lands, `import_local_xlsx` sets
   `descriptions_trusted:true` and starts itemizing — so it becomes a diagnostics-producing
   path. Plan 070 must then include the `.xlsx` command; note the ordering in both plans.

## Why this matters

The local `.xlsx` import throws away **all** cell notes — and the cell notes are
where every obligation's description and itemization live (`R$ x - Aluguel /
R$ y - Fatura …`). So importing a downloaded `.xlsx` yields a ledger with generic
`"Saída {data}"` descriptions and **zero `line_item` rows**, while importing the
same data via the Google Sheets API keeps the full breakdown. The notes are
physically present in the `.xlsx` (it contains `xl/comments1.xml`,
`xl/comments2.xml`); `calamine` simply doesn't expose them. This is the single
biggest precision gap in the sheet→app mapping: the richest data is in the file
and silently discarded.

## Current state

- `src-tauri/src/commands/sheets_import.rs:296` — `import_local_xlsx` opens the
  workbook with `calamine::open_workbook` and, per sheet, builds `rows:
Vec<Vec<String>>` from `worksheet_range`. At **line 372** it calls the parser
  with an **empty** notes slice, and the comment says why:

  ```rust
  // xlsx (calamine) não expõe notas de célula → fallback "Entrada/Saída {data}". As
  // notas só vêm pelo caminho ao vivo (Sheets API), então o fallback não vira base
  // canônica de descrição.
  let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &[])?;
  ```

  Just below, `ImportRowsOptions { descriptions_trusted: false }` (line ~377).

- The **Google Sheets API path** does it right: `SheetsClient::get_sheet_notes`
  (`src-tauri/src/google_sheets/mod.rs:88`) fetches notes and builds a
  `Vec<Vec<String>>`; the parser reads a cell's note via
  `cell_raw_note(notes, row, col)` (`import.rs:839`), and
  `parse_rows_with_layout(rows, layout, mappings, notes: &[Vec<String>])`
  (`import.rs:1144`) takes that grid. So the parser already handles notes — the
  `.xlsx` path just never supplies them.

- The note→items pipeline is downstream and unchanged: `parse_itemized_note`
  (`import.rs:978`) turns a note into `line_item` rows.

The fix is therefore scoped: build the same `Vec<Vec<String>>` notes grid from the
`.xlsx`'s own comment XML and pass it at line 372 (with `descriptions_trusted:
true` when notes exist), instead of `&[]`.

## Commands you will need

| Purpose    | Command                                                              | Expected            |
| ---------- | -------------------------------------------------------------------- | ------------------- |
| Rust build | `cargo check --manifest-path src-tauri/Cargo.toml --locked`          | exit 0              |
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- import` | all pass            |
| Clippy     | `npm run rust:clippy`                                                | exit 0, no warnings |
| Deps doc   | (if you add a crate) update `docs/version-matrix.md`                 | —                   |

## Scope

**In scope**:

- `src-tauri/src/commands/sheets_import.rs` — read comments, pass the notes grid.
- A new helper in `sheets_import.rs` (or under `commands/`) that parses `.xlsx` comment
  XML into a per-sheet `Vec<Vec<String>>`. Do NOT create `google_sheets/xlsx_notes.rs` —
  a new module there needs a `mod.rs` declaration, which breaks the "no change to
  `google_sheets/mod.rs`" done criterion.
- Its unit tests + a fixture `.xlsx` with a known comment.

**Out of scope**:

- The Google Sheets API path (`get_sheet_notes`) — already correct.
- `parse_rows_with_layout` / `parse_itemized_note` — unchanged; they already take
  and use the notes grid.
- The `xlsx_cell_to_string` numeric handling.

## Git workflow

- Branch: `advisor/068-xlsx-notes`
- Commit style matches the repo log (Portuguese subject, e.g.
  `Import .xlsx recupera notas de célula (fim do fallback sem itemização)`).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Confirm calamine does not already expose comments

Check the `calamine` version (`grep calamine src-tauri/Cargo.toml` → `=0.35.0`) and
its API. If `calamine 0.35` exposes a comments/annotations accessor, **STOP and
report** — use that API instead of hand-parsing XML (simpler and safer).

**Verify**: read-only. Proceed only if calamine has no comment accessor.

### Step 2: Parse the `.xlsx` comment XML into a notes grid

Add `read_xlsx_comments(path: &Path) -> Result<HashMap<String, Vec<Vec<String>>>, String>`
(sheet display name → notes grid indexed `[row][col]`, matching the API path's
shape). A `.xlsx` is a zip; read, in order:

- `xl/workbook.xml` → sheet **name** → `sheetId` + `r:id`.
- `xl/_rels/workbook.xml.rels` → `r:id` → `xl/worksheets/sheetN.xml`.
- `xl/worksheets/_rels/sheetN.xml.rels` → the `.../commentsM.xml` target (a sheet
  with no comments has no such rel — return an empty grid for it).
- `xl/commentsM.xml` → `<commentList><comment ref="A1"><text>…`. Flatten each
  comment's `<text>` runs to a plain string **byte-identically to how the Sheets-API
  path (`get_sheet_notes`) joins runs** — if the two producers differ, the `.xlsx` note
  trips `note_changed` (import.rs:561) / a new checksum spuriously (add a test that both
  produce the same string for the same note). Decode (row, col) from the `ref` A1 address
  with a **base-26 multi-letter column** decoder (real refs run past Z: AZ16/BL32/BP32),
  then **subtract `range.start()`** (Step 3) so the grid aligns to calamine's used-range
  origin, matching `cell_raw_note`'s indexing.

**Dependencies:** `zip` and `quick-xml` are transitive-only (in Cargo.lock, not
Cargo.toml). Add **`zip`** as a direct dep (pin to the resolved version). For the XML,
either hand-parse the tiny comment files, or add **`quick-xml` at ≥0.41** as a direct
dep — do NOT pin to the transitive `0.39.4`, which is under RUSTSEC-2026-0194/0195
(ignored for the transitive path in `.cargo/audit.toml`, but a _direct_ dep should be on
the fixed line). Document any new dep in `docs/version-matrix.md`.

**Verify**: `cargo check --manifest-path src-tauri/Cargo.toml --locked` → exit 0.

### Step 3: Supply the notes grid on the import path

In `import_local_xlsx`, call `read_xlsx_comments(&workbook_path)` once before the
sheet loop. Inside the loop, for the current `sheet_name`, look up its grid
(default empty) and pass it to `parse_rows_with_layout` at line 372 instead of
`&[]`. When the sheet has a non-empty grid, set
`ImportRowsOptions { descriptions_trusted: true }` (mirror the API path) and use
`compute_import_checksum(&imported_rows, true)`; keep `false` when there are no
notes (unchanged behavior). **Align the grids to the same origin:** `calamine 0.35`'s
`worksheet_range()` starts at the first non-empty cell (`HeaderRow::FirstNonEmptyRow`),
not A1 — read `range.start()` from that same call and subtract it from every decoded
comment (row, col) before inserting (drop/clamp comments above/left of the range). The
notes grid and the values grid must be indexed identically.

**Verify**: `cargo test ... -- import` → all pass (existing import tests, incl.
`reimport_*` idempotency, stay green).

### Step 4: Fallback when comment parsing fails

If `read_xlsx_comments` errors (malformed zip, unexpected layout), do **not** fail the
import — fall back to the current empty-notes behavior with a warning. **Make the existing
UNCONDITIONAL disclaimer at `sheets_import.rs:440-450` conditional:** emit "arquivos .xlsx
não carregam notas…" only when a sheet's notes were actually empty/unreadable, and drop it
on success (otherwise every successful notes-carrying import still nags). A degraded import
beats a failed one.

**Verify**: a deliberately malformed comments fixture still imports (values only) and its
summary carries the warning; a **successful** notes-carrying import's summary does **not**
contain the disclaimer.

## Test plan

- New unit test for `read_xlsx_comments`: a small fixture `.xlsx` (checked into
  `src-tauri/tests/fixtures/` or generated in-test) with a known comment on a Saída
  cell → assert the grid has that note at the right (row, col).
- Integration test modeled after the API-path itemization tests (find them:
  `grep -n "line_item" src-tauri/src/google_sheets/import.rs` in the `#[cfg(test)]`
  region): import the fixture, assert the transaction description comes from the
  note and `line_item` rows exist with the parsed amounts/sections.
- Regression: an `.xlsx` **without** comments imports exactly as before (generic
  description, no items) — no behavior change on note-less files.
- **Pool-level, not the command**: `import_local_xlsx` takes `tauri::State`, which tests
  can't construct — call `read_xlsx_comments` + `parse_rows_with_layout` + the pool-level
  `import_rows` directly (mirror `line_items_stored_when_note_sums_match_total`'s style).
- **Reimport idempotency (new path)**: import an `.xlsx` with comments **twice** → the
  second run is a no-op with the same checksum (`compute_import_checksum` twice yields the
  same value; no duplicate rows).
- Verification: `cargo test ... -- import` → all pass, including the new tests.

## Done criteria

- [ ] Importing a `.xlsx` that has cell comments produces real descriptions and
      `line_item` rows (not `"Saída {data}"`).
- [ ] `cargo check`, `cargo test ... -- import`, and `npm run rust:clippy` exit 0.
- [ ] An `.xlsx` without comments imports unchanged (regression test passes).
- [ ] Reimport of an `.xlsx` with comments is idempotent (twice → same checksum, no dup).
- [ ] A successful notes-carrying import's summary does NOT contain the `.xlsx` disclaimer.
- [ ] If a dependency was added, `docs/version-matrix.md` is updated.
- [ ] No change to `src-tauri/src/google_sheets/mod.rs` (API path untouched).
- [ ] `plans/README.md` row updated.

## STOP conditions

- `calamine 0.35` already exposes comments — use its API; this plan's XML parsing
  is unnecessary.
- The `.xlsx` comment→cell mapping in the real file does not match the assumed
  OOXML layout (e.g. comments are threaded comments in `xl/threadedComments/`
  instead of legacy `xl/comments*.xml`) — report what you found before improvising.
- Flipping `descriptions_trusted` to `true` breaks the `reimport_*` /
  `compute_checksum` idempotency tests — the checksum must stay stable across
  re-imports; reconcile before proceeding.
- Building the notes grid would require touching the parser (`parse_rows_with_layout`
  / `parse_itemized_note`) — it shouldn't; the grid is an input. If it does, stop.

## Maintenance notes

- The API path and the `.xlsx` path now both feed the same notes grid into the same
  parser — keep them converged; a future change to the note shape must update both
  producers, not the parser.
- Threaded comments (Excel's newer comment model) live elsewhere in the OOXML zip;
  if users start producing those, extend `read_xlsx_comments` to read
  `xl/threadedComments/*` too.
- This unblocks plan 069 (obligation concept) and plan 067 (scenario overrides) on
  the `.xlsx` path — both depend on `line_item` breakdown being present.
- **Coordinate with plan 070**: once this lands, `import_local_xlsx` sets
  `descriptions_trusted:true` and itemizes, so it becomes a diagnostics-producing path —
  plan 070 must extend its scope to include the `import_local_xlsx` command (today 070
  excludes it because the `.xlsx` path can't itemize).
