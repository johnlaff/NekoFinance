ALTER TABLE sync_log ADD COLUMN source_sheet TEXT;
ALTER TABLE sync_log ADD COLUMN checksum TEXT;

CREATE INDEX IF NOT EXISTS idx_sync_log_source ON sync_log(source_sheet, checksum);
