# Plan 062: Auto-economia writes back to the spreadsheet Economia tab (app ↔ sheet match)

> **Executor instructions**: Follow step by step; run every verification command. Any real
> Google Sheets write must go through a structured diff + human approval — never auto-write.
> If a "STOP condition" occurs, stop and report. When done, update `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src-tauri/src/commands/write_back_cmds.rs src-tauri/src/google_sheets`
> Compare excerpts to live code before proceeding.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (Google Sheets write path — guarded by diff + approval)
- **Depends on**: plans/060-engine-five-type-alignment.md
- **Category**: methodology / sync
- **Planned at**: commit `da2d3e9`, 2026-06-22
- **Completed**: 2026-06-23 on branch `advisor/062-economia-tab-writeback-sync`

## Why this matters

Once the engine derives Economia automatically (plan 060) from the classified note items, the
spreadsheet's **Economia tab** (where the owner today types the monthly economia by hand) must be
kept in sync — the owner's requirement: _"o Neko sempre deve casar com a planilha"_. This plan
feeds the auto-derived monthly economia totals into the **existing economia write-back path** so
the app proposes filling the Economia tab, with a diff the owner approves, and a clean
round-trip (re-import sees the same value, no duplicate/drift).

## Current state

- Neko already has an **economia write-back** path (built in plans 052 / 055): grep for
  `preview_economia_write_back_status`, `economia` in `src-tauri/src/commands/write_back_cmds.rs`
  and the e2e mock `tests/e2e/tauri-mock.ts` (`preview_economia_write_back_status` is mocked).
  Confirm the exact command names + the cell-targeting logic it uses for the Economia tab.
- The **Economia tab** layout (from the real sheet): standalone side-table, sheet name
  `Economia`; two year blocks — 2025 in columns B–E, 2026 in columns G–J; rows 5–16 = Jan–Dec,
  row 18 = TOTAL. The **Economia amount** column is `D` (2025) / `I` (2026); the `%` column has a
  formula `=D{n}/C{n}` (do NOT overwrite the % formula — only the amount column).
- The amount column is currently **manual** (all zeros). After plan 060 the app can compute the
  monthly economia total = sum of `ItemKind::Economia` line-items for that month (NOT patrimônio —
  patrimônio is excluded from the Economia%).
- The general write-back contract (`AGENTS.md`): "Any material Google Sheets write must produce a
  structured diff, pass validation, and require human approval." The existing year-grid write-back
  (`preview_write_back_status` → `apply_write_back`) is the pattern to mirror; the
  write-back↔re-import identity (no duplicate) was established in plan 055 — preserve it.

## Commands you will need

| Purpose         | Command                                                    | Expected |
| --------------- | ---------------------------------------------------------- | -------- |
| Rust check+test | `npm run rust:check`                                       | exit 0   |
| Targeted test   | `cargo test --manifest-path src-tauri/Cargo.toml economia` | pass     |
| Frontend types  | `npm run typecheck`                                        | exit 0   |
| E2E             | `npm run e2e`                                              | pass     |

## Scope

**In scope:**

- The economia write-back command(s) in `src-tauri/src/commands/write_back_cmds.rs` (+ its
  helpers) — source the proposed Economia-tab amount from the engine's auto-derived monthly
  economia (plan 060) instead of (or in addition to) a manual value.
- The diff/preview for the Economia tab (mirror the existing `preview_economia_write_back_status`).
- The Settings/Sync UI that surfaces the economia diff for approval (the `WriteBackPending` /
  GoogleSheetsPanel path already render write-back diffs — extend, do not rebuild).
- Tests.

**Out of scope:**

- The engine math (plan 060) and the classifier (plan 059).
- Overwriting the `%` formula column or any year-grid cell — only the Economia **amount** column.
- Writing to the sheet without the existing diff + approval gate. Never auto-apply.

## Git workflow

- Branch: `advisor/062-economia-tab-writeback-sync`
- Message: `feat(sync): write auto-derived economia to the Economia tab (diff + approval, round-trip)`

## Steps

### Step 1: Locate + confirm the existing economia write-back

`grep -rn "economia" src-tauri/src/commands/write_back_cmds.rs` and read the
`preview_economia_write_back_status` / apply path. Confirm how it currently targets the Economia
tab cells (block by year, row by month) and how the diff is shaped. Write down the exact command
names. **Verify**: you can state the current behavior; `npm run rust:check` baseline → exit 0.

### Step 2: Source the proposed amount from the engine

Make the economia write-back compute the proposed monthly amount = sum of `ItemKind::Economia`
line-items for that month (from plan 060), per year block. Patrimônio (`INVESTIMENTO`) is
EXCLUDED. Target only the amount column (`D`/`I`), never the `%` formula. **Verify**: a unit test
asserts the proposed amount equals the month's economia total for a fixture.

### Step 3: Diff + approval + round-trip

Ensure the preview produces a structured diff (current Economia-tab value vs proposed) and that
applying requires explicit human approval (reuse the existing 2nd-confirmation dialog). Ensure
the write-back↔re-import identity holds: after a write + re-import, the Economia tab value matches
the app and no duplicate is created (mirror plan 055's identity guarantee). **Verify**:
`cargo test ... economia` includes a round-trip identity test.

### Step 4: Surface in the UI

The Sync section already renders write-back diffs (`WriteBackPending` in Settings). Make the
economia diff appear there for approval alongside the year-grid diff. **Verify**:
`npm run typecheck && npm run e2e` → pass.

## Test plan

- Proposed economia amount = sum of Economia line-items for the month (patrimônio excluded).
- Round-trip identity: write → re-import → app value == sheet value, no duplicate (plan 055 pattern).
- Diff requires approval; cancel writes nothing (mirror the existing write-back tests).
- Pattern: the existing economia write-back tests (plans 052/055) + `write_back_cmds.rs` tests.

## Done criteria

- [x] Economia-tab amount column is proposed from the engine's auto economia (patrimônio excluded)
- [x] Only the amount column is written; the `%` formula column is never overwritten
- [x] Write requires the structured diff + human approval; round-trip identity holds (no duplicate)
- [x] `npm run rust:check`, `npm run typecheck`, `npm run e2e` exit 0
- [x] `plans/README.md` updated

## Completion notes

- `load_economia_by_month` now sources the proposed amount from `line_item` rows whose section
  classifies as `ItemKind::Economia`; it excludes Patrimônio, stale `economia_annotation`, manual
  transfers, excluded-tag parents, and description/bank-name fallback.
- The existing Economia write-back planner still writes only the discovered `Economia` amount column
  in the target year block; existing tests continue to guard `Entradas`/`%` formula columns.
- Round-trip audit still aligns `economia_annotation`, and a regression test ensures that annotation
  is not added back on top of auto-derived items.

Verification run:

- `cargo test --manifest-path src-tauri/Cargo.toml economia`
- `npm run typecheck`
- `npm run test:run`
- `npm run rust:check`
- `npm run e2e`
- `npm run doctor`
- `npm run ui:audit`
- `npm run privacy:scan`
- `npm run deadcode`
- `npm run check`

## STOP conditions

- No existing economia write-back path is found (grep returns nothing) → STOP and report; this
  plan assumes plans 052/055 landed it. Building a write-back from scratch is a separate plan.
- The Economia-tab layout in the live sheet differs from the excerpt (block columns / rows) → STOP.
- Applying would touch the `%` formula or a year-grid cell → STOP (out of scope).

## Maintenance notes

- Patrimônio (previdência) is intentionally NOT written to the Economia tab — it is a separate
  long-term bucket (owner decision). If a Patrimônio column/tab is added later, that is a new plan.
- Reviewer: confirm the approval gate and the round-trip identity (no drift, no duplicate).
