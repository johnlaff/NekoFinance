# Plan 023: Rewrite the note-marker convention to method-faithful #dividir:/#reembolso: with net-zero

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 51afe33..HEAD -- src-tauri/src/google_sheets/import.rs`
> If the file changed since this plan was written, compare the "Current state"
> excerpts against the live code before proceeding; on a mismatch, treat it as
> a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `51afe33`, 2026-06-20

## Why this matters

Plan 004 (DONE) introduced an opt-in note-marker parser that detects `@nome:`
as a split owner and `#credito` as payment method. Two issues have since been
identified:

1. `@nome:` does not match the user's actual note style (real notes are
   itemized free-text lines like `R$ 530 - Cartões Gio`, not `@Name:` prefixes)
   and collides with the method's vocabulary for third-party shares.
2. `#credito` is meaningless: credit is always a lump on the due date regardless;
   the spreadsheet has no per-transaction card column; setting `payment_method='credit'`
   from a note marker is an invention that neither the method nor the spreadsheet
   supports.

The method's actual practice for a third-party share is: record the full Saída,
then create a compensating Entrada (reimbursement) on the same or due date — net-zero
to cashflow. This plan replaces the two broken markers with method-faithful
`#dividir:<quem>` and `#reembolso:<quem>` tags (forward-only, opt-in), drops
`#credito` entirely, and adds the net-zero compensating Entrada that the method
prescribes. Untagged notes continue to behave exactly as today — byte-for-byte.

## Current state

### File in scope

- `src-tauri/src/google_sheets/import.rs` — sheet parser + row importer; all
  changes in this plan go here. (2 605 lines as of planned commit.)

### Current `NoteMarkers` struct (`import.rs:685–692`)

```rust
/// Marcadores OPT-IN extraídos de uma nota de célula (`parse_note_markers`).
///
/// SEGURO POR PADRÃO: uma nota sem marcador devolve `NoteMarkers::default()`
/// (sem owners, `is_credit=false`), de modo que o import se comporta byte-a-byte
/// como hoje.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct NoteMarkers {
    pub owners: Vec<String>,
    pub is_credit: bool,
}
```

### Current `parse_note_markers` function (`import.rs:694–753`)

The doc-comment block begins at line 694 with:

```
/// GRAMÁTICA DAS NOTAS (contrato público — opt-in, explícito, seguro por padrão).
///
/// Formas reconhecidas:
///   `@<nome>: <resto>`  — MARCADOR DE TITULAR. ...
///   `#credito`          — MARCADOR DE MÉTODO DE PAGAMENTO. ...
```

The function body at lines 725–753:

```rust
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut owners: Vec<String> = Vec::new();
    let mut is_credit = false;

    for line in note.lines() {
        let trimmed = line.trim();

        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("#credito")
            && rest.chars().next().is_none_or(char::is_whitespace)
        {
            is_credit = true;
        }

        if let Some(rest) = trimmed.strip_prefix('@')
            && let Some(colon_pos) = rest.find(':')
        {
            let name = rest[..colon_pos].trim().to_string();
            if !name.is_empty() {
                owners.push(name);
            }
        }
    }

    NoteMarkers { owners, is_credit }
}
```

### Current call site and writer block (`import.rs:396–466`)

```rust
        // --- Plan 004: splits de titular + payment_method='credit' via gramática da nota ---
        let markers = parse_note_markers(&row.raw_note);

        if markers.is_credit {
            sqlx::query("UPDATE \"transaction\" SET payment_method = 'credit' WHERE id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("set payment_method credit: {e}"))?;
        }

        if !markers.owners.is_empty() {
            sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear splits for txn: {e}"))?;

            let split_amount = row.amount.abs();
            for owner_name in &markers.owners {
                let person_id: String = {
                    let existing: Option<(String,)> = sqlx::query_as(
                        "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
                    )
                    .bind(owner_name)
                    .fetch_optional(&mut **tx)
                    ...

                let split_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
                     VALUES (?1, ?2, ?3, ?4)",
                )
                ...
            }
        }
        // --- fim Plan 004 ---
```

### Current unit tests (`import.rs:2315–2392`)

Tests are grouped under `// Plan 004: gramática das notas (parse puro, sem DB)` and
`// Plan 004: testes de integração (DB)`. The pure-parse tests include:

- `parse_note_markers_empty_note`
- `parse_note_markers_owner_only` — input `"@Pessoa A: 150,00"`
- `parse_note_markers_credit_only` — input `"#credito"`
- `parse_note_markers_credit_case_insensitive`
- `parse_note_markers_credit_substring_not_matched`
- `parse_note_markers_owner_and_credit`
- `parse_note_markers_multiple_owners`
- `parse_note_markers_free_prose_ignored` — already passes with line `"R$ 65,00 - Vivo · faltou só o frango"`
- `parse_note_markers_owner_name_trimmed`
- `parse_note_markers_at_without_colon_ignored`

The integration tests begin at line 2398 (`import_sets_credit_payment_method_from_note`,
`import_creates_split_with_owner_from_note`, etc.).

### Safety test already in place

`parse_note_markers_free_prose_ignored` (line 2371) proves that prose lines matching
the user's real note format (`"R$ 65,00 - Vivo · faltou só o frango"`) never trigger
a marker. The new grammar must keep this test passing without modification.
`import_unmarked_prose_note_leaves_payment_method_null_and_no_splits` (line 2539) does
the same at DB level.

### `ImportedRow` struct and `raw_note` field (`import.rs:82–93`)

```rust
pub struct ImportedRow {
    pub date: String,
    pub amount: i64,     // integer cents, positive magnitude for income, negative for expense
    pub description: String,
    pub is_projection: bool,
    pub kind: RowKind,
    /// Nota de célula CRUA (multi-linha, preservando `\n`). Usada por
    /// `import_rows_core` para extrair splits de titular e `payment_method` via
    /// `parse_note_markers`. String vazia quando não há nota.
    pub raw_note: String,
}
```

`raw_note` is already populated by `cell_raw_note()` (line 670) at all three
`ImportedRow` construction sites (lines 825–833, 841–848, ~860 for Diário).

### `row_id` identity scheme (`import.rs:149–160`)

```rust
pub fn row_id(sheet: &str, date: &str, kind: RowKind, slot: usize) -> String {
    let mut h = Sha256::new();
    h.update(b"txn-v1|");
    h.update(sheet.as_bytes());
    h.update(b"|");
    h.update(date.as_bytes());
    h.update(b"|");
    h.update(kind.as_str().as_bytes());
    h.update(b"|");
    h.update(slot.to_le_bytes());
    hex::encode(h.finalize())
}
```

Sheet-row transactions have deterministic ids. The compensating Entrada introduced
by this plan is NOT a sheet row — it must not use `row_id` (which would collide with
a real Entrada row for the same date). Use a deterministic derived id instead (see
Step 3).

### Diff-delete mechanism (`import.rs:487–517`)

The diff-delete loop removes transactions whose ids are in `sync_log` for this sheet
but NOT in `current_ids` (the set of ids seen in the current import):

```rust
let existing: Vec<(String,)> = sqlx::query_as(
    "SELECT entity_id FROM sync_log WHERE source_sheet = ?1 AND entity_type = 'transaction'",
)
...
for (eid,) in existing {
    if !current_ids.contains(&eid) {
        sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
            .bind(&eid)
            ...
```

**Critical constraint**: the compensating Entrada (net-zero reimbursement row)
must NOT be inserted into `sync_log` with `entity_type = 'transaction'`. If it
were, the diff-delete would remove it on the next re-import (it has no corresponding
sheet row). Keep the derived Entrada out of `sync_log` entirely, or use a distinct
`entity_type` (e.g. `'derived_reembolo'`). The simplest and safest approach is:
do NOT insert a `sync_log` row for the derived Entrada. It survives re-import
because `current_ids` is built only from sheet rows; the diff-delete only deletes
transactions whose `sync_log` row is stale — the derived Entrada has no `sync_log`
row, so it is never diff-deleted.

If the parent transaction is diff-deleted, the derived Entrada must also be removed.
Anchor it with a FK or a prefix-keyed id so removal is deterministic (see Step 3).

### `transaction` schema (`src-tauri/migrations/20240608000006_transaction.sql`)

```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,
    description TEXT,
    date TEXT NOT NULL,
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

No schema migration is needed. The derived Entrada is just a regular `income`
transaction.

### `split` schema (`src-tauri/migrations/20240608000007_split.sql`)

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

ON DELETE CASCADE means that when the parent Saída transaction is diff-deleted, its
splits are automatically removed. The compensating Entrada carries no split (the full
Entrada belongs to the primary owner).

### Person resolve pattern already in `import_rows_core` (`import.rs:428–449`)

```rust
let existing: Option<(String,)> = sqlx::query_as(
    "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
)
.bind(owner_name)
.fetch_optional(&mut **tx)
.await
.map_err(|e| format!("lookup person '{owner_name}': {e}"))?;

match existing {
    Some((id,)) => id,
    None => {
        let new_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&new_id)
            .bind(owner_name)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("create person '{owner_name}': {e}"))?;
        new_id
    }
}
```

Reuse this exact pattern for both `#dividir:` and `#reembolso:` person resolution.

### Repo conventions (mandatory)

- **Money**: integer cents; `amount` on `transaction` is always a positive magnitude
  (`row.amount.abs()` convention). A compensating Entrada has `type='income'` and a
  positive `amount`.
- **Functional-core / imperative-shell**: `parse_note_markers` must remain a pure
  function (no I/O, no DB). The DB writes live in `import_rows_core`.
- **React Compiler**: irrelevant (Rust-only plan).
- **Method-neutral language**: in code comments and test names, do not reference any
  third-party product, course, or author. Use "the method" and "the spreadsheet".
- **Public repo**: do not embed personal names, real account details, or private
  spreadsheet data in tests. Use generic names (`"Pessoa A"`, `"Pessoa B"`).
- Commit message style: `fix: <short description>` (conventional commits). See
  recent `git log` for examples: `fix: revisão completa da app (rodada 9)`.
- Branch: `advisor/023-rewrite-note-markers`.

## New grammar (full specification — implement exactly this)

The grammar replaces the Plan 004 grammar. It is **opt-in** and **forward-only**:
an untagged note is never modified and behaves byte-for-byte as today.

### Recognized forms (each LINE of the note is parsed independently)

```
R$ <valor> - <descrição> #reembolso:<quem>
```

The FULL `<valor>` of that line is expected to be reimbursed by `<quem>`.
Neko creates a net-zero compensating Entrada of `<valor>` cents, dated at the
transaction's date, with `description = "Reembolso: <quem>"`. Person `<quem>`
is resolved case-insensitively (create-on-demand if absent). The original Saída
is unchanged — the obligation stays visible; cashflow nets to zero.

```
R$ <valor> - <descrição> #dividir:<quem>
OR
R$ <valor> - <descrição> #dividir:<quem>:<valor_da_parte>
```

`<quem>`'s share of that line. Default share = 50% of `<valor>` rounded down to
whole cents. When `:<valor_da_parte>` is provided, use that exact amount.
Two actions:

1. Create a `split` row on the PARENT transaction: `owner_person_id = person(<quem>)`,
   `amount = share_cents`.
2. Create a net-zero compensating Entrada for `share_cents`, dated at the transaction's
   date, with `description = "Dividir: <quem>"`. Person `<quem>` is resolved
   case-insensitively (create-on-demand if absent).

### Dropped

`@<nome>:` — no longer a recognized marker. Old notes containing `@` in free text
are still ignored (the safety test `parse_note_markers_free_prose_ignored` covers this).

`#credito` — dropped entirely. No new code must set `payment_method='credit'` from
a note marker. The existing `payment_method` column is unaffected elsewhere in the
codebase.

### Amount extraction

The tagged line must begin with `R$` (after trimming). Extract the number between
`R$` and the first `-`. Use the existing `parse_number` helper (already in scope
in `import.rs`) which handles commas and dots. If the line does not match the
`R$ <valor> - ... #<marker>:<quem>` shape, the tag is silently ignored (safe default).

### The `R$ <valor>` regex / pattern

Match: `R\$\s*<número>\s*-\s*<descrição>\s+#(reembolso|dividir):<quem>(:<valor_da_parte>)?`

In Rust, parse it procedurally (no regex crate needed):

1. `let lower = trimmed.to_ascii_lowercase();`
2. Detect `#reembolso:` or `#dividir:` suffix (find last `#` in line, check prefix).
3. Extract `<quem>` and optional `:<valor_da_parte>` after the colon.
4. Strip the tag suffix, then parse the `R$ <valor>` prefix of the remaining line.

### Net-zero Entrada id scheme (deterministic)

For each tagged line that generates a compensating Entrada, compute a deterministic id:

```rust
let derived_id = format!("derived:reembolso:{txn_id}:{line_index}");
// or for dividir:
let derived_id = format!("derived:dividir:{txn_id}:{line_index}");
```

where `line_index` is the 0-based index of the tagged line in the note. This id:

- Is stable across re-imports (same parent txn id + same line index → same derived id).
- Is unique per tagged line (multiple tags on the same transaction get distinct ids).
- Does NOT collide with `row_id` output (which uses hex-encoded SHA-256, not the
  `derived:` prefix).
- Allows cleanup when the parent is diff-deleted: delete `WHERE id LIKE 'derived:%:{txn_id}:%'`
  after the parent `DELETE`.

### Re-import idempotency for derived rows

On each import of a transaction with note markers:

1. Delete all derived rows for this parent first:
   `DELETE FROM "transaction" WHERE id LIKE 'derived:%:{txn_id}:%'`
2. Delete all splits for this parent (as today):
   `DELETE FROM split WHERE transaction_id = ?1`
3. Re-insert the derived rows and splits from the current note.

This means re-import is a clean replace: the note is authoritative.

### Diff-delete cleanup for derived rows

After the main diff-delete loop (lines 487–517 in `import_rows_core`), add a
second pass that removes derived rows whose parents were just deleted:

```rust
// Derived rows (net-zero Entradas) have no sync_log row; clean them up
// after their parent transactions are diff-deleted.
// A derived row's id encodes its parent: "derived:<kind>:<parent_txn_id>:<i>"
// Parent is already gone from "transaction" → ON DELETE CASCADE would handle this
// IF the FK is set. Since SQLite enforces FK here, derived rows referencing a
// deleted parent are already gone via CASCADE if we model them as children.
```

**Alternative (simpler)**: insert the derived Entrada with `from_account_id` pointing
to the same account as the parent (NULL is fine), and do NOT add a FK back to the
parent transaction (the schema has no such FK column). Instead, rely on the
deterministic id + a cleanup `DELETE WHERE id LIKE 'derived:%:<parent_id>:%'` inside
the diff-delete loop, immediately after each parent `DELETE`. This avoids a schema
migration. Implement this approach.

The cleanup inside the diff-delete loop becomes:

```rust
// After deleting the stale parent transaction:
sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
    .bind(format!("derived:%:{}:%", &eid))
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("delete derived rows for {eid}: {e}"))?;
```

## Commands you will need

| Purpose                       | Command                                                                       | Expected on success |
| ----------------------------- | ----------------------------------------------------------------------------- | ------------------- |
| Rust typecheck + clippy + fmt | `npm run rust:check`                                                          | exit 0, no warnings |
| Rust tests (all)              | `cargo test --manifest-path src-tauri/Cargo.toml --locked`                    | all pass            |
| Rust tests (filtered)         | `cargo test --manifest-path src-tauri/Cargo.toml --locked parse_note_markers` | target tests pass   |
| Full gate                     | `npm run check`                                                               | exit 0              |
| Typecheck (frontend)          | `npm run typecheck`                                                           | exit 0, no errors   |
| Lint (frontend)               | `npm run lint`                                                                | exit 0              |

> `npm run rust:check` runs `cargo fmt --check`, then `cargo clippy -- -D warnings`,
> then `cargo test`. If `cargo fmt --check` fails, run
> `cargo fmt --manifest-path src-tauri/Cargo.toml` first, then re-run `rust:check`.

## Suggested executor toolkit

- No special skills needed. This is a Rust-only plan; no frontend files change.
- The `parse_number` helper is already in scope in `import.rs` — use it; do not
  duplicate its logic.

## Scope

**In scope** (the only file to modify):

- `src-tauri/src/google_sheets/import.rs`

**Out of scope** (do NOT touch, even if they look related):

- `src-tauri/src/recurrence.rs` — not related to note grammar.
- `src-tauri/src/splits.rs` — read-only reference; no changes needed.
- `src-tauri/migrations/` — no schema migration required; the derived Entrada is a
  regular `transaction` row with a deterministic `derived:` id.
- Any UI component (`.ts`, `.tsx`, `.css`) — the UI for assigning owners is deferred.
- `src-tauri/src/lib.rs`, `commands.rs` — no new Tauri commands.
- Any other Rust file outside `import.rs`.

## Git workflow

- Branch: `advisor/023-rewrite-note-markers`
- One commit per step (or group steps 1–2 as one commit and step 3 onward as a
  second). Keep the codebase compilable between commits.
- Commit message style: `fix: replace @nome:/#credito markers with #dividir:/#reembolso: and net-zero Entrada`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Replace `NoteMarkers` struct and `parse_note_markers` (pure function)

**What to do**: Replace the `NoteMarkers` struct (lines 685–692) and the
`parse_note_markers` function + its doc block (lines 694–753) with the new
struct and function below. Keep the same `pub(crate)` visibility; keep the same
position in the file (after `cell_raw_note`, before `parse_rows_with_layout`).

**New struct**:

```rust
/// Marcadores OPT-IN extraídos de uma nota de célula (`parse_note_markers`).
///
/// SEGURO POR PADRÃO: uma nota sem marcador devolve `NoteMarkers::default()`
/// (sem entradas em `tagged_lines`), de modo que o import se comporta
/// byte-a-byte como hoje.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct NoteMarkers {
    /// Linhas da nota que carregam um marcador reconhecido, na ordem em que
    /// aparecem. Linhas sem marcador não aparecem aqui.
    pub tagged_lines: Vec<TaggedLine>,
}

/// Uma linha de nota com marcador reconhecido.
#[derive(Debug, PartialEq)]
pub(crate) struct TaggedLine {
    /// Índice 0-based da linha dentro da nota (para id determinístico).
    pub line_index: usize,
    /// Valor da linha em centavos inteiros (magnitude positiva).
    /// Extraído do prefixo `R$ <valor>` da linha.
    pub line_amount_cents: i64,
    /// Nome do terceiro que divide ou reembolsa (sem normalização de caixa).
    pub person_name: String,
    /// Tipo do marcador.
    pub kind: NoteMarkerKind,
}

/// Tipo de marcador de nota.
#[derive(Debug, PartialEq)]
pub(crate) enum NoteMarkerKind {
    /// `#reembolso:<quem>` — o VALOR INTEGRAL da linha será reembolsado por <quem>.
    /// Gera uma Entrada compensatória de `line_amount_cents`.
    Reembolso,
    /// `#dividir:<quem>` ou `#dividir:<quem>:<valor>` — a parte de <quem>.
    /// `share_cents` é 50% de `line_amount_cents` (arredondado para baixo) quando
    /// não explicitado; caso contrário, o valor explícito.
    /// Gera um split para <quem> E uma Entrada compensatória de `share_cents`.
    Dividir {
        /// Parte de <quem> em centavos (já resolvida: padrão 50% ou valor explícito).
        share_cents: i64,
    },
}
```

**New function**:

```rust
/// GRAMÁTICA DAS NOTAS (contrato público — opt-in, explícito, seguro por padrão).
///
/// Cada linha da nota é analisada de forma independente. Uma linha só vira
/// marcador quando casa EXATAMENTE com uma das formas estruturadas abaixo;
/// uma nota sem marcador não produz split nem Entrada compensatória
/// (idêntico ao comportamento anterior — provado por teste).
///
/// A sintaxe foi escolhida para não colidir com a convenção pessoal de prosa livre
/// do usuário (validado contra a planilha de referência: zero linhas começando com
/// `R$` E terminando com `#reembolso:` ou `#dividir:`).
///
/// Formas reconhecidas (cada linha analisada individualmente):
///
///   `R$ <valor> - <descrição> #reembolso:<quem>`
///       O valor INTEGRAL da linha é reembolsado por <quem>.
///       Gera uma Entrada compensatória de <valor> centavos, datada na data
///       da transação pai, `description = "Reembolso: <quem>"`.
///       Cashflow líquido = zero (Saída anulada pela Entrada).
///
///   `R$ <valor> - <descrição> #dividir:<quem>`
///       50% de <valor> (arredondado para baixo) é a parte de <quem>.
///       Gera: (1) split na transação pai com owner=<quem>, amount=share;
///             (2) Entrada compensatória de share centavos.
///
///   `R$ <valor> - <descrição> #dividir:<quem>:<valor_da_parte>`
///       Igual, mas com valor explícito para a parte de <quem>.
///
/// Exemplos:
///   `"R$ 530 - Cartões Gio #reembolso:Gio"`  → Entrada R$530, owner Gio
///   `"R$ 200 - Almoço #dividir:Pessoa B"`     → split+Entrada R$100 (50%)
///   `"R$ 200 - Almoço #dividir:Pessoa B:80"`  → split+Entrada R$80 (explícito)
///   `"R$ 1.200 - Parcela carro"`              → NENHUM marcador (prosa livre)
///   `"Mercado da semana"`                     → NENHUM marcador
///
/// Pura — sem I/O, sem DB, sem panics. Testável sem pool.
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut tagged_lines: Vec<TaggedLine> = Vec::new();

    for (line_index, line) in note.lines().enumerate() {
        let trimmed = line.trim();

        // Encontra o último `#` na linha (o marcador vem depois da descrição).
        let Some(hash_pos) = trimmed.rfind('#') else {
            continue;
        };
        let (before_hash, after_hash) = trimmed.split_at(hash_pos);
        // after_hash começa com `#`.

        // Detecta o tipo de marcador e extrai <quem> (e valor opcional para dividir).
        let lower_after = after_hash.to_ascii_lowercase();
        let (kind, person_raw) = if let Some(rest) = lower_after.strip_prefix("#reembolso:") {
            let person_name = &after_hash["#reembolso:".len()..];
            (NoteMarkerKind::Reembolso, person_name.trim())
        } else if let Some(rest) = lower_after.strip_prefix("#dividir:") {
            let payload = &after_hash["#dividir:".len()..].trim();
            // Formato: `<quem>` ou `<quem>:<valor>`.
            let (person_raw, explicit_valor) = if let Some(colon) = payload.find(':') {
                (&payload[..colon], Some(payload[colon + 1..].trim()))
            } else {
                (*payload, None)
            };
            let _ = rest; // silencia unused
            (NoteMarkerKind::Dividir { share_cents: 0 }, (person_raw, explicit_valor))
        } else {
            continue; // marcador não reconhecido
        };
        // NOTE: the match above has two arms returning incompatible tuple shapes; split into
        // a procedural block instead (see implementation note below).
```

**Implementation note**: the two marker kinds return different data shapes, so implement
the body procedurally without trying to unify the return types in a single `if let` chain.
The pattern to follow:

```rust
pub(crate) fn parse_note_markers(note: &str) -> NoteMarkers {
    let mut tagged_lines: Vec<TaggedLine> = Vec::new();

    for (line_index, line) in note.lines().enumerate() {
        let trimmed = line.trim();

        // Marcador deve estar no sufixo: localiza o último '#' na linha.
        let Some(hash_pos) = trimmed.rfind('#') else {
            continue;
        };
        let before_hash = &trimmed[..hash_pos];
        let tag_suffix = &trimmed[hash_pos..]; // inclui o '#'

        let tag_lower = tag_suffix.to_ascii_lowercase();

        // Extrai <quem> e opcional <valor_da_parte> do sufixo reconhecido.
        // Retorna (person_name, Option<valor_da_parte_str>).
        let (marker_kind_tag, raw_payload) =
            if let Some(rest) = tag_lower.strip_prefix("#reembolso:") {
                let person = tag_suffix["#reembolso:".len()..].trim();
                ("reembolso", (person, None::<&str>))
            } else if let Some(_rest) = tag_lower.strip_prefix("#dividir:") {
                let payload = tag_suffix["#dividir:".len()..].trim();
                if let Some(colon) = payload.find(':') {
                    let person = payload[..colon].trim();
                    let val = payload[colon + 1..].trim();
                    ("dividir", (person, Some(val)))
                } else {
                    ("dividir", (payload, None::<&str>))
                }
            } else {
                continue; // tag não reconhecida
            };

        let (person_raw, explicit_valor_str) = raw_payload;
        let person_name = person_raw.to_string();
        if person_name.is_empty() {
            continue; // <quem> vazio → ignora
        }

        // Extrai R$ <valor> do prefixo `before_hash`.
        // Formato esperado: `R$ <número> - <descrição> ` (com espaço antes do `#`).
        let before = before_hash.trim();
        let line_amount_cents = if let Some(rest) = before.to_ascii_lowercase().strip_prefix("r$") {
            // Tudo antes do primeiro ` - ` é o valor.
            let value_part = if let Some(dash) = rest.find(" - ") {
                &rest[..dash]
            } else {
                rest
            };
            // Usa parse_number existente (lida com vírgula/ponto).
            // parse_number retorna i64 em centavos.
            parse_number(value_part.trim())
        } else {
            continue; // linha não começa com R$ → ignora
        };

        if line_amount_cents <= 0 {
            continue; // valor inválido ou zero → ignora
        }

        let kind = match marker_kind_tag {
            "reembolso" => NoteMarkerKind::Reembolso,
            "dividir" => {
                let share_cents = if let Some(val_str) = explicit_valor_str {
                    let v = parse_number(val_str);
                    if v > 0 { v } else { line_amount_cents / 2 }
                } else {
                    line_amount_cents / 2 // 50% arredondado para baixo
                };
                NoteMarkerKind::Dividir { share_cents }
            }
            _ => continue,
        };

        tagged_lines.push(TaggedLine {
            line_index,
            line_amount_cents,
            person_name,
            kind,
        });
    }

    NoteMarkers { tagged_lines }
}
```

**Verify**: `npm run rust:check` → exit 0.

---

### Step 2: Replace the writer block in `import_rows_core`

**What to do**: Find the writer block that begins at line 396 with the comment
`// --- Plan 004: splits de titular + payment_method='credit' via gramática da nota ---`
and ends at line 466 with `// --- fim Plan 004 ---`. Replace it entirely with the
new block below.

Keep the same position: after the UPSERT of the transaction row, before the `sync_log`
UPSERT (around line 468).

**Also add**: inside the diff-delete loop (lines 495–516), after the
`DELETE FROM "transaction" WHERE id = ?1` (line 497), add the derived-row cleanup.

**New writer block** (replaces lines 396–466):

```rust
        // --- Plan 023: gramática das notas (#reembolso:/#dividir:) ---
        // Opt-in e forward-only: nota sem marcador → no-op (idêntico ao comportamento de hoje).
        let markers = parse_note_markers(&row.raw_note);

        if !markers.tagged_lines.is_empty() {
            // Idempotência no re-import: descarta as linhas derivadas e splits anteriores
            // desta transação, depois re-insere a partir da nota atual.
            sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
                .bind(format!("derived:%:{}:%", txn_id))
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear derived rows for {txn_id}: {e}"))?;

            sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
                .bind(&txn_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("clear splits for {txn_id}: {e}"))?;

            for tagged in &markers.tagged_lines {
                // Resolve a pessoa pelo nome (case-insensitive); cria sob demanda na MESMA tx.
                let person_id: String = {
                    let existing: Option<(String,)> = sqlx::query_as(
                        "SELECT id FROM person WHERE LOWER(name) = LOWER(?1) LIMIT 1",
                    )
                    .bind(&tagged.person_name)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| format!("lookup person '{}': {e}", tagged.person_name))?;

                    match existing {
                        Some((id,)) => id,
                        None => {
                            let new_id = uuid::Uuid::new_v4().to_string();
                            sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                                .bind(&new_id)
                                .bind(&tagged.person_name)
                                .execute(&mut **tx)
                                .await
                                .map_err(|e| {
                                    format!("create person '{}': {e}", tagged.person_name)
                                })?;
                            new_id
                        }
                    }
                };

                match &tagged.kind {
                    NoteMarkerKind::Reembolso => {
                        // Entrada compensatória: valor integral da linha.
                        let derived_id =
                            format!("derived:reembolso:{}:{}", txn_id, tagged.line_index);
                        let desc = format!("Reembolso: {}", tagged.person_name);
                        sqlx::query(
                            "INSERT OR REPLACE INTO \"transaction\" \
                             (id, type, amount, description, date, is_fixed, is_projection, \
                              created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(tagged.line_amount_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert reembolso Entrada: {e}"))?;
                    }
                    NoteMarkerKind::Dividir { share_cents } => {
                        // Split na transação pai para <quem>.
                        let split_id = uuid::Uuid::new_v4().to_string();
                        sqlx::query(
                            "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
                             VALUES (?1, ?2, ?3, ?4)",
                        )
                        .bind(&split_id)
                        .bind(&txn_id)
                        .bind(share_cents)
                        .bind(&person_id)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| {
                            format!("insert split for '{}': {e}", tagged.person_name)
                        })?;

                        // Entrada compensatória pela parte de <quem>.
                        let derived_id =
                            format!("derived:dividir:{}:{}", txn_id, tagged.line_index);
                        let desc = format!("Dividir: {}", tagged.person_name);
                        sqlx::query(
                            "INSERT OR REPLACE INTO \"transaction\" \
                             (id, type, amount, description, date, is_fixed, is_projection, \
                              created_at, updated_at) \
                             VALUES (?1, 'income', ?2, ?3, ?4, 0, 0, ?5, ?5)",
                        )
                        .bind(&derived_id)
                        .bind(share_cents)
                        .bind(&desc)
                        .bind(&row.date)
                        .bind(&now)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| format!("insert dividir Entrada: {e}"))?;
                    }
                }
            }
        }
        // --- fim Plan 023 ---
```

**New diff-delete cleanup** (add immediately after
`sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")` in the diff-delete loop,
around line 497):

```rust
            // Derived rows (Entradas compensatórias) têm ids determinísticos prefixados com
            // "derived:<kind>:<parent_id>:<i>" e NÃO têm linha em sync_log. Limpamos aqui
            // quando o pai é removido pelo diff-delete.
            sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
                .bind(format!("derived:%:{}:%", &eid))
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("delete derived rows for {eid}: {e}"))?;
```

**Verify**: `npm run rust:check` → exit 0. All existing tests still pass.

---

### Step 3: Replace the unit tests for `parse_note_markers`

**What to do**: Find the test group starting with `// Plan 004: gramática das notas (parse puro, sem DB)`
at line 2315. Replace all tests under that comment through `parse_note_markers_at_without_colon_ignored`
(line 2392) with the tests below. Keep the section comment and the surrounding test module
structure unchanged.

**New pure-parse tests** (rename the section header to `// Plan 023: gramática das notas (parse puro, sem DB)`):

```rust
    // ===================================================================
    // Plan 023: gramática das notas (parse puro, sem DB)
    // ===================================================================

    #[test]
    fn parse_note_markers_empty_note() {
        let m = parse_note_markers("");
        assert!(m.tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_free_prose_ignored() {
        // Notas de prosa livre NÃO disparam marcador algum.
        // Formato real da planilha: "R$ X - descrição" sem tag.
        let note = "R$ 65,00 - Vivo · faltou só o frango";
        assert!(parse_note_markers(note).tagged_lines.is_empty());

        // Linha sem R$ também é ignorada.
        assert!(parse_note_markers("Mercado da semana").tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_reembolso_full_value() {
        let note = "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].line_index, 0);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 53000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa B");
        assert_eq!(m.tagged_lines[0].kind, NoteMarkerKind::Reembolso);
    }

    #[test]
    fn parse_note_markers_dividir_default_50_percent() {
        let note = "R$ 200,00 - Almoço #dividir:Pessoa A";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 20000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa A");
        assert_eq!(
            m.tagged_lines[0].kind,
            NoteMarkerKind::Dividir { share_cents: 10000 } // 50% de 200
        );
    }

    #[test]
    fn parse_note_markers_dividir_explicit_value() {
        let note = "R$ 200,00 - Almoço #dividir:Pessoa A:80,00";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(
            m.tagged_lines[0].kind,
            NoteMarkerKind::Dividir { share_cents: 8000 } // valor explícito
        );
    }

    #[test]
    fn parse_note_markers_multiple_tagged_lines() {
        // Nota com duas linhas marcadas e uma linha de prosa livre.
        let note = "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B\n\
                    R$ 1.200,00 - Parcela carro\n\
                    R$ 191,00 - Empréstimo Pessoa C #reembolso:Pessoa C";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 2);
        assert_eq!(m.tagged_lines[0].line_index, 0);
        assert_eq!(m.tagged_lines[0].line_amount_cents, 53000);
        assert_eq!(m.tagged_lines[0].person_name, "Pessoa B");
        assert_eq!(m.tagged_lines[1].line_index, 2);
        assert_eq!(m.tagged_lines[1].line_amount_cents, 19100);
        assert_eq!(m.tagged_lines[1].person_name, "Pessoa C");
    }

    #[test]
    fn parse_note_markers_case_insensitive_tag() {
        // O marcador é case-insensitive.
        let note = "R$ 100,00 - Teste #REEMBOLSO:Pessoa A";
        let m = parse_note_markers(note);
        assert_eq!(m.tagged_lines.len(), 1);
        assert_eq!(m.tagged_lines[0].kind, NoteMarkerKind::Reembolso);
    }

    #[test]
    fn parse_note_markers_no_rs_prefix_ignored() {
        // Linha sem `R$` não é marcador — mesmo que termine com `#reembolso:`.
        let note = "Transferência bancária #reembolso:Pessoa A";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_empty_person_ignored() {
        // `#reembolso:` sem <quem> → ignora.
        let note = "R$ 100,00 - Teste #reembolso:";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_old_at_syntax_ignored() {
        // `@Pessoa A: 150,00` (sintaxe Plan 004) já não é um marcador reconhecido.
        let note = "@Pessoa A: 150,00";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }

    #[test]
    fn parse_note_markers_old_credito_ignored() {
        // `#credito` (Plan 004) não é mais reconhecido.
        let note = "#credito";
        assert!(parse_note_markers(note).tagged_lines.is_empty());
    }
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked parse_note_markers`
→ all new tests pass. `npm run rust:check` → exit 0.

---

### Step 4: Replace the integration tests

**What to do**: Find the integration test group starting with
`// Plan 004: testes de integração (DB)` at line 2394 and replace all tests in that
section (through the end of the file or the next unrelated section) with the tests below.

**New integration tests** (rename section to `// Plan 023: testes de integração (DB)`):

```rust
    // ===================================================================
    // Plan 023: testes de integração (DB)
    // ===================================================================

    #[tokio::test]
    async fn import_reembolso_creates_compensating_entrada() {
        // #reembolso: gera uma Entrada compensatória; cashflow líquido = zero.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-10".into(),
            amount: -53000, // R$530 Saída
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // A Entrada compensatória deve existir.
        let (tipo, amount, desc): (String, i64, String) = sqlx::query_as(
            "SELECT type, amount, description FROM \"transaction\" \
             WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tipo, "income");
        assert_eq!(amount, 53000);
        assert!(desc.contains("Pessoa B"), "descrição menciona a pessoa");

        // Cashflow líquido: Saída 530 + Entrada 530 = 0.
        let (net,): (i64,) = sqlx::query_as(
            "SELECT SUM(CASE type WHEN 'income' THEN amount ELSE -amount END) \
             FROM \"transaction\" WHERE date = '2026-01-10'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(net, 0, "cashflow líquido deve ser zero");
    }

    #[tokio::test]
    async fn import_dividir_creates_split_and_compensating_entrada() {
        // #dividir: gera split + Entrada compensatória pela parte de <quem>.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-01-15".into(),
            amount: -20000, // R$200 Saída
            description: "Almoço".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 200,00 - Almoço #dividir:Pessoa A".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // Split criado com 50% do valor.
        let (split_amount,): (i64,) = sqlx::query_as(
            "SELECT s.amount FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             WHERE LOWER(p.name) = 'pessoa a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(split_amount, 10000, "50% de R$200");

        // Entrada compensatória para 50%.
        let (tipo, amount): (String, i64) = sqlx::query_as(
            "SELECT type, amount FROM \"transaction\" \
             WHERE id LIKE 'derived:dividir:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tipo, "income");
        assert_eq!(amount, 10000);
    }

    #[tokio::test]
    async fn import_dividir_explicit_value() {
        // #dividir:<quem>:<valor> usa o valor explícito.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-02-01".into(),
            amount: -20000,
            description: "Almoço".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 200,00 - Almoço #dividir:Pessoa A:80,00".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (split_amount,): (i64,) = sqlx::query_as(
            "SELECT s.amount FROM split s \
             JOIN person p ON p.id = s.owner_person_id \
             WHERE LOWER(p.name) = 'pessoa a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(split_amount, 8000, "valor explícito R$80");
    }

    #[tokio::test]
    async fn import_multiple_tagged_lines_same_note() {
        // Nota com duas linhas marcadas: dois reembolsos independentes.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-03-01".into(),
            amount: -72100, // R$721 total
            description: "Múltiplas despesas".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B\n\
                       R$ 1.200,00 - Parcela carro\n\
                       R$ 191,00 - Empréstimo Pessoa C #reembolso:Pessoa C"
                .into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        // Duas Entradas compensatórias.
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:reembolso:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn reimport_replaces_derived_rows_idempotently() {
        // Re-import substitui as Entradas derivadas e splits (idempotente).
        let pool = test_pool().await;

        let v1 = vec![ImportedRow {
            date: "2026-04-01".into(),
            amount: -53000,
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
        }];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Segundo import com nota diferente.
        let v2 = vec![ImportedRow {
            date: "2026-04-01".into(),
            amount: -53000,
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa C".into(),
        }];
        import_rows(&pool, "2026", &v2, "p1").await.unwrap();

        // Deve haver exatamente uma Entrada derivada (a do segundo import).
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "re-import substituiu a Entrada derivada");

        let (desc,): (String,) = sqlx::query_as(
            "SELECT description FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(desc.contains("Pessoa C"), "nova Entrada aponta para Pessoa C");
    }

    #[tokio::test]
    async fn diff_delete_removes_derived_rows() {
        // Quando a transação pai é removida pelo diff-delete, as Entradas derivadas também somem.
        let pool = test_pool().await;

        let v1 = vec![ImportedRow {
            date: "2026-05-01".into(),
            amount: -53000,
            description: "Cartões Pessoa B".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Pessoa B #reembolso:Pessoa B".into(),
        }];
        import_rows(&pool, "2026", &v1, "p1").await.unwrap();

        // Confirma que a Entrada derivada existe.
        let (before,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(before, 1);

        // Re-import com lista vazia → diff-delete remove a transação pai.
        import_rows(&pool, "2026", &[], "p1").await.unwrap();

        let (after,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(after, 0, "Entrada derivada removida junto com o pai");
    }

    #[tokio::test]
    async fn import_no_note_leaves_no_derived_rows_and_no_splits() {
        // PROVA DE SEGURANÇA: nota ausente → comportamento idêntico ao de hoje.
        let pool = test_pool().await;
        let rows = vec![imported("2026-06-01", -10000)];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (derived,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(derived, 0, "sem nota → sem Entradas derivadas");

        let (splits,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM split")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(splits, 0, "sem nota → sem splits");
    }

    #[tokio::test]
    async fn import_unmarked_prose_note_leaves_no_derived_rows() {
        // PROVA DE SEGURANÇA reforçada: nota de prosa livre real NÃO dispara marcadores.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-06-02".into(),
            amount: -72100,
            description: "Contas mensais".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 530,00 - Cartões Gio\nR$ 1.200,00 - Parcela carro\n\
                       R$ 191,00 - Empréstimo Viagem Jane"
                .into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (derived,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'derived:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(derived, 0, "prosa livre → sem Entradas derivadas");

        let (splits,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM split")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(splits, 0, "prosa livre → sem splits");
    }

    #[tokio::test]
    async fn import_person_created_on_demand_for_reembolso() {
        // Pessoa não-existente é criada sob demanda na mesma transação DB.
        let pool = test_pool().await;
        let rows = vec![ImportedRow {
            date: "2026-07-01".into(),
            amount: -10000,
            description: "Teste".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 100,00 - Teste #reembolso:Nova Pessoa".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM person WHERE LOWER(name) = 'nova pessoa'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "pessoa criada sob demanda");
    }

    #[tokio::test]
    async fn import_person_reuse_case_insensitive() {
        // Pessoa pré-existente é reutilizada (sem duplicata) mesmo com caixa diferente.
        let pool = test_pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pid-pa', 'Pessoa A')")
            .execute(&pool)
            .await
            .unwrap();

        let rows = vec![ImportedRow {
            date: "2026-08-01".into(),
            amount: -10000,
            description: "Teste".into(),
            is_projection: false,
            kind: RowKind::Saida,
            raw_note: "R$ 100,00 - Teste #reembolso:PESSOA A".into(),
        }];
        import_rows(&pool, "2026", &rows, "p1").await.unwrap();

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM person WHERE LOWER(name) = 'pessoa a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "nenhuma pessoa duplicada criada");
    }
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked` → all tests pass,
including the new 10 integration tests. `npm run rust:check` → exit 0.

---

### Step 5: Remove the `#credito` writer path (if any residue remains)

**What to do**: Grep the file for any remaining reference to `is_credit` or `#credito`
in non-test, non-comment code:

```
grep -n "is_credit\|#credito\|payment_method.*credit" src-tauri/src/google_sheets/import.rs
```

All matches should be either in comments or inside deleted test functions. If any
active (non-test, non-comment) code still references `is_credit` or sets
`payment_method='credit'` via the note-marker path, remove it.

Note: the `payment_method` column and the `CHECK(payment_method IN ('debit','credit','pix','cash'))`
constraint remain in the schema — they are valid for other uses. Do not remove the schema column.

**Verify**: `npm run rust:check` → exit 0.

---

### Step 6: Final gate

**What to do**:

```bash
git diff --name-only HEAD
```

Expected: only `src-tauri/src/google_sheets/import.rs`.

Then run:

```bash
npm run check
```

Expected: exit 0 (full gate: fmt, clippy, tests, frontend typecheck, lint, privacy scan).

**Verify**: `npm run check` → exit 0. No files outside `import.rs` are modified.

## Test plan

**New pure-parse tests** (Step 3 — rename section from Plan 004 to Plan 023):

- `parse_note_markers_empty_note` (kept, adapted)
- `parse_note_markers_free_prose_ignored` (kept, adapted — must still pass)
- `parse_note_markers_reembolso_full_value`
- `parse_note_markers_dividir_default_50_percent`
- `parse_note_markers_dividir_explicit_value`
- `parse_note_markers_multiple_tagged_lines`
- `parse_note_markers_case_insensitive_tag`
- `parse_note_markers_no_rs_prefix_ignored`
- `parse_note_markers_empty_person_ignored`
- `parse_note_markers_old_at_syntax_ignored` (regression: old `@nome:` no longer fires)
- `parse_note_markers_old_credito_ignored` (regression: old `#credito` no longer fires)

**New integration tests** (Step 4 — 10 tests):

- `import_reembolso_creates_compensating_entrada`
- `import_dividir_creates_split_and_compensating_entrada`
- `import_dividir_explicit_value`
- `import_multiple_tagged_lines_same_note`
- `reimport_replaces_derived_rows_idempotently`
- `diff_delete_removes_derived_rows`
- `import_no_note_leaves_no_derived_rows_and_no_splits`
- `import_unmarked_prose_note_leaves_no_derived_rows`
- `import_person_created_on_demand_for_reembolso`
- `import_person_reuse_case_insensitive`

**Key regression to verify**: the two safety tests (`import_no_note_...` and
`import_unmarked_prose_note_...`) must pass. These prove that existing untagged notes
— including real-format lines like `"R$ 530,00 - Cartões Gio"` without a `#` tag —
never trigger derived rows or splits.

**Structural pattern**: model all async integration tests after
`reimport_preserves_transaction_identity_and_enrichment` (import.rs ~line 1683); use
`test_pool()` for in-memory SQLite with migrations applied.

**Verification command**: `cargo test --manifest-path src-tauri/Cargo.toml --locked`
→ all tests pass (≥ 21 new tests across steps 3–4, plus all pre-existing tests).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (fmt + clippy + all Rust tests)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; at least
      21 new tests exist (11 pure + 10 integration) and all pass
- [ ] `grep -n "is_credit\|@Pessoa\|@Ana\|@Bruno\|#credito" src-tauri/src/google_sheets/import.rs`
      returns no matches in active (non-comment) code outside test functions
- [ ] `grep -n "tagged_lines\|NoteMarkerKind\|Reembolso\|Dividir" src-tauri/src/google_sheets/import.rs`
      returns matches (struct/enum definitions + call site + tests)
- [ ] `grep -n "derived:reembolso\|derived:dividir" src-tauri/src/google_sheets/import.rs`
      returns matches (writer block + diff-delete cleanup + integration tests)
- [ ] `git diff --name-only HEAD` shows only `src-tauri/src/google_sheets/import.rs`
- [ ] `npm run check` exits 0 (full gate including frontend typecheck, lint, privacy scan)
- [ ] `plans/README.md` status row for plan 023 updated to DONE

## STOP conditions

Stop and report (do not improvise) if:

1. **Drift**: the code at the locations in "Current state" does not match the excerpts.
   Compare the live file against this plan's "Current state" section and report the
   delta. Do not guess line number offsets.

2. **Net-zero Entrada conflicts with the UPSERT/diff-delete**: if inserting a derived
   Entrada (with a `derived:` prefixed id) causes an FK violation, a constraint failure,
   or is silently removed by the diff-delete loop before the cleanup patch in Step 2 is
   in place — STOP. Report the exact error and the state of the diff-delete loop. The
   cleanest anchoring is the deterministic `derived:<kind>:<parent_txn_id>:<line_index>`
   id + an explicit cleanup step (already specified), but if the schema or the loop
   structure has changed in a way that breaks this, do not improvise.

3. **`parse_number` returns unexpected units**: if `parse_number("530,00")` does NOT
   return `53000` (integer cents), the amount extraction will be wrong. Verify with a
   unit test before writing the DB path. If `parse_number` returns raw floats or
   thousands, STOP and report — do not adapt silently.

4. **`LIKE` pattern on `derived:%:<txn_id>:%` matches unintended rows**: if the
   diff-delete cleanup `DELETE WHERE id LIKE 'derived:%:<eid>:%'` removes rows it
   should not (e.g., if any existing non-derived transaction id contains the substring
   `derived:`), STOP. Verify with a test. Since `row_id` produces hex-only SHA-256
   output, no collision is expected — but confirm.

5. **A step's verification fails twice** after a reasonable fix attempt. Do not proceed.

6. **The fix requires touching a file outside `import.rs`**. Report which file and why,
   and wait for approval.

7. **Clippy `-D warnings` triggers on new code** and the fix is non-obvious. Report
   the warning text; do not `#[allow(...)]` without understanding it.

## Maintenance notes

- **This plan supersedes the note-grammar part of plan 004**. Plan 004 (DONE) is
  otherwise complete (atomic import, `raw_note` field, person resolution, `split` table
  usage); only the grammar and writer block are replaced here. Do NOT re-execute plan 004
  on top of this plan.

- **Forward-only convention**: historical notes that lack `#reembolso:` or `#dividir:` are
  unchanged. No backfill is performed. If the user wants to retag old notes, they add the
  tag to the spreadsheet cell and re-import — the idempotent re-import will pick it up.

- **Deferred: `reimbursed_by` FK on `transaction`**: a formal FK column
  (`reimbursed_by TEXT REFERENCES "transaction"(id)`) would make the parent↔derived
  relationship explicit in the schema and enable efficient cleanup without `LIKE`.
  Deferred because it requires a migration and no query currently needs the FK.
  Add in a follow-up migration when the invoice/fatura entity (spike 019) needs it.

- **Deferred: UI for assigning tags**: the note-marker grammar is the only entry point
  for `#reembolso:` and `#dividir:` in this plan. A UI to assign these tags post-import
  is a follow-up (related to plan 015's owner-assignment UI).

- **Deferred: "suggest tag for recurring free-text pattern"**: if a line like
  `"R$ 530,00 - Cartões Gio"` appears every month, a future "suggest tag" assist could
  propose `#reembolso:Gio` based on the recurring free-text pattern. Do NOT build here.

- **Reviewer checklist for the PR**:
  1. Confirm that `parse_note_markers_free_prose_ignored` still passes — this is the
     primary safety regression guard.
  2. Confirm that `diff_delete_removes_derived_rows` passes — this proves the
     diff-delete cleanup in Step 2 works.
  3. Verify the `INSERT OR REPLACE` for derived Entradas is safe: since the derived id
     is deterministic, a re-import will replace (not duplicate) the row.
  4. Verify that no `sync_log` row is inserted for derived Entradas — they must not
     appear in the diff-delete `SELECT entity_id FROM sync_log` query.
  5. Confirm `parse_note_markers_old_credito_ignored` and
     `parse_note_markers_old_at_syntax_ignored` pass — these prove backward grammar
     markers are dead.
