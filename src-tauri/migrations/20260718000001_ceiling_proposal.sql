-- Proposta de teto do Diário lida da cerimônia documentada em nota de célula da planilha.
-- O import PROPÕE, nunca escolhe: o teto só vira `daily_budget` com confirmação explícita do
-- dono na UI. A identidade da proposta é o hash da nota normalizada — a mesma nota nunca
-- re-propõe (aceita ou dispensada), e uma nota NOVA supersede a pendente anterior.
CREATE TABLE IF NOT EXISTS ceiling_proposal (
    id             TEXT    PRIMARY KEY NOT NULL,
    note_hash      TEXT    NOT NULL UNIQUE,
    per_day_cents  INTEGER NOT NULL,
    divisor_days   INTEGER NOT NULL,
    -- [{"name": "...", "amount_cents": N}] na ordem da nota (itens mensais da cerimônia).
    items_json     TEXT    NOT NULL,
    -- "YYYY-MM" da célula (mais recente) onde a nota vive.
    source_month   TEXT    NOT NULL,
    status         TEXT    NOT NULL CHECK(status IN ('pending','accepted','dismissed')),
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    resolved_at    TEXT
);
