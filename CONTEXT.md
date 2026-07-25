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
A real financial instrument — bank account, credit card, wallet, or savings account. Not a logical grouping (see Sheet Mapping). Has `type` (bank, credit_card, wallet, savings, business, meal_voucher, pension, fgts) and optional fields per type (institution, closing_day, credit_limit, linked_account_id for additional cards). Credit-card accounts have their own writer (outside pockets — the card is a liability, not a pocket): holder cards own a cycle (`closing_day` 1–28 + `due_day`), additional cards link to a holder via `linked_account_id`, carry their own `owner_person_id` and inherit the holder's cycle. Card names plus explicit `card_alias` rows are the import-matching identities; `credit_limit` is discreet display only, never a ruler input.
_Avoid_: Sheet, category, group

**Invoice** (invoice):
The persisted credit-card statement: one row per card × cycle, keyed by the due-date month (`UNIQUE(account_id, cycle_month)`), with explicit `closing_date`/`due_date` per invoice (changing a card's cycle never rewrites history). Status (`prevista · aberta · fechada · paga`) is derived from the calendar, never stored. `stated_total_cents` is the authority when present (the sheet line value / direct adjustment); linked purchases are the itemization detail, and every purchase-changing gesture adjusts the stated total additively in the same SQL transaction. Stated-vs-itemized divergence renders as a synthetic reconciliation line ("não itemizado") — never hidden, never an item. Three-way merge on the stated total uses `source_stated_total_cents` as base; both-changed lands in the existing import-conflict queue and blocks write-back. See ADR-0004.
_Avoid_: Bill, statement (in code), fatura estimada

**Card Series** (card_series):
One entity for both subscriptions (`count NULL`, materialized through December of the current year, minimum 3 occurrences) and installments (`count N`). Occurrences are projected purchases anchored to **consecutive invoices** (not dates); `n/N` derives from the cycle index and is never stored. Editing regenerates open/future occurrences under the same identity; canceling a subscription takes effect from a cycle onward. Ordinary `recurrence` is never used for card purchases.

**Refund link** (on Transaction):
A reimbursement is an income row **linked** to at most one target — `refund_invoice_id` (partial allowed), `refund_txn_id` (a purchase) or `refund_series_id` — never a reduction of the invoice or of any judging ruler (gross regime: full income + full outflow). The link powers a marked, didactic net reading only. The `#reembolso:` note marker's derived income gets the invoice link at import time.

**Card Proposal** (card_proposal):
An unknown card alias found in a cards note section at import becomes a pending proposal (identity = normalized alias; the same alias never re-proposes). Accepting asks for cycle/owner and creates the account + alias; accounts are never created silently.

**Transaction**:
A normalized financial movement. Has `type` (income, expense, transfer), `payment_method` (debit, credit, pix, cash), and optional `is_fixed` flag. For transfers, uses `from_account_id` and `to_account_id` on the same row. Ownership (who is responsible, who paid, who benefited) is expressed through Split and Account relationships, not stored on the transaction directly.
_Avoid_: Entry, record, movement

**Split**:
Allocation of one transaction across multiple responsible persons. Carries `amount` and `owner_person_id`. A R$300 market purchase split into R$200 (owner A) + R$100 (owner B) is two split rows on the same transaction. "Where the money went" is no longer a category on the split — categorization was demoted to free **tags** (diagnostic only), per the method (categories are for diagnosis, never for planning).

**Payment Method** (enum on Transaction):
`debit`, `credit`, `pix`, `cash`. The methodology distinguishes debit (immediate balance impact) from credit (delayed: the bill folds into a single Saída lump on the due date — "fatura"). Debit/PIX/cash are the daily (Diário) spend; credit becomes the due-date lump.

### Classification

**Category** (demoted — specs 014/015):
The granular per-category tree was the "budget-by-category" anti-pattern the method rejects, so it was **demoted to free tags** (N:N, color/emoji, summed per month). What remains of `category` is only its `nature` attribute on the transaction. The `parent_id` tree is dormant (no budgeting UI); new labels are first-class tags.

**Category Nature** (enum, kept on the transaction):
`fixed` — Predictable, recurring, non-negotiable expenses (Saídas fixas). `variable` — Discretionary spending that becomes the daily budget (Diário).

### Budgeting & Discipline

**Daily Budget** (daily*budget):
A single daily spending limit per Person. Stipulated by the ceiling ceremony (monthly items in `daily_budget_category` ÷ `divisor_days`) or set as a direct per-day value (`divisor_days` NULL). Has `status` (active, under_review, deprecated), `amount` (per-day cents) and `divisor_days`. The displayed ceiling carries an explicit provenance: `chosen` (active budget — the only verdict), `estimate` (previous complete month's Diário average, always shown with the estimate mark) or `none` (no record — dash + CTA, never a fabricated zero).
\_Avoid*: Daily allowance, per-diem

**Ceiling Proposal** (ceiling_proposal):
A ceiling ceremony documented in a Diário cell note of the spreadsheet (items + `Total = R$X` + `R$X / N Dias = R$Y`), detected at import and stored as a proposal keyed by the normalized note's hash. The import only proposes — accepting (which writes `daily_budget` + categories + divisor atomically) or dismissing is an explicit user gesture; the same note never re-proposes, and a new note supersedes the pending one.

**Daily ritual** (the day's Diário spend):
The day's actual spending is the sum of that day's debit/PIX/cash variable (`is_fixed=0`, non-credit) `transaction` rows, compared against the daily*budget. There is **no dedicated check-in table**: the daily ritual is recorded as ordinary Diário `transaction` rows. (A `daily_checkin` table once existed for this but had no production writer and was dropped — ADR-0001 / plan 027.)
\_Avoid*: Daily log, spending log

**Débito/Diário track** (internal name "Régua 1" — Neko's term, not the method's):
The method's core metric: the day's Diário spend compared against daily_budget. Green/amber/red based on budget compliance. In card mode (see Spending Mode) this track steps aside by design instead of showing a fake green: the day reads the faturas, and the stipulated ceiling remains visible as a reference.

**Spending Mode** (débito × cartão, derived — never configured):
A global mode detected purely from the data over a moving window (2 complete months + current): daily constancy (≥ 4 distinct days AND > R$ 50 in a month) ⇒ debit; window without constancy AND a live Cartão event ⇒ card; default debit. Hysteresis is asymmetric by construction — a stray purchase never flips into card mode, one month of constancy flips back to debit. In card mode the day's surface reroutes to the faturas (month's Cartão total + next due date, now derived per card from persisted invoices — `upcoming_invoices`) and a zeroed Diário is legitimate-by-design, not a gap. Card-mode legitimacy carries the method's gate (`card_gate`) with two computable legs — the 20–30% savings alive (annual economia ruler ≥ 20% floor) AND reserve ≥ 6 months — composed honestly: any leg below ⇒ below; both alive ⇒ alive; otherwise unknown (an incomputable leg never fabricates a verdict). The third canonical leg ("no rush toward the next patrimony goal") is didactic copy, never computed. Product principle: guide, never punish.

**Epistemic states** (per ruler):
Every method ruler exposed to the UI judges in explicit states, never numeric sentinels: `verdict` (registered/chosen data), `estimate` (derived number, always displayed with the "Estimativa" mark + the ritual's didactics), `zero` (input present and legitimately zero — dedicated word, e.g. "Sem reserva") and `no_record` (gap — dash + didactic popover with CTA, never a number). DS primitives: `EstimateMark`, `NoRecordDash`, `ModeChip` + the `--state-*` tokens.

**Crédito/Fatura** (the bill is a single due-date lump):
A credit bill is **one outflow on the due date** (Saída lump), not a per-day accrual. During a cycle purchases accrue on the open invoice (see Invoice), and the recorded output is one lump per card at the vencimento: write-back writes one note line per card under the cards section of the due-date cell, merged with the cell's non-card sections. The earlier "credit accumulates daily" track ("Régua 2") was retired (ADR-0001 / plans 022, 027); credit is never compared against income.

**Forecast Engine Types** (forecast `EventKind`):
The projection engine maps each transaction into exactly one of 6 `EventKind` variants (`src-tauri/src/forecast/mod.rs`), aligned 1:1 with the method's 5 movement types (entrada, saída, diário, economia, cartão) plus a 6th bucket the engine splits out of "economia" for long-term/illiquid investment:

- **Income** (Entrada): `type='income'`.
- **FixedOut** (Saída fixa): `type='expense'` with `is_fixed=1`, excluding the credit-card bucket once item/transaction classification knows it.
- **Daily** (Diário): `type='expense'`, `is_fixed=0`, non-credit (débito/PIX/dinheiro).
- **Cartao** (Cartão): credit-card bill/purchase bucket — its own column, folded into a single Saída lump on the due date. Inside custo de vida but tracked apart. With any card configured, the persisted invoice is the single voice of the future: raw credit events dated today or later are suppressed and each non-paid invoice injects one event at its due date (effective total); realized history still follows the sheet. With no card configured, raw classification stands.
- **Economia**: guardar em reserva acessível — leaves the spending balance, feeds Economizado%, excluded from custo de vida.
- **Patrimonio** (Patrimônio): long-term/illiquid investment — leaves the spending balance but is excluded from both custo de vida and accessible Economia%.

Derived metrics: `cost_of_living = FixedOut + Daily(realized) + Cartao`; `Performance = Income − (FixedOut + Daily(realized) + Daily(projected/remaining forecast) + Cartao + Economia + Patrimonio)` — the current month accounts for what is still going to be spent until month-end and improves as real spending stays under the ceiling (spec 021). The UI exposes the same buckets via `MovBadge`; engine and UI vocabularies match 1:1 (specs 011/021, PR #91).

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

**Zero semantics at import**:
Two spreadsheet zeros are resolved at import so they never masquerade as data: **pre-history** (template-evaluated zero balances in months before the sheet's adoption — before its first transaction or first non-zero balance — are trimmed from the Saldo series; those months honestly have no record) and **placeholders** (`R$ 0,00` note items on projected rows persist as zero-amount `line_item`s — the pre-launched structure of the future stays visible without inventing value; realized rows still discard zeros as noise).

### Savings & Forecast (derived metrics)

These live in the forecast DTO (`get_forecast`), computed in the Rust core — not persisted tables.

**AnnualSavings**: year-to-date figures from complete months. `registered_economia_cents / realized_income_cents` is the method's **Economizado%** (Economia transferred to reserve ÷ income — the spreadsheet's `%` column; target 20–30% as an ANNUAL average, never a monthly pass/fail). Distinct from `realized_savings_cents` (net surplus = income − outflow = the **Colchão**), which is the buffer, not Economizado. The judging ruler is `economia_ruler_cents`: registered Economia plus, **only when the liquid reserve covers ≥ 6 months of cost of living**, realized Patrimônio (pension/illiquid) — the method builds liquidity first, then long-term saving counts. The ruler feeds both the savings guardrail and `card_gate`; `economia_state` is `verdict` or `no_record` (the UI then shows the Colchão as a marked estimate — economia has no zero-diagnostic branch, since the real spreadsheet uses typed zero and blank interchangeably).

**MonthCoverage**: per future month, how much of the typical baseline outflow has been pre-launched. Drives the Previsibilidade card — an empty future month makes the projection optimistic until salary/fixed/fatura/diário are entered.

**AnnualRuler** (`forecast::annual_ruler`): the method's 20–30% band applied to one year's twelve `MonthMetric`s, and the **only** definition of it. **Lastro test**: a month ahead only backs the annual verdict when its launched outflow reaches 60% of the typical spend (median of the lived months' outflow, Economia included — what matters is whether the month looks lived); below that the month is _suspect_, carries the `missing_cents` it would take to cost a typical month, and the ruler falls back to the lived cut, printing the scope alongside the number so a partial year never reads as a closed one. Percentages truncate, like the monthly engine and the display; the shortfall to the 20% floor rounds, since it is printed as an invitation. `band_verdict` reads the ruler against the band — zeroed Economia with a ≥ 6-month reserve is the method's order kept, never a miss. `year_end_scenario` closes the year on the last month that has a balance and discounts only the suspect months inside that window. The Ano screen (`get_annual_ruler` → `src/screens/anoView.ts`) and the conversation facade (`get_year_analysis`) both read these functions and compose nothing of their own; `mia::tests::the_screen_and_the_conversation_read_the_same_ruler` is the proof. Comparing two years uses `avg_income_cents` over months **with** income, never year totals: a year in progress against a closed one would fake an income drop.

**binding_guardrail**: "pode gastar até X hoje" is the MIN of two limits — cash (no day goes negative) and savings (keep the annual Economizado% ≥ target). `binding_guardrail` says which one bit.

**Colchão**: net surplus kept in cash instead of a formal Economia transfer — a valid adaptation the app recognizes before teaching (ColchaoCard). Shown beside `registered_economia` so the two are never conflated.

**Phase** (adaptação): `map` (mapping — few lançamentos / no realized month) → `calibrate` (tuning the diário) → `operate` (Economizado% ≥ 20% and reserve ≥ 6 months). Derived from summary + forecast (`colchaoPhase`), not stored.

**Reserve months** (dashboard): derived live as reserve-account balance ÷ monthly cost of living (`realized_monthly_baseline`); the `reserve.current_months` column has no production writer. Carries an epistemic state: `verdict` with the full 6-complete-month window, `estimate` ("living portrait") with 1–5 months, `zero` (mapped reserve accounts at zero — "Sem reserva"), `no_record` (no mapped accounts or no baseline).

## Rules

- Daily budget is per Person, not per Profile. Multi-device inherits the same budget.
- Do not hardcode "me" into persisted data. Use Person rows.
- Fixed expenses are transactions with `is_fixed=true` and specific dates. Variable expenses are the daily (Diário) spend — ordinary `transaction` rows with `is_fixed=false`.
- Credit card = pay later. Debit/PIX = pay now. The methodology distinguishes them; the schema preserves that distinction.
- The reserve is separate from the savings account. A savings account may or may not contain the reserve.
- Categories are for diagnosis, not planning. The daily_budget is a single number.
- All writes to Google Sheets require structured diff, validation, and explicit human approval.
- Local labels may use personal names; public fixtures and docs must use generic names.
