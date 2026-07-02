# Multiuser Model

Neko Finance is single-device and local-first today, but the domain model must not assume a single person forever.

## Goals

- Support the primary user now.
- Support a partner/additional-card user later without reworking transactions.
- Keep future cloud sync possible without adding a backend today.

## Domain Concepts (implemented)

| Concept       | Meaning                                                                                                                                                                       |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `person`      | A human represented in local finance data (`person` table).                                                                                                                   |
| `profile`     | App login/profile on this device. Initially one profile.                                                                                                                      |
| `account`     | Bank account, credit card, wallet, or savings/business account; has `owner_person_id`.                                                                                        |
| `transaction` | Normalized financial movement (`type`, `payment_method`, `is_fixed`).                                                                                                         |
| `split`       | Allocation of one transaction across multiple responsible people. Carries `amount` and `owner_person_id` (who is responsible for that slice) — see `src-tauri/src/splits.rs`. |

Ownership today is expressed through `account.owner_person_id` and `split.owner_person_id` only.
There is no separate "who paid" or "who benefited" dimension on the transaction or split — those
would need their own columns (see Future below).

## Rules

- Do not hardcode "me" into persisted transactions. Use a local `person` row for the primary user.
- Additional-card spending must be representable as a separate owner/responsibility dimension.
- Shared expenses should be splits, not duplicated transactions.
- Every future sync event should include a stable local actor/profile ID.
- Local labels may use personal names, but public fixtures and docs must use generic names.

## Future / Planned (not implemented)

These concepts are useful shapes to keep in mind for later multi-person work, but do not exist in
the schema or code today — no `payer_person_id` or `beneficiary_person_id` column exists anywhere:

- A **payer** dimension: who paid at the point of sale, when it differs from who is responsible
  (`owner_person_id`).
- A **beneficiary** dimension: who benefited from the purchase, if relevant and distinct from payer
  and owner.

If added, these should be explicit nullable columns (not overload `owner_person_id`), with a
migration and tests, following the same pattern as `split.owner_person_id`.

## Future SaaS Path

If this becomes multi-device or SaaS:

- Add authentication and remote sync after the local schema is stable.
- Keep local IDs and remote IDs separate.
- Use an append-only sync/event table for conflict resolution.
- Encrypt sensitive data at rest and in transit.
- Keep human approval for shared-sheet writes.
