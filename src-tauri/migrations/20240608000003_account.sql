CREATE TABLE IF NOT EXISTS account (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('bank','credit_card','wallet','savings','business')),
    owner_person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    institution TEXT,
    balance INTEGER NOT NULL DEFAULT 0,
    credit_limit INTEGER,
    closing_day INTEGER,
    due_day INTEGER,
    linked_account_id TEXT REFERENCES account(id),
    provider TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
