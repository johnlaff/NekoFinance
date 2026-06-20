# Plan 014: Documentation truth fixes

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- docs/testing-strategy.md docs/architecture.md docs/version-matrix.md README.md src-tauri/migrations/20240612000010_drop_unused_fts.sql`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Stale documentation that contradicts the actual repo state actively misleads contributors and agents (including future executor models). A wrong "blocking gate" description causes developers to wait for a slow browser test they don't need to pass; a "planned" FTS label hides a completed attempt-and-removal; a wrong keyring version in the planned-deps table is a silent trap; missing `patchelf` breaks first-time Linux/WSL2 setup with a cryptic linker error; and the dangling pointer in the migration comment sends readers to a dead anchor. These five fixes bring the docs into exact agreement with the installed code so the repo can be trusted on its face.

## Current state

### Files and their roles

- `docs/testing-strategy.md` — coverage policy, Playwright smoke doc, React Doctor doc. Lines 9–11 claim the Playwright E2E smoke is part of `npm run check` (the blocking gate).
- `docs/architecture.md` — runtime-layer table and MVP-slice log. Line 13 labels full-text search as "planned."
- `docs/version-matrix.md` — planned-dependency table. Line 57 lists `keyring` at `4.0.1`.
- `README.md` — project readme, Linux/WSL2 prereq list. Lines 64–68 contain the `apt install` list without `patchelf`.
- `src-tauri/migrations/20240612000010_drop_unused_fts.sql` — SQL that drops the FTS tables. Line 4 ends with `(ver docs/testing-strategy)`, a pointer to a section that does not exist.

### Verified source of truth for each finding

**Finding 1 — testing-strategy.md, blocking gate wording**

`package.json` (checked live):

```
"check": "npm run format:check && npm run lint && npm run typecheck && npm run e2e:typecheck && npm run test:run && npm run build && npm run rust:check && npm run privacy:scan && npm run ui:audit"
```

`npm run check` includes `npm run e2e:typecheck` (type-checks Playwright `.ts` test files via `tsc -p tsconfig.playwright.json --noEmit`) but does NOT include `npm run e2e` (the actual browser test run). The current text at `docs/testing-strategy.md:10-11` reads:

```
signal — the blocking gate (`npm run check`, CI) runs `test:run` (no coverage threshold) plus lint,
typecheck, the Playwright E2E smoke, clippy, and the privacy scan.
```

"the Playwright E2E smoke" is wrong; it is not run by `npm run check`.

**Finding 2 — architecture.md, FTS described as "planned"**

`docs/architecture.md:13`:

```
| Local storage    | SQLite (WAL) for normalized finance data, settings, and sync state. Full-text search is planned.    |
```

Reality: FTS5 tables were created in `src-tauri/migrations/20240608000015_fts5.sql` and then intentionally dropped in `src-tauri/migrations/20240612000010_drop_unused_fts.sql` because no writer populated them and client-side filtering was used instead. FTS is not "planned" — it was tried and removed.

**Finding 3 — version-matrix.md, keyring version in Planned table**

`docs/version-matrix.md:57`:

```
| OS keychain       | `keyring`                                     | `4.0.1`        | For OAuth refresh tokens/API keys if needed.                                                     |
```

`src-tauri/Cargo.toml:35`:

```
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service"] }
```

`src-tauri/Cargo.lock` resolves this to `keyring 3.6.3`. The crate is already installed and locked at `3.6.3`, not a future candidate at `4.0.1`.

**Finding 4 — README.md, missing patchelf prerequisite**

`README.md:65-68` (the Linux/WSL2 `apt install` block):

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev pkg-config
```

`patchelf` is required by Tauri's AppImage bundler for Linux builds. Without it, `npm run tauri dev` or any Tauri build fails for new Linux/WSL2 developers with a confusing error. It must be added to this list.

**Finding 5 — drop_unused_fts.sql, stale pointer**

`src-tauri/migrations/20240612000010_drop_unused_fts.sql:4`:

```sql
-- a busca de Lançamentos é filtrada no cliente. Manter tabelas mortas confunde o schema. Recriar
-- com triggers + rebuild quando a busca full-text for de fato implementada (ver docs/testing-strategy).
```

`docs/testing-strategy.md` contains no section on FTS, FTS rebuild, or anything matching this pointer. The `(ver docs/testing-strategy)` parenthetical leads nowhere useful.

## Commands you will need

| Purpose                      | Command                                                                                                                                                                   | Expected on success                                                           |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Drift check                  | `git diff --stat d183bbf..HEAD -- docs/testing-strategy.md docs/architecture.md docs/version-matrix.md README.md src-tauri/migrations/20240612000010_drop_unused_fts.sql` | empty or no in-scope files changed                                            |
| Verify no e2e in check       | `grep '"check"' package.json`                                                                                                                                             | line does not contain `run e2e ` (space after `e2e`; `e2e:typecheck` is fine) |
| Verify keyring in Cargo.toml | `grep 'keyring' src-tauri/Cargo.toml`                                                                                                                                     | `version = "3"`                                                               |
| Verify keyring in Cargo.lock | `grep -A2 'name = "keyring"' src-tauri/Cargo.lock`                                                                                                                        | `version = "3.x.x"`                                                           |
| Lint                         | `npm run lint`                                                                                                                                                            | exit 0, no errors                                                             |
| Typecheck                    | `npm run typecheck`                                                                                                                                                       | exit 0, no errors                                                             |
| Privacy scan                 | `npm run privacy:scan`                                                                                                                                                    | exit 0                                                                        |
| Full gate                    | `npm run check`                                                                                                                                                           | exit 0                                                                        |

## Scope

**In scope** (the only files you should modify):

- `docs/testing-strategy.md`
- `docs/architecture.md`
- `docs/version-matrix.md`
- `README.md`
- `src-tauri/migrations/20240612000010_drop_unused_fts.sql`

**Out of scope** (do NOT touch, even though they look related):

- `package.json` — the gate definition is correct; only the doc description was wrong.
- `src-tauri/Cargo.toml` — already correct (`version = "3"`); the version-matrix.md table is the only artifact to reconcile.
- `src-tauri/Cargo.lock` — never edit manually.
- Any migration other than `20240612000010_drop_unused_fts.sql`.
- Any other docs or source files.
- CI workflow files — the e2e step in CI runs `npm run e2e` separately from `npm run check`; that is correct and does not change.

## Git workflow

- Branch: `advisor/014-docs-truth-fixes`
- One commit per step, or a single commit covering all five doc edits — either is acceptable for a docs-only change. Conventional commit style observed in this repo: `fix: …` / `docs: …` / `chore: …`. Example from `git log`: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`.
- Suggested message: `docs: correct stale statements in testing-strategy, architecture, version-matrix, README, and drop-fts migration comment`
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Confirm drift and source-of-truth assertions

Run the drift check command from the header:

```bash
git diff --stat d183bbf..HEAD -- docs/testing-strategy.md docs/architecture.md docs/version-matrix.md README.md src-tauri/migrations/20240612000010_drop_unused_fts.sql
```

Then confirm each assertion individually:

```bash
grep '"check"' package.json
grep 'keyring' src-tauri/Cargo.toml
grep -A2 'name = "keyring"' src-tauri/Cargo.lock
```

**Verify**: All three grep outputs match the excerpts in "Current state". The `check` script does not include bare `run e2e `. Keyring Cargo.toml says `version = "3"`. Cargo.lock says `version = "3.x.x"`. If any assertion fails, STOP.

### Step 2: Fix docs/testing-strategy.md — remove false claim that e2e is in the blocking gate

Locate lines 9–11. The sentence currently reads:

```
signal — the blocking gate (`npm run check`, CI) runs `test:run` (no coverage threshold) plus lint,
typecheck, the Playwright E2E smoke, clippy, and the privacy scan.
```

Replace "the Playwright E2E smoke, clippy, and the privacy scan" with an accurate list. The corrected sentence must convey that `npm run check` runs `test:run`, lint, typecheck, Playwright typecheck (`e2e:typecheck`), build, Rust checks (clippy + rustfmt + Rust tests), and the privacy scan — but does NOT run the Playwright browser tests (`npm run e2e`). The E2E smoke is a separate, non-blocking command.

Target text (exact wording may vary, but this substance is required):

```
signal — the blocking gate (`npm run check`, CI) runs `test:run` (no coverage threshold) plus lint,
typecheck, Playwright typecheck (`e2e:typecheck`), build, Rust checks (clippy, rustfmt, Rust tests),
and the privacy scan. The Playwright browser smoke (`npm run e2e`) is a separate non-blocking command
— run it manually or via the dedicated CI workflow; it is not part of `npm run check`.
```

**Verify**: `grep "Playwright E2E smoke, clippy" docs/testing-strategy.md` returns no output (old text is gone). `grep "non-blocking" docs/testing-strategy.md` returns a match.

### Step 3: Fix docs/architecture.md — describe FTS as tried-and-removed, not "planned"

Locate line 13:

```
| Local storage    | SQLite (WAL) for normalized finance data, settings, and sync state. Full-text search is planned.    |
```

Replace the cell content to accurately reflect the history: FTS5 tables were created (migration `20240608000015_fts5.sql`) but never populated; they were removed in migration `20240612000010_drop_unused_fts.sql` because the Lançamentos search is filtered client-side. FTS is not "planned" — it was tried and dropped.

Target text for that table cell (adjust whitespace to preserve column alignment if needed):

```
| Local storage    | SQLite (WAL) for normalized finance data, settings, and sync state. Full-text search was prototyped (migration 0015) and removed (migration 0010-drop) — tables were never populated; search is client-side. Re-add with triggers and rebuild when FTS is actually implemented. |
```

**Verify**: `grep "Full-text search is planned" docs/architecture.md` returns no output. `grep "removed" docs/architecture.md` returns a match on that line.

### Step 4: Fix docs/version-matrix.md — reconcile keyring to installed version

Locate line 57 in the "Planned Dependencies" table:

```
| OS keychain       | `keyring`                                     | `4.0.1`        | For OAuth refresh tokens/API keys if needed.                                                     |
```

This row is in the "Planned Dependencies" table but `keyring` is now installed. The version is also wrong (`4.0.1` vs installed `3.6.3`). Move this row to the "Installed In This Scaffold" section (or add a note under the "Planned Dependencies" table that it has since been installed), and update the version to `3.6.3` with a note explaining the constraint: `keyring 4.x` was not chosen because it dropped the `sync-secret-service` feature flag needed for Linux; `3.x` (currently `3.6.3` in `Cargo.lock`) is used instead.

After editing, the "Planned Dependencies" table must no longer list `keyring`. The "Installed In This Scaffold" section (or a "Runtime Rust Crates" subsection) must list it at `3.6.3`.

**Verify**: `grep "4.0.1" docs/version-matrix.md` returns no output. `grep "keyring" docs/version-matrix.md` returns exactly one match showing `3.6.3`.

### Step 5: Fix README.md — add patchelf to Linux/WSL2 prerequisites

Locate the `apt install` block at lines 64–68:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev pkg-config
```

Add `patchelf` to the package list. It must appear on one of the two continuation lines (either inline or as a third line). The addition is required by Tauri's AppImage bundler on Linux. Keep `libdbus-1-dev` and `pkg-config` on the second line; append `patchelf` at the end of that line or start a new continuation line.

Target (one acceptable form):

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev pkg-config \
  patchelf
```

**Verify**: `grep "patchelf" README.md` returns a match on the `apt install` block.

### Step 6: Fix src-tauri/migrations/20240612000010_drop_unused_fts.sql — remove stale pointer

Line 4 ends with `(ver docs/testing-strategy)`. That pointer is wrong: `docs/testing-strategy.md` has no FTS section. Remove or replace the pointer so it refers to no external doc (since no doc currently describes FTS rebuild), or reword the sentence to stand alone.

Current comment block (lines 1–4):

```sql
-- Remove a infraestrutura FTS5 nunca populada. As tabelas `transaction_fts`/`category_fts` foram
-- criadas (migration 0015) mas nenhum writer de produção as alimenta (sem triggers, sem rebuild) e
-- a busca de Lançamentos é filtrada no cliente. Manter tabelas mortas confunde o schema. Recriar
-- com triggers + rebuild quando a busca full-text for de fato implementada (ver docs/testing-strategy).
```

Replace the final comment line to remove the dead pointer. Acceptable replacement for line 4:

```sql
-- com triggers + rebuild quando a busca full-text for de fato implementada.
```

**Verify**: `grep "ver docs/testing-strategy" src-tauri/migrations/20240612000010_drop_unused_fts.sql` returns no output.

### Step 7: Run the full quality gate

```bash
npm run lint
npm run typecheck
npm run privacy:scan
npm run check
```

**Verify**: All four commands exit 0 with no errors or warnings.

### Step 8: Update plans/README.md

Mark this plan's status row as DONE in `plans/README.md`.

**Verify**: `grep "014" plans/README.md` shows `DONE`.

## Test plan

This plan modifies only documentation and one SQL migration comment. No runtime behavior changes. No new tests are required.

The verification gates in each step are the test plan: grep assertions confirm old text is gone and correct text is present. `npm run check` (step 7) confirms no accidental syntax or lint regression was introduced.

## Done criteria

ALL of the following must hold (machine-checkable):

- [ ] `grep "Playwright E2E smoke, clippy" docs/testing-strategy.md` → no output
- [ ] `grep "non-blocking" docs/testing-strategy.md` → at least one match (near the E2E section)
- [ ] `grep "Full-text search is planned" docs/architecture.md` → no output
- [ ] `grep "removed" docs/architecture.md` → match on the Local storage row
- [ ] `grep "4.0.1" docs/version-matrix.md` → no output
- [ ] `grep "keyring" docs/version-matrix.md` → exactly one match, showing `3.6.3`
- [ ] `grep "patchelf" README.md` → match inside the `apt install` block
- [ ] `grep "ver docs/testing-strategy" src-tauri/migrations/20240612000010_drop_unused_fts.sql` → no output
- [ ] `npm run lint` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run privacy:scan` exits 0
- [ ] `npm run check` exits 0
- [ ] `git diff --name-only HEAD` (after staging) shows only files from the in-scope list
- [ ] `plans/README.md` status row for plan 014 is `DONE`

## STOP conditions

Stop and report back (do not improvise) if:

- Any file at the cited line numbers does not match the excerpt in "Current state" (the codebase has drifted since this plan was written).
- `grep 'run e2e ' package.json` (space after `e2e`) finds a match inside the `check` script — that would mean e2e was added to the gate and finding 1 is no longer valid; do not make the testing-strategy.md change until clarified.
- `grep 'keyring' src-tauri/Cargo.toml` shows `version = "4"` — the installed version changed; update version-matrix.md to the actual installed version rather than `3.6.3`, and note the discrepancy.
- Any step's verification fails twice after a reasonable correction attempt.
- Making any fix appears to require modifying a file outside the in-scope list.
- `npm run check` fails in step 7 for a reason unrelated to this plan's changes (report the failure; do not try to fix unrelated issues).

## Maintenance notes

- If `npm run e2e` is later added to `npm run check` (or a new "full" gate is introduced), revisit `docs/testing-strategy.md` lines 9–11 again to re-describe the gate correctly.
- When FTS is actually re-implemented (with triggers and a rebuild step), update `docs/architecture.md:13`, restore a comment pointer in the migration, and add a section to `docs/testing-strategy.md` covering FTS rebuild behavior in tests.
- When `keyring` is upgraded from `3.x` to `4.x` (or any other version), update the row in `docs/version-matrix.md` and document why the upgrade was unblocked (the `sync-secret-service` Linux feature flag situation).
- If `patchelf` becomes a transitive install of another required package, the explicit mention in README.md is still harmless — but a comment noting why it was added (`# required by Tauri AppImage bundler`) would help future maintainers understand if it can be removed.
- A reviewer approving this PR should confirm only that each piece of new text accurately describes the current repo state — no logic review is required.
