# Changelog

All notable changes to Neko Finance are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), entries are written
for humans, and versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Added

- `mia-bench bakeoff` measures the pinned model matrix and decides which one becomes
  the conversation's default — the choice moved from intuition to measurement. A live
  canary verifies every pin against the provider's zero-retention catalog before any
  paid round; a cost probe runs one repetition per model and refuses upfront when the
  whole design would not fit the cap; a one-repetition sieve runs every candidate plus
  the reference ceiling, and a three-repetition final runs the survivors. Nothing is
  decided on partial measurement: the sieve must cover every cleared pin and the final
  every selected finalist. Blind-judgment answers go to a separate sheet that names no
  model, and `mia-bench julgar` closes the loop offline, writing the final decision into
  the report. Adopting the pin stays a manual gesture.

- The Horizonte screen became the cash radar: the only view that looks strictly
  forward (projected, in months, to the end of the data), answering the method's
  question — is there a hole in the road? It opens on a three-voice verdict (clear
  path · a squeeze ahead · nothing booked yet) with the smallest point named and
  the honest twin (where December ends if the un-ballasted months cost the usual).
  The road to December draws the booked line, the un-ballasted zone, the dotted
  "if it costs the usual" trace, and the zero and low point, with a numbers fold.
  A twelve-month signal grid colours each month by its end-of-month balance band
  and carries the three epistemic states (lived · projected-with-ballast · review ·
  no record), each month opening in the Calendar. The projected commitments group
  by month (installment `n/N`, reimbursement as a linked income), and the "E se?"
  entry carries the two financing-gate rulers. Delineated from its neighbours: O
  ano judges the method, Horizonte guards the cash, and Hoje's "can spend" is
  proven by the horizon's lowest point. The ballast rule, typical spend, trust
  frontier and typical trace all come from the forecast engine — no backend change.
- The Teto do diário screen became the record of a decision with proof: it opens
  on the ceiling itself with the detected spending mode stating what the day is
  measured against, then shows the ceremony that produced it (the variable-month
  items, the `total ÷ days` formula rounded up, and the original spreadsheet note
  reproduced verbatim behind a disclosure), the age of that ceremony against the
  method's three-month cadence, and how the day reads the ceiling. Editing is no
  longer the screen's permanent state: it became a three-beat rite on the surface
  (items → divisor → before/after acceptance), with a guard that explains the
  consequence when the new ceiling is lower and still lets it through, a guided
  five-question ceremony for whoever has no ceiling yet, and a calm inline refusal
  when the divisor is missing. The spreadsheet proposal moved from a banner into a
  verdict state of its own, and the ceiling's provenance (the note and the month
  the ceremony was made) now survives the import.
- The O ano screen became the place where the method actually judges: it opens
  on a verdict and the band ruler (fixed 0–40% scale with the 20–30% target
  zone) instead of KPI tiles and a seven-column table, because the savings rate
  is only meaningful as a yearly average. A ballast test now gates that verdict
  — a month ahead only supports it when its booked outflow reaches 60% of
  typical spend; below that the month is flagged for review and the verdict
  falls back to what was actually lived, with the sample size printed on the
  ruler so it never claims to measure a full year from a few months. "Where
  December ends" projects the year in two scenarios (as booked, and if the
  flagged months cost the usual), the twelve months read as one row each
  (income rail, savings fill, 20% tick), the yearly numbers moved into an
  expandable list, and income is compared across years. On desktop the verdict
  and ruler span full width with the supporting cards in a two-column bento.
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

- The public build variable that carries the Google desktop-client credential is
  now `VITE_GOOGLE_DESKTOP_CLIENT_KEY`. A local `.env` that still names it
  `VITE_GOOGLE_CLIENT_SECRET` silently loses background token refresh — rename the
  key, the value is unchanged. Anything a `VITE_` prefix inlines into the browser
  bundle is published, so the name no longer promises a secrecy it cannot keep.
- "Performance acumulada" is gone from the product. Summing monthly performance
  over a year is not something the method does — once you start setting money
  aside, savings is the number that matters, not the leftover. The yearly
  reading it used to occupy is now "where December ends", projected in two
  scenarios. The "Comparar anos" tab went with it: the comparison the method
  asks for is income across years, so that now lives on the screen permanently
  instead of behind a tab.
- The "Estimativa" mark no longer stacks a dotted underline on top of its pill
  border — the chip alone reads as tappable, and doubling the signal made the
  line noisy wherever the mark appears.
- Tags now carry four independent exclusion flags (Performance, Custo de vida,
  Economia, Diário médio) instead of a single all-or-nothing toggle — each flag
  controls whether a tagged movement counts in that ruler. The balance chain
  remains untouched (Saldo always reflects real cash movement). The Tags screen
  is now a ruler-exception panel: it displays the cost-of-living verdict with
  current exclusions, third-party movement aggregations, and per-tag effects on
  each ruler.
- The Configurações screen was recomposed under the identity direction: it
  opens with a trust verdict ("Tudo neste dispositivo" plus a live state line
  that reports disconnection, pending changes and import conflicts with the
  same weight as good news), organizes everything into Conexão, Privacidade,
  Bolsos, Aparência and Rotina sections, and folds the dense spreadsheet
  panel and local import behind a "Gerenciar" door. A dark-theme switch now
  lives in Aparência next to the accent selector, and the design system
  gained a proper Switch control whose off state stays visible in the light
  theme.
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
- Import recognizes a card reimbursement by identity, not only by the
  `#reembolso:` note marker: an income naming a card in the lexicon on that
  invoice's due date is linked to it. The marker keeps precedence because it
  carries who reimburses, inference only acts when the reimbursement accounts
  for the whole income, and a link the owner removed is never recreated.
- The open-invoice block reads the net commitment, labelling the part that
  comes back, and ranks cards by it. Auditable receipts stay gross: the net
  reading exists only where it is marked.
- A zero-valued invoice no longer counts as an open one — it leaves the day's
  list, the card counter and the total, and no longer hides a real invoice of
  the same card. The row stays recorded, and the card's history still shows
  the cycle.
- The cash-limit sentence names the day the calculation actually used — the
  lowest projected balance over the whole horizon — instead of the end of the
  current month, and says which month when that day lies ahead.
