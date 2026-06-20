-- Plan 034: flag opt-in de exclusão por tag.
-- Quando exclude_from_totals = 1, lançamentos que carregam esta tag são omitidos das
-- métricas derivadas (Performance, Custo de vida, Economizado%) — mas NÃO do Saldo
-- (a cadeia de caixa reflete o movimento real de dinheiro). DEFAULT 0 preserva o
-- comportamento atual (tag incluída) sem migração de dados.
ALTER TABLE tag ADD COLUMN exclude_from_totals INTEGER NOT NULL DEFAULT 0;
