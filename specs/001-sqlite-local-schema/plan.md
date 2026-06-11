# Plan: SQLite Local Schema

## Architecture

```
Tauri startup
  → sqlx::migrate! runs embedded migrations
  → SQLite database ready at app_data_dir/neko-finance.db
  → Plugin exposes connection to frontend via tauri-plugin-sql
```

Migrations are embedded in the Rust binary via `sqlx::migrate!` macro. They run idempotently at startup. The frontend never accesses the database directly — all queries go through Tauri commands that wrap `sqlx`.

## Schema

### Tables

```sql
-- Identity
person (id TEXT PK, name TEXT NOT NULL, email TEXT, created_at TEXT)
profile (id TEXT PK, person_id TEXT FK, device_id TEXT, created_at TEXT)

-- Financial instruments
account (
  id TEXT PK,
  name TEXT NOT NULL,
  type TEXT NOT NULL CHECK(type IN ('bank','credit_card','wallet','savings','business')),
  owner_person_id TEXT FK,
  institution TEXT,
  balance INTEGER DEFAULT 0,
  credit_limit INTEGER,
  closing_day INTEGER,
  due_day INTEGER,
  linked_account_id TEXT FK,
  provider TEXT,
  created_at TEXT
)

-- Classification
category (
  id TEXT PK,
  name TEXT NOT NULL,
  parent_id TEXT FK,
  nature TEXT NOT NULL CHECK(nature IN ('fixed','variable')),
  created_at TEXT
)

-- Transactions
transaction (
  id TEXT PK,
  type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount INTEGER NOT NULL,
  description TEXT,
  date TEXT NOT NULL,
  payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
  is_fixed INTEGER DEFAULT 0,
  from_account_id TEXT FK,
  to_account_id TEXT FK,
  created_at TEXT,
  updated_at TEXT
)

-- Split allocation
split (
  id TEXT PK,
  transaction_id TEXT FK NOT NULL,
  amount INTEGER NOT NULL,
  category_id TEXT FK,
  owner_person_id TEXT FK NOT NULL,
  note TEXT
)

-- Budgeting
daily_budget (
  id TEXT PK,
  person_id TEXT FK NOT NULL,
  amount INTEGER NOT NULL,
  start_date TEXT NOT NULL,
  end_date TEXT,
  status TEXT NOT NULL CHECK(status IN ('active','under_review','deprecated')),
  free_income INTEGER,
  calculated_at TEXT NOT NULL
)

daily_checkin (
  id TEXT PK,
  person_id TEXT FK NOT NULL,
  date TEXT NOT NULL,
  daily_spend INTEGER DEFAULT 0,
  credit_spend INTEGER DEFAULT 0,
  daily_budget_id TEXT FK,
  note TEXT,
  created_at TEXT NOT NULL
)

-- Reserve
reserve (
  id TEXT PK,
  person_id TEXT FK NOT NULL,
  target_months INTEGER NOT NULL,
  current_months REAL NOT NULL,
  trend TEXT CHECK(trend IN ('up','down','flat')),
  last_calculated_at TEXT NOT NULL
)

reserve_snapshot (
  id TEXT PK,
  reserve_id TEXT FK NOT NULL,
  snapshot_date TEXT NOT NULL,
  current_months REAL NOT NULL,
  monthly_expense_avg INTEGER,
  total_reserve_amount INTEGER
)

-- Data import
sheet_mapping (
  id TEXT PK,
  sheet_name TEXT NOT NULL,
  column_letter TEXT NOT NULL,
  column_header TEXT,
  target_table TEXT NOT NULL,
  target_field TEXT NOT NULL,
  sheet_row_offset INTEGER DEFAULT 0
)

-- Sync
sync_log (
  id TEXT PK,
  event_type TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  profile_id TEXT FK NOT NULL,
  timestamp TEXT NOT NULL,
  metadata TEXT
)
```

### FTS5

```sql
CREATE VIRTUAL TABLE transaction_fts USING fts5(description, content='transaction', content_rowid='rowid');
CREATE VIRTUAL TABLE category_fts USING fts5(name, content='category', content_rowid='rowid');
```

### Indexes

- `idx_transaction_date` on `transaction(date)`
- `idx_transaction_account` on `transaction(from_account_id, to_account_id)`
- `idx_split_transaction` on `split(transaction_id)`
- `idx_daily_checkin_person_date` on `daily_checkin(person_id, date)`
- `idx_daily_budget_person_status` on `daily_budget(person_id, status)`
- `idx_reserve_person` on `reserve(person_id)`

## Risks

1. **sqlx migration ordering**: migrations are sequential. Must plan order carefully (identity → accounts → categories → transactions → splits → budgeting → reserve → mapping → sync → FTS5).
2. **Tauri plugin-sql vs raw sqlx**: `tauri-plugin-sql` wraps sqlx but the schema DDL runs via `sqlx::migrate!` at startup, not via the plugin. Clarify boundaries in implementation.
3. **SQLite in WSL**: file locking may behave differently. Test with `PRAGMA journal_mode=WAL`.

## Dependencies

- `tauri-plugin-sql` (Rust crate, Tauri plugin)
- `sqlx` with sqlite feature (Rust crate)
- `uuid` crate for ID generation
- Existing Tauri shell and Vite frontend

## Data Boundaries

- Database file: `app_data_dir/neko-finance.db` — local, gitignored.
- No API keys, OAuth tokens, or private methodology text in the database.
- Synthetic fixtures for tests only.

## Testing Strategy

- **Rust unit tests**: `cargo test` — test each migration's up/down, test CRUD operations via sqlx.
- **Integration tests**: Tauri commands that exercise the full schema (create person → create account → record transaction → split → checkin → query reserve).
- **TDD required**: write failing test → implement migration → green.
- **Coverage target**: 90% on Rust domain logic.

## Release Implications

- First real persistence layer. Backward compatibility starts now — migrations must never be edited, only added.
- Future migrations append new files to `migrations/` directory.
- No data migration needed (fresh install only for MVP).
