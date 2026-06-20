# ADR-0001: Daily Spend vs Credit Card

> **Superseded in part (plan 027, 2026-06-20).** The "credit accumulates daily"
> half below ("Régua 2") was retired and the `daily_checkin` table that backed
> it was dropped. See **Update** at the bottom. The daily debit ritual
> ("Régua 1") remains, but it is recorded as ordinary Diário `transaction` rows,
> not in a dedicated table.

The methodology prescribes a single daily budget number (daily spend) tracked against a daily spending limit (daily_budget). However, when a user pays with credit cards, the daily spend is zero and the balance doesn't move — making the daily discipline metric artificially green.

## Original decision (historical)

Track two parallel metrics per daily check-in:

> "Régua 1/2" are Neko's internal names for the two tracks; they are not the method's terminology.

1. **Daily/debit track** ("Régua 1", daily spend): sum of debit/PIX/cash expenses. Compared against daily_budget.
2. **Credit/invoice track** ("Régua 2", credit spend): sum of credit card expenses. The original idea was to accumulate this per day into the invoice that lands on the due date, so a "green" daily track would not hide a growing bill.

Both were to be stored in a dedicated `daily_checkin` table.

## Considered alternatives

- **Single metric forcing credit into daily spend**: violates the methodology's core insight that credit spending doesn't immediately affect the balance. Also misleads the user about their real liquidity.
- **No tracking of parallel credit**: greenwashes the scenario. The user thinks they're within budget because the daily spend is zero, but the credit bill is accruing silently.

## Update (plan 027 — "credit accumulates daily" retired)

The "Régua 2 / daily-accruing credit" model never matched the method's source of
truth: a credit bill is **a single outflow on the due date**, not a per-day
ledger. During a cycle you increment one running total, but the recorded output
is one Saída lump at the vencimento — not a daily accrual. Neko already does
exactly this (the `classify()` routing + write-back fold credit into a single
due-date outflow), so the per-day credit track was redundant.

Consequences:

- The daily-accruing credit track ("Régua 2") is **retired**. There is no
  per-day credit accumulation; the bill is one lump on the due date (plan 022).
- The `daily_checkin` table — which only ever held these per-day numbers and had
  **no production writer** (always empty in production) — was **dropped** (plan
  027, forward-only migration). Its two readers were dead fallbacks over an
  always-empty table and were removed; behavior is unchanged for real users.
- The daily debit ritual ("Régua 1") **remains**, recorded as ordinary Diário
  `transaction` rows. Today's daily spend is computed directly from those rows.

A future "explicit daily-ritual gesture" feature would be a _new_ design
decision, not a revival of this table.
