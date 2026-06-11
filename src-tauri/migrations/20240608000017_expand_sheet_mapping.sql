ALTER TABLE sheet_mapping ADD COLUMN layout_id TEXT REFERENCES sheet_layout(id);
ALTER TABLE sheet_mapping ADD COLUMN block_offset INTEGER DEFAULT 0;
ALTER TABLE sheet_mapping ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_sheet_mapping_layout ON sheet_mapping(layout_id);
CREATE INDEX IF NOT EXISTS idx_sheet_mapping_sheet ON sheet_mapping(sheet_name);
