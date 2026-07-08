# Plan 071: ~~Make `line_item` identity survive note reordering~~ — REJECTED

> **Status: REJECTED (2026-07-04)** after adversarial review against the code. Do
> not implement. Kept as a record so the idea is not re-proposed.

## Why this was rejected

The plan proposed switching `line_item.id` from position-based (`li:<txn_id>:<position>`,
`import.rs:600`) to a content/description hash so per-item enrichment would survive a
user **reordering** the items inside a cell note. Reading the actual re-import path
shows the premise does not hold:

- `note_changed = prev_source_note.as_deref() != Some(row.raw_note.as_str())`
  (`import.rs:561`) is an **exact string compare** of the whole note.
- `keep_local = has_user_edited > 0 && !note_changed` (`import.rs:562`).
- **Reordering the items changes the raw note string**, so `note_changed` is always
  `true` on a reorder → `keep_local = false` → the importer **DELETEs every line item
  for that transaction and re-derives them from the note** (`import.rs:590`), resetting
  `is_user_edited` to 0.

So per-item enrichment is **not "migrated to the wrong item" on reorder — it is wiped
entirely**, and it is preserved only when the note is byte-for-byte unchanged (in which
case items aren't touched at all and their ids are irrelevant). The id scheme therefore
never needs to survive a note change: items are either untouched (note identical) or
fully re-derived (note changed). Changing position→hash cannot preserve anything the
current all-or-nothing `keep_local` mechanism drops.

The only real effect of the id scheme is idempotency of the importer's own
auto-derived rows across repeated imports of an **unchanged** note — which the existing
`li:<txn_id>:<position>` scheme already provides (stable positions on an unchanged note).

## If per-item edit durability across reorders is ever wanted

That is a different, larger change: make `keep_local` **per-item** (merge user edits by
a stable item key when the note changes) instead of all-or-nothing at the transaction
level. That is not what this plan proposed, and it should be its own spec if pursued.

## Evidence

- `src-tauri/src/google_sheets/import.rs:561-562` (note_changed / keep_local)
- `src-tauri/src/google_sheets/import.rs:590` (DELETE all items when `!keep_local`)
- `src-tauri/src/google_sheets/import.rs:600` (the `li:<txn_id>:<position>` id)
- `is_user_edited` is set only by the app edit path (`update_transaction_items_cmd`,
  `commands/transactions.rs`), and reset to 0 on every note-derived re-insert.

Confirmed independently by two adversarial reviewers and a direct read of the keep-local
path (all HIGH confidence).
