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
A real financial instrument — bank account, credit card, wallet, or savings account. Not a logical grouping (see Sheet Mapping). Has `type` (bank, credit_card, wallet, savings, business, meal_voucher, pension, fgts) and optional fields per type (institution, closing_day, credit_limit, linked_account_id for additional cards). Credit-card accounts have their own writer (outside pockets — the card is a liability, not a pocket): holder cards own a cycle (`closing_day` + `due_day`, both 1–31), additional cards link to a holder via `linked_account_id`, carry their own `owner_person_id` and inherit the holder's cycle. Card names plus explicit `card_alias` rows are the import-matching identities; `credit_limit` is discreet display only, never a ruler input.
_Avoid_: Sheet, category, group

**Invoice** (invoice):
The persisted credit-card statement: one row per card × cycle, keyed by the due-date month (`UNIQUE(account_id, cycle_month)`), with explicit `closing_date`/`due_date` per invoice (changing a card's cycle never rewrites history). Status (`prevista · aberta · fechada · paga`) is derived from the calendar, never stored. `stated_total_cents` is the authority when present (the sheet line value / direct adjustment); linked purchases are the itemization detail, and every purchase-changing gesture adjusts the stated total additively in the same SQL transaction. Stated-vs-itemized divergence renders as a synthetic reconciliation line ("não itemizado") — never hidden, never an item. Three-way merge on the stated total uses `source_stated_total_cents` as base; both-changed lands in the existing import-conflict queue and blocks write-back. See ADR-0004.
_Avoid_: Bill, statement (in code), fatura estimada

**Card Series** (card_series):
One entity for both subscriptions (`count NULL`, materialized through December of the current year, minimum 3 occurrences) and installments (`count N`). Occurrences are projected purchases anchored to **consecutive invoices** (not dates); `n/N` derives from the cycle index and is never stored. Editing regenerates open/future occurrences under the same identity; canceling a subscription takes effect from a cycle onward. Ordinary `recurrence` is never used for card purchases.

**Refund link** (on Transaction):
A reimbursement is an income row **linked** to at most one target — `refund_invoice_id` (partial allowed), `refund_txn_id` (a purchase) or `refund_series_id` — never a reduction of the invoice or of any judging ruler (gross regime: full income + full outflow). The link powers a marked, didactic net reading only. The `#reembolso:` note marker's derived income gets the invoice link at import time. Beyond `#reembolso:`, import recognizes a reimbursement when an income names a card in the lexicon on that invoice's due date; the marker takes precedence because it carries who reimburses, and an owner-declined link is not recreated by inference.

**Card Proposal** (card_proposal + card_proposal_alias):
An unknown card name found in a cards note section at import becomes a pending proposal. Identity is the alias ROOT — the sheet marks cycles inside the name (`Nubank (26/09)`, `Nubank (26/12)`), which is human annotation, not identity; every spelling seen is kept as a proposal alias, and `source_month` is the OLDEST month the card appears in (the scan walks the grid by day before by month, so first-seen is a visit order, not a date). Resolving a proposal goes two ways: accept (asks for cycle/owner, creates the account carrying every alias) or attach (the proposal is another spelling of a card that already exists — adds the aliases to it, creates nothing). Accounts are never created silently.

**Card cycle: mould × invoice**:
The account's `closing_day`/`due_day` is a MOULD, used only to derive cycles that do not exist yet; each `invoice` persists its own `closing_date`/`due_date`, so the real dates are already per-cycle. Both days accept 1–31 and shorten to the month's last day when the day does not fit (Feb closes on the 28th for a card that closes on the 29th) — shortening keeps the purchase in its cycle, where a fixed day-28 fallback would push it a whole cycle forward. Same-month pairs above day 28 are rejected at the boundary: in February both shorten to the same day, and a cycle that closes on its due date does not exist. When the issuer moves a single month's closing (business day, holiday), the owner corrects THAT invoice (`set_invoice_dates`) instead of changing the mould — the app derives no business-day rule of its own. A corrected `closing_date` survives import; a corrected `due_date` yields to the sheet on the next one, because for an imported invoice the due date is observed from the row, not configured.

**Card Lexicon** (`cards::CardLexicon`):
The identities the domain already knows a card by, in the form that resolves a note line. Exists because the sheet declares an invoice two ways: under the `CARTÕES` section header and — when the header is missing — as an ordinary line naming the card (`Fatura Bradesco`). The second form is only recognizable against identities the owner already declared: never by issuer or bank keyword, which would read "Fatura Vivo" as a card. Sources are the sheet's own cards sections, registered accounts, and pending proposals; a dismissed proposal is out, because the owner already said it is not a card. Section stays the authority — the lexicon only recovers what a missing header would otherwise drop into fixed outflow, where it would also escape the invoice lump's precedence and double-charge the due date.

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
A global mode detected purely from the data over a moving window (2 complete months + current): daily constancy (≥ 4 distinct days AND > R$ 50 in a month) ⇒ debit; window without constancy AND card movement ⇒ card; default debit. Card movement is what the SHEET declares — a cards section, or a line naming a card in the lexicon, on the expense side only (on the income side the same name is the refund of whoever splits the bill). Deriving it from the resolved `EventKind::Cartao` instead made the mode depend on the owner having registered the account, and read someone who spends entirely on credit as a debit user. The two Debit outcomes are distinguished by `spending_mode_detected`: the window supports the verdict, or it is the insufficient-data default — the surface must not claim "detected from your data" about the value that is left when the engine does not know. Hysteresis is asymmetric by construction — a stray purchase never flips into card mode, one month of constancy flips back to debit. In card mode the day's surface reroutes to the faturas (month's Cartão total + next due date, now derived per card from persisted invoices — `upcoming_invoices`) and a zeroed Diário is legitimate-by-design, not a gap. Card-mode legitimacy carries the method's gate (`card_gate`) with two computable legs — the 20–30% savings alive (annual economia ruler ≥ 20% floor) AND reserve ≥ 6 months — composed honestly: any leg below ⇒ below; both alive ⇒ alive; otherwise unknown (an incomputable leg never fabricates a verdict). The third canonical leg ("no rush toward the next patrimony goal") is didactic copy, never computed. Product principle: guide, never punish.

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

### Frontend Architecture

**View (funnel gate)**:
A screen's `*View.ts` is the only place allowed to translate `src/lib/api.ts`'s raw shim DTOs into the domain shapes the screen renders — every other file under `src/` is fenced off from importing `lib/api` directly, type-only included, by the `no-restricted-imports` gate in `eslint.config.js`. See ADR-0006 for the exception zones (a view and its tests, the Mia runtime, `src/hooks/**`, the IPC mock infra). The legacy allowlist (`eslint.lib-api-allowlist.mjs`) is empty since ADR-0011 closed the funnel — `npm run check` is green with zero violations of the rule and the anti-rot check stays wired in to keep it that way.
_Avoid_: DTO, wire type, raw API import

**View as full shim gate (write + cache key)**:
The full migration pattern (ADR-0007, `tagsView.ts` is the reference) goes past ADR-0006's read-only gate: the view also owns the stable `useCommand` fetcher, the cache-key builder (`tagsScreenCacheKey(ym)`), and one wrapper per write command in the screen's domain vocabulary (`createTagCmd`, `updateTagCmd`, `toggleTagRuler`), even when a wrapper only forwards to the shim untouched. The screen still owns _when_ to call `invalidateCommands()` (generic `lib/useCommand` infra, not `lib/api`) after a write resolves — the view only decides _what_ to call. A screen at this full pattern imports only its `*View.ts` and `lib/useCommand`, never `lib/api` in any form; reaching this bar is what earns a screen's removal from `eslint.lib-api-allowlist.mjs`. `scenariosView.ts` is an earlier, partial stage (read/pure-computation only, `scenarios.tsx` still on the allowlist) — not itself the reference for a full migration.
_Avoid_: manual cache-key string, bare shim write call from a screen

**Compositor extends the domain view, no `composeView.ts`**:
`shell/Compose.tsx` (the create/edit drawer opened from any screen) and the Lançamentos screen's support modules (`newTransactionCard.ts`, `newTransactionOptions.ts`, `NewTransactionForm.tsx`, `ObligationsPanel.tsx`) write and read the same transaction/series/obligation/card domain as `TransactionsScreen.tsx` — so they all reach the full pattern through `lancamentosView.ts`, not a shell-local view of their own. `lancamentosView.ts` owns every fetcher/cache-key (`monthTransactionsFetcher`, `monthGridFetcher`, `dashboardSummaryFetcher`, `forecastFetcher`, `obligationHistoryFetcher`, `previewObligationFetcher`, `pocketsFetcher`, `listTagsCmd`, `listObligationsCmd`, `listCardsCmd`) and every write wrapper (`createTransactionCmd`, `updateTransactionCmd`, `updateTransactionItemsCmd`, `deleteTransactionCmd`, `updateSeriesAllCmd`/`updateSeriesFromCmd`, `deleteSeriesAllCmd`/`deleteSeriesFromCmd`, `setTransactionTagsCmd`, `createCardSeriesCmd`, `registerCardPurchaseCmd`, `createObligationCmd`, `deleteObligationCmd`, `getLineItemsCmd`) this domain needs, across both `src/screens/` and `src/shell/` importers — an intentional exception to "a view lives in its own directory": the boundary here is the domain (one transaction/series/obligation model), not the screen that happens to render it. `no-restricted-imports` has no screen/shell boundary of its own, so `Compose.tsx` importing `../screens/lancamentosView` is a plain relative import, same as any other consumer of a `*View.ts`.
_Avoid_: a `composeView.ts` duplicating lançamento reads/writes, `Compose.tsx` importing `lib/api` directly

**Shell session view**:
App-level cross-cutting state (auth status, local app-setting reads, sync recency) has no owning screen, so it doesn't fit a `src/screens/*View.ts`. ADR-0008 extends the funnel gate to `src/shell/*View.ts` — `shellView.ts` is the read-only reference: reexported types (`AuthStatus`) and stable fetchers (`fetchAuthStatus`, `fetchAppSetting`, `fetchLastSyncAt`), same contract as ADR-0006, no write wrappers since nothing at the shell level writes. `App.tsx` and `AppShell.tsx` import only from `shellView.ts`, `hojeView.ts` (for the shared `fetchForecast` behind the nav hints), and `lib/useCommand` — never `lib/api`. The decision line: a plain read the generic `useCommand` already resolves belongs in a `*View.ts`; state with its own lifecycle (retry, polling, event subscription) belongs in a `src/hooks/**` hook instead (see `useWriteBackPending.ts`).
_Avoid_: reading shell-only commands from `lib/api` inside `App.tsx`/`AppShell.tsx`, inventing a hook for a plain fetch

**Sheets domain view**:
The Sheets/write-back flow (Google connect, remote + local `.xlsx` import, write-back preview/apply) is split across `features/sheets/` (`GoogleSheetsPanel.tsx`, `LocalXlsxImport.tsx`, `WriteBackPreview.tsx`, `writeBack.ts`), `screens/dashboard/WriteBackPending.tsx` (the dashboard's "Sincronizar" shortcut) and Settings' Conexão section — no single one of them owns the domain. ADR-0009 extends the funnel gate to `src/features/sheets/sheetsView.ts`, same domain-boundary reasoning as `lancamentosView.ts`: `WriteBackPreview.tsx` and `WriteBackPending.tsx` call the exact same preview/apply fetchers, so a per-file view would duplicate that read. All five consumers (+ their tests) import only from `sheetsView.ts`, never `lib/api`. Settings' non-Sheets commands (backup, app info, Mia consent, reminder, sync recency) reach the same full pattern through `screens/configView.ts` instead; `getDailyBudget` already had a home in `tetoView.fetchDailyBudget`.
_Avoid_: a per-component Sheets view duplicating the write-back preview/apply fetchers, `SettingsScreen.tsx` importing `lib/api` for a Sheets or Settings command directly

**Feature-local views (Pockets, Reconcile, Onboarding) and design-system prop types**:
ADR-0010 closes the last named-exception features and the design system out of the `lib/api` allowlist. `features/pockets/pocketsView.ts`, `features/reconcile/reconcileView.ts`, and `features/onboarding/onboardingView.ts` follow the `sheetsView.ts` precedent — a feature-local view for a domain with no owning screen — each holding only the read/write wrappers its one feature needs. Design-system components never import a DTO from `lib/api`, type-only included: `BalanceTrajectory` and `LineItemEditor` declare a local structural prop type (`BalanceTrajectoryPoint`, `LineItemEditorItem`) matching only the fields they read/write, and any domain type shaped the same way (e.g. `lancamentosView.ts`'s `LineItemDraft`) satisfies it via structural typing, with no cast.
_Avoid_: a design-system component importing `ForecastDay`/`LineItemDraft`/any shim DTO, a feature reading `lib/api` directly instead of through its own `*View.ts`

**Funnel closed (empty legacy allowlist)**:
ADR-0011 zeroes `eslint.lib-api-allowlist.mjs` — the last entry, `useShowReceipt.ts`, moved from `src/lib/` to `src/hooks/useShowReceipt.ts` since it's cross-screen preference state (Teto and Copilot) with no owning screen, the same shape as `useWriteBackPending.ts`. `LIB_API_ALLOWLIST_CEILING` is now `0`; `scripts/check-lib-api-allowlist.mjs` keeps running in `npm run check` so a future direct import can't quietly reopen the allowlist instead of earning a view.
_Avoid_: adding a path back to `eslint.lib-api-allowlist.mjs` instead of routing a new import through a `*View.ts` or `src/hooks/**`

### Screen Copy

**Selo do veredito**:
The single line of body copy under a screen's headline that changes with the verdict's state (ui-standards rule 42 allows exactly one). It reads as prose but is not didactic: it varies with the user's data, so it stays inline instead of collapsing behind "Como funciona?" (ADR-0013).
_Avoid_: subtitle, caption (too generic — this line specifically tracks the verdict)

**Legenda de cálculo**:
The caption naming the operands of a number printed just above it (ui-standards rule 3: inline stays for variable data). Never collapses, and is never mistaken for a permanent didactic paragraph even when it reads as a full sentence (ADR-0013).
_Avoid_: helper text, description (too generic — this caption specifically names operands)

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

**AnnualSavings**: year-to-date figures over the **months lived, the current month included** — the window the spreadsheet's `%` column uses, which never waits for a month to close. `economia_ruler_cents / realized_income_cents` is the method's **Economizado%** (Economia transferred to reserve ÷ income; target 20–30% as an ANNUAL average, never a monthly pass/fail), truncated. Distinct from `realized_savings_cents` (net surplus = income − outflow = the **Colchão**), which is the buffer, not Economizado. **Patrimônio (pension/FGTS/illiquid) never enters this ruler** under any reserve condition: it is a bucket of its own, published alongside for reading. The whole figure comes from `forecast::annual_ruler` — deriving it a second time here would open two truths. `card_gate` and the day's savings guardrail read this same ruler — one window, one criterion. `registered_economia_cents` (Economia over the COMPLETE months) is a portrait published alongside, never a second denominator. `economia_state` is `verdict` or `no_record` (the UI then shows the Colchão as a marked estimate — economia has no zero-diagnostic branch, since the real spreadsheet uses typed zero and blank interchangeably).

**MonthCoverage**: per future month, how much of the typical baseline outflow has been pre-launched. Drives the Previsibilidade card — an empty future month makes the projection optimistic until salary/fixed/fatura/diário are entered.

**AnnualRuler** (`forecast::annual_ruler`): the method's 20–30% band applied to one year's twelve `MonthMetric`s, and the **only** definition of it. **Lastro test**: a month ahead only backs the annual verdict when its launched outflow reaches 60% of the typical spend (median of the lived months' outflow, Economia included — what matters is whether the month looks lived); below that the month is _suspect_, carries the `missing_cents` it would take to cost a typical month, and the ruler falls back to the lived cut, printing the scope alongside the number so a partial year never reads as a closed one. Percentages truncate, like the monthly engine and the display; the shortfall to the 20% floor rounds, since it is printed as an invitation. `band_verdict` reads the ruler against the band — zeroed Economia with a ≥ 6-month reserve is the method's order kept, never a miss. `year_end_scenario` closes the year on the last month that has a balance and discounts only the suspect months inside that window. The Ano screen (`get_annual_ruler` → `src/screens/anoView.ts`) and the conversation facade (`get_year_analysis`) both read these functions and compose nothing of their own; `mia::tests::the_screen_and_the_conversation_read_the_same_ruler` is the proof. Comparing two years uses `avg_income_cents` over months **with** income, never year totals: a year in progress against a closed one would fake an income drop.

**binding_guardrail**: "pode gastar até X hoje" is the MIN of two limits — cash (no day goes negative) and savings (keep the year's Economizado% ≥ the 20% floor). `savings_headroom_cents` is the AnnualRuler's shortfall to that floor with the sign flipped, over the window the ruler judges — a derivation, never a second division; `null` when that window has no income (the ruler is inactive, not zero). `binding_guardrail` says which one bit. `ForecastDto` also carries `savings_band_verdict` / `savings_band` / `savings_band_scope_lived` — the same `band_verdict`, floor/ceiling and `scope_lived` the Ano screen publishes on `AnnualRulerDto`, over the same ruler, so both screens read one verdict rather than each re-deriving the band from a number's sign. The Hoje screen (`src/screens/hojeView.ts`, `savingsBandBroken`) reads this verdict directly — `in_band`/`above_band` is a live band, anything else releases the day's savings guardrail and drives the diagnostic copy.

**Colchão**: net surplus kept in cash instead of a formal Economia transfer — a valid adaptation the app recognizes before teaching (ColchaoCard). Shown beside `registered_economia` so the two are never conflated.

**Phase** (adaptação): `map` (mapping — few lançamentos / no realized month) → `calibrate` (tuning the diário) → `operate` (Economizado% ≥ 20% and reserve ≥ 6 months). Derived from summary + forecast (`colchaoPhase`), not stored.

**Reserve months** (dashboard): derived live as reserve-account balance ÷ monthly cost of living (`realized_monthly_baseline`); the `reserve.current_months` column has no production writer. Carries an epistemic state: `verdict` with the full 6-complete-month window, `estimate` ("living portrait") with 1–5 months, `zero` (mapped reserve accounts at zero — "Sem reserva"), `no_record` (no mapped accounts or no baseline). Once the balance passes `reserve_target_cents` (cost of living × `RESERVE_MIN_MONTHS`), `reserve_surplus_cents` states the excess — the method's next question is not "how much is missing" but what the surplus funds.

**The savings ruler protects the band, it does not punish a broken one.** The 20–30% floor is an ANNUAL AVERAGE — "some months are more, some are less; the YEAR has to be 20 to 30, on average" — and the method's question about a new commitment is prospective: _will this stop me from saving 20–30%?_ So `safe_to_spend_today` lets the savings guardrail bind only while `band_verdict` reads the band as alive (`in_band` / `above_band`); on `below_band`, `zero_by_choice` and `no_record` the ruler releases and cash decides — the verdict is the boundary, not the sign of a number, which is why zeroed Economia over a standing reserve needs no special branch. The accumulated shortfall belongs to the year behind, and no amount of not-spending today undoes it — gating the day would punish what does not come back, and would turn the ceiling into a permanent zero for whoever is furthest from the target. The diagnosis leaves the ceiling, not the screen: the surface still states that the year is under 20% and points at the month's performance, which is the method's actual remedy. Same shape as the reserve decision below, and the same conclusion the market reached — YNAB resolves overspending at month roll so each month is "a fresh start … without the baggage of past mistakes".

**The reserve does not gate the day.** It is the shock absorber that gets USED when the balance goes negative, never the floor that forbids spending — so it stays out of `safe_to_spend_today`, whose two rulers are cash (don't go into the red) and savings (keep the year's 20–30%). Treating the method's 6-month target as a cash floor zeroed the day's ceiling for exactly the person still building the reserve, which inverts the instrument and contradicts "guide, never punish"; it also double-charged, since mapped reserve accounts are already excluded from the liquid seed. When the projected balance does go negative, the surface offers the withdrawal as an Entrada with the shortfall pre-filled — the launch stays the owner's, and the repayment is theirs to schedule. Without a mapped reserve the advice changes to the month's performance, because suggesting an impossible withdrawal is empty counsel.

## Rules

- Daily budget is per Person, not per Profile. Multi-device inherits the same budget.
- Do not hardcode "me" into persisted data. Use Person rows.
- Fixed expenses are transactions with `is_fixed=true` and specific dates. Variable expenses are the daily (Diário) spend — ordinary `transaction` rows with `is_fixed=false`.
- Credit card = pay later. Debit/PIX = pay now. The methodology distinguishes them; the schema preserves that distinction.
- The reserve is separate from the savings account. A savings account may or may not contain the reserve.
- Categories are for diagnosis, not planning. The daily_budget is a single number.
- All writes to Google Sheets require structured diff, validation, and explicit human approval.
- Local labels may use personal names; public fixtures and docs must use generic names.
- The conversation runtime refuses a round without a durable consent record (`app_setting.mia_consent`, versioned to the consent text it accepted — a text change invalidates the record). The provider key lives in the OS keychain under its own service and never reaches an event, log, database row, or error payload. Without consent or key, the six local answers still respond offline.
- The conversation's eval catalog lives in `evals/mia/` (public, synthetic fixtures; six families) and runs through the `mia-bench` binary (`src-tauri/src/mia/bench/`), which drives the REAL loop, facade, and HTTP provider adapter — never a reimplementation. It spends real money under a double lock (accumulated-cost cap in the runner + a dedicated provider key with its own panel limit), writes a dated report to `evals/mia/reports/`, and refuses to run in CI. `evals/mia/README.md` is the catalog's contract.
- `mia-bench bakeoff` measures the pin matrix and decides the default model: a live canary verifies every pin against the provider's zero-retention catalog before any paid round, a one-repetition sieve runs every candidate plus the reference ceiling, and a three-repetition final runs the surviving two or three. The default goes to whoever cleared the mechanical suite in a complete run, lowest recorded cost first — but only if the final actually compared two finalists end to end, and only once no blind-judgment answer is still pending (until then the report carries `leading_model` and a null `default_model`). Adopting it — moving the `Default` role in `src-tauri/src/mia/provider/pins.rs` — stays a manual, deliberate gesture. The bakeoff refuses every narrowing flag (`--model`, `--only`, `--cases-dir`) and only lets `--max-spend-usd` lower the cap, because a verdict drawn from a slice would read exactly like one drawn from the whole catalog. A cost probe runs one repetition per cleared pin before the sieve and projects the whole design; if the projection exceeds the cap — or the probe itself cannot finish one round per model — the run stops there with the number, instead of spending the cap to discover the same thing. A single spend lock spans every run, sized from the matrix cardinality, with each round's cap tightened to whatever is left of both the total and the phase; a round the lock cuts short counts as neither a model error nor a measurement. `mia-bench bakeoff --resume <report>` retakes an interrupted run: it recomputes every inherited run from the raw repetitions (never the report's `score` block), refuses a report whose catalog, case families, pin matrix, or per-run pin identity (model, endpoint, operator, beta headers, reasoning floor, token-cap field — all recorded in every run report) no longer matches, demands a complete cost probe covering the whole matrix — it is the only projection the resumed final has — reconciles the declared spend against probe plus runs, requires the sieve to be complete, and reuses a final run only when it survives the same check and belongs to the recomputed survivors — anything doubtful sends that pin back to run again. Inherited money never counts against the new run's spend lock; the report publishes `spent_micro_usd` (this run), `inherited_micro_usd`, and their sum as `total_cost_micro_usd`, with each inherited run carrying `inherited_from` and its original timestamp. A report written before the run record carried the request configuration can only be resumed with `--assume-pin-identity`, which supplies the _absent_ fields and never overrides a recorded mismatch; the new report then carries `pin_identity_assumed: true`. Every cost in a report is what the provider charged at measurement time — never a price table in the code — so an inherited cost is the price of its own date. When the cost tie-break compares runs from different dates, the published rationale names each pin with its own date and calls the tie-break a weak basis instead of claiming "cheapest"; same-date or single-eligible comparisons still assert what they measured. The same rule applies to the post-blind-judgment decision. "Nothing is decided on partial measurement" carries one carve-out: a final run ended by `cost_meter_broken` (two rounds with no declared cost from the same pin) leaves the comparison entirely — the instrument failed, not the candidate — so the final's quorum drops to one and whoever remains can win by walkover, with the rationale naming the excluded pin, the reason, and the walkover itself. A walkover waives the opponent and nothing else: the lone winner clears the very same gates as a compared one (complete run, zeroed mechanical suite, no echoed bait, no rejected blind ticket). `spend_ceiling` and `operational` still veto the decision. The cost-date rule is a safeguard, not a correction: it holds identically when the tariff did not move between the two dates, because neither the file nor the code knows whether it did. Blind-judgment answers go to a separate `<date>-julgamento-cego.json` sheet that names no model — the ticket-to-model key lives in the main report, read afterwards. `mia-bench julgar --report … --verdicts …` closes the loop offline (no provider, no key, no spend): it demands one verdict per ticket, refuses a sheet from another run, rejects a model outright on a single rejected ticket, and writes the final `default_model` into the report.
