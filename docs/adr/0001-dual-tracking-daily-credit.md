# ADR-0001: Dual Tracking — Daily Spend vs Credit Card

The methodology prescribes a single daily budget number (daily_spend) tracked against a daily spending limit (daily_budget). However, when a user pays with credit cards, the daily_spend is zero and the balance doesn't move — making the daily discipline metric artificially green.

## Decision

Track two parallel metrics per daily check-in:

1. **Régua 1 — Diário** (daily_spend): sum of debit/PIX/cash expenses. Compared against daily_budget.
2. **Régua 2 — Crédito** (credit_spend): sum of credit card expenses. Compared against monthly income for fat-shaming the credit bill.

Both are stored in `daily_checkin`. The Mia copilot reports both metrics independently, preventing self-deception when the user is 100% credit.

## Considered alternatives

- **Single metric forcing credit into daily_spend**: violates the methodology's core insight that credit spending doesn't immediately affect the balance. Also misleads the user about their real liquidity.
- **No tracking of parallel credit**: greenwashes the scenario. The user thinks they're within budget because daily_spend is zero, but the credit bill is accruing silently.

## Why record it here

This decision is surprising without context — a reasonable engineer would collapse both into one spend number. Future agents implementing Mia's diagnostic tools need to know there are two independent metrics, not one.
