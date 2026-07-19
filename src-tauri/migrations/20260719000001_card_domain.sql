-- Domínio do cartão: fatura persistida (cartão×ciclo), série única de
-- assinatura/parcelamento, aliases de import e proposta de cartão.
-- Status da fatura NÃO é armazenado: deriva de today × closing_date × due_date
-- (função pura em cards.rs) — sem drift entre colunas e calendário.

CREATE TABLE IF NOT EXISTS invoice (
    id           TEXT PRIMARY KEY NOT NULL,
    account_id   TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    -- identidade cartão×ciclo: "YYYY-MM" do mês do VENCIMENTO
    cycle_month  TEXT NOT NULL,
    closing_date TEXT NOT NULL,
    due_date     TEXT NOT NULL,
    -- autoridade quando presente (import/ajuste manual); NULL = derivar da
    -- soma das compras vinculadas
    stated_total_cents INTEGER,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (account_id, cycle_month)
);

CREATE INDEX IF NOT EXISTS idx_invoice_account_due ON invoice (account_id, due_date);

CREATE TABLE IF NOT EXISTS card_series (
    id                TEXT PRIMARY KEY NOT NULL,
    account_id        TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    description       TEXT NOT NULL,
    amount_cents      INTEGER NOT NULL CHECK (amount_cents > 0),
    -- NULL = assinatura (infinita); N = parcelamento em N vezes
    count             INTEGER CHECK (count IS NULL OR count BETWEEN 1 AND 120),
    -- "YYYY-MM" da primeira fatura (ancoragem: ocorrência por fatura consecutiva)
    start_cycle_month TEXT NOT NULL,
    -- assinatura cancelada a partir desta fatura (inclusive); NULL = ativa
    canceled_from_cycle_month TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_card_series_account ON card_series (account_id);

-- Alias de import: casa a descrição normalizada da linha da seção de cartões.
CREATE TABLE IF NOT EXISTS card_alias (
    id         TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    alias      TEXT NOT NULL UNIQUE
);

-- Alias desconhecido no import vira PROPOSTA (nunca conta-fantasma); mesma
-- identidade-única do padrão ceiling_proposal: o mesmo alias nunca re-propõe.
CREATE TABLE IF NOT EXISTS card_proposal (
    id           TEXT PRIMARY KEY NOT NULL,
    alias        TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    source_month TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('pending','accepted','dismissed')),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at  TEXT
);

-- Compra → fatura (padrão série-dona); NULL = transação sem cartão.
ALTER TABLE "transaction" ADD COLUMN invoice_id TEXT REFERENCES invoice(id);
CREATE INDEX IF NOT EXISTS idx_transaction_invoice ON "transaction" (invoice_id);

-- Ocorrência de série; apagar a série leva as ocorrências junto (CASCADE).
ALTER TABLE "transaction" ADD COLUMN card_series_id TEXT REFERENCES card_series(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_transaction_card_series ON "transaction" (card_series_id);

-- Reembolso = Entrada VINCULADA (nunca redução): no máximo UM alvo por
-- Entrada — fatura (inclusive parcial), compra ou série. Invariante de
-- exclusividade validada na fronteira dos commands.
ALTER TABLE "transaction" ADD COLUMN refund_invoice_id TEXT REFERENCES invoice(id) ON DELETE SET NULL;
ALTER TABLE "transaction" ADD COLUMN refund_txn_id TEXT REFERENCES "transaction"(id) ON DELETE SET NULL;
ALTER TABLE "transaction" ADD COLUMN refund_series_id TEXT REFERENCES card_series(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_transaction_refund_invoice ON "transaction" (refund_invoice_id);
