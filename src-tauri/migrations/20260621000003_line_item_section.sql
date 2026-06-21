-- Plan 048: preserve note section headers in the write-back round-trip.
-- `section` stores the header line (e.g. "CONTAS:", "CARTÕES:") that appeared
-- immediately before this item in the original cell note. NULL = no header.
ALTER TABLE line_item ADD COLUMN section TEXT;
