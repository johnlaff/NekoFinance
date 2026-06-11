CREATE TABLE IF NOT EXISTS daily_checkin (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    daily_spend INTEGER NOT NULL DEFAULT 0,
    credit_spend INTEGER NOT NULL DEFAULT 0,
    daily_budget_id TEXT REFERENCES daily_budget(id),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_daily_checkin_person_date ON daily_checkin(person_id, date);
CREATE INDEX IF NOT EXISTS idx_daily_budget_person_status ON daily_budget(person_id, status);
