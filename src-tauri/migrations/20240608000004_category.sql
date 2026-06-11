CREATE TABLE IF NOT EXISTS category (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    parent_id TEXT REFERENCES category(id),
    nature TEXT NOT NULL CHECK(nature IN ('fixed','variable')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
