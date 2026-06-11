CREATE TABLE IF NOT EXISTS sync_log (
    id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT
);
