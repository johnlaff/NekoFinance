# Plan 070: Surface note-parse failures and item↔cell divergences (make precision visible)

> **Executor instructions**: Follow step by step, run every verification, honor the
> STOP conditions, and update this plan's row in `plans/README.md` when done.
>
> **Drift check (run first)**:
> `git diff --stat b65f0c6..HEAD -- src-tauri/src/google_sheets/import.rs src-tauri/src/commands/sheets_import.rs`
> On any change, reconcile the "Current state" excerpts against live code first.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MEDIUM (return-type blast radius on frontend arithmetic + checksum-skip interaction)
- **Depends on**: none
- **Category**: dx / correctness-visibility
- **Planned at**: commit `b65f0c6`, 2026-07-04 · **Reconciled at `80710e3`, 2026-07-05**
  (plan 068 merged): `import.rs` — where Step 2 lives — is UNCHANGED, so its references
  hold; but `import_local_xlsx` now itemizes (`descriptions_trusted: notes_found`), so the
  "coordinate with 068" note is now ACTIVE — thread diagnostics through `import_local_xlsx`
  too. Line numbers in `sheets_import.rs` shifted by 068's ~990-line addition — reference
  functions by name and read the live code.

## Adversarial-review corrections (2026-07-04) — integrated below (changelog)

1. **Diagnostics must survive a checksum-DEDUPED import.** They're collected inside
   `import_rows_core`, reached only *after* the duplicate-checksum gate; on a no-op re-import
   (`import_one_tab`/`import_rows_with_options` `return Ok(0)` early) the diagnostics vanish.
   Either derive them from a direct query over persisted `transaction`+`line_item` state
   (independent of whether this run ran the full pipeline), or have the checksum-skip path
   still recompute them.
2. **Define `NoteNotItemized` on the RAW parser output, before the `has_breakdown` gate.** A
   single-item note with no section header is *intentionally* not a breakdown (import.rs:586-587)
   — that's not a diagnostic. Use strictly:
   `!raw_note.trim().is_empty() && parse_itemized_note(&raw_note).is_empty()`.
3. **Keep a numeric field in the return type.** `import_sheet_data`/`import_one_tab` currently
   return `Result<usize, String>` consumed as `Promise<number>` and used in **arithmetic** in
   `GoogleSheetsPanel.tsx` + `importAllTabs`'s `Acc`. Return `{ count, summary, diagnostics }`
   and update those consumers — do not replace the number with a string. (Only
   `import_local_xlsx` returns a summary string today; correct the "Current state" accordingly.)
4. **Reuse the EXISTING signed-residual mechanism.** The codebase already reconciles the
   `cell − Σitems` residual with sign (import.rs ~534/581, forecast_cmds.rs ~819-831, with a
   regression test) as the "AJUSTES / Diferença" convention — the real spreadsheet uses that
   header literally. The `ItemsDoNotSumToCell` diagnostic must *report* that residual, not
   change the (correct) data handling.
5. **A third, SYSTEMATIC note shape exists** in real data: a tab-separated monthly budget-plan
   (`Mensal⇥R$…⇥categoria` ×5 + `Total = R$…` + `R$… / N Dias = R$…`) on recurring Diário cells.
   It fires `ItemsDoNotSumToCell` every month — give it a specific diagnostic label, not a
   generic "itens não batem," so it doesn't read as a one-off typo.
6. **Enumerate all consumers of the changed return type:** frontend (`GoogleSheetsPanel.tsx`,
   `importAllTabs`, `src/lib/api.ts`) **and** `src-tauri/src/sync_task.rs:254` (auto-sync, safe
   only because it ignores the Ok value). Threading a new out-param through `import_rows_core`
   cascades to ~56 call sites (mostly tests) — a mechanical add.
7. **Fixes:** the "cell" field has no real spreadsheet address at collection time — use a
   synthetic `"{date} ({kind})"` id (mirroring `row_id`'s inputs). Drop the `import.rs:318`
   citation (that's the 3-way amount/description merge, a different mechanism). Remove the
   "plan 071" cross-reference in Out-of-scope (071 is REJECTED and was only about `line_item.id`,
   not grammar).

## Why this matters

Two precision-relevant conditions are currently handled **silently**: (1) a cell
note that doesn't fit the `R$ <valor> - <descrição>` grammar produces no line
items (the note grammar has drifted across years — spec 008), and (2) the parsed
items' sum diverges from the cell total. Today the importer keeps the cell as the
source of truth and moves on without telling the user. That is the right *data*
decision (the cell owns the total), but the *silence* hides where the app's picture
of the sheet is incomplete. Surfacing these makes the app's precision **visible** —
the user can go fix the note at the source, which is exactly how trust is earned in
a tool that mirrors a hand-kept spreadsheet.

## Current state

- The divergence decision is already made and commented (`import.rs`): items are
  persisted even when their sum diverges — "a célula continua dona do total"
  (`import.rs:973`, ~534, ~581). The residual `cell − Σitems` is already reconciled
  **with sign** as the spreadsheet's own "AJUSTES / Diferença" convention
  (forecast_cmds.rs ~819-831, with a regression test). So the *data* handling is correct
  and must not change — the new `ItemsDoNotSumToCell` diagnostic must **report** that
  existing residual, not re-handle it. (Do **not** cite `import.rs:318` — that is the
  3-way amount/description merge, a different mechanism.)
- `parse_itemized_note(note: &str) -> Vec<NoteLineItem>` (`import.rs:978`) returns
  an empty vec (no items) for a note it can't itemize; nothing records that a
  non-empty note produced zero items.
- **Return-type reality:** only `import_local_xlsx` returns `Result<String, String>` (a
  summary). `import_sheet_data` / `import_one_tab` return `Result<usize, String>` (a row
  count), consumed by the frontend as `Promise<number>` and used in **arithmetic**
  (`GoogleSheetsPanel.tsx`, `importAllTabs`'s `Acc`) — and by `sync_task.rs:254` (auto-sync,
  which ignores the value). Any new return shape must keep a numeric `count` field.

The change: while importing, **collect diagnostics** — (a) cells whose note is
non-empty but yielded 0 items ("nota não itemizada"), and (b) cells where
`Σ items ≠ cell` ("itens não batem: Σ X vs célula Y") — and return them so the UI
can show a small "N notas precisam de atenção" affordance. No data-handling change.

## Commands you will need

| Purpose    | Command                                                              | Expected |
|------------|---------------------------------------------------------------------|----------|
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- import`| all pass |
| Typecheck  | `npm run typecheck`                                                  | exit 0   |
| Full gate  | `npm run check`                                                      | exit 0   |

## Scope

**In scope**:
- `src-tauri/src/google_sheets/import.rs` — collect diagnostics during the per-cell
  itemization (the block around `import.rs:518–621`, "nota itemizada → line_item").
- The import command(s) in `src-tauri/src/commands/sheets_import.rs` — thread the
  diagnostics into the returned summary / a structured field.
- A minimal UI surface (one screen) that displays the diagnostics count + list.
- Tests.
- **Coordinate with plan 068**: today the diagnostics live on the live (Sheets-API) import
  only, since `import_local_xlsx` can't itemize. **Once plan 068 lands** (it makes the
  `.xlsx` path itemize with `descriptions_trusted:true`), extend this plan to thread
  diagnostics through `import_local_xlsx` too.

**Out of scope**:
- The divergence *data* decision (cell owns the total) — unchanged.
- The note grammar / `parse_itemized_note` logic itself (a future parser-hardening plan;
  here we only *report*, not re-parse). Not plan 071 — that is REJECTED and was only about
  `line_item.id`.

## Steps

### Step 1: Define a diagnostics type

Add a small struct, e.g. `ImportDiagnostic { sheet: String, cell: String, kind:
DiagKind, detail: String }` with `DiagKind ∈ { NoteNotItemized, ItemsDoNotSumToCell,
MonthlyBudgetPlanNote }`. The third kind labels the real recurring tab-separated
budget-plan note shape (`Mensal⇥R$…⇥categoria` ×5 + `Total = R$…` + `R$… / N Dias =
R$…`) so it doesn't read as a one-off typo. `cell` has no real spreadsheet address at
collection time — use a synthetic `"{date} ({kind})"` id (label only; collisions are
acceptable — it's a cosmetic identifier, not a key). Keep it serde-serializable.

**Verify**: `cargo check --manifest-path src-tauri/Cargo.toml --locked` → exit 0.

### Step 2: Collect diagnostics during itemization

In the itemization block (`import.rs:518–621`): define `NoteNotItemized` strictly on the
RAW parser output, **before** the `has_breakdown` gate (import.rs:586-587) — i.e.
`!raw_note.trim().is_empty() && parse_itemized_note(&raw_note).is_empty()`. A single-item
note with no section header is *intentionally* not a breakdown; that is NOT a diagnostic.
For divergence, report the existing signed residual (`ItemsDoNotSumToCell`, both totals in
`detail`) — **except** when `ItemsDoNotSumToCell` would fire AND the raw note matches the
recurring tab-separated budget-plan shape (a `Mensal⇥…` line, a `Total = R$…` line, and a
`R$… / N Dias = R$…` divisor line): label it `MonthlyBudgetPlanNote` instead, so the known
recurring convention doesn't read as a one-off typo. Thread a `&mut Vec<ImportDiagnostic>`
through the import function(s) — this cascades to ~56 call sites of
`import_rows_core`/`import_rows_with_options*` (mostly tests; a mechanical add).

**Survive a checksum-deduped import.** Diagnostics collected inside `import_rows_core` are
reached only *after* the duplicate-checksum gate — on a no-op re-import
(`import_one_tab`/`import_rows_with_options` `return Ok(0)` early) they would vanish. Either
derive the diagnostics from a direct query over persisted `transaction`+`line_item` state
(independent of whether this run ran the pipeline), or recompute them on the checksum-skip
path.

**Verify**: `cargo test ... -- import` → all pass (data unchanged; only a new out-param).

### Step 3: Return diagnostics from the command

Change the import command return to a DTO that **keeps a numeric field**:
`{ count: usize, summary: String, diagnostics: Vec<ImportDiagnostic> }`. Do NOT replace
the number with a string — `import_sheet_data`/`import_one_tab` return `Result<usize>`
consumed as `Promise<number>` in **arithmetic** (`GoogleSheetsPanel.tsx`, `importAllTabs`'s
`Acc`). Update `src/lib/api.ts`, the callers in `src/features/sheets/`, and extend
`importAllTabs`'s `Acc` with `diagnostics`. `sync_task.rs:254` ignores the value (safe
today — preserve that).

**Verify**: `npm run typecheck` → exit 0.

### Step 4: Surface it in the UI (one place)

In the sheets panel (`src/features/sheets/GoogleSheetsPanel.tsx`) or the import
result area, show "N notas precisam de atenção" when diagnostics is non-empty, with
an expandable list (sheet + cell + reason). Use existing design-system components
and tokens; money via `<Money>`. This is informational, not blocking.

**Verify**: `npm run test:run` → all pass; `npm run e2e` smoke stays green.

## Test plan

- Unit: feed the itemization path a note with prose-only content → assert one
  `NoteNotItemized` diagnostic and 0 items (data unchanged).
- Unit: feed items that don't sum to the cell → assert one `ItemsDoNotSumToCell`
  with the correct totals, and the items are still persisted (cell owns total).
- A clean note (items sum to cell) → **zero** diagnostics.
- Frontend: the panel renders the count when diagnostics exist and nothing when
  empty (model after an existing `GoogleSheetsPanel` test).

## Done criteria

- [ ] Non-itemizable notes and item↔cell divergences are reported (count + list),
      never silent.
- [ ] The divergence *data* behavior is unchanged (cell still owns the total; items
      still persisted) — verified by an assertion, not just prose.
- [ ] `npm run check` exits 0.
- [ ] `plans/README.md` row updated.

## STOP conditions

- Adding diagnostics would require changing which value is authoritative (it must
  not — the cell owns the total).
- The import command's return type is consumed in more places than
  `src/features/sheets/` and `src/lib/api.ts` (also `sync_task.rs:254`) — enumerate them
  before changing the shape, and keep the numeric `count` field (frontend arithmetic
  depends on it); do NOT collapse it to a summary string.

## Maintenance notes

- This is the reporting layer; a future parser-hardening plan can *reduce* the
  `NoteNotItemized` count by widening the grammar. Keep the diagnostic kinds stable
  so a trend ("notas com atenção" over time) is possible.
- Pairs naturally with plan 069 (obligation concept): a recurring obligation whose
  note stops parsing shows up here first.
