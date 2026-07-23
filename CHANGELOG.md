# Changelog

All notable changes to Neko Finance are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), entries are written
for humans, and versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Added

- The Hoje screen was recomposed around the daily verdict: a greeting hero
  ("Pode gastar hoje …" with the binding guardrail named and a teaching
  layer), the assistant's curation line, a day block that in card mode shows
  open invoices grouped by due date (per-card lines with status context,
  reimbursement tag and an honest footer for idle cards), a month insight in
  Mia's voice derived from the projected balance chain, upcoming movements
  (bills plus the next expected income), and a saldo + reserve pair with
  gauges. Desktop composes in two columns; mobile stacks under a large title
  coordinated with the app bar.
- Credit cards are now a first-class domain: register multiple cards (with
  additional cards per person inheriting the holder's cycle), track persisted
  invoices per card × cycle with derived status, and follow subscriptions and
  installments that pre-launch into future statements.
- New Cartões screen: card list with next due dates, invoice drill-down
  (purchases, series, linked reimbursements, reconciliation line, per-person
  sub-totals), direct statement-total adjustment, and card proposals surfaced
  from the spreadsheet with explicit accept/dismiss.
- Import recognizes card lines in cell notes by alias, materializes future
  invoices even when the cell total is zero, and never creates a card
  silently — unknown aliases become pending proposals.
- The card-mode gate now has two computable legs (savings alive **and**
  reserve ≥ 6 months), each reported honestly with its own state.

### Changed

- Registering from Hoje now always goes through the compose flow (dock FAB,
  sidebar CTA or the N shortcut) with explicit approval — the day block is a
  reading surface and no longer embeds a quick-register form.
- Write-back now writes one note line per card in the due-date cell instead
  of collapsing all credit into a single card's lump; the multi-card warning
  is gone because the limitation is gone.
- The forecast derives credit events from invoices (per card, by due date)
  for open and future cycles; realized history still follows the spreadsheet.

### Fixed

- Purchases made after a card's closing day are now assigned to the cycle
  that closes the following month; previously they could land on an invoice
  whose due date preceded the purchase itself.
