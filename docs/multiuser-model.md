# Multiuser Model

Neko Finance is single-device and local-first today, but the domain model must not assume a single person forever.

## Goals

- Support the primary user now.
- Support a partner/additional-card user later without reworking transactions.
- Keep future cloud sync possible without adding a backend today.

## Domain Concepts

| Concept                 | Meaning                                                                      |
| ----------------------- | ---------------------------------------------------------------------------- |
| `person`                | A human represented in local finance data.                                   |
| `profile`               | App login/profile on this device. Initially one profile.                     |
| `account`               | Bank account, credit card, wallet, or Google Sheet logical account.          |
| `transaction`           | Normalized financial movement.                                               |
| `owner_person_id`       | Person financially responsible for the transaction.                          |
| `payer_person_id`       | Person/account that paid at the point of sale.                               |
| `beneficiary_person_id` | Person who benefited from the purchase, if relevant.                         |
| `split`                 | Allocation of one transaction across multiple responsible people/categories. |

## Rules

- Do not hardcode "me" into persisted transactions. Use a local `person` row for the primary user.
- Additional-card spending must be representable as a separate owner/responsibility dimension.
- Shared expenses should be splits, not duplicated transactions.
- Every future sync event should include a stable local actor/profile ID.
- Local labels may use personal names, but public fixtures and docs must use generic names.

## Future SaaS Path

If this becomes multi-device or SaaS:

- Add authentication and remote sync after the local schema is stable.
- Keep local IDs and remote IDs separate.
- Use an append-only sync/event table for conflict resolution.
- Encrypt sensitive data at rest and in transit.
- Keep human approval for shared-sheet writes.
