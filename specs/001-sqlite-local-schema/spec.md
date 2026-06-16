# Spec: SQLite Local Schema

## Summary

Implement the local-first SQLite database schema for Neko Finance using `tauri-plugin-sql` with `sqlx::migrate!` in Rust. The schema models the full domain: identity, financial instruments, classification, budgeting, savings, data import, and sync.

## Motivation

The app currently has a synthetic UI shell with no persistence. Every domain operation — tracking expenses, checking daily budget compliance, diagnosing reserve health — requires a local database that mirrors the private methodology's concepts.

## User Stories

### US1 — Person and Profile setup

**As** the primary user
**I want** to create a Person row for myself and a Profile for this device
**So that** the app knows who I am and can distinguish multi-person data in the future.

**Acceptance**: On first launch, the app creates a default Person and Profile. The profile is linked to the person (profile.person_id FK). The person has a local UUID-based ID.

### US2 — Account registration

**As** the primary user
**I want** to register my financial accounts (e.g. a bank checking account, a credit card, a digital wallet)
**So that** transactions can be associated with real financial instruments.

**Acceptance**: Create accounts with type (bank, credit_card, wallet, savings, business), owner_person_id, and type-specific optional fields (institution, credit_limit, closing_day, linked_account_id for additional cards).

### US3 — Category tree

**As** the primary user
**I want** a default category tree (Fixas/Variáveis with subcategories)
**So that** I can classify expenses immediately without manual setup.

**Acceptance**: On first migration, seed default categories: Sem categoria (root, variable), Fixas > Moradia/Transporte/Saúde, Variáveis > Alimentação/Lazer/Vestuário, Cartões > Cartão Adicional. Every category has a `nature` (fixed/variable).

### US4 — Transaction recording with payment method

**As** the primary user
**I want** to record transactions with type (income/expense/transfer), payment method (debit/credit/pix/cash), and optional is_fixed flag
**So that** the methodology's débito vs crédito distinction is preserved.

**Acceptance**: Create a transaction row. For transfers, from_account_id and to_account_id are set on the same row. Amount is always positive. Owner is derived via Split.

### US5 — Split allocation

**As** the primary user
**I want** to split a single transaction across multiple categories and/or responsible persons
**So that** a shared expense or multi-purpose purchase is correctly allocated.

**Acceptance**: Create split rows linked to a transaction. Each split has amount, category_id, and owner_person_id. The sum of split amounts equals the transaction amount (enforced at application level, not DB constraint for MVP).

### US6 — Daily check-in with dual tracking

**As** the primary user
**I want** to log my daily spending (debit + credit separately) against my daily budget
**So that** I can track both methodology-pure spending (Régua 1) and real credit consumption (Régua 2).

**Acceptance**: Create a daily_checkin row with date, person_id, daily_spend (debit/cash), credit_spend, and optional note. The checkin references the active daily_budget for comparison.

### US7 — Daily budget management

**As** the primary user
**I want** to set and review my daily budget (single number per person)
**So that** the app can show green/amber/red against my actual daily spend.

**Acceptance**: Create daily_budget rows with amount, person_id, status (active/under_review/deprecated), start_date. Only one active budget per person at a time. Mia can suggest recalculations.

### US8 — Reserve tracking

**As** the primary user
**I want** to track my emergency reserve in months-of-expenses with trend direction
**So that** I know if my financial foundation is strengthening or weakening.

**Acceptance**: Create reserve row with target_months, current_months, trend (up/down/flat), last_calculated_at. Monthly snapshots recorded in reserve_snapshot for trend analysis.

### US9 — Sheet mapping for Google Sheets import

**As** the primary user
**I want** to map Google Sheets columns to internal schema fields
**So that** imported data lands in the correct tables and columns.

**Acceptance**: Create sheet_mapping rows for each column-to-field mapping with `date_direction` (past_only, future_only, both). Import rule: rows with date < today go to `transaction`, rows with date >= today go to `projection`. Supports the raw cash-flow diary format (daily aggregated entries).

### US10 — Future projections

**As** the primary user
**I want** to record projected future income and expenses (forward-looking)
**So that** I can plan the coming months rather than only review the past.

**Acceptance**: Create projection rows with the same structure as transactions but a boolean `is_projection=true` (or separate `projection` table). Projections are distinguished from realized transactions. Daily check-in compares actual spend against projections.

### US11 — FTS5 full-text search

**As** the primary user
**I want** to search transactions by description and categories by name
**So that** I can quickly find past expenses and diagnose spending patterns.

**Acceptance**: FTS5 virtual tables for transactions (description) and categories (name). Queries return ranked results with snippet context.

## Non-functional requirements

- Migrations run at Tauri startup, before the frontend loads.
- All IDs use TEXT (UUID v4) for future sync compatibility.
- Timestamps use ISO 8601 TEXT format.
- Monetary values use INTEGER (cents) to avoid floating-point errors.
- TDD: every migration and query module must have unit tests before implementation.
