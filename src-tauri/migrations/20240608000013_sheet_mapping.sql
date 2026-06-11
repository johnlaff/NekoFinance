CREATE TABLE IF NOT EXISTS sheet_mapping (
    id TEXT PRIMARY KEY NOT NULL,
    sheet_name TEXT NOT NULL,
    column_letter TEXT NOT NULL,
    column_header TEXT,
    target_table TEXT NOT NULL,
    target_field TEXT NOT NULL,
    date_direction TEXT NOT NULL DEFAULT 'both' CHECK(date_direction IN ('past_only','future_only','both')),
    sheet_row_offset INTEGER NOT NULL DEFAULT 0
);
