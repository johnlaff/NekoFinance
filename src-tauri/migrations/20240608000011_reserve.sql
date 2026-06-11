CREATE TABLE IF NOT EXISTS reserve (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    target_months INTEGER NOT NULL,
    current_months REAL NOT NULL,
    trend TEXT CHECK(trend IN ('up','down','flat')),
    last_calculated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_reserve_person ON reserve(person_id);
