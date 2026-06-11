# Tasks: SQLite Local Schema

## Phase 1 — Rust dependencies and migration framework

- [ ] T1.1 Add `sqlx` (with sqlite, runtime-tokio, migrate features) and `uuid` to Cargo.toml
- [ ] T1.2 Add `tauri-plugin-sql` to Cargo.toml and register in lib.rs
- [ ] T1.3 Write a smoke test that opens an in-memory SQLite and runs a migration
- [ ] T1.4 Configure app_data_dir for database path

## Phase 2 — Core identity

- [ ] T2.1 Create migration `0001_person.sql`: person table
- [ ] T2.2 Create migration `0002_profile.sql`: profile table with FK to person
- [ ] T2.3 Test: create person, create profile, query join

## Phase 3 — Financial instruments

- [ ] T3.1 Create migration `0003_account.sql`: account table with CHECK constraints
- [ ] T3.2 Test: create accounts of each type (bank, credit_card, wallet, savings, business)
- [ ] T3.3 Test: credit card with linked_account_id (additional card scenario)

## Phase 4 — Classification

- [ ] T4.1 Create migration `0004_category.sql`: category table with parent_id + nature CHECK
- [ ] T4.2 Create migration `0005_seed_categories.sql`: seed default category tree
- [ ] T4.3 Test: query category tree, verify fixed/variable nature

## Phase 5 — Transactions and splits

- [ ] T5.1 Create migration `0006_transaction.sql`: transaction table with payment_method
- [ ] T5.2 Create migration `0007_split.sql`: split table with FKs to transaction, category, person
- [ ] T5.3 Test: create expense with debit, expense with credit, income, transfer
- [ ] T5.4 Test: create splits across two owners and two categories
- [ ] T5.5 Create migration `0008_indexes_core.sql`: indexes on transaction, split

## Phase 6 — Budgeting and daily discipline

- [ ] T6.1 Create migration `0009_daily_budget.sql`: daily_budget table
- [ ] T6.2 Create migration `0010_daily_checkin.sql`: daily_checkin with dual tracking
- [ ] T6.3 Test: create active budget, daily checkin with daily_spend and credit_spend, query dual régua metrics

## Phase 7 — Reserve

- [ ] T7.1 Create migration `0011_reserve.sql`: reserve table
- [ ] T7.2 Create migration `0012_reserve_snapshot.sql`: reserve_snapshot table
- [ ] T7.3 Test: create reserve with target=6, current=4.5, trend=down, add 3 snapshots, verify trend history

## Phase 8 — Import and sync

- [ ] T8.1 Create migration `0013_sheet_mapping.sql`: sheet_mapping table
- [ ] T8.2 Create migration `0014_sync_log.sql`: sync_log table
- [ ] T8.3 Test: create sheet mapping for real spreadsheet column layout

## Phase 9 — FTS5 search

- [ ] T9.1 Create migration `0015_transaction_fts.sql`: FTS5 virtual table
- [ ] T9.2 Create migration `0016_category_fts.sql`: FTS5 virtual table
- [ ] T9.3 Test: insert transactions with descriptions, run FTS5 MATCH queries, verify ranking

## Phase 10 — Tauri integration

- [ ] T10.1 Create Tauri command `init_database` that runs migrations at startup
- [ ] T10.2 Wire plugin-sql for frontend to query via Tauri invoke
- [ ] T10.3 Integration test: full schema lifecycle (person → reserve snapshot)
- [ ] T10.4 Run `npm run check` — ensure all gates green with new Rust deps

## Parallelization notes

- Phases 1-3 are sequential (identity must exist first).
- Phases 4-7 can be developed in parallel after Phase 3.
- Phases 8-9 can be developed in parallel after Phase 5.
- Phase 10 depends on all previous phases.
