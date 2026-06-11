CREATE TABLE IF NOT EXISTS "transaction" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
    amount INTEGER NOT NULL,
    description TEXT,
    date TEXT NOT NULL,
    payment_method TEXT CHECK(payment_method IN ('debit','credit','pix','cash')),
    is_fixed INTEGER NOT NULL DEFAULT 0,
    from_account_id TEXT REFERENCES account(id),
    to_account_id TEXT REFERENCES account(id),
    is_projection INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
