CREATE TABLE IF NOT EXISTS reserve_snapshot (
    id TEXT PRIMARY KEY NOT NULL,
    reserve_id TEXT NOT NULL REFERENCES reserve(id) ON DELETE CASCADE,
    snapshot_date TEXT NOT NULL,
    current_months REAL NOT NULL,
    monthly_expense_avg INTEGER,
    total_reserve_amount INTEGER
);
