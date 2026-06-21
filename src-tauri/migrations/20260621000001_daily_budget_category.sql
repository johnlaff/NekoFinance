-- Plano 045: quebra por categoria do orçamento mensal do gasto variável (Diário).
-- Cada linha é uma categoria nomeada com um valor-alvo mensal (centavos inteiros positivos).
-- A soma das linhas ativas DEVE bater com o `daily_budget.amount` ativo (validado na camada de
-- aplicação, não no banco — a soma é derivada na leitura). FK para `daily_budget` (não para
-- `person`) para que, ao deprecar um orçamento, as categorias viajem junto para referência
-- histórica. Aditiva: NENHUMA migração existente é editada.
CREATE TABLE IF NOT EXISTS daily_budget_category (
    id           TEXT    PRIMARY KEY NOT NULL,
    budget_id    TEXT    NOT NULL REFERENCES daily_budget(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,   -- ex.: "Alimentação", "Transporte" (rótulos genéricos)
    amount_cents INTEGER NOT NULL,   -- alvo mensal, magnitude positiva
    position     INTEGER NOT NULL DEFAULT 0  -- ordem de exibição, 0-based
);

CREATE INDEX IF NOT EXISTS idx_daily_budget_category_budget_id
    ON daily_budget_category (budget_id);
