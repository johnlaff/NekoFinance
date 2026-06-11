CREATE TABLE IF NOT EXISTS profile (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    device_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
