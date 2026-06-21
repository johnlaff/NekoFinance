# Plan 048: F2 annotation fidelity: preserve note section headers in write-back round-trip + thermometer −R$500 boundary

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
>   src-tauri/src/google_sheets/import.rs \
>   src-tauri/src/google_sheets/write_back.rs \
>   src/lib/saldoHeatmap.ts \
>   src/lib/saldoHeatmap.test.ts \
>   src-tauri/migrations/
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
- **Planned at**: commit `26ea4c9`, 2026-06-21

## Why this matters

Two annotation-fidelity gaps cause the app to be unfaithful to the user's spreadsheet.
First, cell notes contain section headers (non-`R$` lines such as `"CONTAS:"`, `"CARTÕES:"`, `"Investimento:"`) that group itemized parts; these headers are correctly skipped during import but are silently dropped by `build_itemized_note` on write-back, so an import → edit → write-back round-trip produces a structurally different note than the user originally wrote.
Second, `saldoBand` classifies exactly −R$500,00 (−50 000 cents) as `"critical"` (strong red) using `<=`, but the spreadsheet's conditional-formatting rule uses strict `lessThan −500`, so −500,00 should be `"negative"` (light red) — the app shows one band too alarming at the exact boundary.
Fixing both makes the app a byte-faithful front-end of the spreadsheet.

## Current state

### File roles

- `src-tauri/src/google_sheets/import.rs` — sheet import engine; `parse_itemized_note` (line 899) extracts `R$` lines from a cell note into `NoteLineItem` items; header lines are already correctly skipped.
- `src-tauri/src/google_sheets/write_back.rs` — write-back engine; `build_itemized_note` (line 135) reconstructs the note from `TxnLineItem` items — currently flat, no section headers.
- `src/lib/saldoHeatmap.ts` — thermometer classifier; `saldoBand` (line 46) uses `<= t.critical` for the critical band.
- `src/lib/saldoHeatmap.test.ts` — thermometer unit tests; currently asserts `[-50_000, "critical"]` (line 26), which will become `[-50_000, "negative"]` after the fix.
- `src-tauri/migrations/` — SQLite migration files; need a new migration to add `section` column to `line_item`.

### `NoteLineItem` struct (import.rs lines 866–874)

```rust
/// Plan 035: uma parte itemizada extraída de uma linha da nota de célula.
#[derive(Debug, PartialEq)]
pub(crate) struct NoteLineItem {
    /// Magnitude em centavos (positiva). Mesma convenção de `transaction.amount`.
    pub amount_cents: i64,
    pub description: String,
    /// Posição 0-based na nota (ordem de aparição).
    pub position: usize,
}
```

The struct has no `section` field today. `section` must be added as `Option<String>` so existing callers that destructure or pattern-match the struct continue to compile (they use field access, not tuple destructuring — safe to add a field).

### `parse_itemized_note` core loop (import.rs lines 899–929)

```rust
pub(crate) fn parse_itemized_note(note: &str) -> Vec<NoteLineItem> {
    let mut items = Vec::new();
    for (pos, line) in note.lines().enumerate() {
        let trimmed = line.trim();
        // Só linhas que começam com `R$` (case-insensitive) são itens.
        let rest = if trimmed.len() >= 2 && trimmed[..2].eq_ignore_ascii_case("r$") {
            trimmed[2..].trim_start()
        } else {
            continue;   // ← header lines are already skipped here
        };
        // ...parse value_part, desc_part...
        items.push(NoteLineItem {
            amount_cents,
            description: desc_part.to_string(),
            position: pos,
        });
    }
    items
}
```

The `continue` on non-`R$` lines correctly skips headers now. After the fix, instead of `continue`, capture the most-recently-seen non-`R$` non-blank line as `current_section` and store it on each item.

### `TxnLineItem` struct (write_back.rs lines 36–41)

```rust
/// Uma parte itemizada de uma célula no caminho de escrita (plano 036). Magnitude positiva.
#[derive(Debug, Clone)]
pub struct TxnLineItem {
    pub amount_cents: i64,
    pub description: String,
}
```

`section: Option<String>` must be added here too, so the write-back path can re-emit headers.

### `build_itemized_note` (write_back.rs lines 128–148)

```rust
/// Plano 036: monta a NOTA por-parte que acompanha a fórmula, no formato do dono — uma linha por
/// item `R$ <valor> - <descrição>` (vírgula decimal, 2 casas). É a INVERSA do parser de notas
/// itemizadas do plano 035 (`parse_itemized_note`): o que esta função escreve, aquele parser relê.
///
/// Descrição vazia vira `"<sem descrição>"` ...
pub fn build_itemized_note(items: &[TxnLineItem]) -> String {
    items
        .iter()
        .map(|it| {
            let desc = if it.description.trim().is_empty() {
                "<sem descrição>"
            } else {
                it.description.trim()
            };
            format!("R$ {} - {}", cents_to_ptbr(it.amount_cents), desc)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

This function must be rewritten to emit a section header line before the first item of each distinct section, separated from the previous block by a blank line (matching the original note grammar).

### `saldoBand` critical boundary (saldoHeatmap.ts lines 46–55)

```typescript
export function saldoBand(
  cents: number,
  t: SaldoBandThresholds = SALDO_BAND_THRESHOLDS_CENTS,
): SaldoBand {
  if (cents <= t.critical) return "critical"; // BUG: <= should be <
  if (cents < t.positive) return "negative";
  if (cents <= t.tight) return "tight";
  if (cents <= t.ok) return "ok";
  return "comfortable";
}
```

The fix is a one-character change: `<=` → `<` on the `critical` guard.

### Existing line_item DB schema (migration 20260620000001_line_item.sql)

```sql
CREATE TABLE IF NOT EXISTS line_item (
    id        TEXT    PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL
        REFERENCES "transaction"(id) ON DELETE CASCADE,
    amount_cents   INTEGER NOT NULL,
    description    TEXT    NOT NULL DEFAULT '',
    position       INTEGER NOT NULL DEFAULT 0
);
```

A new migration adds `section TEXT` (nullable, default NULL) to this table.

### Existing test that asserts the wrong boundary (saldoHeatmap.test.ts lines 25–29)

```typescript
    // abaixo de −R$ 500 → vermelho forte (crítico)
    [-50_000, "critical"],
```

This assertion is wrong per the spreadsheet's `lessThan -500` semantics and must be changed to `"negative"`. Add a new assertion for `[-50_001, "critical"]` to cover the strict-less-than boundary.

### Conventions to follow

- **Functional core / imperative shell**: `parse_itemized_note` and `build_itemized_note` are pure functions with no I/O — keep them that way.
- **Money = positive-magnitude integer cents**: `amount_cents` is always positive. Do not change this convention.
- **React Compiler ON**: no manual `memo` in the TS layer — not relevant here (no React changes in scope).
- **Migration naming**: `YYYYMMDD<6-digit-seq>_<slug>.sql` — use date `20260621` and next available sequence for 2026-06-21.
- **Test patterns**: Rust tests live in `#[cfg(test)] mod tests { ... }` at the bottom of the file. TypeScript tests use Vitest `describe`/`it`/`expect`. Match the style of adjacent tests.
- **Method-neutral language**: do not name the official app, course, or reverse-engineered artefacts in comments, tests, or migration files. Use generic examples such as `"CONTAS:"`, `"CARTÕES:"`, `"Investimento:"`.

## Commands you will need

| Purpose         | Command                                                                                | Expected on success |
| --------------- | -------------------------------------------------------------------------------------- | ------------------- |
| Rust typecheck  | `npm run rust:check`                                                                   | exit 0, no errors   |
| TS typecheck    | `npm run typecheck`                                                                    | exit 0, no errors   |
| Lint            | `npm run lint`                                                                         | exit 0              |
| Unit tests      | `npm run test:run`                                                                     | all pass            |
| Rust tests only | `cargo test -p neko-finance-lib --manifest-path src-tauri/Cargo.toml 2>&1 \| tail -30` | all pass            |
| TS tests only   | `npx vitest run src/lib/saldoHeatmap.test.ts`                                          | all pass            |
| Full gate       | `npm run check`                                                                        | exit 0              |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/migrations/20260621000003_line_item_section.sql` (create)
- `src-tauri/src/google_sheets/import.rs` — add `section` to `NoteLineItem`; update `parse_itemized_note` to capture headers
- `src-tauri/src/google_sheets/write_back.rs` — add `section` to `TxnLineItem`; rewrite `build_itemized_note` to re-emit headers
- `src/lib/saldoHeatmap.ts` — fix `<=` → `<` on the `critical` guard
- `src/lib/saldoHeatmap.test.ts` — update boundary assertion; add strict-boundary test

**Out of scope** (do NOT touch, even though they look related):

- Any change to `transaction.amount_cents` or Saldo math — this plan is annotation-only.
- `src-tauri/src/google_sheets/write_back.rs` write-back dispatch / `execute_write_back` function — the 028 approval gate is unchanged.
- Any React component — no UI changes required.
- `parse_note_markers` (import.rs ~line 966) — independent parser, not affected.
- Other migration files — do not alter existing migrations.

## Git workflow

- Branch: `advisor/048-annotation-fidelity-headers-thermometer`
- Commit per logical unit; message style follows repo convention: `fix: <description> (plano 048)` (see `git log --oneline` — recent messages use conventional commits with a `(plano NNN)` suffix).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Add the `section` column migration

Create `src-tauri/migrations/20260621000003_line_item_section.sql`:

```sql
-- Plan 048: preserve note section headers in the write-back round-trip.
-- `section` stores the header line (e.g. "CONTAS:", "CARTÕES:") that appeared
-- immediately before this item in the original cell note. NULL = no header.
ALTER TABLE line_item ADD COLUMN section TEXT;
```

Use `ALTER TABLE … ADD COLUMN` (SQLite supports this for nullable columns without a default constraint issue).

**Verify**: `ls src-tauri/migrations/ | grep 048` prints `20260621000003_line_item_section.sql`.

### Step 2: Add `section` to `NoteLineItem` in import.rs

In `src-tauri/src/google_sheets/import.rs`, update the `NoteLineItem` struct (lines 866–874):

```rust
#[derive(Debug, PartialEq)]
pub(crate) struct NoteLineItem {
    pub amount_cents: i64,
    pub description: String,
    /// Posição 0-based na nota (ordem de aparição).
    pub position: usize,
    /// Cabeçalho de seção imediatamente anterior a este item na nota original
    /// (ex.: "CONTAS:", "CARTÕES:"). `None` quando o item não está sob um cabeçalho.
    pub section: Option<String>,
}
```

**Verify**: `npm run rust:check` exits 0 (other callers use field access — adding a field is backward compatible only if every construction site is updated; check next step).

### Step 3: Update `parse_itemized_note` to capture section headers

Replace the body of `parse_itemized_note` (import.rs lines 899–929). The logic:

1. Track a `current_section: Option<String>` variable, initialized to `None`.
2. Before the `R$` check: if the trimmed line is non-empty and does NOT start with `R$` (case-insensitive), update `current_section = Some(trimmed.to_string())` and then `continue`.
3. Blank lines (trimmed is empty): leave `current_section` unchanged (a blank line between a header and its items does not clear the section) and `continue`.
4. For `R$` lines: push `NoteLineItem { amount_cents, description, position: pos, section: current_section.clone() }`.

The resulting implementation:

```rust
pub(crate) fn parse_itemized_note(note: &str) -> Vec<NoteLineItem> {
    let mut items = Vec::new();
    let mut current_section: Option<String> = None;
    for (pos, line) in note.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Blank line: preserve the current section context, skip.
            continue;
        }
        // Non-R$ line → treat as section header (or free prose); update context.
        if trimmed.len() < 2 || !trimmed[..2].eq_ignore_ascii_case("r$") {
            current_section = Some(trimmed.to_string());
            continue;
        }
        let rest = trimmed[2..].trim_start();
        let (value_part, desc_part) = if let Some(idx) = rest.find('-') {
            (rest[..idx].trim_end(), rest[idx + 1..].trim_start())
        } else {
            (rest, "")
        };
        let amount_cents = parse_number(value_part.trim());
        if amount_cents <= 0 {
            continue;
        }
        items.push(NoteLineItem {
            amount_cents,
            description: desc_part.to_string(),
            position: pos,
            section: current_section.clone(),
        });
    }
    items
}
```

**Verify**: `npm run rust:check` exits 0. Then run `cargo test -p neko-finance-lib --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "FAILED|error"` — must print nothing (no failures, no errors).

### Step 4: Update the `line_item` INSERT to persist `section`

In `import.rs` near line 580, find the `INSERT INTO line_item` statement and add `section` to both the column list and the bound values:

Current:

```rust
"INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited) \
 VALUES (?1, ?2, ?3, ?4, ?5, 0) \
 ON CONFLICT(id) DO UPDATE SET \
   amount_cents=excluded.amount_cents, \
   description=excluded.description, \
   position=excluded.position, \
   is_user_edited=0",
```

Replace with:

```rust
"INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6) \
 ON CONFLICT(id) DO UPDATE SET \
   amount_cents=excluded.amount_cents, \
   description=excluded.description, \
   position=excluded.position, \
   is_user_edited=0, \
   section=excluded.section",
```

And add `.bind(item.section.as_deref())` after `.bind(item.position as i64)` (it will bind `NULL` when `section` is `None`).

**Verify**: `npm run rust:check` exits 0.

### Step 5: Add `section` to `TxnLineItem` in write_back.rs

In `src-tauri/src/google_sheets/write_back.rs`, update `TxnLineItem` (lines 36–41):

```rust
#[derive(Debug, Clone)]
pub struct TxnLineItem {
    pub amount_cents: i64,
    pub description: String,
    /// Cabeçalho de seção original, se existir (ver `NoteLineItem::section`).
    pub section: Option<String>,
}
```

Also update the test helper `ti` (line 836) to set `section: None` so existing tests compile:

```rust
fn ti(amount_cents: i64, description: &str) -> TxnLineItem {
    TxnLineItem {
        amount_cents,
        description: description.into(),
        section: None,
    }
}
```

**Verify**: `npm run rust:check` exits 0.

### Step 6: Rewrite `build_itemized_note` to re-emit section headers

Replace `build_itemized_note` (write_back.rs lines 135–148) with a version that tracks the last-emitted section and inserts a header line (plus a blank-line separator from the previous block) before the first item of each new section:

```rust
pub fn build_itemized_note(items: &[TxnLineItem]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut last_section: Option<&str> = None;

    for it in items {
        let this_section = it.section.as_deref();

        // Emit a section header when it changes (None → Some, or Some("A") → Some("B")).
        // A blank line separates blocks when transitioning from a previous section.
        if this_section != last_section {
            if let Some(header) = this_section {
                if !lines.is_empty() {
                    lines.push(String::new()); // blank separator between sections
                }
                lines.push(header.to_string());
            }
            last_section = this_section;
        }

        let desc = if it.description.trim().is_empty() {
            "<sem descrição>"
        } else {
            it.description.trim()
        };
        lines.push(format!("R$ {} - {}", cents_to_ptbr(it.amount_cents), desc));
    }

    lines.join("\n")
}
```

Note: when `section` is `None` for all items the output is identical to the current flat format — no regression for existing callers passing `ti(…)` (which set `section: None`).

**Verify**: `npm run rust:check` exits 0.

### Step 7: Add Rust unit tests for the section round-trip

Add the following tests inside the existing `#[cfg(test)] mod tests { ... }` block in `import.rs` (after the existing `itemized_*` tests near line 3210):

```rust
// Plan 048: parse_itemized_note captures section headers from non-R$ lines.
#[test]
fn itemized_captures_section_header() {
    let note = "CONTAS:\nR$ 100,00 - Item A\nR$ 50,00 - Item B";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
    assert_eq!(items[1].section.as_deref(), Some("CONTAS:"));
}

#[test]
fn itemized_two_sections_assign_correct_header() {
    let note = "CONTAS:\nR$ 100,00 - Item A\n\nCARTÕES:\nR$ 200,00 - Item B";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].section.as_deref(), Some("CONTAS:"));
    assert_eq!(items[1].section.as_deref(), Some("CARTÕES:"));
}

#[test]
fn itemized_no_header_yields_none_section() {
    let note = "R$ 150,00 - Item sem cabeçalho";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 1);
    assert!(items[0].section.is_none());
}
```

Add the following tests inside `write_back.rs` tests block (after the existing `build_itemized_note_round_trips_to_parse` test near line 885):

```rust
// Plan 048: build_itemized_note re-emits section headers on round-trip.
fn ti_sec(amount_cents: i64, description: &str, section: Option<&str>) -> TxnLineItem {
    TxnLineItem {
        amount_cents,
        description: description.into(),
        section: section.map(|s| s.to_string()),
    }
}

#[test]
fn build_itemized_note_with_single_section() {
    let items = vec![
        ti_sec(10_000, "Item A", Some("CONTAS:")),
        ti_sec(5_000, "Item B", Some("CONTAS:")),
    ];
    assert_eq!(
        build_itemized_note(&items),
        "CONTAS:\nR$ 100,00 - Item A\nR$ 50,00 - Item B",
    );
}

#[test]
fn build_itemized_note_with_two_sections() {
    let items = vec![
        ti_sec(10_000, "Item A", Some("CONTAS:")),
        ti_sec(20_000, "Item B", Some("CARTÕES:")),
    ];
    assert_eq!(
        build_itemized_note(&items),
        "CONTAS:\nR$ 100,00 - Item A\n\nCARTÕES:\nR$ 200,00 - Item B",
    );
}

#[test]
fn build_itemized_note_no_section_stays_flat() {
    // Items with section=None produce the same output as before (no regression).
    let items = vec![ti(5000, "Conta A"), ti(7500, "Conta B")];
    assert_eq!(
        build_itemized_note(&items),
        "R$ 50,00 - Conta A\nR$ 75,00 - Conta B",
    );
}

#[test]
fn build_itemized_note_section_round_trips_through_parse() {
    use super::super::import::parse_itemized_note;
    let items = vec![
        ti_sec(10_000, "Item A", Some("CONTAS:")),
        ti_sec(20_000, "Item B", Some("CARTÕES:")),
    ];
    let note = build_itemized_note(&items);
    let reparsed = parse_itemized_note(&note);
    assert_eq!(reparsed.len(), 2);
    assert_eq!(reparsed[0].amount_cents, 10_000);
    assert_eq!(reparsed[0].section.as_deref(), Some("CONTAS:"));
    assert_eq!(reparsed[1].amount_cents, 20_000);
    assert_eq!(reparsed[1].section.as_deref(), Some("CARTÕES:"));
}
```

**Verify**: `cargo test -p neko-finance-lib --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20` — all tests pass, including the 7 new ones.

### Step 8: Fix the thermometer boundary in saldoHeatmap.ts

In `src/lib/saldoHeatmap.ts` line 50, change:

```typescript
if (cents <= t.critical) return "critical";
```

to:

```typescript
if (cents < t.critical) return "critical";
```

That is the entire source change for Finding 2.

**Verify**: `npm run typecheck` exits 0.

### Step 9: Update the thermometer test in saldoHeatmap.test.ts

In `src/lib/saldoHeatmap.test.ts`, the `it.each` table (lines 10–30) contains:

```typescript
    // abaixo de −R$ 500 → vermelho forte (crítico)
    [-50_000, "critical"],
    [-60_000, "critical"],
```

Change to:

```typescript
    // R$ 0 a −R$ 500,00 → vermelho claro (negativo). −R$500,00 EXATO = negativo (planilha: lessThan, não <=).
    [-1, "negative"],
    [-49_999, "negative"],
    [-50_000, "negative"],   // boundary: −500,00 exato → negativo (strict <)
    // abaixo de −R$ 500 (strict) → vermelho forte (crítico)
    [-50_001, "critical"],
    [-60_000, "critical"],
```

Note: the existing `[-1, "negative"]` and `[-49_999, "negative"]` lines stay. You are adding `[-50_000, "negative"]` and `[-50_001, "critical"]`, and removing `[-50_000, "critical"]`.

**Verify**: `npx vitest run src/lib/saldoHeatmap.test.ts` exits 0, all cases pass.

### Step 10: Run the full quality gate

```
npm run check
```

Expected: exit 0. If `npm run check` includes E2E, it also runs Playwright smoke — confirm screenshots look correct (no thermometer-related visual regressions expected since the color band change only affects the exact −R$500,00 boundary).

**Verify**: `npm run check` exits 0.

## Test plan

### New Rust tests (import.rs)

Location: `#[cfg(test)] mod tests` in `src-tauri/src/google_sheets/import.rs`, after line 3210.

| Test name                                     | What it covers                                                  |
| --------------------------------------------- | --------------------------------------------------------------- |
| `itemized_captures_section_header`            | Single section header assigned to all items under it            |
| `itemized_two_sections_assign_correct_header` | Two sections, blank line between, each item gets correct header |
| `itemized_no_header_yields_none_section`      | Items with no preceding header get `section = None`             |

### New Rust tests (write_back.rs)

Location: `#[cfg(test)] mod tests` in `src-tauri/src/google_sheets/write_back.rs`, after line 898.

| Test name                                               | What it covers                                                    |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| `build_itemized_note_with_single_section`               | Header emitted before items in a single-section note              |
| `build_itemized_note_with_two_sections`                 | Two sections with blank-line separator between them               |
| `build_itemized_note_no_section_stays_flat`             | `section=None` items → same flat output as before (no regression) |
| `build_itemized_note_section_round_trips_through_parse` | Full round-trip: `build` → `parse` → sections preserved           |

### Updated TypeScript tests (saldoHeatmap.test.ts)

Changes to the `it.each` table in `saldoHeatmap.test.ts`:

- Remove `[-50_000, "critical"]`
- Add `[-50_000, "negative"]` (the fixed boundary)
- Add `[-50_001, "critical"]` (strict-less-than confirmed)

Run with:

```
npx vitest run src/lib/saldoHeatmap.test.ts
```

Expected: all cases pass.

### Model test to follow for structure

- Rust import tests: follow `itemized_skips_header_lines` (import.rs line 3143).
- Rust write_back tests: follow `build_itemized_note_round_trips_to_parse` (write_back.rs line 885).
- TypeScript tests: follow the existing `it.each` table in `saldoHeatmap.test.ts` (line 10).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; 7 new Rust tests + 2 new TS boundary cases exist and pass
- [ ] `npm run check` exits 0
- [ ] `src/lib/saldoHeatmap.ts` line ~50 contains `cents < t.critical` (strict `<`, not `<=`)
- [ ] `saldoHeatmap.test.ts` does NOT contain `[-50_000, "critical"]` (the wrong assertion is gone)
- [ ] `NoteLineItem` in `import.rs` has a `section: Option<String>` field
- [ ] `TxnLineItem` in `write_back.rs` has a `section: Option<String>` field
- [ ] Migration `src-tauri/migrations/20260621000003_line_item_section.sql` exists
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at any "Current state" location does not match the excerpts in this plan (codebase drifted since `26ea4c9`).
- Adding `section` to `NoteLineItem` or `TxnLineItem` causes a compilation error at a call site outside the in-scope files (meaning another module constructs these structs directly — that file would need to be updated, which is outside scope and requires advisor review).
- The new migration filename conflicts with an existing file in `src-tauri/migrations/` (check with `ls src-tauri/migrations/ | grep 20260621000003`).
- The `build_itemized_note_section_round_trips_through_parse` test fails even after implementing both steps 3 and 6 — this would indicate a grammar mismatch between the parser and builder that requires advisor analysis.
- `npm run check` fails for a reason unrelated to this plan's changes (pre-existing failure — do not mask it).

## Maintenance notes

- The `section` field is forward-only: items imported before this migration will have `section = NULL`. The write-back for those items will emit a flat note (no headers), which is correct — there is no original header to restore. This is acceptable since write-back requires human approval per plan 028 and the user can inspect the diff.
- If the user's note grammar evolves (e.g., nested sections, or section headers that start with `R$`), the header-detection heuristic in `parse_itemized_note` (any non-`R$`, non-blank line) will need to be refined. File an issue at that time rather than pre-optimizing now.
- The thermometer fix changes only the `saldoBand` classifier. Any component that renders the band color (e.g., `SALDO_BAND_FILL`) automatically picks up the corrected classification — no component changes needed.
- A future "user-configurable thresholds" feature (hinted at in `SaldoBandThresholds`) must use strict `<` for the critical guard to stay byte-faithful to the spreadsheet's `lessThan` operator semantics.
