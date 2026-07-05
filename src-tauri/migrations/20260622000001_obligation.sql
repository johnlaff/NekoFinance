-- Plan 069: user-confirmed "obligation" identity — the recurring series the spreadsheet
-- doesn't store. The sheet has no concept of a recurring obligation: a monthly rent is just
-- one item repeated inside a Saída cell every month, with nothing linking the twelve
-- occurrences. `obligation` is a NEKO EXTENSION (not a method artifact): the user names a
-- recurring item ONCE and Neko tracks every matching line item — always via a user-confirmed
-- preview, never inferred/silent.
--
-- This is a MATCH RULE, not a foreign key on line_item: `line_item` rows are RE-DERIVED from
-- the cell note on every import (clear + reinsert keyed on transaction_id), so any identity
-- stored directly on a line_item row would be wiped on the next sync. Resolving membership at
-- query time (rule -> matching rows) is the only design that survives re-import.
--
-- `match_desc` stores the NORMALIZED description (case/accent-folded, installment counter
-- stripped) — never the raw text — so the resolver only needs a straight equality check.
-- `match_section` stores the `normalize_item_section`-folded header, or NULL to match any
-- section. `kind` is derived from `match_section` via `classify_line_item` at creation time
-- (display-only cache; the section fold is still the source of truth).
--
-- View/index only: an obligation NEVER mutates line_item/transaction amounts and never touches
-- the cell-owns-total rule.
CREATE TABLE IF NOT EXISTS obligation (
    id            TEXT PRIMARY KEY NOT NULL,
    -- Authorship only: `transaction` has no person_id column (ownership flows through
    -- to_account_id -> account.owner_person_id), so this is NEVER part of the match — it just
    -- records who created the obligation.
    person_id     TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    match_desc    TEXT NOT NULL,
    match_section TEXT,
    kind          TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_obligation_person_id
    ON obligation (person_id);
