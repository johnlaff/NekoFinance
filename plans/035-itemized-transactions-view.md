# Plan 035: Itemized transactions — model + import parse + view the breakdown (past/future/new)

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
> git diff --stat e62ecb6..HEAD -- \
>   src-tauri/migrations/ \
>   src-tauri/src/google_sheets/import.rs \
>   src-tauri/src/commands/transactions.rs \
>   src/lib/api.ts \
>   src/screens/TransactionsScreen.tsx \
>   src/test/commands.ts
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: none
- **Category**: feature
- **Package**: D
- **Planned at**: commit `e62ecb6`, 2026-06-20

## Why this matters

The user's primary annotation mechanism is the itemized cell: a single Entrada
or Saída cell holds a total that is the sum of named parts, with each part
described in the cell note as `R$ <valor> - <descrição>`. Today Neko imports
the total and shows it as a single line — the breakdown is invisible. This plan
models the breakdown as child `line_item` rows, parses them from the cell note
during import (for all realized and projected transactions), and surfaces them
in the Livro-razão as an expandable disclosure. No data is ever lost: if the
note can't be parsed cleanly, the single-total transaction is kept intact with
the raw note attached. Writing back itemized cells to the spreadsheet is
explicitly deferred to plan 036.

## Current state

### Grammar of the cell note (from `src-tauri/src/google_sheets/import.rs`)

`cell_raw_note` (line 719–726) extracts the raw multi-line note and assigns it
to `ImportedRow.raw_note` (struct at line 82–93). The note is already included
in the `compute_checksum` (line 129) so editing a note triggers re-import.

The existing note-marker parser `parse_note_markers` (line 805–893) handles
`#reembolso:` and `#dividir:` markers and already demonstrates the pattern for
toleration of `R$ <valor> - <descrição>` lines (it reads `R$` prefixes and
calls `parse_number`). The new itemized-line parser must co-exist without
conflicting: it is a SEPARATE function, not a modification of `parse_note_markers`.

**Itemized-line grammar to handle** (verified against real usage; plan file uses
only generic examples):

| Form                        | Example                                  | Treatment                                                                                                                  |
| --------------------------- | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Standard                    | `R$ 50,00 - Descrição do item`           | Parse: 5000 cents, "Descrição do item"                                                                                     |
| No space around dash        | `R$ 50,00-Descrição do item`             | Tolerate: same                                                                                                             |
| `R$` without trailing space | `R$50,00 - Descrição do item`            | Tolerate: same                                                                                                             |
| Header line (no `R$`)       | `CONTAS:` or `Detalhes`                  | Skip silently                                                                                                              |
| Budget/projection line      | `Mensal\tR$ 300,00\tCategoria`           | Skip (no leading `R$`)                                                                                                     |
| Total trailer               | `Total = R$ 1.250,00`                    | Skip (no leading `R$`)                                                                                                     |
| Marker line                 | `R$ 200,00 - Item A #reembolso:Pessoa X` | Parse as line item AND pass to `parse_note_markers` independently — the two parsers are independent reads of the same note |

Values: comma-decimal, optional dot-thousands (same rules as the existing
`parse_number`). The cell total comes from `UNFORMATTED_VALUE` (already in
`row.amount`). Reconciliation rule: **if** `|Σ parts − cell_total| ≤ 1 cent`
(rounding tolerance), attach the items; otherwise keep the single total +
raw_note with zero line items (never lose or alter the total).

### Existing split pattern (reference for idempotency)

`src-tauri/src/splits.rs` lines 1–42 show `splits_for_transaction` — the model
for a read-side getter returning child rows for a parent transaction.

The idempotency pattern for re-import is at
`src-tauri/src/google_sheets/import.rs` lines 404–418:

```rust
// clear derived rows then reinsert from the authoritative note
sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
    .bind(format!("derived:%:{txn_id}:%"))
    .execute(&mut **tx)
    ...
sqlx::query("DELETE FROM split WHERE transaction_id = ?1")
    .bind(&txn_id)
    .execute(&mut **tx)
    ...
```

Apply the same pattern for `line_item`: clear-then-reinsert per transaction on
every import, anchored to the deterministic `txn_id`.

### DB schema (existing migrations, last at `20240621000001_tag_exclude_from_totals.sql`)

The `transaction` table (migration `20240608000006_transaction.sql`):

```sql
CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,   -- magnitude positive cents (convention)
    description TEXT,
    date TEXT NOT NULL,
    ...
    is_projection INTEGER NOT NULL DEFAULT 0,
    ...
);
```

The `split` table (migration `20240608000007_split.sql`) is the closest
structural precedent for child rows:

```sql
CREATE TABLE IF NOT EXISTS split (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES "transaction"(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    ...
);
```

### Frontend — transaction list

`src/screens/TransactionsScreen.tsx` lines 220–309 contain `LedgerDataRow` — the
`<tr>` for each transaction. Lines 658–700 render each row:

```tsx
<LedgerDataRow
  t={t}
  tagEditOpen={ui.tagEditId === t.id}
  actionOpen={ui.actionRowId === t.id}
  onToggleTagEditor={...}
  onToggleAction={...}
/>
{ui.actionRowId === t.id && <ActionPanelRow ... />}
{ui.tagEditId === t.id && <TagEditorRow ... />}
```

The pattern for an expandable sub-row is established: a separate `<tr>` opened
by a toggle in state. The new `LineItemsRow` follows the same `<tr
className="txn-tag-editor">…<td colSpan={6}>` shape.

The UI state reducer (lines 168–218) uses `useReducer` with a discriminated
union — add a new `expandItemsId` field and a `toggleItems` action.

### API types

`src/lib/api.ts` lines 48–64: `TransactionRow` interface. Add `line_items:
LineItem[]` field, where `LineItem` is a new interface.

`src-tauri/src/commands/transactions.rs` lines 12–29: Rust `TransactionRow`
struct with `owners` and `tags` loaded as a separate batch query. Add
`line_items: Vec<LineItemOnRow>` to match, populated by a per-transaction
query (or a batch query by ids, like tags).

### Repo conventions

- Money = positive-magnitude integer cents. `line_item.amount_cents` stores the
  part value as a positive integer (same direction as the parent total), signed
  only if the parent is an expense (consistent with `amount` in `transaction`).
  For simplicity, store the **absolute magnitude** in cents; the parent type
  determines the direction.
- Rust: functional core, imperative shell. The parser `parse_itemized_note` is
  a pure function (no I/O) with unit tests, mirroring `parse_note_markers`
  (lines 805–893 of `import.rs`).
- React Compiler is ON: no manual `useMemo`/`useCallback`; hoist static styles
  as module-level `const` objects (see `ACTION_CELL_STYLE` at line 50–54 of
  `TransactionsScreen.tsx`).
- Error handling: `Result<_, String>` in Rust; no panic.
- Test structure: Rust `#[tokio::test]` in `#[cfg(test)] mod tests` at the
  bottom of the owning module; TS in `*.test.tsx` beside the component.

## Commands you will need

| Purpose          | Command              | Expected on success       |
| ---------------- | -------------------- | ------------------------- |
| Rust checks      | `npm run rust:check` | exit 0, no errors         |
| Typecheck        | `npm run typecheck`  | exit 0, no errors         |
| Unit tests (all) | `npm run test:run`   | exit 0, all pass          |
| Full gate        | `npm run check`      | exit 0, green             |
| React Doctor     | `npm run doctor`     | 0 findings                |
| Lint             | `npm run lint`       | exit 0                    |
| E2E visual smoke | `npm run e2e`        | exit 0, screenshots match |

## Suggested executor toolkit

- Use the `shadcn` skill if you need a ready-made `<details>`/disclosure component
  that already follows the DS token system — but a plain `<details>`/`<summary>`
  element or a controlled `<tr>` toggle is sufficient and lower-risk.
- Reference `src-tauri/src/splits.rs` for the getter + Tauri command wrapper
  pattern (pure function → async DB query → `#[tauri::command]` thin wrapper).
- Reference `src-tauri/src/google_sheets/import.rs:805–893` (`parse_note_markers`)
  for the pure-function note-parser shape.

## Scope

**In scope** (the only files you should modify or create):

Rust / migration:

- `src-tauri/migrations/20260620000001_line_item.sql` (new — forward migration)
- `src-tauri/src/google_sheets/import.rs` (add `parse_itemized_note`, call it in
  `import_rows_core`, add unit tests)
- `src-tauri/src/commands/transactions.rs` (add `LineItemOnRow`, add
  `line_items` field to `TransactionRow`, add batch query, add
  `get_line_items_cmd` Tauri command)
- `src-tauri/src/lib.rs` (register `get_line_items_cmd` in the handler list)

Frontend:

- `src/lib/api.ts` (add `LineItem` interface, add `line_items` field to
  `TransactionRow`, add `getLineItems` function)
- `src/screens/TransactionsScreen.tsx` (add `LineItemsRow`, expand-toggle in
  state, render)
- `src/screens/TransactionsScreen.test.tsx` (new tests for the breakdown)
- `src/test/commands.ts` (add `line_items` field to fixture `TXNS` — empty
  array for backward compat)

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/write_back.rs` — writing itemized cells back to
  Sheets is plan 036.
- `parse_note_markers` in `import.rs` — the two parsers are independent; do not
  merge or modify the existing marker parser.
- Any change to the `transaction.amount` field or the Saldo chain — items are
  descriptive children; the parent total is sacrosanct.
- The dashboard hero/forecast screen — no layout changes outside
  `TransactionsScreen.tsx`.
- The `split` table or the `splits.rs` module — line items are a separate
  concept (annotation breakdown, not multi-owner cost sharing).

## Git workflow

- Branch: `advisor/035-itemized-transactions-view`
- Commit style (match repo): `feat: <scope> — <what>` or `fix:` / `chore:`
  with imperative lowercase. Recent examples from `git log`:
  `feat: toggle "Ignorar nos cálculos" nas tags (plano 034)`
- One commit per step is acceptable; squash is fine before PR.
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Forward migration — create `line_item` table

Create `src-tauri/migrations/20260620000001_line_item.sql`:

```sql
-- Plan 035: breakdown of an itemized cell into its constituent parts.
-- Each row is one line of the cell note parsed as `R$ <valor> - <descrição>`.
-- Items are descriptive children: the parent transaction total is unchanged.
-- ON DELETE CASCADE: removing the parent cleans up its items automatically.
CREATE TABLE IF NOT EXISTS line_item (
    id        TEXT    PRIMARY KEY NOT NULL,
    -- FK to the parent transaction (may be realized OR projected).
    transaction_id TEXT NOT NULL
        REFERENCES "transaction"(id) ON DELETE CASCADE,
    -- Absolute magnitude in cents (positive integer). Direction = parent type.
    amount_cents   INTEGER NOT NULL,
    description    TEXT    NOT NULL DEFAULT '',
    -- 0-based insertion order, preserving the note line order.
    position       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_line_item_transaction_id
    ON line_item (transaction_id);
```

**Verify**: `npm run rust:check` → exit 0 (sqlx migrations run; if the test
suite runs `sqlx::migrate!` in memory, the new table must appear without error).

### Step 2: Pure-function parser `parse_itemized_note` in `import.rs`

Add the function **before** `parse_note_markers` (around line 805 of
`src-tauri/src/google_sheets/import.rs`).

```rust
/// Resultado de uma linha itemizada da nota de célula.
#[derive(Debug, PartialEq)]
pub(crate) struct NoteLineItem {
    /// Magnitude em centavos (positiva). Mesma convenção de `transaction.amount`.
    pub amount_cents: i64,
    pub description: String,
    /// Posição 0-based na nota (ordem de aparição).
    pub position: usize,
}

/// Parseia as linhas itemizadas de uma nota de célula.
///
/// GRAMÁTICA: cada linha começando com `R$` (com ou sem espaço entre `R$` e o número)
/// seguida de um separador ` - ` (com ou sem espaços ao redor) é tratada como um item.
/// Linhas que não começam com `R$` (cabeçalhos, trailers `Total = …`, linhas em branco,
/// linhas de marcadores, etc.) são puladas silenciosamente.
///
/// Tolerâncias:
/// - `R$<número>` e `R$ <número>` (espaço opcional após `R$`)
/// - ` - ` e `-` (espaço opcional ao redor do traço)
/// - Valor em pt-BR (`1.234,56`) ou xlsx float (`1234.5600`)
/// - Linhas com marcadores `#reembolso:` / `#dividir:` atrás: parseia o item normalmente
///   (o marcador fica na descrição), independente do que `parse_note_markers` faz.
///
/// SEGURO POR PADRÃO: nota vazia ou sem linhas `R$` → lista vazia.
/// PURA — sem I/O, sem DB, sem panics.
pub(crate) fn parse_itemized_note(note: &str) -> Vec<NoteLineItem> {
    let mut items = Vec::new();
    for (pos, line) in note.lines().enumerate() {
        let trimmed = line.trim();
        // Linhas que começam com R$ (case-insensitive).
        let rest = if trimmed.len() >= 2
            && trimmed[..2].eq_ignore_ascii_case("r$")
        {
            trimmed[2..].trim_start()
        } else {
            continue;
        };
        // Separador ` - ` com espaços opcionais.  Usa o PRIMEIRO traço para permitir
        // descrições como "Produto A - loja B" sem truncar.
        let (value_part, desc_part) = if let Some(idx) = rest.find('-') {
            let before = rest[..idx].trim_end();
            let after = rest[idx + 1..].trim_start();
            (before, after)
        } else {
            // Sem separador → a linha inteira é o valor, sem descrição.
            (rest, "")
        };
        let amount_cents = parse_number(value_part.trim());
        if amount_cents <= 0 {
            continue; // valor inválido ou zero → pula
        }
        items.push(NoteLineItem {
            amount_cents,
            description: desc_part.to_string(),
            position: pos,
        });
    }
    items
}
```

**Important nuance on the separator**: the grammar says `R$ <valor> - <descrição>`.
The `rest.find('-')` approach finds the FIRST `-`. That works for the canonical
`R$ 300,00 - Categoria` but would break `R$ -300,00 - Desc` (negative value in
a note). Since the method stores only positive magnitudes in notes (the cell
holds the signed total), `amount_cents <= 0` will filter out any negative
result. If you need to handle edge cases, trim leading sign from `value_part`
before the separator search.

**Alternative implementation**: if the note format is always `R$ <valor> - <desc>`,
you can also split on `-` (space-dash-space) first and fall back to `-`:

```rust
let sep = if rest.contains(" - ") { " - " } else { "-" };
let (value_part, desc_part) = match rest.split_once(sep) {
    Some((v, d)) => (v.trim(), d.trim()),
    None => (rest.trim(), ""),
};
```

Either implementation is acceptable; the tests (Step 3) are the spec.

**Verify**: `npm run rust:check` → exit 0.

### Step 3: Unit tests for `parse_itemized_note`

Add a `mod itemized_tests` block (or extend the existing `mod tests`) at the
bottom of `import.rs`. **Model after** the existing `parse_note_markers` tests
already in that file (look for `fn test_parse_note_markers` in the test block,
around line 1317+).

Required test cases (names are suggestive — adjust to match your impl):

```rust
// Happy path: standard grammar.
#[test]
fn itemized_standard_form_parses_parts() {
    let note = "R$ 150,00 - Categoria A\nR$ 200,50 - Categoria B";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].amount_cents, 15_000);
    assert_eq!(items[0].description, "Categoria A");
    assert_eq!(items[0].position, 0);
    assert_eq!(items[1].amount_cents, 20_050);
    assert_eq!(items[1].description, "Categoria B");
}

// No space after R$.
#[test]
fn itemized_tolerates_no_space_after_rs() {
    let items = parse_itemized_note("R$300,00 - Item");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].amount_cents, 30_000);
}

// Header line (no R$) is skipped.
#[test]
fn itemized_skips_header_lines() {
    let note = "CONTAS:\nR$ 100,00 - Item A\nTotal = R$ 100,00";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 1, "só a linha R$ do meio é item");
    assert_eq!(items[0].amount_cents, 10_000);
}

// Budget/projection line (tab-separated, no leading R$) is skipped.
#[test]
fn itemized_skips_tab_separated_budget_lines() {
    let note = "Mensal\tR$ 300,00\tCategoria\nR$ 50,00 - Outro item";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].description, "Outro item");
}

// Empty note → no items (safe by default).
#[test]
fn itemized_empty_note_yields_no_items() {
    assert!(parse_itemized_note("").is_empty());
    assert!(parse_itemized_note("   ").is_empty());
}

// Note with only a header → no items.
#[test]
fn itemized_no_rs_lines_yields_no_items() {
    assert!(parse_itemized_note("Descrição geral sem itens").is_empty());
}

// Line with marker suffix: item is parsed, marker stays in description.
// (parse_note_markers handles its own job on the same note independently.)
#[test]
fn itemized_line_with_marker_parses_as_item() {
    let note = "R$ 200,00 - Item X #reembolso:Pessoa Y";
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].amount_cents, 20_000);
    // Description includes the marker suffix — that's acceptable; it's raw.
}

// Sum-match reconciliation example (tested at the DB layer in Step 4, but the
// parse layer must produce correct parts):
#[test]
fn itemized_mismatched_sum_still_parses_individual_amounts() {
    // The reconciliation decision (attach or discard) is in the DB layer, not here.
    // This test confirms parse works regardless.
    let note = "R$ 100,00 - Item A\nR$ 100,00 - Item B"; // sum = 200
    let items = parse_itemized_note(note);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].amount_cents + items[1].amount_cents, 20_000);
}
```

**Verify**: `npm run test:run` → all tests pass, including the new ones.

### Step 4: Persist line items in `import_rows_core`

In `src-tauri/src/google_sheets/import.rs`, inside `import_rows_core` (starting
at line 271), **after the Plan 023 block ends** (around line 506, after the
`// --- fim Plan 023 ---` comment):

Add a new block for Plan 035:

```rust
// --- Plan 035: itemized note → line_item rows ---
// Idempotência: clear-then-reinsert per transaction on every import.
// Tolerância: se a nota não tem linhas R$ ou o somatório não bate com o total
// (dentro de 1 centavo), nenhum item é gravado — só o total da transação fica.
// Nunca altera o total do pai.
{
    let items = parse_itemized_note(&row.raw_note);
    // Reconciliation: Σ parts ≈ parent total (±1 cent for rounding).
    let parts_sum: i64 = items.iter().map(|i| i.amount_cents).sum();
    let parent_total = row.amount.abs();
    let sum_matches = items.len() >= 2
        && (parts_sum - parent_total).abs() <= 1;

    // Always clear old items for this txn (idempotent).
    sqlx::query("DELETE FROM line_item WHERE transaction_id = ?1")
        .bind(&txn_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("clear line_items for {txn_id}: {e}"))?;

    if sum_matches {
        for item in &items {
            let item_id = format!("li:{}:{}", txn_id, item.position);
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET \
                   amount_cents=excluded.amount_cents, \
                   description=excluded.description, \
                   position=excluded.position",
            )
            .bind(&item_id)
            .bind(&txn_id)
            .bind(item.amount_cents)
            .bind(&item.description)
            .bind(item.position as i64)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("insert line_item {item_id}: {e}"))?;
        }
    }
    // If sum doesn't match: no items inserted; parent total untouched. Safe.
}
// --- fim Plan 035 ---
```

**Key constraint**: this block does NOT modify `row.amount` or any field of the
parent `transaction` row. It only inserts into `line_item`. The Saldo chain is
untouched.

**Verify**: `npm run rust:check` → exit 0.

### Step 5: Unit tests for line item persistence (Rust integration tests)

Add `#[tokio::test]` tests inside the existing `mod tests` block at the bottom
of `import.rs`. **Model after** the existing async import tests in that file
(they use `sqlx::sqlite::SqlitePoolOptions::new().connect("sqlite::memory:")`,
then `sqlx::migrate!("./migrations").run(&p)`).

Required test cases:

```rust
// Happy path: note with 2 R$ lines summing to the parent total → items inserted.
#[tokio::test]
async fn line_items_stored_when_note_sums_match_total() {
    // Build an in-memory pool, run migrations, insert a transaction via import,
    // then check SELECT COUNT(*) FROM line_item WHERE transaction_id = <id>.
    // Use a note "R$ 100,00 - Parte A\nR$ 50,00 - Parte B" and amount = 15_000
    // (R$ 150,00). The sum of parts (10_000 + 5_000 = 15_000) equals the total.
    // Expected: 2 line_item rows.
    todo!("implement")
}

// Mismatch: note has parts that don't sum to total → no items, parent total unchanged.
#[tokio::test]
async fn line_items_not_stored_when_sum_mismatches() {
    // Note "R$ 60,00 - A\nR$ 60,00 - B" sums to 12_000 but parent total is 10_000.
    // Expected: 0 line_item rows; transaction.amount = 10_000 (unchanged).
    todo!("implement")
}

// Re-import idempotency: same data → same items (no duplicates).
#[tokio::test]
async fn line_items_are_idempotent_on_reimport() {
    // Import twice with the same rows; check COUNT(*) = 2 (not 4).
    todo!("implement")
}

// Re-import note change: updated note → items updated.
#[tokio::test]
async fn line_items_update_on_note_change() {
    // First import: note with 2 items. Second import: note with 3 items (same txn).
    // Expected: 3 line_item rows after second import.
    todo!("implement")
}

// Safe-by-default: empty note → 0 items, no error.
#[tokio::test]
async fn line_items_empty_note_inserts_none() {
    // Transaction with raw_note = "". Expected: 0 line_item rows.
    todo!("implement")
}
```

Replace each `todo!` with the actual implementation following the pattern in
`splits_for_transaction` tests (lines 86–176 in `splits.rs`).

**Verify**: `npm run test:run` → all pass. `npm run rust:check` → exit 0.

### Step 6: Read-side getter and Tauri command

In `src-tauri/src/commands/transactions.rs`, add:

```rust
/// Uma parte de um lançamento itemizado (do `line_item`).
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct LineItemOnRow {
    pub id: String,
    pub transaction_id: String,
    pub amount_cents: i64,
    pub description: String,
    pub position: i64,
}

/// Retorna as partes itemizadas de um lançamento (vazio = lançamento não itemizado).
pub(crate) async fn line_items_for_transaction(
    pool: &SqlitePool,
    transaction_id: &str,
) -> Result<Vec<LineItemOnRow>, String> {
    sqlx::query_as::<_, LineItemOnRow>(
        "SELECT id, transaction_id, amount_cents, description, position \
         FROM line_item WHERE transaction_id = ?1 ORDER BY position",
    )
    .bind(transaction_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("line_items_for_transaction: {e}"))
}

#[tauri::command]
pub async fn get_line_items_cmd(
    pool: State<'_, SqlitePool>,
    transaction_id: String,
) -> Result<Vec<LineItemOnRow>, String> {
    line_items_for_transaction(pool.inner(), &transaction_id).await
}
```

Also extend `TransactionRow` and the `recent_transactions` function to include
`line_items: Vec<LineItemOnRow>` by fetching them in a batch query (same pattern
as the tag batch at lines 78–109):

1. After the tag batch query, run a second batch query for `line_item` keyed
   by the same `ids: Vec<String>`:
   ```rust
   let li_rows: Vec<LineItemOnRow> = if ids.is_empty() {
       Vec::new()
   } else {
       let placeholders = vec!["?"; ids.len()].join(",");
       let sql = format!(
           "SELECT id, transaction_id, amount_cents, description, position \
            FROM line_item WHERE transaction_id IN ({placeholders}) \
            ORDER BY transaction_id, position"
       );
       let mut q = sqlx::query_as::<_, LineItemOnRow>(sqlx::AssertSqlSafe(sql));
       for id in &ids { q = q.bind(id); }
       q.fetch_all(pool).await.map_err(|e| format!("line_item query: {e}"))?
   };
   let mut items_by_txn: std::collections::HashMap<String, Vec<LineItemOnRow>> =
       std::collections::HashMap::new();
   for li in li_rows {
       items_by_txn.entry(li.transaction_id.clone()).or_default().push(li);
   }
   ```
2. Add `line_items: Vec<LineItemOnRow>` to the `TransactionRow` struct
   definition (after `tags`).
3. Populate it from `items_by_txn` in the `.map(|r| TransactionRow { … })`.

Register the new command in `src-tauri/src/lib.rs` by adding
`get_line_items_cmd` to the `tauri::generate_handler![…]` list. Locate the
handler list (search for `generate_handler!` in `lib.rs`) and add the new
command in alphabetical position.

**Verify**: `npm run rust:check` → exit 0.

### Step 7: Frontend API bindings

In `src/lib/api.ts`:

1. Add the `LineItem` interface (after `TagRef`, around line 47):

   ```ts
   /** Uma parte de um lançamento itemizado (breakdown da nota de célula). */
   export interface LineItem {
     id: string;
     transaction_id: string;
     amount_cents: number;
     description: string;
     position: number;
   }
   ```

2. Add `line_items: LineItem[]` field to `TransactionRow` (after `provenance`):

   ```ts
   /** Partes itemizadas da nota (vazio = lançamento não itemizado). */
   line_items: LineItem[];
   ```

3. Add the getter function (after `getRecentTransactions`):

   ```ts
   export function getLineItems(transactionId: string): Promise<LineItem[]> {
     return invoke<LineItem[]>("get_line_items_cmd", { transactionId });
   }
   ```

4. In `src/test/commands.ts`, add `line_items: []` to every `TransactionRow`
   fixture in `TXNS` (lines 53–93) and `RECURRING_TXN` (line 96–108):
   ```ts
   // Each entry in TXNS:
   {
     id: "t1",
     ...
     line_items: [],   // add this
   },
   ```
   This ensures backward compatibility — no existing test breaks.

**Verify**: `npm run typecheck` → exit 0.

### Step 8: Frontend — `LineItemsRow` component and expand toggle

In `src/screens/TransactionsScreen.tsx`:

#### 8a: Static styles

Add two new module-level style constants (after `EDIT_FORM_WRAP_STYLE`, around
line 65):

```tsx
const LINE_ITEMS_ROW_STYLE: React.CSSProperties = {
  paddingLeft: "var(--space-6)",
  paddingTop: "var(--space-1)",
  paddingBottom: "var(--space-1)",
};

const LINE_ITEM_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-3)",
  alignItems: "baseline",
  fontSize: "var(--text-sm)",
  color: "var(--text-muted)",
};
```

#### 8b: `LineItemsRow` component

Add after `TagEditorRow`:

```tsx
/** Sub-linha de breakdown itemizado de um lançamento (partes da nota de célula). */
function LineItemsRow({ t }: { t: TransactionRow }) {
  if (t.line_items.length === 0) return null;
  return (
    <tr
      className="txn-tag-editor"
      aria-label={`Itens de ${t.description || "lançamento"}`}
    >
      <td colSpan={6}>
        <ul style={LINE_ITEMS_ROW_STYLE} aria-label="Itens do lançamento">
          {t.line_items.map((li) => (
            <li key={li.id} style={LINE_ITEM_STYLE}>
              <Money cents={li.amount_cents} size="sm" sign="auto" />
              <span>{li.description}</span>
            </li>
          ))}
        </ul>
      </td>
    </tr>
  );
}
```

#### 8c: UI state — expand toggle

In `TransactionsUiState` (around line 129), add:

```tsx
expandItemsId: string | null; // qual linha tem o breakdown aberto
```

In `INITIAL_UI_STATE` (around line 156), add:

```tsx
expandItemsId: null,
```

In `TransactionsUiAction` (around line 141), add:

```tsx
| { type: "toggleItems"; id: string }
```

In `transactionsUiReducer` (around line 168), add the case:

```tsx
case "toggleItems":
  return {
    ...state,
    expandItemsId: state.expandItemsId === action.id ? null : action.id,
  };
```

#### 8d: Expand trigger in `LedgerDataRow`

The trigger should be on the description cell: if `t.line_items.length > 0`,
render a small inline `<button>` (or use a chevron icon from lucide-react) that
calls `onToggleItems`. Pass `itemsOpen` and `onToggleItems` as new props.

Minimal change to `LedgerDataRow` — add two props and wire them:

```tsx
function LedgerDataRow({
  t,
  tagEditOpen,
  actionOpen,
  itemsOpen,          // add
  onToggleTagEditor,
  onToggleAction,
  onToggleItems,      // add
}: {
  t: TransactionRow;
  tagEditOpen: boolean;
  actionOpen: boolean;
  itemsOpen: boolean;              // add
  onToggleTagEditor: () => void;
  onToggleAction: () => void;
  onToggleItems: () => void;       // add
}) {
  ...
  // Inside the description <td>, after the existing content and before </td>:
  {t.line_items.length > 0 && (
    <button
      type="button"
      className="txn-tag-btn"
      aria-label={`${itemsOpen ? "Fechar" : "Ver"} itens de ${t.description || "lançamento"}`}
      aria-expanded={itemsOpen}
      onClick={onToggleItems}
    >
      {/* Use ChevronDown/ChevronRight from lucide-react (already imported) or a text toggle */}
      {itemsOpen ? "▾" : "▸"}
    </button>
  )}
```

#### 8e: Wire in the render loop

In the `visible.map(...)` render block (around line 658), update the call:

```tsx
<LedgerDataRow
  t={t}
  tagEditOpen={ui.tagEditId === t.id}
  actionOpen={ui.actionRowId === t.id}
  itemsOpen={ui.expandItemsId === t.id}           // add
  onToggleTagEditor={...}
  onToggleAction={...}
  onToggleItems={() => dispatchUi({ type: "toggleItems", id: t.id })}  // add
/>
{ui.actionRowId === t.id && <ActionPanelRow ... />}
{ui.tagEditId === t.id && <TagEditorRow ... />}
{ui.expandItemsId === t.id && <LineItemsRow t={t} />}  // add
```

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

### Step 9: Frontend tests

In `src/screens/TransactionsScreen.test.tsx`, add a new `describe` block:

```tsx
describe("TransactionsScreen — breakdown itemizado", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("não mostra botão de expand quando não há itens", async () => {
    // TXNS has line_items: [] on all entries → no expand button.
    mockCommands({ get_recent_transactions: TXNS });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });
    // No expand button for the non-itemized transaction.
    expect(
      screen.queryByRole("button", { name: /Ver itens de Despesa demo variável/ }),
    ).not.toBeInTheDocument();
  });

  it("mostra e oculta os itens ao clicar no botão de expand", async () => {
    const user = userEvent.setup();
    const itemizedTxn: TransactionRow = {
      ...TXNS[0]!,
      id: "t-itemized",
      description: "Despesa com itens",
      amount: 15000,
      line_items: [
        {
          id: "li:t-itemized:0",
          transaction_id: "t-itemized",
          amount_cents: 10000,
          description: "Parte A",
          position: 0,
        },
        {
          id: "li:t-itemized:1",
          transaction_id: "t-itemized",
          amount_cents: 5000,
          description: "Parte B",
          position: 1,
        },
      ],
    };
    mockCommands({ get_recent_transactions: [itemizedTxn] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa com itens")).toBeInTheDocument();
    });

    // Items hidden initially.
    expect(screen.queryByText("Parte A")).not.toBeInTheDocument();
    expect(screen.queryByText("Parte B")).not.toBeInTheDocument();

    // Click to expand.
    await user.click(
      screen.getByRole("button", { name: /Ver itens de Despesa com itens/ }),
    );
    expect(screen.getByText("Parte A")).toBeInTheDocument();
    expect(screen.getByText("Parte B")).toBeInTheDocument();

    // Click again to collapse.
    await user.click(
      screen.getByRole("button", { name: /Fechar itens de Despesa com itens/ }),
    );
    expect(screen.queryByText("Parte A")).not.toBeInTheDocument();
  });

  it("mostra itens de lançamentos projetados (futuro)", async () => {
    const projectedItemized: TransactionRow = {
      ...TXNS[2]!, // the projected one
      id: "t-projected-itemized",
      description: "Receita projetada com itens",
      line_items: [
        {
          id: "li:t-projected-itemized:0",
          transaction_id: "t-projected-itemized",
          amount_cents: 200000,
          description: "Parte projetada",
          position: 0,
        },
      ],
    };
    mockCommands({ get_recent_transactions: [projectedItemized] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Receita projetada com itens")).toBeInTheDocument();
    });
    await userEvent.setup().click(
      screen.getByRole("button", {
        name: /Ver itens de Receita projetada com itens/,
      }),
    );
    expect(screen.getByText("Parte projetada")).toBeInTheDocument();
  });
});
```

**Verify**: `npm run test:run` → all pass, including the 3 new describe-block tests.

### Step 10: Full gate

Run:

```
npm run check
```

Expected: exit 0 — typecheck, lint, tests, rust:check, doctor all green.

Then:

```
npm run e2e
```

Expected: exit 0 — visual smoke passes (no layout regressions in existing
screenshots; the new expand button on non-itemized rows is invisible, so
existing snapshots should not drift).

**Verify**: both commands exit 0.

## Test plan

### New Rust unit tests (pure parser — `import.rs`)

Located in `#[cfg(test)] mod tests` of `import.rs`:

- `itemized_standard_form_parses_parts` — happy path, two items
- `itemized_tolerates_no_space_after_rs` — `R$` without space
- `itemized_skips_header_lines` — non-`R$` lines ignored
- `itemized_skips_tab_separated_budget_lines` — tab-separated form skipped
- `itemized_empty_note_yields_no_items` — safe default
- `itemized_no_rs_lines_yields_no_items` — safe default
- `itemized_line_with_marker_parses_as_item` — marker suffix tolerated
- `itemized_mismatched_sum_still_parses_individual_amounts` — parse works regardless

### New Rust integration tests (DB layer — `import.rs`)

- `line_items_stored_when_note_sums_match_total` — items inserted when Σ ≈ total
- `line_items_not_stored_when_sum_mismatches` — no items when Σ ≠ total; parent total unchanged
- `line_items_are_idempotent_on_reimport` — re-import same data → same count
- `line_items_update_on_note_change` — re-import changed note → updated count
- `line_items_empty_note_inserts_none` — empty note → 0 items

Model these after `slots_for_transaction` / `owner_totals_for_month` tests in
`splits.rs` (lines 86–176).

### New TS tests (`TransactionsScreen.test.tsx`)

- `não mostra botão de expand quando não há itens` — no button rendered
- `mostra e oculta os itens ao clicar no botão de expand` — toggle works
- `mostra itens de lançamentos projetados (futuro)` — works for projected rows

**Verification command**: `npm run test:run` → all pass, ≥ 11 new tests.

## Done criteria

All must hold before marking this plan DONE:

- [ ] Migration `20260620000001_line_item.sql` exists and `npm run rust:check`
      runs migrations without error (sqlx checks them in tests).
- [ ] `parse_itemized_note` exists in `import.rs` and is pure (no I/O).
- [ ] `npm run rust:check` exits 0 with no warnings treated as errors.
- [ ] `npm run test:run` exits 0; ≥ 8 new Rust unit tests and ≥ 3 new TS tests
      exist and pass.
- [ ] `TransactionRow` in both Rust (`transactions.rs`) and TS (`api.ts`) carries
      `line_items`.
- [ ] `get_line_items_cmd` is registered in `lib.rs` handler list.
- [ ] `LineItemsRow` renders in `TransactionsScreen.tsx`; expand toggle works.
- [ ] `npm run typecheck` exits 0.
- [ ] `npm run lint` exits 0.
- [ ] `npm run doctor` shows 0 findings.
- [ ] `npm run e2e` exits 0 with no snapshot regressions.
- [ ] `npm run check` exits 0 (full gate).
- [ ] No file outside the in-scope list is modified (`git diff --name-only HEAD`
      shows only in-scope files).
- [ ] `plans/README.md` status row for plan 035 updated to DONE.
- [ ] `transaction.amount` has NOT changed for any existing transaction
      (verified by spot-checking DB or test assertions).

## STOP conditions

Stop and report (do not improvise) if:

- The code excerpts in "Current state" don't match the live files (the
  codebase has drifted since this plan was written). Run the drift-check
  command at the top of the plan.
- `import_rows_core` in `import.rs` doesn't end at the expected location
  (lines 271–569) — the Plan 023 block has moved or been refactored.
- Storing line items would require modifying `transaction.amount` or any field
  of the parent transaction row.
- A step's verification fails twice after a reasonable fix attempt.
- The sum-reconciliation logic is unclear for a specific real note format —
  report the raw note verbatim (no personal data) so the advisor can refine
  the grammar.
- Adding `line_items` to `TransactionRow` causes a type error in files outside
  the in-scope list (means the type is re-exported or used elsewhere).
- `npm run e2e` produces new visual diffs on existing screenshots (investigate
  before proceeding — the change may have inadvertently shifted layout).

## Maintenance notes

- **Plan 036** (write-back) will need to read `line_item` rows to reconstruct
  the cell note for the Sheets write-back. The `id` format `li:<txn_id>:<pos>`
  is load-bearing for that; do not change it without updating plan 036.
- **Re-import note changes**: changing a cell note in Sheets and re-importing
  will clear and reinsert `line_item` for that transaction (because `raw_note`
  enters the checksum and triggers a diff). This is the intended behavior.
- **Grammar expansion**: if the user adopts new note formats (e.g., plain `50 -
Item` without `R$`), the parser tolerations list in `parse_itemized_note`
  can be extended without schema changes.
- **Reviewer focus areas**: (a) the sum-match tolerance (±1 cent) — confirm it
  is tight enough not to silently accept wrong breakdowns; (b) the batch query
  for `line_items` in `recent_transactions` — confirm it doesn't add N+1 for
  the common case of transactions without items; (c) the `ON DELETE CASCADE`
  in the migration — confirm it covers diff-delete of the parent transaction.
- **Deferred**: editing itemized line items from the UI (plan 036). The expand
  UI is read-only in this plan; the "Editar" action in `ActionPanelRow` still
  edits only the parent transaction's description/amount.
