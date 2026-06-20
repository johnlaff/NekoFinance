-- Plan 035: breakdown of an itemized cell into its constituent parts.
-- Each row is one line of the cell note parsed as `R$ <valor> - <descrição>`.
-- Items are descriptive children: the parent transaction total is unchanged.
-- ON DELETE CASCADE: removing the parent cleans up its items automatically.
CREATE TABLE IF NOT EXISTS line_item (
    id        TEXT    PRIMARY KEY NOT NULL,
    -- FK to the parent transaction (may be realized OR projected).
    transaction_id TEXT NOT NULL
        REFERENCES "transaction"(id) ON DELETE CASCADE,
    -- Absolute magnitude in cents (positive integer). Direction = parent type.
    amount_cents   INTEGER NOT NULL,
    description    TEXT    NOT NULL DEFAULT '',
    -- 0-based insertion order, preserving the note line order.
    position       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_line_item_transaction_id
    ON line_item (transaction_id);
