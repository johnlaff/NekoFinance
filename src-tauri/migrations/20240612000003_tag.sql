-- Spec 014: tags livres (nome + cor), transversais a qualquer lançamento, somam por mês.
-- `emoji` e `is_special` (fixa no topo) são afordâncias próprias do Neko, não do modelo de tags do
-- método. Substitui o orçamento-por-categoria (anti-padrão do método): a árvore granular de
-- `category` é rebaixada para tags; só `category.nature` (fixed/variable) permanece como atributo
-- do lançamento.

CREATE TABLE IF NOT EXISTS tag (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT 'var(--cat-jade)',
    emoji TEXT,
    is_special INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- N:N lançamento ↔ tag. ON DELETE CASCADE limpa o vínculo quando o lançamento some (diff-delete
-- do import). A identidade estável do import (spec 012) preserva o vínculo em re-imports normais.
CREATE TABLE IF NOT EXISTS transaction_tag (
    transaction_id TEXT NOT NULL REFERENCES "transaction"(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (transaction_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_transaction_tag_tag ON transaction_tag(tag_id);
