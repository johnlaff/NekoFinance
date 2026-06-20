# Plan 019: SPIKE: first-class invoice (credit-bill) entity

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **This is a DESIGN/SPIKE plan.** The deliverable is a schema sketch, a set
> of design decisions with rationale, an open-questions list, and a small
> validation prototype. No production feature is shipped. The executor's job
> is to read, reason, prototype (in tests and migration sketches), and
> produce a written design record. Do NOT implement the full invoice UI or
> pipeline; that is a future implementation plan that will cite this spike.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands.rs src-tauri/src/splits.rs src-tauri/migrations/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: — (spike, no priority ranking)
- **Effort**: spike
- **Risk**: MED
- **Depends on**: plans/004 (importer must carry `owner_person_id` on splits
  and `payment_method='credit'` on transactions before a first-class invoice
  entity can be linked to them)
- **Category**: direction
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The app currently tracks credit card activity in two disconnected ways: (1)
`daily_checkin.credit_spend` accumulates per day (ADR-0001 / Régua 2), and (2)
`forecast::classify()` collapses any `payment_method='credit'` expense into a
`FixedOut` lump on the account's `due_day` (for the projected balance).
Neither representation gives the user a day-by-day "velocímetro" of the
accumulating bill, shows whose items are whose on a joint card, or links a
third-party reimbursement (an Entrada on the due date that cancels a companion's
share) to the bill that generated it.

The method's reference behavior requires this: credit spending does not touch
the daily track (Régua 1) but it is not invisible — the bill accumulates item
by item and lands as a Saída lump on the due date. The spec 008 "Módulo Crédito"
design validated this as a central requirement. Without a first-class `invoice`
entity the app can never show:

- the running total of the open bill (what is owed right now);
- a per-item breakdown with per-owner attribution;
- a reimbursement link (net-zero Entrada at due date for a companion's share);
- the collapse to a single Saída lump on write-back (ADR-0003).

This spike resolves the open design questions (schema, engine interaction,
collapse contract, owner-split / reimbursement model) so that the
implementation plan that follows can build on settled decisions.

## Current state

### Schema facts (verified against live migrations)

**`account` table** (`src-tauri/migrations/20240608000003_account.sql`):

```sql
CREATE TABLE IF NOT EXISTS account (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('bank','credit_card','wallet','savings','business')),
    owner_person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    institution TEXT,
    balance INTEGER NOT NULL DEFAULT 0,
    credit_limit INTEGER,
    closing_day INTEGER,
    due_day INTEGER,
    linked_account_id TEXT REFERENCES account(id),
    provider TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

After migration `20240612000001_account_liquidity.sql`, the account type check
is extended to include `'meal_voucher'`, `'pension'`, `'fgts'`, and a `liquidity`
column (`'liquid'`, `'reserve'`, `'restricted'`, `'illiquid'`) is added.
`closing_day` and `due_day` are already present per-account; `linked_account_id`
identifies a companion card (additional card pointing to the primary).

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
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

`payment_method='credit'` is the marker for credit items. There is no
`invoice_id` column today.

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

No `reimbursed_by_transaction_id` column today.

**`daily_checkin` table** (`src-tauri/migrations/20240608000010_daily_checkin.sql`):

```sql
CREATE TABLE IF NOT EXISTS daily_checkin (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    daily_spend INTEGER NOT NULL DEFAULT 0,
    credit_spend INTEGER NOT NULL DEFAULT 0,
    daily_budget_id TEXT REFERENCES daily_budget(id),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

`credit_spend` is how Régua 2 is tracked today — a daily aggregate, not
linked to any invoice.

### Engine facts (verified against live source)

**`forecast::classify()`** (`src-tauri/src/forecast/mod.rs:242–266`):

```rust
pub fn classify(
    txn_type: &str,
    is_fixed: bool,
    payment_method: Option<&str>,
    to_liquidity: Option<&str>,
) -> Option<EventKind> {
    match txn_type {
        "income" => Some(EventKind::Income),
        "expense" => {
            if is_fixed || payment_method == Some("credit") {
                Some(EventKind::FixedOut)
            } else {
                Some(EventKind::Daily)
            }
        }
        "transfer" => match to_liquidity {
            Some("reserve") | Some("illiquid") => Some(EventKind::Economia),
            _ => None,
        },
        _ => None,
    }
}
```

Credit expenses already route to `FixedOut`. The lump is assembled in
`commands.rs:779–794` where `credit_by_due` aggregates `daily_checkin.credit_spend`
by `cycle_due_date(checkin_date, closing_day, due_day)` and emits a single
`FixedOut` event per due date.

**`cycle_due_date()`** (`src-tauri/src/forecast/mod.rs:281–306`):

```rust
pub fn cycle_due_date(checkin_date: NaiveDate, closing_day: u32, due_day: u32) -> NaiveDate {
    // ... cycles close on closing_day of current/prev month;
    // due date is in the month after the cycle closes.
}
```

The cycle model already exists — it is used only for daily_checkin aggregation
today, not for a first-class entity.

**CONTEXT.md domain vocabulary** (mandatory for naming new types):

- `Transaction.payment_method`: `credit` = "pay later / fatura"; distinct from
  `debit` / `pix` / `cash` (Régua 1). Credit feeds Régua 2.
- `Split.owner_person_id`: whose expense this is. `linked_account_id` on account
  identifies companion cards. The companion card's `owner_person_id` on account
  is the third party.
- `EventKind::FixedOut`: the bucket that receives the fatura lump on the due date.
  Credit items collapse here; they must NOT double-count as Daily.
- `daily_checkin.credit_spend`: today's rolling Régua 2 aggregate. A first-class
  invoice replaces this as the underlying source; the daily_checkin view is derived.
- Write-back contract (ADR-0003): every material write to the spreadsheet must
  collapse rich structure into the spreadsheet's canonical shape — the invoice
  collapses to a single Saída lump on the due date plus a structured note.
- Human approval gate: always required before any sheet write.

### Dual-tracking contract (ADR-0001)

Two parallel metrics per check-in:

1. `daily_spend` (Régua 1): sum of debit/PIX/cash expenses.
2. `credit_spend` (Régua 2): sum of credit card expenses per day.

A first-class invoice must feed Régua 2 (credit_spend on the item's date) and
collapse to a FixedOut lump at the due date for the forecast — never leaking
into Régua 1 or Daily.

### Spec 008 "Módulo Crédito" reference

`specs/008-auto-import/spec.md` (deferred, design-approved) defines the invoice
entity at lines 535–569 under "Modelo de dados":

- `invoice`: account_id, cycle, closing/due date, status (open/closed/paid/
  partially_paid), total, residual balance.
- `transaction.invoice_id`: links an item to its invoice.
- `split.reimbursed_by_transaction_id`: links a companion's split to the Entrada
  that reimburses it.
- `counterparty_balance`: view/entity tracking residual owed by third party.

This spike is the design gate before implementing those tables. The spec 008
data model is the starting point; this spike validates and tightens it.

### Last migration timestamp convention

All migration files follow the pattern `YYYYMMDDHHMMSS_slug.sql`. The last
migration is `20240612000010_drop_unused_fts.sql`. New migrations for the
invoice entity must use a timestamp strictly after `20240612000010`.

## Commands you will need

| Purpose                       | Command                                                                | Expected on success |
| ----------------------------- | ---------------------------------------------------------------------- | ------------------- |
| Rust typecheck + clippy + fmt | `npm run rust:check`                                                   | exit 0, no warnings |
| Rust unit tests only          | `cargo test --manifest-path src-tauri/Cargo.toml --locked`             | all pass            |
| Full gate                     | `npm run check`                                                        | exit 0              |
| Run a single test             | `cargo test --manifest-path src-tauri/Cargo.toml --locked <test_name>` | 1 passed            |
| Schema inspection (in test)   | use `sqlx::query_as("PRAGMA table_info(?)")` inside a test pool        | returns column rows |

> `npm run rust:check` runs `cargo fmt --check`, `cargo clippy -- -D warnings`,
> and `cargo test`. If `cargo fmt --check` fails, run
> `cargo fmt --manifest-path src-tauri/Cargo.toml` first.

## Suggested executor toolkit

- Read `specs/008-auto-import/spec.md` in full before starting (especially
  §Módulo Crédito, §Modelo de dados, EC6–EC7, EC12, EC15) — this spike
  crystallises design that spec 008 left open.
- Read `docs/adr/0001-dual-tracking-daily-credit.md` and
  `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md` in full —
  every design decision in this spike must be consistent with those ADRs.
- Read `src-tauri/src/forecast/mod.rs` in full — the engine must keep working
  with a first-class invoice; no engine signature changes in this spike.

## Scope

**In scope** (the only files to create or modify):

- `src-tauri/migrations/<new_timestamp>_invoice.sql` — migration sketch (new
  file, created in this spike as a validated draft, not yet wired into the
  app's migration runner flow — see Step 3 note).
- `src-tauri/src/forecast/mod.rs` — add one pure helper function for cycle
  membership (Step 4); no changes to existing public API.
- `specs/019-invoice-entity/spike.md` — written design record: final schema
  decisions, open questions resolved, reimbursement model, collapse contract,
  follow-up tasks (Step 5).

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/commands.rs` — no new Tauri commands in this spike; the
  `credit_by_due` aggregation stays as-is until the implementation plan.
- `src-tauri/src/google_sheets/import.rs` — plan 004 owns the importer changes;
  this spike must not touch import logic.
- Any frontend (`.ts`, `.tsx`, `.css`) file — no UI in this spike.
- `src-tauri/src/splits.rs` — read-only reference; not modified.
- `daily_checkin` table — no schema changes to this table; the spike will
  decide how it relates to the invoice entity but will not alter the table.
- `specs/008-auto-import/` — read-only reference; do not edit.

## Git workflow

- Branch: `advisor/019-spike-invoice-entity`
- One commit for the migration sketch + forecast helper (Steps 1–4), one commit
  for the spike design record (Step 5).
- Commit message style: `docs: spike — first-class invoice entity design`
  (conventional commits, lower-case).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Read and map the existing invoice-related surface

**What to do**: Before writing any code, read the following files in full and
build a mental map of where the invoice concept currently lives:

1. `src-tauri/src/forecast/mod.rs` — note the `cycle_due_date` function
   (lines 281–306) and the `EventKind::FixedOut` branch in `classify`
   (lines 248–256). These are the engine's current handles on credit; the
   spike must keep them intact.
2. `src-tauri/src/commands.rs` — search for `credit_by_due` (around line 780)
   and `closing_day`/`due_day` (around lines 741–795). Note how the current
   approach aggregates `daily_checkin.credit_spend` by due date to emit a
   FixedOut event. This aggregation will eventually be replaced by reading from
   a first-class `invoice` table, but that replacement is NOT in this spike.
3. `specs/008-auto-import/spec.md` §Modelo de dados (lines 535–569) — the
   proposed schema entities from spec 008.

Document (in comments or in the spike.md) which current fields and functions
will be touched by the implementation plan. This map is the input to Steps 2–4.

**Verify**: no files modified yet.
`git diff --name-only HEAD` → no output (clean working tree).

---

### Step 2: Draft and validate the proposed `invoice` schema

**What to do**: Design the migration SQL for the `invoice` entity. The schema
must satisfy all of the following requirements:

**R1 — Cycle**: an invoice belongs to one `credit_card` account (`account_id`),
has a cycle identified by its `closing_date` (TEXT `YYYY-MM-DD`) and
`due_date` (TEXT `YYYY-MM-DD`). Closing and due dates are derived from
`account.closing_day` and `account.due_day` at creation time and stored
explicitly so that account-level changes don't silently shift historical cycles.

**R2 — Status lifecycle**: `status TEXT CHECK(status IN
('open','closed','paid','partially_paid'))`. Default `'open'`.

**R3 — Financial totals** (all integer cents, positive magnitude):

- `total_cents INTEGER NOT NULL DEFAULT 0` — sum of all item amounts in the
  invoice.
- `paid_cents INTEGER NOT NULL DEFAULT 0` — amount actually paid so far
  (supports partial payment, EC6 in spec 008).
- A residual is always `total_cents - paid_cents`; do not store it separately
  (avoid derived-column drift).

**R4 — Item link**: `transaction.invoice_id TEXT REFERENCES invoice(id)` — a
new nullable column on `transaction`; each credit expense item is linked to
exactly one open invoice.

**R5 — Owner-split / reimbursement**:

- `split.owner_person_id` (already exists) marks whose expense each split is.
- Add `split.reimbursed_by_transaction_id TEXT REFERENCES "transaction"(id)` —
  links the third-party's portion of a split to the Entrada that reimburses it.
  This is nullable; it is set when the reimbursement Entrada is created/matched.

**R6 — Counterparty balance view** (not a stored table): at query time, derive
the outstanding balance for each third-party (`owner_person_id` on a split whose
`reimbursed_by_transaction_id IS NULL`): `SUM(split.amount) - SUM(reimbursed
transaction amounts)`. This is a view or a query, not a stored table, to avoid
derived-data drift. See open question OQ-3 below.

**Migration sketch to write** (file:
`src-tauri/migrations/<timestamp>_invoice.sql` — use timestamp
`20240613000001`):

```sql
-- Spike 019: first-class invoice (credit-bill) entity.
-- This migration is a VALIDATED DRAFT from the spike.
-- It must be reviewed and sequenced by the implementation plan
-- before being run in production.

-- 1. Invoice cycle entity.
CREATE TABLE IF NOT EXISTS invoice (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    closing_date TEXT NOT NULL,  -- YYYY-MM-DD; cycle closes on this day
    due_date TEXT NOT NULL,       -- YYYY-MM-DD; payment is due on this day
    status TEXT NOT NULL DEFAULT 'open'
        CHECK(status IN ('open','closed','paid','partially_paid')),
    total_cents INTEGER NOT NULL DEFAULT 0,  -- sum of item amounts
    paid_cents INTEGER NOT NULL DEFAULT 0,   -- amount paid so far
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_invoice_account_due
    ON invoice(account_id, due_date);
CREATE INDEX IF NOT EXISTS idx_invoice_status
    ON invoice(status);

-- 2. Link credit expense transactions to their invoice.
--    NULL = not yet linked (pre-invoice items, or non-credit transactions).
ALTER TABLE "transaction" ADD COLUMN invoice_id TEXT REFERENCES invoice(id);
CREATE INDEX IF NOT EXISTS idx_transaction_invoice
    ON "transaction"(invoice_id);

-- 3. Reimbursement link on split: whose portion this split is,
--    and which Entrada transaction reimburses it (nullable).
ALTER TABLE split ADD COLUMN
    reimbursed_by_transaction_id TEXT REFERENCES "transaction"(id);
```

Write this SQL to the migration file. Do NOT run it against the production DB
yet (the implementation plan will sequence it after plan 004 and a full
integration test pass).

**Design decisions to document** (write these into `specs/019-invoice-entity/spike.md`
in Step 5):

- **D1**: `closing_date` and `due_date` are stored on the invoice (not computed
  from account fields at query time) so historical invoices are stable if the
  account's `closing_day`/`due_day` changes.
- **D2**: `paid_cents` is stored; residual is derived. Avoids a trigger or a
  view that could drift.
- **D3**: No separate `invoice_item` table — each credit `transaction` row IS
  the item (linked via `invoice_id`). This avoids duplication and keeps the
  existing transaction ledger as the single source of item truth.
- **D4**: `reimbursed_by_transaction_id` on `split` (not on `transaction`) —
  the reimbursement link is per-portion (a split row), not per-transaction,
  because a single transaction can have multiple owners where only one is a
  third party.
- **D5**: No `counterparty_balance` stored table; it is a derived query.

**Verify**: the migration file exists and is valid SQL.
`sqlite3 :memory: < src-tauri/migrations/20240613000001_invoice.sql` → no error
(exit 0). If `sqlite3` is not installed, skip and note in the spike.md.

---

### Step 3: Write a test that applies the migration and inspects the schema

**What to do**: In `src-tauri/src/forecast/mod.rs` test block, or better in a
new test file `src-tauri/src/invoice_spike_test.rs` (added to `lib.rs` under
`#[cfg(test)]`), write a Rust integration test that:

1. Creates an in-memory SQLite pool using the same `test_pool()` helper from
   `src-tauri/src/google_sheets/import.rs` — copy the pattern (use `sqlx`
   migrations applied in sequence).
2. Inserts a `credit_card` account with `closing_day = 5` and `due_day = 15`.
3. Inserts a credit expense `transaction` (type=`expense`, payment_method=`credit`,
   amount=`10000`).
4. Inserts an `invoice` row for this account, sets `invoice_id` on the transaction.
5. Inserts a `split` row with `owner_person_id` = a third party's person id.
6. Inserts a reimbursement `transaction` (type=`income`) and sets
   `split.reimbursed_by_transaction_id` to it.
7. Queries the derived counterparty balance:
   ```sql
   SELECT SUM(s.amount) - COALESCE(SUM(rt.amount), 0) AS balance_cents
   FROM split s
   LEFT JOIN "transaction" rt ON rt.id = s.reimbursed_by_transaction_id
   WHERE s.transaction_id IN (
       SELECT id FROM "transaction" WHERE invoice_id = ?1
   )
   AND s.owner_person_id = ?2
   ```
   Assert the result matches `10000 - 0 = 10000` (unreimbursed), then update
   `reimbursed_by_transaction_id` and assert the balance becomes `0`.

**Important**: the test pool in `import.rs` (`test_pool()`) applies all
migrations via `sqlx::migrate!("../migrations")`. The new migration file
`20240613000001_invoice.sql` must therefore be in
`src-tauri/migrations/` for the test pool to apply it. If it is applied
successfully, the test will work. If the migration file has a syntax error,
the pool creation will fail — the test is the validation gate for the
migration SQL.

**File to create**: `src-tauri/src/invoice_spike.rs` — this module holds the
spike test only. Add `#[cfg(test)] mod invoice_spike;` to `lib.rs`.

Alternatively, if modifying `lib.rs` is awkward, add the tests to a new
`#[cfg(test)]` block at the bottom of `src-tauri/src/splits.rs` (that file
is already in scope for reading; this is the only case where a small addition
is acceptable). Ask the operator if unsure.

**Target test shape** (place in `invoice_spike.rs`):

```rust
//! Spike 019 validation tests — first-class invoice entity.
//! These tests validate the migration SQL and the derived counterparty-balance
//! query. They do NOT test any UI or Tauri command — the implementation plan
//! that follows this spike will wire those.

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    async fn spike_pool() -> SqlitePool {
        // Apply all migrations including the new invoice migration.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn invoice_schema_applies_cleanly() {
        let pool = spike_pool().await;
        // If we got here without panic, the migration applied.
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invoice")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn transaction_has_invoice_id_column() {
        let pool = spike_pool().await;
        // PRAGMA table_info returns one row per column.
        let cols: Vec<(String,)> =
            sqlx::query_as("PRAGMA table_info('transaction')")
                .fetch_all(&pool)
                .await
                .unwrap()
                .into_iter()
                .map(|(_, name, _, _, _, _): (i64, String, String, i64, Option<String>, i64)| (name,))
                .collect();
        let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();
        assert!(names.contains(&"invoice_id"), "invoice_id column missing");
    }

    #[tokio::test]
    async fn split_has_reimbursed_by_column() {
        let pool = spike_pool().await;
        let cols: Vec<(String,)> =
            sqlx::query_as("PRAGMA table_info('split')")
                .fetch_all(&pool)
                .await
                .unwrap()
                .into_iter()
                .map(|(_, name, _, _, _, _): (i64, String, String, i64, Option<String>, i64)| (name,))
                .collect();
        let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();
        assert!(
            names.contains(&"reimbursed_by_transaction_id"),
            "reimbursed_by_transaction_id column missing from split"
        );
    }

    #[tokio::test]
    async fn counterparty_balance_query_derives_correctly() {
        let pool = spike_pool().await;

        // Seed minimal data: person, account, invoice, item transaction, split.
        let primary_id = "person-primary";
        let third_party_id = "person-third";
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Owner'), (?2, 'ThirdParty')")
            .bind(primary_id).bind(third_party_id)
            .execute(&pool).await.unwrap();

        let acct_id = "acct-card";
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES (?1, 'My Card', 'credit_card', ?2, 5, 15)",
        )
        .bind(acct_id).bind(primary_id)
        .execute(&pool).await.unwrap();

        let inv_id = "inv-1";
        sqlx::query(
            "INSERT INTO invoice (id, account_id, closing_date, due_date, total_cents) \
             VALUES (?1, ?2, '2026-06-05', '2026-06-15', 10000)",
        )
        .bind(inv_id).bind(acct_id)
        .execute(&pool).await.unwrap();

        let item_txn_id = "txn-item";
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES (?1, 'expense', 10000, '2026-05-20', 'credit', 0, 0, ?2)",
        )
        .bind(item_txn_id).bind(inv_id)
        .execute(&pool).await.unwrap();

        let split_id = "split-1";
        sqlx::query(
            "INSERT INTO split (id, transaction_id, amount, owner_person_id) \
             VALUES (?1, ?2, 10000, ?3)",
        )
        .bind(split_id).bind(item_txn_id).bind(third_party_id)
        .execute(&pool).await.unwrap();

        // Before reimbursement: balance = 10000.
        let (balance_before,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(s.amount), 0) - COALESCE(SUM(rt.amount), 0) \
             FROM split s \
             LEFT JOIN \"transaction\" rt ON rt.id = s.reimbursed_by_transaction_id \
             WHERE s.transaction_id IN \
               (SELECT id FROM \"transaction\" WHERE invoice_id = ?1) \
             AND s.owner_person_id = ?2",
        )
        .bind(inv_id).bind(third_party_id)
        .fetch_one(&pool).await.unwrap();
        assert_eq!(balance_before, 10000);

        // Record the reimbursement Entrada.
        let reimb_id = "txn-reimb";
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1, 'income', 10000, '2026-06-15', 0, 0)",
        )
        .bind(reimb_id)
        .execute(&pool).await.unwrap();

        sqlx::query(
            "UPDATE split SET reimbursed_by_transaction_id = ?1 WHERE id = ?2",
        )
        .bind(reimb_id).bind(split_id)
        .execute(&pool).await.unwrap();

        // After reimbursement: balance = 0.
        let (balance_after,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(s.amount), 0) - COALESCE(SUM(rt.amount), 0) \
             FROM split s \
             LEFT JOIN \"transaction\" rt ON rt.id = s.reimbursed_by_transaction_id \
             WHERE s.transaction_id IN \
               (SELECT id FROM \"transaction\" WHERE invoice_id = ?1) \
             AND s.owner_person_id = ?2",
        )
        .bind(inv_id).bind(third_party_id)
        .fetch_one(&pool).await.unwrap();
        assert_eq!(balance_after, 0);
    }
}
```

**Note on PRAGMA query shape**: `PRAGMA table_info` returns 6 columns
`(cid, name, type, notnull, dflt_value, pk)`. The `sqlx::query_as` tuple must
match. If the exact tuple type causes a compile error, replace with a
`sqlx::query` returning `sqlx::Row` and extract by column name. Do not fight
the compiler — adapt the query shape to what compiles.

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked invoice_spike`
→ 4 tests pass. Then `npm run rust:check` → exit 0.

---

### Step 4: Add a pure cycle-membership helper to the forecast engine

**What to do**: Add one pure function to `src-tauri/src/forecast/mod.rs` that
answers "does this transaction date fall within the cycle that closes on
`closing_date`?" This will be needed by the implementation plan to assign items
to invoices at import time, and to compute the day-by-day running total.

Place it after `cycle_due_date` (around line 306):

```rust
/// Returns true if `txn_date` falls within the billing cycle that closes on
/// `closing_date`. The cycle runs from the day AFTER the previous closing date
/// up to and including `closing_date` itself.
///
/// `prev_closing_date` is `closing_date` minus one cycle (usually the same
/// calendar day in the prior month). The caller computes it from
/// `account.closing_day` using `cycle_due_date` or equivalent arithmetic.
///
/// Pure — no I/O, no panics.
pub fn in_cycle(txn_date: NaiveDate, prev_closing_date: NaiveDate, closing_date: NaiveDate) -> bool {
    txn_date > prev_closing_date && txn_date <= closing_date
}
```

Add unit tests for it in the existing `#[cfg(test)]` block at the bottom of
`mod.rs`:

```rust
#[test]
fn in_cycle_on_closing_day_is_included() {
    use chrono::NaiveDate;
    let prev = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap();
    let close = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    assert!(in_cycle(close, prev, close)); // closing day itself is in cycle
}

#[test]
fn in_cycle_day_after_prev_closing_is_first_day() {
    use chrono::NaiveDate;
    let prev = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap();
    let close = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let first_day = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
    assert!(in_cycle(first_day, prev, close));
}

#[test]
fn in_cycle_prev_closing_day_is_excluded() {
    use chrono::NaiveDate;
    let prev = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap();
    let close = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    assert!(!in_cycle(prev, prev, close)); // prev closing day is NOT in the new cycle
}

#[test]
fn in_cycle_after_closing_day_is_out() {
    use chrono::NaiveDate;
    let prev = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap();
    let close = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
    let after = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
    assert!(!in_cycle(after, prev, close));
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked in_cycle`
→ 4 tests pass. Then `npm run rust:check` → exit 0.

---

### Step 5: Write the spike design record

**What to do**: Create `specs/019-invoice-entity/spike.md` (create the
directory if it does not exist). The file must contain:

1. **Summary**: what this spike decided, in 3–5 sentences.

2. **Proposed schema** (copy the exact SQL from Step 2's migration draft;
   include D1–D5 decisions with rationale).

3. **Engine interaction model**:

   - **Day-by-day accumulation** (Régua 2 / velocímetro): when a new credit
     transaction is created and linked to an open invoice via `invoice_id`,
     `invoice.total_cents` is incremented by the item's amount. The UI reads
     `total_cents` from the open invoice to display the running bill. No change
     to `forecast::classify()` is needed — the item itself remains in the DB
     as a transaction with `payment_method='credit'`.

   - **Forecast lump at due_date** (FixedOut): the `credit_by_due` aggregation
     in `commands.rs` today reads `daily_checkin.credit_spend`. Once the
     implementation plan lands, it should instead read:

     ```sql
     SELECT due_date, total_cents - paid_cents AS remaining_cents
     FROM invoice
     WHERE account_id = ?1 AND status IN ('open','closed')
     ```

     and emit one `FixedOut` event per invoice at its `due_date`. The engine
     `classify()` signature does NOT change; the shell wiring changes.
     The `daily_checkin.credit_spend` column is kept for backward compatibility
     (legacy path) and can be deprecated in a follow-up.

   - **Month-metrics**: `MonthMetric.fixed_out_cents` already includes the
     fatura lump (via `FixedOut` events); no change needed once the shell
     reads from `invoice` instead of `daily_checkin`.

4. **Write-back collapse contract** (ADR-0003 alignment):

   When the bidirectional write-back is enabled (spec 018 / ADR-0003), an
   invoice must collapse to:
   - A single Saída lump on `invoice.due_date` in the spreadsheet's Cartão
     column (or the Saída column per the account's block mapping).
   - A structured note appended to that cell listing the items (amount +
     description), the per-owner breakdown, and any partial-payment residual.
   - A human-approval gate (as per ADR-0003) must be triggered before any
     write; no auto-approve path exists.
   - The reimbursement Entrada (for the third party's share) collapses to an
     Entrada lump on the same due_date in the Entrada column with a note
     referencing the companion person's name and the invoice cycle.

5. **Owner-split and reimbursement model**:

   The companion card case: account B has `linked_account_id = account A`
   (primary card). Account B's `owner_person_id` identifies the third party.
   When a credit expense arrives on account B:
   - It is linked to account A's open invoice (the bill is on the primary
     card).
   - A `split` row is created with `owner_person_id = account_B.owner_person_id`
     (the companion person's id) and `amount = item amount`.
   - An Entrada projection (type=`income`, `is_projection=1`) is created at
     `invoice.due_date` representing the expected reimbursement.
   - `split.reimbursed_by_transaction_id` is set to that projection's id.
   - When the real reimbursement Entrada arrives (matched by the auto-import
     pipeline per spec 008 EC15), the projection is realized and
     `reimbursed_by_transaction_id` stays pointing to the realized transaction.

   The third-party-payer case (someone else pays a shared expense): same
   model, but the primary and companion are swapped. The expense belongs to
   the primary person's invoice; a split row with the other person's id marks
   their portion; a reimbursement Entrada projection is created.

6. **Open questions** (resolved by this spike):

   - **OQ-1 (resolved — D3)**: Should each credit transaction item have its
     own `invoice_item` table or use the existing `transaction` table?
     Decision: use `transaction.invoice_id`. Rationale: the transaction is
     already the atomic ledger unit; a separate item table would duplicate
     amount, date, description. If an item has installment metadata, that is
     modeled by the existing `recurrence_id` / `installment_plan` concept
     (spec 008 §EC7), not by a new item table.

   - **OQ-2 (resolved — D5)**: Should `counterparty_balance` be a stored
     table or a derived view? Decision: derived query (no stored table).
     Rationale: a stored counter must be kept in sync on every split update,
     payment match, and rollback — a view is simpler and cannot drift.
     If query performance becomes an issue, a materialized view or a cache
     column can be added in a follow-up.

   - **OQ-3 (resolved)**: How does the forecast engine know a particular
     invoice's due date before the invoice is closed? Decision: the
     implementation plan's shell reads from `invoice` where status is
     `'open'` or `'closed'` (both have a known `due_date`). The `FixedOut`
     event is emitted at `due_date` for `total_cents - paid_cents`. This is
     consistent with the current `cycle_due_date` computation — just migrated
     from an ad-hoc aggregation to a first-class row.

   - **OQ-4 (resolved)**: What happens to `daily_checkin.credit_spend` once
     the invoice entity exists? Decision: keep the column; the implementation
     plan deprecates its use in `credit_by_due` gradually. It remains the
     Régua 2 source for the daily check-in UI until the invoice-driven
     UI (Módulo Crédito) is shipped and validated.

   - **OQ-5 (open — not resolved by this spike)**: What is the UX for
     matching a real reimbursement Entrada (from auto-import, spec 008) to an
     existing `split.reimbursed_by_transaction_id` projection? The matching
     algorithm (EC15 in spec 008) is not part of this spike — it belongs to
     the auto-import implementation slice.

   - **OQ-6 (open)**: Should partial payment of an invoice (EC6) update
     `invoice.paid_cents` and emit a revised `FixedOut` projection for the
     residual, or should the residual be a new invoice row for the next cycle?
     This requires a human decision before the implementation plan proceeds.
     Recommendation: update `paid_cents` on the existing invoice and emit a
     residual `FixedOut` at the next cycle's due_date as a separate projection
     transaction (not a new invoice row); document this in the implementation
     plan.

7. **Follow-up tasks** (for the implementation plan that cites this spike):

   - FT-1: Wire the shell (`commands.rs` `credit_by_due`) to read from
     `invoice` instead of `daily_checkin.credit_spend`.
   - FT-2: Build the invoice creation/item-assignment logic in the import
     pipeline (plan 004 must land first; the companion card detection uses
     `account.linked_account_id`).
   - FT-3: Build the Módulo Crédito UI screen (faturas list, items,
     counterparty balance, running total).
   - FT-4: Implement the write-back collapse (per ADR-0003 collapse contract
     above); requires spec 018 write-back infrastructure.
   - FT-5: Resolve OQ-6 (partial payment model) before starting FT-2.
   - FT-6: Deprecate `daily_checkin.credit_spend` after FT-1 and FT-3 are
     validated in dogfooding.

**Verify**: the file exists and is readable.
`ls specs/019-invoice-entity/spike.md` → file exists.
`wc -l specs/019-invoice-entity/spike.md` → more than 80 lines (a real
design document, not a stub).

---

### Step 6: Final gate

**What to do**: Confirm the working tree contains only the expected new/modified
files and that all checks pass.

```
git diff --name-only HEAD
```

Expected output (no other files):

```
specs/019-invoice-entity/spike.md
src-tauri/migrations/20240613000001_invoice.sql
src-tauri/src/forecast/mod.rs
src-tauri/src/invoice_spike.rs
src-tauri/src/lib.rs
```

(If the spike tests were added to `splits.rs` instead, `lib.rs` and
`invoice_spike.rs` are absent and `splits.rs` appears instead.)

**Verify**: `npm run check` → exit 0.

If the frontend checks (`npm run typecheck`, `npm run lint`) fail for reasons
unrelated to this spike (pre-existing failures), document the failure text in
a comment and confirm that no in-scope file caused it. Do NOT fix pre-existing
frontend errors as part of this spike.

## Test plan

**New Rust tests** (Steps 3–4):

In `src-tauri/src/invoice_spike.rs` (or `splits.rs` test block):

- `invoice_schema_applies_cleanly` — migration applies without error
- `transaction_has_invoice_id_column` — `PRAGMA table_info` confirms the column
- `split_has_reimbursed_by_column` — confirms the new split column
- `counterparty_balance_query_derives_correctly` — the derived-balance query
  returns 10000 before reimbursement, 0 after

In `src-tauri/src/forecast/mod.rs` test block:

- `in_cycle_on_closing_day_is_included`
- `in_cycle_day_after_prev_closing_is_first_day`
- `in_cycle_prev_closing_day_is_excluded`
- `in_cycle_after_closing_day_is_out`

**Structural pattern**: model async tests after `test_pool()` in
`src-tauri/src/google_sheets/import.rs` (in-memory pool with migrations applied
via `sqlx::migrate!`).

**Verification**: `cargo test --manifest-path src-tauri/Cargo.toml --locked`
→ all tests pass, including the 8 new ones. `npm run rust:check` → exit 0.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` exits 0; at
      least 8 new tests exist and pass (4 schema/counterparty-balance, 4 in_cycle)
- [ ] `npm run rust:check` exits 0 (fmt + clippy + tests)
- [ ] `ls src-tauri/migrations/20240613000001_invoice.sql` → file exists
- [ ] `ls specs/019-invoice-entity/spike.md` → file exists
- [ ] `wc -l specs/019-invoice-entity/spike.md` → output is ≥ 80
- [ ] `git diff --name-only HEAD` shows only the files listed in Step 6
      (no commands.rs, no import.rs, no frontend files)
- [ ] `grep -n "in_cycle" src-tauri/src/forecast/mod.rs` → at least 5 matches
      (fn definition + 4 test calls)
- [ ] `grep -n "invoice_id" src-tauri/migrations/20240613000001_invoice.sql` →
      at least 1 match (the ALTER TABLE statement)
- [ ] `npm run check` exits 0 (full gate)
- [ ] `plans/README.md` status row for plan 019 updated to DONE

## STOP conditions

Stop and report (do not improvise) if:

1. **Drift**: the code at any location cited in "Current state" does not match
   the excerpts (the file changed since commit `d183bbf`). Report the delta.

2. **Migration timestamp conflict**: a file named `20240613000001_*.sql` already
   exists in `src-tauri/migrations/`. Choose the next available timestamp
   strictly greater than `20240612000010` and report the new name used.

3. **Migration SQL fails the sqlite3 smoke test** (Step 2) with a syntax error
   other than a missing column on `transaction` or `split` (those are expected
   to succeed only when prior migrations run first). If the invoice table itself
   has a syntax error, fix it. If the `ALTER TABLE` fails in isolation (expected
   — there is no `transaction` table in a bare sqlite3), note it and run the
   full test instead.

4. **The `test_pool()` helper is not in `import.rs`** or has moved. Locate the
   actual in-memory pool helper and adapt Step 3 accordingly; report the
   location found.

5. **`sqlx::migrate!` does not pick up the new migration file** in the test run
   (e.g., the test module path resolves to a different migrations directory).
   Do not hard-code migration SQL in the test. Instead, find the correct
   `sqlx::migrate!("path")` path and report it.

6. **A step's verification fails twice** after a reasonable fix attempt. Report
   the error verbatim and the attempted fix.

7. **The fix for any step requires touching `commands.rs` or `import.rs`**.
   This spike is read-only on the app shell. Report what change would be needed
   and why, and wait for approval to expand scope.

8. **Clippy `-D warnings` triggers on `in_cycle` or the spike tests** for a
   non-obvious reason (e.g., dead_code on the spike module). Use
   `#[cfg(test)]` scoping and `#[allow(dead_code)]` only where the compiler
   insists and the reason is clear; document any allow in a comment.

9. **OQ-6 (partial payment model) is needed to make the migration SQL
   compile or the tests pass**. It should not be — the spike intentionally
   leaves OQ-6 open. If it turns out to be a blocker, report it; do not
   resolve it unilaterally.

## Maintenance notes

- **This spike is the prerequisite for the implementation plan** that builds
  the full invoice pipeline (Módulo Crédito screen, import wiring, write-back
  collapse). That plan must cite `specs/019-invoice-entity/spike.md` and
  implement the FT-1 through FT-6 tasks listed there.

- **Migration sequencing**: `20240613000001_invoice.sql` is a validated draft.
  The implementation plan must confirm its timestamp is still the next available
  one (if other migrations were added between this spike and that plan's
  execution, renumber it). Never run a draft migration against production
  without re-running `npm run check` first.

- **`in_cycle()` is a pure helper without a caller yet** — it will trigger a
  dead-code warning in non-test builds. Add `#[allow(dead_code)]` with a
  comment pointing to the implementation plan, or move it to `pub(crate)` so
  the future shell caller in `commands.rs` can use it without another PR.

- **Reviewer should scrutinize**:
  - The derived `counterparty_balance` query in Step 3 for edge cases: what
    happens when a split has `amount = 0`? What if `reimbursed_by_transaction_id`
    points to a transaction with a different amount than the split (partial
    reimbursement)? The implementation plan must extend the test coverage.
  - Whether `split.reimbursed_by_transaction_id` should CASCADE or SET NULL on
    transaction delete. Current migration uses no explicit ON DELETE clause
    (SQLite default = NO ACTION). The implementation plan should add
    `ON DELETE SET NULL` to handle reimbursement transaction deletion without
    orphaning the split.
  - The `invoice.total_cents` update strategy: the spike assumes it is updated
    by the application layer (not a trigger). The implementation plan must
    decide between an app-level update, a trigger, or a computed column.

- **OQ-6 (partial payment) must be resolved before FT-2 implementation
  starts.** The recommended resolution is in the spike.md; a human reviewer
  should confirm it before the implementation plan is written.

- **`daily_checkin.credit_spend` deprecation (FT-6)** is a multi-step
  migration risk — the column feeds the existing Régua 2 UI and the forecast.
  Deprecate only after both FT-1 (shell reads from `invoice`) and FT-3 (Módulo
  Crédito UI) are validated in dogfooding and the existing `credit_spend`-based
  paths have confirmed parity.
