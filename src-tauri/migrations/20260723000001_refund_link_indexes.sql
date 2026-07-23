-- O vínculo de reembolso por linha (has_refund_link) roda um EXISTS correlacionado
-- pelos três alvos; fatura já tinha índice — compra e série ganham os seus, parciais
-- (só Entradas realmente vinculadas entram, o caso raro).
CREATE INDEX IF NOT EXISTS idx_transaction_refund_txn
    ON "transaction" (refund_txn_id) WHERE refund_txn_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_transaction_refund_series
    ON "transaction" (refund_series_id) WHERE refund_series_id IS NOT NULL;
