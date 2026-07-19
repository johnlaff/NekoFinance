# ADR-0004: Persisted Invoices as the Credit-Card Substrate

Before this decision, credit was a projection-time artifact: any expense with
`payment_method='credit'` was folded into a single due-date lump derived from the *first*
configured card's cycle, and the forecast aggregated credit events by date with no card identity.
Multiple cards, shared statements with per-person sub-totals, subscriptions/installments that
pre-launch into future statements, and reimbursements tied to a statement could not be expressed.

## Decision

The card domain gets first-class persisted state, with the sheet still the human-readable
projection (ADR-0003):

- **`invoice` table** — one row per card × cycle, keyed by the due-date month
  (`UNIQUE(account_id, cycle_month)`). Dates are explicit per invoice, so changing a card's cycle
  never rewrites history. Status (`prevista·aberta·fechada·paga`) is **derived from the calendar,
  never stored** — no drift between columns and time.
- **`stated_total_cents` is the authority** when present; the purchase sum is the itemization
  detail. Every purchase-changing gesture adjusts the stated total additively **in the same SQL
  transaction** (register adds, delete subtracts, re-assign moves, series occurrences add/remove).
  Divergence between stated and itemized renders as a synthetic reconciliation line — never
  hidden, never an editable item.
- **Three-way merge for the stated total** (`source_stated_total_cents` as base), mirroring the
  transaction-level reconcile: sheet-only change applies, local-only change survives, both-changed
  goes to the existing conflict queue and blocks write-back until resolved.
- **`card_series`** — one entity for subscriptions (`count NULL`) and installments (`count N`);
  occurrences are projected purchases anchored to **consecutive invoices** (not dates), with `n/N`
  derived from the cycle index, never stored.
- **Refund = linked income** (`refund_invoice_id | refund_txn_id | refund_series_id`, at most one
  target): reimbursements never reduce an invoice or any judging ruler (gross regime); the link
  powers a marked, didactic net reading only.
- **Import matches by alias** (normalized card names + explicit aliases) via a direct note-grid
  scan outside the row/checksum flow, so a future invoice whose cell total is zero still
  materializes structure; an unknown alias becomes a pending proposal — accounts are never
  created silently.
- **Write-back writes one note line per card** under the cards section of the due-date cell,
  surgically merged with non-card sections; the post-write audit realigns each invoice's
  stated/source pair from its own written line.

## Considered alternatives

- **Derive invoices at query time (no table)**: rejected — statement totals from the sheet
  (`stated_total`) need a home with merge state; series need stable anchoring; and "the invoice
  the day reads" must exist even when no purchase does (zero-total future cells).
- **Store invoice status**: rejected — status is a pure function of today × closing × due; a
  stored copy could contradict the calendar after any clock or cycle change.
- **Effective total = derived sum with stated as a mere annotation**: rejected — the method's
  gesture is "add to the open statement total"; the sheet line value is what the owner audits, so
  it must be the authority, with itemization as supporting detail.
