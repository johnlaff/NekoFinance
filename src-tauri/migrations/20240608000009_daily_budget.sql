CREATE TABLE IF NOT EXISTS daily_budget (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    status TEXT NOT NULL CHECK(status IN ('active','under_review','deprecated')),
    free_income INTEGER,
    calculated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
