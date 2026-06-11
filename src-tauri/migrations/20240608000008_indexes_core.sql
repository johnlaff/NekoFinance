CREATE INDEX IF NOT EXISTS idx_transaction_date ON "transaction"(date);
CREATE INDEX IF NOT EXISTS idx_transaction_account ON "transaction"(from_account_id, to_account_id);
CREATE INDEX IF NOT EXISTS idx_split_transaction ON split(transaction_id);
