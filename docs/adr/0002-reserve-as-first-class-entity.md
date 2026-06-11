# ADR-0002: Reserve as a First-Class Entity

The methodology treats the emergency reserve as the foundation of financial health: "sem reserva = dívida". It's not just a number in a savings account — it's a concept with its own metrics (target months of expenses, current months, trend up/down/flat, recalculated periodically).

## Decision

Model the emergency reserve as its own entity (`reserve` table) rather than a field on `account` or a derived query. Include `target_months`, `current_months`, `trend`, and a separate `reserve_snapshot` table for monthly history to detect direction.

## Considered alternatives

- **Account sub-type (type=savings + goal field)**: loses the methodology's first-class treatment. Can't track trend independently. Mixes the instrument (savings account) with the concept (emergency fund). A person might have a savings account that is NOT the reserve, and the reserve might span multiple accounts.
- **Derived from transaction history**: fragile. Depends on perfect categorization of every transfer-in/out. Doesn't capture the deliberate "this is reserved, don't touch" nature of the methodology's reserve concept.

## Why record it here

The decision to separate reserve from account is not obvious from the multiuser model alone. A future engineer would likely model it as an account field, losing the trend tracking and methodology alignment.
