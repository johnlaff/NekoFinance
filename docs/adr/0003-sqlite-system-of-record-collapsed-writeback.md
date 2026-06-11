# ADR-0003: SQLite as System of Record, Collapsed Write-Back to the Sheet

The app mirrors a personal finance spreadsheet whose canonical format is simple and hand-edited:
one row per day with aggregate income/fixed/daily columns. The app's domain model is richer than
those columns can express — per-transaction credit invoices accruing daily, per-person attribution
on consolidated card statements, net-zero reimbursement links, account liquidity classes, tags,
and what-if scenarios. The two representations cannot be kept losslessly equal in both directions.

## Decision

The local SQLite database is the **system of record**. The spreadsheet seeds it on import and
remains a human-friendly projection of it:

1. **Import** parses sheet rows into normalized transactions/splits (layout detection + mapping,
   spec 002). Imports are deduplicated by checksum; the sheet is never the loser of a sync.
2. **Write-back collapses, never expands.** When the app writes a material change to the sheet, it
   collapses rich structure into the sheet's canonical shape — e.g. a credit-card cycle becomes a
   single outflow lump on the due date plus a structured note — keeping the sheet exactly the
   format its owner edits by hand.
3. Every material write requires a structured before→after diff, validation, and explicit human
   approval (`sync_log` checksums detect concurrent sheet edits and force re-review).

## Considered alternatives

- **Sheet as system of record, app as a pure view**: rejected — the methodology features the app
  exists for (daily invoice accrual, reimbursement attribution, liquidity pockets, scenarios)
  are not representable in the sheet's columns; they would have to live in side files anyway.
- **Two-way lossless sync**: rejected — it forces either polluting the sheet with machine columns
  (breaking the hand-edited format) or losing data on every round trip. Conflict resolution cost
  is unbounded for a solo, local-first tool.
- **Abandoning the sheet entirely**: rejected for now — the sheet is the owner's trusted,
  battle-tested interface; the app must earn trust incrementally by staying compatible with it.

## Why record it here

This is the project's most consequential data-flow decision and it is easy to get backwards: a
reasonable engineer would treat the cloud spreadsheet as the source of truth and the local DB as a
cache. Future agents implementing sync, the approval UI, or the copilot's write tools must
preserve the collapse-on-write rule and the human-approval gate, or hand-edited sheet data will be
corrupted.
