-- Conciliação avançada (spec 013): snapshot do último import por campo (o "base" do merge de
-- 3 vias) + fila de conflitos para o gate humano.

-- Base: valor do campo como veio da planilha no último import. NULL = nunca importado (manual).
ALTER TABLE "transaction" ADD COLUMN source_amount INTEGER;
ALTER TABLE "transaction" ADD COLUMN source_description TEXT;

-- Conflitos de import (ambos local e planilha mudaram desde o base). Um por (transação, campo).
CREATE TABLE IF NOT EXISTS import_conflict (
    id              TEXT PRIMARY KEY,
    transaction_id  TEXT NOT NULL,
    field           TEXT NOT NULL,
    base_value      TEXT,
    local_value     TEXT NOT NULL,
    sheet_value     TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at     TEXT,
    resolution      TEXT,
    UNIQUE (transaction_id, field)
);

CREATE INDEX IF NOT EXISTS idx_import_conflict_unresolved
    ON import_conflict (resolved_at)
    WHERE resolved_at IS NULL;
