CREATE TABLE IF NOT EXISTS split (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES "transaction"(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    category_id TEXT REFERENCES category(id),
    owner_person_id TEXT NOT NULL REFERENCES person(id),
    note TEXT
);
