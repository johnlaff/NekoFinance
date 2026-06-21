-- Plan 036: make the itemized breakdown EDITABLE in the app and keep local edits
-- alive across a re-import.
--
-- The importer (plan 035) re-derives line_item rows from the cell note on every
-- import (clear + reinsert keyed on transaction_id). Without a marker that tells
-- import "these items were typed by the user", a re-import would silently wipe a
-- breakdown the user just entered in the app.
--
-- `is_user_edited` flags rows written by the app's edit path. `source_note` on the
-- parent transaction snapshots the cell note as last seen by the importer (the
-- BASE for a 3-way-style decision on the breakdown, mirroring `source_amount`):
--   * note unchanged since last import + items are user-edited  -> keep local items
--   * note changed (sheet note is authoritative)                -> re-derive from note
ALTER TABLE line_item ADD COLUMN is_user_edited INTEGER NOT NULL DEFAULT 0;

-- Last cell note observed by the importer for this transaction. NULL = never imported
-- (manual app row) or pre-036 row. Used only to detect "did the sheet note change?".
ALTER TABLE "transaction" ADD COLUMN source_note TEXT;
