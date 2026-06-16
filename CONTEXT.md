# Neko Finance

Local-first Tauri desktop app for personal finance: Google Sheets integration, dashboards, and AI copilot (Mia).

## Language

### Identity

**Person**:
A human whose finances are tracked in the app. Has a name, optional email, and a local identifier. The domain does not assume a single person.
_Avoid_: User, member, client

**Profile**:
A local login/session on one device. 1:1 with Person (one person per profile). One Person may have Profiles on multiple devices in the future (multi-device sync).
_Avoid_: Account, login, session

**Device**:
The physical machine running the Tauri app. Each device has one active Profile at a time. Future: a Person may have profiles on multiple Devices.

### Financial Instruments

**Account**:
A real financial instrument — bank account, credit card, wallet, or savings account. Not a logical grouping (see Sheet Mapping). Has `type` (bank, credit*card, wallet, savings, business) and optional fields per type (institution, closing_day, credit_limit, linked_account_id for additional cards).
\_Avoid*: Sheet, category, group

**Transaction**:
A normalized financial movement. Has `type` (income, expense, transfer), `payment_method` (debit, credit, pix, cash), and optional `is_fixed` flag. For transfers, uses `from_account_id` and `to_account_id` on the same row. Ownership (who is responsible, who paid, who benefited) is expressed through Split and Account relationships, not stored on the transaction directly.
_Avoid_: Entry, record, movement

**Split**:
Allocation of one transaction across multiple responsible persons. Carries `amount` and `owner_person_id`. A R$300 market purchase split into R$200 (owner A) + R$100 (owner B) is two split rows on the same transaction. "Where the money went" is no longer a category on the split — categorization was demoted to free **tags** (diagnostic only), per the method (categories are for diagnosis, never for planning).

**Payment Method** (enum on Transaction):
`debit`, `credit`, `pix`, `cash`. The methodology distinguishes debit (immediate balance impact) from credit (delayed, tracked separately as "fatura"). Debit/PIX/cash feed Régua 1; credit feeds Régua 2.

### Classification

**Category** (demoted — specs 014/015):
The granular per-category tree was the "budget-by-category" anti-pattern the method rejects, so it was **demoted to free tags** (N:N, color/emoji, summed per month). What remains of `category` is only its `nature` attribute on the transaction. The `parent_id` tree is dormant (no budgeting UI); new labels are first-class tags.

**Category Nature** (enum, kept on the transaction):
`fixed` — Predictable, recurring, non-negotiable expenses (Saídas fixas). `variable` — Discretionary spending that becomes the daily budget (Diário).

### Budgeting & Discipline

**Daily Budget** (daily*budget):
A single daily spending limit per Person, calculated as (free income ÷ 30). Does not subdivide by category — categories are for backward-looking diagnosis, not forward-looking allocation. Has `status` (active, under_review, deprecated) and `amount`. Recalculated on: 3-month review, income change, or when Mia detects >15% deviation for 2+ weeks.
\_Avoid*: Daily allowance, per-diem

**Daily Check-in** (daily_checkin):
A daily record of actual spending vs budget. Contains two independent metrics:

- **daily_spend**: sum of debit/PIX/cash expenses for the day (Régua 1 — methodology pure)
- **credit_spend**: sum of credit card expenses for the day (Régua 2 — reality check)
  _Avoid_: Daily log, spending log

**Débito/Diário track** (internal name "Régua 1" — Neko's term, not the method's):
The method's core metric: daily_spend compared against daily_budget. Green/amber/red based on budget compliance. Goes silent (always green) when the user pays exclusively with credit.

**Crédito/Fatura track** (internal name "Régua 2"):
Credit bill tracking: SUM(credit_spend for the month) accumulates into the invoice that lands on the due date. Prevents self-deception when the daily track is green but the credit bill is accumulating silently. The engine tracks the two independently; it does not compare credit against income.

**Forecast Engine Types** (forecast `EventKind`):
The projection engine maps each transaction into exactly one bucket. The method has 5 movement types (entrada, saída, diário, economia, cartão); the engine collapses them into 4 `EventKind` variants because the card has no column of its own — its bill folds into the Saída lump at the due date:

- **Income** (Entrada): `type='income'`.
- **FixedOut** (Saída fixa + Cartão): `type='expense'` with `is_fixed=1`, **or** any `payment_method='credit'` expense (the fatura lands as a Saída lump on the due date).
- **Daily** (Diário): `type='expense'`, `is_fixed=0`, non-credit (débito/PIX/dinheiro).
- **Economia**: `type='transfer'` to a `reserve`/`illiquid` account (set aside, not spent).

Derived metrics: `cost_of_living = FixedOut + Daily`; `Performance = Income − (cost_of_living + Economia + previsão de diário restante)`. The UI exposes the 5 method types via `MovBadge` (Cartão = credit expense); the engine buckets are the 4 above.

### Savings & Protection

**Reserve**:
The emergency fund — the foundation of the methodology. Stored as a first-class entity (not an account field). Tracks `target_months` (how many months of expenses should be covered), `current_months` (actual coverage), and `trend` (up, down, flat). Monthly snapshots in `reserve_snapshot` enable trend detection.
_Avoid_: Emergency fund, savings goal

**Protection**:
Insurance products that protect against financial catastrophe. Light schema presence (tag on account or category) until methodology demands more detail.

### Data Import & Sync

**Sheet Mapping**:
A mapping layer between Google Sheets columns and the internal schema. One row per column mapping (e.g. "Sheet '2025' column C → transaction.amount"). Separates the data source from the domain model — accounts, categories, and transactions reference Sheet Mapping as source, not the raw sheet.
_Avoid_: Import config, column mapping

**Sync Log**:
Append-only table for sync events. Records what was imported/modified, when, and by which profile. Enables conflict resolution in future multi-device scenarios.

## Rules

- Daily budget is per Person, not per Profile. Multi-device inherits the same budget.
- Do not hardcode "me" into persisted data. Use Person rows.
- Fixed expenses are transactions with `is_fixed=true` and specific dates. Variable expenses feed the daily check-in.
- Credit card = pay later. Debit/PIX = pay now. The methodology distinguishes them; the schema preserves that distinction.
- The reserve is separate from the savings account. A savings account may or may not contain the reserve.
- Categories are for diagnosis, not planning. The daily_budget is a single number.
- All writes to Google Sheets require structured diff, validation, and explicit human approval.
- Local labels may use personal names; public fixtures and docs must use generic names.
