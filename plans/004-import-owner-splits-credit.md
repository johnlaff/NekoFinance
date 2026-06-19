# Plan 004: Import owner splits and credit payment method

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/google_sheets/import.rs src-tauri/src/splits.rs`
> If either file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/002 (atomic import single SQLite transaction — these per-row
  split/payment_method writes must land inside an existing `tx`, not before it)
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Every imported transaction currently arrives with `owner_person_id` unset on its
splits and `payment_method` as NULL. This means `owner_totals_for_month` always
returns an empty list (no splits → no attributions), and credit-card expenses
(faturas) are indistinguishable from fixed debit Saídas in the engine — which
routes them correctly only by accident. The cell-note column is the natural carrier
for both markers (the method already uses it to log who spent what and on which
payment method), so parsing it at import time is the minimal intervention that
connects the data the sheet already holds to the schema fields SQLite already has.
When this lands, per-person attribution and the dual-tracking distinction between
debit and credit expenses will be populated from day one of import instead of
requiring post-import manual enrichment.

## Current state

### Files in scope

- `src-tauri/src/google_sheets/import.rs` — sheet parser + row importer; all
  changes in this plan go here.
- `src-tauri/src/splits.rs` — read-only reference for the `split` INSERT pattern
  and `owner_totals_for_month`; **not modified** in this plan.

### Relevant schema

**`transaction` table** (`src-tauri/migrations/20240608000006_transaction.sql`):
```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,
    description TEXT,
    date TEXT NOT NULL,
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    ...
);
```
The `payment_method` column accepts `'credit'` (among others) or NULL.

**`split` table** (`src-tauri/migrations/20240608000007_split.sql`):
```sql
CREATE TABLE IF NOT EXISTS split (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES "transaction"(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    category_id TEXT REFERENCES category(id),
    owner_person_id TEXT NOT NULL REFERENCES person(id),
    note TEXT
);
```
`owner_person_id` is a NOT NULL FK to `person(id)`. The importer must ensure the
`person` row exists before inserting a split.

### `ImportedRow` struct (import.rs:82–88)

```rust
pub struct ImportedRow {
    pub date: String,
    pub amount: i64,
    pub description: String,
    pub is_projection: bool,
    pub kind: RowKind,
}
```
No owner or payment_method field today.

### `cell_description` — how notes reach `ImportedRow` (import.rs:521–542)

```rust
fn cell_description(
    notes: &[Vec<String>],
    row: usize,
    col: usize,
    date: &str,
    kind: &str,
) -> String {
    let note = notes
        .get(row)
        .and_then(|nr| nr.get(col))
        .map(|s| s.trim())
        .unwrap_or("");
    if note.is_empty() {
        format!("{kind} {date}")
    } else {
        note.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
```
The raw note text (multi-line) is available here. Today it is only used to build
`description`; this plan adds a second pass to parse owner/credit markers from it.

### Where `ImportedRow` is built in `parse_rows_with_layout` (import.rs:601–645)

The three branches (Entrada / Saída / Diário) each call `cell_description` and
push an `ImportedRow`. Example for Saída (import.rs:616–629):

```rust
if let Some(out_off) = amount_out_offset
    && offset + out_off < row.len()
{
    let amount_out = parse_number(&row[offset + out_off]);
    if amount_out > 0 {
        imported.push(ImportedRow {
            date: date.clone(),
            amount: -amount_out,
            description: cell_description(notes, r, offset + out_off, &date, "Saída"),
            is_projection,
            kind: RowKind::Saida,
        });
    }
}
```

### Where rows are written to the DB in `import_rows_with_options` (import.rs:199–396)

The INSERT for a new row is at import.rs:266–280:

```rust
sqlx::query(
    "INSERT INTO \"transaction\" (id, type, amount, description, date, is_fixed, \
     is_projection, source_amount, source_description, created_at, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?3, ?4, ?8, ?8)",
)
.bind(&txn_id)
.bind(row.kind.txn_type())
.bind(sheet_amount)
.bind(trusted_desc)
.bind(&row.date)
.bind(row.kind.is_fixed() as i64)
.bind(row.is_projection as i64)
.bind(&now)
.execute(&mut *tx)
```
`payment_method` is not bound — it stays NULL.

The UPDATE for an existing row is at import.rs:297–314:
```rust
sqlx::query(
    "UPDATE \"transaction\" SET type=?2, amount=?3, description=?4, date=?5, \
       is_fixed=?6, is_projection=?7, source_amount=?8, source_description=?9, updated_at=?10 \
     WHERE id=?1",
)
```
`payment_method` is also missing here.

No `split` INSERT exists anywhere in `import.rs` today.

### Split INSERT pattern (splits.rs:121–130)

The existing exemplar for split inserts (used in tests):
```rust
sqlx::query(
    "INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1,?2,?3,?4)",
)
.bind(id)
.bind(txn_id)
.bind(amount)
.bind(owner)
```
Use this shape; include the ON DELETE CASCADE (already in the schema DDL) so
diff-deletes in the importer cascade to split rows automatically.

### Existing person upsert pattern (import.rs:425–431, `resolve_profile_id`)

Person bootstrap precedent: the import already creates a `person` row on demand
inside the same transaction (`&mut tx`). The same pattern must be followed for
named owners — look up `person.name` case-insensitively, create the row if absent.
Use `uuid::Uuid::new_v4().to_string()` for the generated `id`, matching the rest
of the codebase.

### Relevant CONTEXT.md vocabulary (mandatory for naming)

- **Payment Method**: `debit`, `credit`, `pix`, `cash` (enum on Transaction).
  `credit` = delayed; feeds the fatura lump at due date; distinct from debit/PIX/cash.
- **Split**: allocation of one transaction across multiple `owner_person_id` rows.
- **Person**: the human whose finances are tracked (`person.name`, `person.id`).
- **FixedOut**: engine bucket for `is_fixed=1` **or** `payment_method='credit'`
  expenses — both collapse into the fatura lump. Setting `payment_method='credit'`
  is what routes an imported credit expense into the correct engine bucket.

### ADR-0001 constraint

`payment_method='credit'` expenses must NOT be double-counted in both Régua 1
(daily_spend) and Régua 2 (credit_spend). Setting the field at import time is
correct; the engine already handles the routing. No change to engine logic is
needed here.

### Repo conventions

- Money is **integer cents**; amounts on Transaction are absolute positive magnitude
  (`amount = row.amount.abs()` at import.rs:246).
- All finance-core functions must be pure/deterministic and unit-testable without a
  DB pool (see `parse_rows_with_layout` and `parse_number` — pure fns with tests
  in the same file).
- Functional-core / imperative-shell: add a new pure parsing function for note
  grammar; call it from the imperative import loop.
- **React Compiler is enabled** — irrelevant here (Rust-only plan).
- Commit message style: `fix: <short description>` (conventional commits, lower-case,
  Portuguese or English body).
- Branch: `advisor/004-import-owner-splits-credit`.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Rust typecheck + clippy + fmt | `npm run rust:check` | exit 0, no warnings |
| Rust unit tests only | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass |
| Full gate | `npm run check` | exit 0 |
| Run a single test | `cargo test --manifest-path src-tauri/Cargo.toml --locked <test_name>` | 1 passed |

> `npm run rust:check` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and
> `cargo test`. Run it between steps. If `cargo fmt --check` fails, run
> `cargo fmt --manifest-path src-tauri/Cargo.toml` first.

## Scope

**In scope** (the only file to modify):

- `src-tauri/src/google_sheets/import.rs`

**Out of scope** (do NOT touch):

- `src-tauri/src/splits.rs` — read-only reference; no changes needed.
- Any UI component — the UI to assign owners after import is plan 015.
- The first-class invoice entity — that is spike 019, which depends on this plan.
- Database migrations — no schema change is needed; `payment_method` and `split`
  exist already.
- `src-tauri/src/lib.rs` or `commands.rs` — no new Tauri commands in this plan.
- Any frontend (`.ts`, `.tsx`, `.css`) file.

## Git workflow

- Branch: `advisor/004-import-owner-splits-credit`
- One commit per step, or group steps 1–2 (pure parsing) as one commit and
  steps 3–4 (DB writes) as a second commit.
- Commit message style: `fix: import owner splits and credit payment_method from note grammar`
- Do NOT push or open a PR unless the operator instructs it.

## STOP condition: note grammar ambiguity (read before starting)

This plan **requires a defined note grammar**. Step 1 defines it. Before writing
any code, inspect the cell notes in the reference spreadsheet
(`docs/example/Finanças.xlsx`):

1. Open or extract the file: `python3 -c "import openpyxl; wb = openpyxl.load_workbook('docs/example/Finanças.xlsx'); ..."`
   or any xlsx reader available. Look at the notes (comments) on cells in the
   Saída and Diário columns.
2. If real notes exist and follow a **consistent, machine-readable pattern** for
   owner name (e.g., a prefix like `@Ana:` or a first-line marker), adopt it.
3. If real notes exist but follow **no consistent convention** — or use free-form
   prose that can't be parsed without guessing — **STOP and report**. Propose a
   minimal explicit convention (see Step 1 for the recommended fallback) and wait
   for human approval before implementing.
4. If no notes exist in the example file (notes are private data), proceed with
   the convention defined in Step 1 as the canonical contract.

This is the single highest-risk STOP in this plan. Do not bypass it.

## Steps

### Step 1: Define and document the note grammar contract

**What to do**: Add a `doc` comment block at the top of `import.rs` (or just
above the new parsing function added in Step 2) that formally defines the
convention Neko will parse. After inspecting the spreadsheet notes (see STOP
condition above), adopt one of:

**Recommended grammar** (use this if no conflicting convention found in the sheet):

```
Line syntax (each line of the cell note is parsed independently):
  @<name>: <amount>   — owner marker; <name> is matched case-insensitively to
                        person.name; <amount> is ignored at import time (the
                        transaction amount is canonical). Creates a split row
                        assigning owner_person_id to <name>'s person.id.
  #credit             — payment method marker; sets payment_method = 'credit'.
                        Exactly the token "#credit" (case-insensitive), alone on
                        a line or at the start of a line.

Examples:
  "@Ana: 150,00"     → split with owner=Ana
  "@Bruno: 50,00"    → split with owner=Bruno (same transaction → two splits)
  "#credit"          → payment_method = 'credit'
  "@Ana: 200\n#credit" → both: split for Ana + credit method
```

Write this grammar as a Rust `/// NOTE GRAMMAR` doc comment above the new
`parse_note_markers` function. This is the contract that spec 019 and any future
note-editing UI must honor.

**Verify**: `npm run rust:check` → exit 0 (comment-only change; should trivially pass).

---

### Step 2: Add pure function `parse_note_markers`

**What to do**: Add a new pure function in `import.rs` (after `cell_description`,
before `parse_rows_with_layout`):

```rust
/// Parses owner and payment-method markers from a raw cell note.
/// See the NOTE GRAMMAR contract above this function for the syntax.
///
/// Returns:
///   - `owners`: display names of persons found via `@<name>:` lines.
///   - `is_credit`: true if any line matches `#credit` (case-insensitive).
///
/// Pure — no I/O, no DB, no panics. Testable without a pool.
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut owners: Vec<String> = Vec::new();
    let mut is_credit = false;

    for line in note.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("#credit") || trimmed.to_ascii_lowercase().starts_with("#credit") {
            is_credit = true;
        }
        if let Some(rest) = trimmed.strip_prefix('@') {
            if let Some(colon_pos) = rest.find(':') {
                let name = rest[..colon_pos].trim().to_string();
                if !name.is_empty() {
                    owners.push(name);
                }
            }
        }
    }

    NoteMarkers { owners, is_credit }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct NoteMarkers {
    /// Display names of owners, in the order they appear in the note.
    pub owners: Vec<String>,
    /// True if `#credit` appeared anywhere in the note.
    pub is_credit: bool,
}
```

Place `NoteMarkers` struct definition just above `parse_note_markers`.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked parse_note_markers` →
should fail (tests don't exist yet — this confirms the test filter is wired). Then:
`npm run rust:check` → exit 0 (the fn compiles; no tests yet).

---

### Step 3: Add unit tests for `parse_note_markers`

**What to do**: In the `#[cfg(test)] mod tests` block at the bottom of `import.rs`
(after the last existing test, currently `replace_is_scoped_to_its_own_sheet` ending
around line 1800), add these tests:

```rust
// --- Plan 004: note grammar (pure, no DB) ---

#[test]
fn parse_note_markers_empty_note() {
    let m = parse_note_markers("");
    assert!(m.owners.is_empty());
    assert!(!m.is_credit);
}

#[test]
fn parse_note_markers_owner_only() {
    let m = parse_note_markers("@Ana: 150,00");
    assert_eq!(m.owners, vec!["Ana"]);
    assert!(!m.is_credit);
}

#[test]
fn parse_note_markers_credit_only() {
    let m = parse_note_markers("#credit");
    assert!(m.owners.is_empty());
    assert!(m.is_credit);
}

#[test]
fn parse_note_markers_credit_case_insensitive() {
    assert!(parse_note_markers("#Credit").is_credit);
    assert!(parse_note_markers("#CREDIT").is_credit);
}

#[test]
fn parse_note_markers_owner_and_credit() {
    let note = "@Ana: 200,00\n#credit";
    let m = parse_note_markers(note);
    assert_eq!(m.owners, vec!["Ana"]);
    assert!(m.is_credit);
}

#[test]
fn parse_note_markers_multiple_owners() {
    let note = "@Ana: 150,00\n@Bruno: 50,00";
    let m = parse_note_markers(note);
    assert_eq!(m.owners, vec!["Ana", "Bruno"]);
    assert!(!m.is_credit);
}

#[test]
fn parse_note_markers_free_prose_ignored() {
    // Existing free-form notes must not accidentally trigger markers.
    let note = "Mercado da semana · faltou só o frango";
    let m = parse_note_markers(note);
    assert!(m.owners.is_empty());
    assert!(!m.is_credit);
}

#[test]
fn parse_note_markers_owner_name_trimmed() {
    let m = parse_note_markers("@ Ana :  valor");
    // Leading/trailing whitespace in name trimmed; colon after space still parses.
    assert_eq!(m.owners, vec!["Ana"]);
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked parse_note_markers` →
8 tests pass. Then `npm run rust:check` → exit 0.

---

### Step 4: Extend `ImportedRow` with `raw_note` field

**What to do**: Add a `raw_note` field to `ImportedRow` (import.rs:82–88) to carry
the unparsed note text through to the DB-write phase:

```rust
pub struct ImportedRow {
    pub date: String,
    pub amount: i64,
    pub description: String,
    pub is_projection: bool,
    pub kind: RowKind,
    /// Raw cell note (multi-line). Used in `import_rows_with_options` to parse
    /// owner splits and payment_method. Empty string when no note exists.
    pub raw_note: String,
}
```

Then update all three `ImportedRow { ... }` construction sites inside
`parse_rows_with_layout` (import.rs:606–610, 621–626, 635–640) to add the field.
For each, capture the raw note before the `cell_description` call:

**Entrada branch** (around import.rs:601–614):
```rust
if let Some(in_off) = amount_in_offset
    && offset + in_off < row.len()
{
    let amount_in = parse_number(&row[offset + in_off]);
    if amount_in > 0 {
        let raw_note = notes
            .get(r)
            .and_then(|nr| nr.get(offset + in_off))
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        imported.push(ImportedRow {
            date: date.clone(),
            amount: amount_in,
            description: cell_description(notes, r, offset + in_off, &date, "Entrada"),
            is_projection,
            kind: RowKind::Entrada,
            raw_note,
        });
    }
}
```

Apply the same pattern for the **Saída** branch (offset `out_off`) and the **Diário**
branch (offset `d_off`).

Also update the test helper `imported_desc` (import.rs:1317–1328) to supply an empty
`raw_note`:
```rust
fn imported_desc(date: &str, amount: i64, description: &str) -> ImportedRow {
    ImportedRow {
        date: date.into(),
        amount,
        description: description.into(),
        is_projection: false,
        kind: if amount >= 0 { RowKind::Entrada } else { RowKind::Saida },
        raw_note: String::new(),   // ← add
    }
}
```

The `imported` helper calls `imported_desc`, so it picks up the change automatically.

**Verify**: `npm run rust:check` → exit 0 (all existing tests still pass; struct is
complete everywhere).

---

### Step 5: Write split rows and set `payment_method` in `import_rows_with_options`

**What to do**: Inside the `import_rows_with_options` loop (import.rs:234–360), after
the INSERT or UPDATE of the `transaction` row succeeds, add the owner-split and
payment_method logic. The insertion point is **after** the `match existing { ... }`
block (after import.rs:341) and **before** the `sync_log` UPSERT (import.rs:344).

The new block should:

1. Call `parse_note_markers(&row.raw_note)` to get `NoteMarkers { owners, is_credit }`.
2. If `is_credit` is true, `UPDATE "transaction" SET payment_method = 'credit'`.
3. If `owners` is non-empty:
   a. Delete existing import-managed splits for this `txn_id` (idempotent on re-import).
   b. For each owner name:
      - Look up `person.id` by `LOWER(name) = LOWER(?1)`.
      - If not found, INSERT a new `person` row (same bootstrap pattern as
        `resolve_profile_id`: `uuid::Uuid::new_v4().to_string()` for the id).
      - INSERT a `split` row: `id = uuid::Uuid::new_v4().to_string()`,
        `transaction_id = txn_id`, `amount = sheet_amount` (use `row.amount.abs()`
        — the same positive-magnitude convention as the transaction amount),
        `owner_person_id = person_id`.

**Shape of the new block** (place after `match existing { ... }` closes at line ~341):

```rust
// --- Plan 004: owner splits + credit payment_method from note grammar ---
let markers = parse_note_markers(&row.raw_note);

if markers.is_credit {
    sqlx::query(
        "UPDATE \"transaction\" SET payment_method = 'credit' WHERE id = ?1",
    )
    .bind(&txn_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("set payment_method credit: {e}"))?;
}

if !markers.owners.is_empty() {
    // Delete previously imported splits so re-import is idempotent.
    // ON DELETE CASCADE handles orphan split rows when the txn is diff-deleted.
    // We use a dedicated note column marker so we can distinguish import-managed
    // splits from manually-added splits; for now we replace all splits for this txn.
    sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
        .bind(&txn_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("clear splits for txn: {e}"))?;

    let split_amount = row.amount.abs();
    for owner_name in &markers.owners {
        // Resolve person by name (case-insensitive).
        let person_id: String = {
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
            )
            .bind(owner_name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("lookup person '{owner_name}': {e}"))?;

            match existing {
                Some((id,)) => id,
                None => {
                    // Bootstrap person on first mention.
                    let new_id = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                        .bind(&new_id)
                        .bind(owner_name)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("create person '{owner_name}': {e}"))?;
                    new_id
                }
            }
        };

        let split_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&split_id)
        .bind(&txn_id)
        .bind(split_amount)
        .bind(&person_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("insert split for '{owner_name}': {e}"))?;
    }
}
// --- end Plan 004 ---
```

**Verify**: `npm run rust:check` → exit 0.

---

### Step 6: Add integration tests for the DB-write path

**What to do**: In the `#[cfg(test)] mod tests` block, after the Plan 004 unit tests
added in Step 3, add async integration tests. These use `test_pool()` (already
defined at import.rs:1302–1311). Pattern: model after `reimport_preserves_transaction_identity_and_enrichment` (import.rs:1587–1626).

```rust
// --- Plan 004: integration tests (DB) ---

#[tokio::test]
async fn import_sets_credit_payment_method_from_note() {
    let pool = test_pool().await;
    let rows = vec![ImportedRow {
        date: "2026-01-10".into(),
        amount: -30000, // R$300 Saída
        description: "Fatura cartão · #credit".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "#credit".into(),
    }];

    import_rows(&pool, "2026", &rows, "p1").await.unwrap();

    let (pm,): (Option<String>,) = sqlx::query_as(
        "SELECT payment_method FROM \"transaction\" WHERE date = '2026-01-10'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pm.as_deref(), Some("credit"));
}

#[tokio::test]
async fn import_creates_split_with_owner_from_note() {
    let pool = test_pool().await;
    let rows = vec![ImportedRow {
        date: "2026-01-15".into(),
        amount: -30000,
        description: "@Ana: 30000".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "@Ana: 30000".into(),
    }];

    import_rows(&pool, "2026", &rows, "p1").await.unwrap();

    // Split row must exist.
    let splits: Vec<(String, i64)> = sqlx::query_as(
        "SELECT p.name, s.amount FROM split s \
         JOIN person p ON p.id = s.owner_person_id \
         JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE t.date = '2026-01-15'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(splits.len(), 1);
    assert_eq!(splits[0].0, "Ana");
    assert_eq!(splits[0].1, 30000); // positive magnitude

    // Person row was bootstrapped.
    let (pcount,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name)='ana'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pcount, 1);
}

#[tokio::test]
async fn import_creates_multiple_splits_for_multiple_owners() {
    let pool = test_pool().await;
    let rows = vec![ImportedRow {
        date: "2026-02-01".into(),
        amount: -30000,
        description: "@Ana: 200 · @Bruno: 100".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "@Ana: 200,00\n@Bruno: 100,00".into(),
    }];

    import_rows(&pool, "2026", &rows, "p1").await.unwrap();

    let (scount,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM split s \
         JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE t.date = '2026-02-01'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(scount, 2);
}

#[tokio::test]
async fn reimport_replaces_splits_idempotently() {
    let pool = test_pool().await;

    // First import: one owner.
    let v1 = vec![ImportedRow {
        date: "2026-03-01".into(),
        amount: -30000,
        description: "@Ana: 30000".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "@Ana: 30000".into(),
    }];
    import_rows(&pool, "2026", &v1, "p1").await.unwrap();

    // Second import (sheet note changed to two owners).
    let v2 = vec![ImportedRow {
        date: "2026-03-01".into(),
        amount: -30000,
        description: "@Ana: 200 · @Bruno: 100".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "@Ana: 200,00\n@Bruno: 100,00".into(),
    }];
    import_rows(&pool, "2026", &v2, "p1").await.unwrap();

    let (scount,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM split s \
         JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE t.date = '2026-03-01'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(scount, 2, "re-import replaced the single split with two");
}

#[tokio::test]
async fn import_no_note_leaves_payment_method_null_and_no_splits() {
    let pool = test_pool().await;
    let rows = vec![imported("2026-04-01", -10000)];
    import_rows(&pool, "2026", &rows, "p1").await.unwrap();

    let (pm, splits): (Option<String>, i64) = sqlx::query_as(
        "SELECT t.payment_method, \
                (SELECT COUNT(*) FROM split WHERE transaction_id = t.id) \
         FROM \"transaction\" t WHERE t.date = '2026-04-01'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(pm.is_none(), "no note → payment_method stays NULL");
    assert_eq!(splits, 0, "no note → no splits created");
}

#[tokio::test]
async fn import_owner_lookup_is_case_insensitive() {
    let pool = test_pool().await;
    // Pre-seed person with mixed-case name.
    sqlx::query("INSERT INTO person (id, name) VALUES ('pid-ana', 'Ana')")
        .execute(&pool)
        .await
        .unwrap();

    let rows = vec![ImportedRow {
        date: "2026-05-01".into(),
        amount: -10000,
        description: "@ana: 100".into(),
        is_projection: false,
        kind: RowKind::Saida,
        raw_note: "@ana: 100".into(),
    }];
    import_rows(&pool, "2026", &rows, "p1").await.unwrap();

    // Must reuse existing person row (not create a duplicate).
    let (pcount,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM person WHERE LOWER(name) = 'ana'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pcount, 1, "no duplicate person created");
    let (owner,): (String,) = sqlx::query_as(
        "SELECT p.name FROM split s \
         JOIN person p ON p.id = s.owner_person_id \
         JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE t.date = '2026-05-01'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, "Ana", "split points to the pre-existing person");
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked` → all pass,
including the 6 new integration tests. Then `npm run rust:check` → exit 0.

---

### Step 7: Final gate

**What to do**: Run the full check and confirm nothing outside `import.rs` was
touched.

```
git diff --name-only HEAD
```
Expected: only `src-tauri/src/google_sheets/import.rs` is modified.

**Verify**: `npm run check` → exit 0 (all gates: fmt, clippy, tests, typecheck, lint,
privacy scan).

## Test plan

**New pure tests** (Step 3, 8 tests in `#[cfg(test)] mod tests`):
- `parse_note_markers_empty_note`
- `parse_note_markers_owner_only`
- `parse_note_markers_credit_only`
- `parse_note_markers_credit_case_insensitive`
- `parse_note_markers_owner_and_credit`
- `parse_note_markers_multiple_owners`
- `parse_note_markers_free_prose_ignored`
- `parse_note_markers_owner_name_trimmed`

**New integration tests** (Step 6, 6 tests):
- `import_sets_credit_payment_method_from_note`
- `import_creates_split_with_owner_from_note`
- `import_creates_multiple_splits_for_multiple_owners`
- `reimport_replaces_splits_idempotently`
- `import_no_note_leaves_payment_method_null_and_no_splits`
- `import_owner_lookup_is_case_insensitive`

**Structural pattern**: model all async tests after `reimport_preserves_transaction_identity_and_enrichment` (import.rs:1587); use `test_pool()` for in-memory SQLite with migrations applied.

**Regression guard**: all existing tests in `import.rs` and `splits.rs` must still
pass without modification (the `raw_note: String::new()` addition to `imported_desc`
is backward-compatible).

**Verification command**: `cargo test --manifest-path src-tauri/Cargo.toml --locked`
→ all tests pass (14 new + all pre-existing).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; exactly 14
  new tests exist (8 unit + 6 integration) and all pass
- [ ] `git diff --name-only HEAD` shows only `src-tauri/src/google_sheets/import.rs`
- [ ] `grep -n "parse_note_markers" src-tauri/src/google_sheets/import.rs` returns
  at least 3 matches (fn definition + call site in the import loop + tests)
- [ ] `grep -n "payment_method.*credit" src-tauri/src/google_sheets/import.rs` returns
  at least 1 match (the UPDATE statement in Step 5)
- [ ] `grep -n "INSERT INTO split" src-tauri/src/google_sheets/import.rs` returns
  at least 1 match (the split INSERT in Step 5)
- [ ] `npm run check` exits 0 (full gate including frontend typecheck, lint, privacy scan)
- [ ] `plans/README.md` status row for plan 004 updated to DONE

## STOP conditions

Stop and report (do not improvise) if:

1. **Drift**: the code at the locations in "Current state" does not match the excerpts
   (the file changed since commit `d183bbf`). Compare and report the delta.
2. **Note grammar ambiguity**: cell notes in `docs/example/Finanças.xlsx` follow a
   different, inconsistent, or unparseable convention (see the STOP condition block
   before Step 1). Propose an explicit convention and wait for human approval.
3. **`split` has additional NOT NULL columns** beyond what is in the schema excerpt
   above. Check with `PRAGMA table_info(split)` inside a test; if the schema has
   drifted, report and do not guess defaults.
4. **`person` FK constraint failure**: if inserting a split fails with an FK violation
   despite the person bootstrap in Step 5, stop — do not lower SQL PRAGMA or wrap in
   IGNORE. Diagnose and report.
5. **A step's verification fails twice** after a reasonable fix attempt. Do not proceed
   to the next step.
6. **The fix requires touching a file outside the in-scope list** (`import.rs` only).
   Report which file and why, and wait for approval.
7. **Clippy `-D warnings` triggers** on new code and the fix is non-obvious. Report
   the warning text; do not `#[allow(...)]` without understanding it.

## Maintenance notes

- **Re-import replaces all splits** for a transaction whenever its note changes. This
  is intentional: the sheet is the authoritative source for grammar markers. If a
  user manually edits splits in the UI (plan 015), those edits will be overwritten on
  the next import — this is a known limitation to be addressed by plan 015 (which
  should introduce a "manually locked" flag on split rows, similar to how
  `source_description` guards description edits).
- **Grammar is a public contract**: any future note-editing UI (plan 015) and the
  first-class invoice spike (plan 019) must document and honor the `@name: amount`
  and `#credit` syntax defined in Step 1. The doc comment in `import.rs` is the
  canonical reference.
- **Split amounts are the full transaction amount** at import time. Multi-person
  partial amounts (e.g., Ana pays 200, Bruno pays 100 on a R$300 transaction) are
  stored as the transaction total for now; plan 019 will introduce the amount-per-owner
  parsing when the invoice entity lands. Revisit this when plan 019 is executed.
- **Reviewer should check in the PR**: (a) that the `DELETE FROM split WHERE
  transaction_id = ?1` before re-inserting covers the idempotent-re-import case
  without accidentally deleting manually-created splits that predate this plan, and
  (b) that the person-bootstrap path doesn't create a `person` without a corresponding
  `profile` row — in this plan that is intentional (person-without-profile is valid for
  non-primary owners), but reviewers should verify it doesn't break any FK chain
  elsewhere in the schema.
- **Follow-up deferred**: amount-per-owner parsing from the note (`@Ana: 150,00`) is
  captured but not used (Step 5 stores `row.amount.abs()` for all splits). The grammar
  already allows it; spike 019 should revisit when the invoice entity creates a
  structured per-owner allocation.
