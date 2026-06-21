# Plan 047: Fix delete-orphan cleanup (P1) + line-item reconcile + budget atomicity + credit-lump bound + legacy staleness gate

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
>   src-tauri/src/commands/transactions.rs \
>   src-tauri/src/commands/forecast_cmds.rs \
>   src-tauri/src/commands/write_back_cmds.rs
> ```
>
> If any of those files changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `26ea4c9`, 2026-06-21

## Why this matters

Five bugs leave the database in subtly corrupt states after user actions that look
successful on screen. The most dangerous (P1): deleting an imported row leaves a
`sync_log` entry behind, so the next import silently re-creates the deleted row; if
there is also an open `import_conflict` for that row the write-back is permanently
blocked. The remaining four (P2) cause silent data divergence: editing a row's
amount without clearing its line-items produces a SUM mismatch at write-back time;
saving a daily budget can leave the total and its categories in an inconsistent state
after a crash; `realign_credit_lump` rescans the full transaction history with no
date bound, risking wrong realignment against old purchases; and the legacy apply
path bypasses the staleness re-check entirely, so a stale diff can reach the sheet
unchecked. All five have regression tests as part of this plan.

## Current state

### Files in scope

- `src-tauri/src/commands/transactions.rs` — `delete_transaction_cmd` (lines 524–536) and `update_transaction_cmd` (lines 542–588); test module at line 639.
- `src-tauri/src/commands/forecast_cmds.rs` — `upsert_daily_budget_with_categories_inner` (lines 397–453); test module at line 1211.
- `src-tauri/src/commands/write_back_cmds.rs` — `guard_sheet_unchanged` (lines 184–198); `apply_write_back` (lines 479–511); `realign_credit_lump` (lines 709–733); test module at line 1086.

### Bug 1 (P1): delete_transaction_cmd does not clean up sync_log / import_conflict / derived rows

Current code at `src-tauri/src/commands/transactions.rs:524-536`:

```rust
// transactions.rs:524-536
/// Apaga um lançamento pelo id (plano 043): inclui linhas importadas. A planilha é a fonte da
/// verdade — apagar aqui NÃO apaga da planilha; o próximo import recria a linha. O painel de ações
/// no Livro-razão avisa o usuário disso (notice de "Linha importada").
#[tauri::command]
pub async fn delete_transaction_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let affected = sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1"#)
        .bind(&id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("delete: {e}"))?
        .rows_affected();
    if affected == 0 {
        return Err("lançamento não encontrado".into());
    }
    Ok(())
}
```

Problems:

- `sync_log` has no FK to `transaction`; deleting the `transaction` row leaves `sync_log` intact → next import finds the `sync_log` entry for that row's `entity_id` and upserts the row back (re-creation).
- `import_conflict` has no FK CASCADE (schema at `migrations/20240612000007_advanced_reconciliation.sql`): `transaction_id` column but no `ON DELETE CASCADE` → orphan conflict rows survive → `guard_no_pending_conflicts` will count them → write-back is permanently blocked.
- Derived child transactions (`id LIKE 'derived:%:<parent_id>:%'`) rely on the parent `transaction` FK, which now has `ON DELETE CASCADE` on `line_item` (confirmed in `migrations/20260620000001_line_item.sql`: `REFERENCES "transaction"(id) ON DELETE CASCADE`). However, derived `transaction` rows reference no FK to their parent and are cleaned up only at import time (`import.rs:412-416`). The delete command must clean them explicitly, mirroring the diff-delete path in `import.rs:639-646`.

Reference — how the diff-delete path in import.rs handles this correctly (`src-tauri/src/google_sheets/import.rs:633-659`):

```rust
// import.rs:633-659 (the diff-delete path — the model to follow)
sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
    .bind(&eid)
    .execute(&mut **tx) ...
// Linhas derivadas … NÃO têm linha em sync_log. Limpamos aqui quando o pai é removido.
sqlx::query("DELETE FROM \"transaction\" WHERE id LIKE ?1")
    .bind(format!("derived:%:{eid}:%"))
    .execute(&mut **tx) ...
sqlx::query(
    "DELETE FROM sync_log WHERE entity_id = ?1 AND source_sheet = ?2 AND entity_type = 'transaction'",
) ...
// Conflitos órfãos somem com a transação removida.
sqlx::query("DELETE FROM import_conflict WHERE transaction_id = ?1")
    .bind(&eid) ...
```

The `delete_transaction_cmd` must do the same four operations in a single `sqlx::Transaction`, but without the `source_sheet` filter on `sync_log` (we delete by `entity_id` across all sheets, since a manual delete removes the row everywhere).

Also note: `sync_log` id is deterministic — `format!("log:{txn_id}")` (see `import.rs:605`) — but the DELETE must use `entity_id = ?1` (not `id`) to be robust against any future id format changes.

### Bug 2 (P2): update_transaction_cmd does not clear line_items when amount changes

Current code at `src-tauri/src/commands/transactions.rs:565-587`:

```rust
// transactions.rs:565-587
let affected = sqlx::query(
    r#"UPDATE "transaction"
       SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
           is_fixed = ?6, date = ?7, is_projection = ?8, updated_at = ?9
       WHERE id = ?1"#,
)
...
```

No check whether the transaction has `line_item` children. If it does, the parent `amount` (the new value) will diverge from `SUM(line_item.amount_cents)`. At write-back, `load_txn_items` (called for itemized rows) will produce a SUM cell that no longer matches the row's declared total. Fix: before the UPDATE, check whether the row has line_items **and** whether the new `amount_cents` differs from the current total; if so, clear the `line_item` rows for this transaction (they no longer reflect the breakdown). Clearing is safe and simpler than blocking: the user can re-enter breakdown via the item editor.

### Bug 3 (P2): upsert_daily_budget_with_categories_inner is non-atomic

Current code at `src-tauri/src/commands/forecast_cmds.rs:397-453`:

```rust
// forecast_cmds.rs:409-451
// Passo 1: escreve/depreca o TOTAL pelo mesmo caminho do teto simples (engine inalterado).
upsert_daily_budget_inner(pool, amount_cents).await?;          // ← commits immediately on pool

// Passo 2: só anexa categorias quando há um teto explícito ativo E uma quebra informada.
if amount_cents > 0 && !categories.is_empty() {
    ...
    let mut tx = pool.begin().await ...;                        // ← separate tx
    sqlx::query("DELETE FROM daily_budget_category WHERE budget_id = ?1")...
    for c in categories { INSERT ... }
    tx.commit().await...
}
```

`upsert_daily_budget_inner` (lines 304–340) operates directly on `pool` (not inside a transaction) and commits immediately. The category DELETE + INSERT happens in a separate transaction begun afterward. A crash between the two leaves an active `daily_budget` row with no category rows, or stale category rows from the previous budget. Fix: wrap the entire operation (deprecate old budgets + insert new total + clear + insert categories) inside **one** `sqlx::Transaction` passed through the call chain, or restructure so `upsert_daily_budget_with_categories_inner` opens a single transaction and performs all steps within it.

Because `upsert_daily_budget_inner` is also called standalone (via the `upsert_daily_budget` command, line 349), the cleanest fix is to add a separate `upsert_daily_budget_with_categories_inner` implementation that opens its own transaction and inlines the total-upsert logic, keeping `upsert_daily_budget_inner` untouched for the simpler path. This avoids changing the public API surface of the simple command.

### Bug 4 (P2): realign_credit_lump scans all credit expenses with no date bound

Current code at `src-tauri/src/commands/write_back_cmds.rs:727-733`:

```rust
// write_back_cmds.rs:727-733
let candidates: Vec<(String, String)> = sqlx::query_as(
    "SELECT id, date FROM \"transaction\" \
     WHERE type='expense' AND payment_method='credit'",
)
.fetch_all(&mut **tx)
.await
.map_err(|e| format!("query credit candidates: {e}"))?;
```

No date filter. `realign_credit_lump` is called from `record_write_back_audit` with a `due_date` (the written cell's date, e.g. `"2026-06-05"`). It checks whether `cycle_due_date(purchase_date, closing, due)` equals `due_date`. An old credit purchase from 2+ years ago could produce the same computed due date (same day-of-month pattern) and be wrongly realigned (its `source_amount` set to NULL), causing a spurious no-conflict result at the next import. Fix: bound the SELECT to credit expenses whose date falls within the relevant cycle window. The window is: purchases from the day after the previous closing day up to and including the closing day of the cycle that produces `due_date`. A conservative but correct bound: restrict to `date >= <first day of due_date's year minus 2 months>`, which is always wide enough to include any single-cycle's purchases while excluding purchases from prior years.

### Bug 5 (P2): guard_sheet_unchanged is a no-op when preview_revision is None (legacy apply path)

Current code at `src-tauri/src/commands/write_back_cmds.rs:184-198`:

```rust
// write_back_cmds.rs:184-198
/// Re-verifica a frescura da planilha: compara o `modifiedTime` atual do Drive com o `preview_revision`
/// que o dono viu na prévia. Se AVANÇOU, aborta (Step 4). `preview_revision = None` ⇒ sem checagem
/// (compatibilidade: o frontend só passa o token a partir do PR-B; até lá o gate fica inerte, e o
/// envio real continua atrás de `WRITE_BACK_ENABLED`).
pub(crate) async fn guard_sheet_unchanged(
    client: &SheetsClient,
    spreadsheet_id: &str,
    preview_revision: Option<&str>,
) -> Result<(), String> {
    let Some(seen) = preview_revision.filter(|s| !s.trim().is_empty()) else {
        return Ok(());   // ← no-op when preview_revision is None
    };
    ...
}
```

And the call site in `apply_write_back` (lines 486–511):

```rust
// write_back_cmds.rs:486-511
// Token de frescura devolvido por `preview_write_back_status` (Step 4). `None` no caminho legado
// da UI atual; quando presente, o apply ABORTA se a planilha mudou desde a prévia.
preview_revision: Option<String>,
...
guard_sheet_unchanged(&client, &spreadsheet_id, preview_revision.as_deref()).await?;
```

When the legacy Settings-panel path calls `apply_write_back` without a `preview_revision`, the staleness re-check is skipped entirely. The stale-diff scenario: user previews, sheet changes while they read the preview, then the legacy apply sends the stale diff. Fix: on the `apply` path, if `preview_revision` is `None`, perform an unconditional freshness fetch and compare against the sheet's current `modifiedTime` taken at the start of `apply_write_back` itself (immediately after `build_write_back_plan` is called, which already fetches the sheet). This ensures the legacy path always re-checks staleness without breaking the `preview_revision`-based gate used by the rich UI.

### Conventions to follow

- Functional-core / imperative-shell: pure logic in `_inner` helpers; commands are thin adapters.
- All DB mutations in one `sqlx::Transaction` (begin → operations → commit); commit is the last step; any error rolls back.
- Money = positive-magnitude integer cents.
- Error strings follow the pattern `format!("<operation>: {e}")` already in the file.
- Test helpers: in-memory SQLite via `sqlite::memory:` + `sqlx::migrate!("./migrations")`. See `transactions.rs:643-651` and `write_back_cmds.rs:1090-1098` for the pattern.

## Commands you will need

| Purpose       | Command                                                                          | Expected on success |
| ------------- | -------------------------------------------------------------------------------- | ------------------- |
| Rust check    | `npm run rust:check`                                                             | exit 0, no errors   |
| All tests     | `npm run test:run`                                                               | all pass            |
| Full gate     | `npm run check`                                                                  | exit 0              |
| Rust tests    | `cargo test --manifest-path src-tauri/Cargo.toml 2>&1`                           | all pass            |
| Single module | `cargo test --manifest-path src-tauri/Cargo.toml -- commands::transactions 2>&1` | pass                |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/transactions.rs`
- `src-tauri/src/commands/forecast_cmds.rs`
- `src-tauri/src/commands/write_back_cmds.rs`

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/import.rs` — the diff-delete path there is already correct; do not change it.
- Any migration file — no schema changes are needed; all bugs are logic-only.
- Any 028-era gate (`guard_no_pending_conflicts`, `write_back::ensure_write_back_enabled`, `ensure_write_scope`) — do NOT weaken or remove any of these.
- Frontend TypeScript — the `preview_revision` field is already threaded through the frontend for the rich UI path; the legacy path passes `None` at the Rust boundary, which this plan fixes on the Rust side only.
- `plans/README.md` — update the status row when done (executor responsibility).

## Git workflow

- Branch: `advisor/047-delete-cleanup-and-correctness`
- Commit style: match the repo's conventional-commits pattern, e.g.:
  `fix: delete-orphan cleanup + line-item reconcile + budget atomicity + credit-lump bound + legacy staleness (plano 047)`
- One commit per logical fix is acceptable; a single commit for all five is also fine.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Fix delete_transaction_cmd — wrap in transaction, clean sync_log + import_conflict + derived rows

In `src-tauri/src/commands/transactions.rs`, replace the body of `delete_transaction_cmd` (currently lines 525–535) with a single `sqlx::Transaction` that:

1. Opens a transaction via `pool.inner().begin()`.
2. DELETEs from `"transaction"` by `id`.
3. If `rows_affected() == 0`, returns `Err("lançamento não encontrado")` (the transaction drops on return, rolling back).
4. DELETEs derived child rows: `DELETE FROM "transaction" WHERE id LIKE 'derived:%:' || ?1 || ':%'` — mirrors the import diff-delete path in `import.rs:642-646`.
5. DELETEs `sync_log` entries: `DELETE FROM sync_log WHERE entity_id = ?1 AND entity_type = 'transaction'` (no `source_sheet` filter — the manual delete removes the import record across all sheets).
6. DELETEs `import_conflict` entries: `DELETE FROM import_conflict WHERE transaction_id = ?1`.
7. Commits the transaction.

The `line_item` rows for the parent are cleaned automatically via `ON DELETE CASCADE` (confirmed in `migrations/20260620000001_line_item.sql`). Do not add a redundant DELETE for those.

Keep the doc-comment above the function — update it to reflect that the command now also cleans up sync metadata to prevent ghost re-creation on the next import.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml -- commands::transactions::tests::delete 2>&1` → new test(s) pass (see Step 6 for what to write). Until Step 6 exists, verify via `npm run rust:check` → exit 0.

### Step 2: Fix update_transaction_cmd — clear line_items when amount changes on an itemized row

In `src-tauri/src/commands/transactions.rs`, inside `update_transaction_cmd` (currently lines 542–588), before the UPDATE statement:

1. Query the current `amount` and whether the row has any `line_item` children:
   ```sql
   SELECT t.amount, COUNT(li.id) AS item_count
   FROM "transaction" t
   LEFT JOIN line_item li ON li.transaction_id = t.id
   WHERE t.id = ?1
   GROUP BY t.amount
   ```
2. If `item_count > 0` and the new `amount_cents != current amount`, run:
   ```sql
   DELETE FROM line_item WHERE transaction_id = ?1
   ```
   before the UPDATE. This clears the stale breakdown; the user can re-enter items via the item editor.

The UPDATE itself is unchanged. No transaction wrapping is needed for this: the DELETE + UPDATE can share the connection pool without a formal transaction because the DELETE is conditional and idempotent (worst case: a crash between the two leaves no line_items and the transaction with the new amount — which is valid, just un-itemized).

> Note: if you prefer a formal transaction for hygiene, that is also acceptable — keep it consistent with the existing style in the file.

**Verify**: `npm run rust:check` → exit 0.

### Step 3: Fix upsert_daily_budget_with_categories_inner — single atomic transaction

In `src-tauri/src/commands/forecast_cmds.rs`, rewrite `upsert_daily_budget_with_categories_inner` (currently lines 397–453) so that the total-upsert and category upsert happen inside **one** `sqlx::Transaction`:

1. Validate categories first (the existing `for c in categories { if c.amount_cents <= 0 { return Err(...) } }` loop stays at the top).
2. Obtain `person_id` from `person` table (same as `upsert_daily_budget_inner` does).
3. Open a single transaction via `pool.begin()`.
4. Inside the transaction: `UPDATE daily_budget SET status='deprecated' WHERE status='active'`.
5. If `amount_cents > 0`: `INSERT INTO daily_budget (id, person_id, amount, start_date, status) VALUES (...)` with a new UUID.
6. If `amount_cents > 0` and `!categories.is_empty()`: look up the budget id just inserted (use the known UUID, not a separate SELECT), then `DELETE FROM daily_budget_category WHERE budget_id = ?1` and re-insert categories.
7. Commit.

Do NOT change `upsert_daily_budget_inner` itself — it is used by the simpler `upsert_daily_budget` command (line 349) and is correct for that single-operation path.

**Verify**: `npm run rust:check` → exit 0.

### Step 4: Fix realign_credit_lump — add a date bound to the credit-candidate SELECT

In `src-tauri/src/commands/write_back_cmds.rs`, in the `realign_credit_lump` function (currently lines 709–733), add a date lower-bound to the SQL at lines 727–729:

Replace:

```sql
SELECT id, date FROM "transaction"
WHERE type='expense' AND payment_method='credit'
```

With (bind the computed cutoff as a placeholder):

```sql
SELECT id, date FROM "transaction"
WHERE type='expense' AND payment_method='credit'
  AND date >= ?2
```

The cutoff is the first day of the year that is two years before the `due_date` year. Compute it from `due_date`:

```rust
let cutoff = {
    let due = NaiveDate::parse_from_str(due_date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Local::now().date_naive());
    // Keep the current year and the prior year — any single credit cycle spans at most
    // ~2 months, so going back 2 full years is a conservative safe window.
    NaiveDate::from_ymd_opt(due.year() - 2, 1, 1)
        .unwrap_or(due)
        .format("%Y-%m-%d")
        .to_string()
};
```

Bind `&cutoff` as `?2` in the query.

**Verify**: `npm run rust:check` → exit 0.

### Step 5: Fix guard_sheet_unchanged / apply_write_back — unconditional staleness on legacy apply path

In `src-tauri/src/commands/write_back_cmds.rs`, inside `apply_write_back` (lines 479–511):

After `build_write_back_plan` returns `(client, plan)` (line 500–508), the `client` already holds a `SheetsClient` that can call `get_file_modified_time`. Change the staleness check to always run:

1. Fetch the current `modifiedTime` from the Drive: `let current_revision = client.get_file_modified_time(&spreadsheet_id).await?;`
2. If `preview_revision` is `Some(seen)` and non-empty: run `staleness_check(seen, &current_revision)?` (exact existing function at line 203).
3. If `preview_revision` is `None` or empty: the legacy path has no "seen" token. To prevent a stale apply, take a fresh snapshot at the **start** of `apply_write_back` (before `build_write_back_plan`) and compare it against the snapshot taken after the plan is built. If they differ, abort. This closes the TOCTOU window even for the legacy path.

Concretely, restructure the top of `apply_write_back` to:

```rust
// Snapshot BEFORE reading sheet values (closes TOCTOU for legacy path).
let early_revision = {
    let early_client = /* build a read-only SheetsClient using the same credentials */;
    early_client.get_file_modified_time(&spreadsheet_id).await?
};
// ... existing gates (guard_no_pending_conflicts, ensure_write_scope) ...
let (client, plan) = build_write_back_plan(...).await?;
// Snapshot AFTER reading sheet values.
let post_plan_revision = client.get_file_modified_time(&spreadsheet_id).await?;

// Staleness gate: either the preview_revision token (rich UI) or the early/post comparison (legacy).
if let Some(seen) = preview_revision.as_deref().filter(|s| !s.is_empty()) {
    staleness_check(seen, &post_plan_revision)?;
} else {
    staleness_check(&early_revision, &post_plan_revision)?;
}
```

Look at `preview_write_back_status` (line 320) for the pattern of building an `early_client` before reading sheet values — mirror that pattern.

Do NOT change `guard_sheet_unchanged` itself (it is used by other paths including `apply_economia_write_back`; keep its `None`-means-skip semantics). The fix is local to `apply_write_back`.

**Verify**: `npm run rust:check` → exit 0.

### Step 6: Write regression tests — one per bug fix

#### 6a. Bug 1 — delete cleans up sync_log, import_conflict, derived rows

In `src-tauri/src/commands/transactions.rs`, add to the `#[cfg(test)] mod tests` block (after line 639):

Test name: `delete_imported_row_cleans_sync_log_conflict_and_derived`.

Setup:

1. Insert a `profile` row (required by `sync_log.profile_id` FK).
2. Insert a `transaction` row (id = `"tx-1"`).
3. Insert a `sync_log` row with `entity_id = "tx-1"`, `entity_type = 'transaction'`, `source_sheet = '2026'`, `profile_id = <the profile id>`.
4. Insert an `import_conflict` row with `transaction_id = "tx-1"`, `field = 'amount'`, `base_value = '100'`, `local_value = '200'`, `sheet_value = '300'`.
5. Insert a derived child transaction (id = `"derived:reembolso:tx-1:0"`) with any valid row data.

Action: call the inner delete logic (or construct a local helper that replicates `delete_transaction_cmd` without the `State` wrapper, same pattern as `run_update_items` at line 675).

Assertions:

- The `transaction` row `"tx-1"` no longer exists.
- The derived row `"derived:reembolso:tx-1:0"` no longer exists.
- `SELECT COUNT(*) FROM sync_log WHERE entity_id = 'tx-1'` = 0.
- `SELECT COUNT(*) FROM import_conflict WHERE transaction_id = 'tx-1'` = 0.

#### 6b. Bug 2 — update clears line_items when amount changes

Test name: `update_amount_clears_stale_line_items`.

Setup: insert a `transaction` (id `"tx-2"`, amount = 5000), insert two `line_item` rows totaling 5000. Call the inner update logic with `amount_cents = 8000` (different).

Assertions:

- `SELECT amount FROM "transaction" WHERE id = 'tx-2'` = 8000.
- `SELECT COUNT(*) FROM line_item WHERE transaction_id = 'tx-2'` = 0.

Also test that updating amount when there are **no** line_items (or when amount is unchanged) does not fail.

#### 6c. Bug 3 — budget categories are atomic

In `src-tauri/src/commands/forecast_cmds.rs`, add to the `#[cfg(test)] mod tests` block (after line 1211):

Test name: `upsert_daily_budget_with_categories_is_atomic`.

Setup: insert a `person` row (required by `daily_budget.person_id` FK, via `upsert_daily_budget_inner`).

Action: call `upsert_daily_budget_with_categories_inner(pool, 10000, &[cat_a, cat_b])`.

Assertions:

- `SELECT COUNT(*) FROM daily_budget WHERE status='active'` = 1.
- `SELECT COUNT(*) FROM daily_budget_category` = 2.
- No deprecated row has any category rows (join check).

Then call again with `amount_cents = 0` (deactivate):

- `SELECT COUNT(*) FROM daily_budget WHERE status='active'` = 0.
- `SELECT COUNT(*) FROM daily_budget_category` = 0 (categories of the now-deprecated budget are gone or the deprecated budget has no active categories — acceptable either way; document the assertion chosen).

#### 6d. Bug 4 — realign_credit_lump ignores old purchases

In `src-tauri/src/commands/write_back_cmds.rs`, add to the `#[cfg(test)] mod tests` block (after line 1086):

Test name: `realign_credit_lump_ignores_purchases_from_prior_years`.

Setup: same card config as the existing `credit_lump_writeback_realigns_source_amount` test (closing day 25, due day 5). Insert:

- A recent purchase on `2026-05-20` (cycle due `2026-06-05`) with `source_amount = 3000`.
- An old purchase on `2023-05-20` (cycle due `2023-06-05` — same day-of-month pattern) with `source_amount = 7000`.

Action: call `record_write_back_audit` (or `realign_credit_lump` directly inside a transaction) for `due_date = "2026-06-05"`.

Assertions:

- Realigned count = 1 (only the 2026 purchase).
- The 2026 purchase has `source_amount = NULL`.
- The 2023 purchase still has `source_amount = 7000` (untouched).

#### 6e. Bug 5 — legacy apply path rejects stale diff

Because `apply_write_back` depends on `SheetsClient` (network IO), write a unit test for the pure staleness logic only:

Test name: `staleness_check_rejects_different_revision`.

In `write_back_cmds.rs` tests:

- `staleness_check("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z")` → `Ok(())`.
- `staleness_check("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z")` → `Err(...)`.

Both cases should already work (the function exists at line 203); verify the test exists or add it as a named test.

**Verify all tests**: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1` → all pass, including the 5 new tests named above.

## Test plan

- File `src-tauri/src/commands/transactions.rs` — add 2 tests: `delete_imported_row_cleans_sync_log_conflict_and_derived`, `update_amount_clears_stale_line_items`.
- File `src-tauri/src/commands/forecast_cmds.rs` — add 1 test: `upsert_daily_budget_with_categories_is_atomic`.
- File `src-tauri/src/commands/write_back_cmds.rs` — add 2 tests: `realign_credit_lump_ignores_purchases_from_prior_years`, `staleness_check_rejects_different_revision`.

Structural pattern for helpers: `async fn test_pool() -> SqlitePool` via `sqlite::memory:` + `sqlx::migrate!("./migrations")` — match `transactions.rs:643-651` exactly. `#[tokio::test]` on each test function.

**Final verification**: `cargo test --manifest-path src-tauri/Cargo.toml 2>&1` → all pass, including the 5 new tests.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 with no errors.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml 2>&1` exits 0; the 5 new tests (`delete_imported_row_cleans_sync_log_conflict_and_derived`, `update_amount_clears_stale_line_items`, `upsert_daily_budget_with_categories_is_atomic`, `realign_credit_lump_ignores_purchases_from_prior_years`, `staleness_check_rejects_different_revision`) exist and pass.
- [ ] `npm run check` exits 0 (full gate: typecheck + lint + test:run + rust:check + privacy:scan).
- [ ] No files outside the in-scope list are modified (`git status` shows only the three `.rs` files and `plans/README.md`).
- [ ] The 028 gates (`guard_no_pending_conflicts`, `ensure_write_back_enabled`, `ensure_write_scope`) are unchanged.
- [ ] `plans/README.md` status row updated to DONE.

## STOP conditions

Stop and report back (do not improvise) if:

- The code at any "Current state" excerpt doesn't match the live file (the codebase drifted since this plan was written — run the drift check at the top first).
- `cargo test` passes but `npm run check` fails on a frontend check unrelated to this plan — report, do not fix unrelated failures.
- Fixing Bug 5 (Step 5) requires changing `guard_sheet_unchanged` in a way that alters the `apply_economia_write_back` path — stop and report; the fix must be local to `apply_write_back`.
- The `build_write_back_plan` function does not expose the `SheetsClient` for the early-revision snapshot (i.e. its return type changed) — report the actual signature and wait for guidance.
- Any step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching a migration file, a frontend file, or any file not in the in-scope list.

## Maintenance notes

- **Bug 1 fix**: the `delete_transaction_cmd` doc-comment currently says "O próximo import recria a linha" — update it to be accurate post-fix: the sync_log entry is now removed, so the next import will NOT re-create the row (the sheet remains the source of truth, but the delete is now sticky).
- **Bug 3 fix**: if a future plan adds a `person_id` selector to `upsert_daily_budget_with_categories_inner` (multi-user support), the person fetch and transaction wrapping should be revised together.
- **Bug 4 fix**: the 2-year cutoff is conservative. If a future plan introduces multi-year write-back in a single session (e.g. backfilling years), revisit and derive the cutoff from the actual year being written rather than a fixed offset.
- **Bug 5 fix**: once the legacy Settings-panel path is removed and all apply calls go through the rich UI (with a `preview_revision` token), the `early_revision` snapshot in `apply_write_back` becomes redundant. Remove it at that point.
- **Reviewer focus**: the single-transaction wrapping in Bugs 1 and 3 — confirm no nested transaction (`begin` inside an already-open `begin`) is accidentally introduced if either function is called from a caller that already holds a transaction.
