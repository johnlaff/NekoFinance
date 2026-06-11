CREATE TABLE IF NOT EXISTS sheet_layout (
    id TEXT PRIMARY KEY NOT NULL,
    sheet_name TEXT NOT NULL,
    year INTEGER,
    month_names_row INTEGER NOT NULL DEFAULT 0,
    header_row INTEGER NOT NULL DEFAULT 1,
    data_start_row INTEGER NOT NULL DEFAULT 2,
    day_column INTEGER NOT NULL DEFAULT 0,
    block_size INTEGER NOT NULL DEFAULT 6,
    date_direction TEXT NOT NULL DEFAULT 'both' CHECK(date_direction IN ('past_only','future_only','both')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
