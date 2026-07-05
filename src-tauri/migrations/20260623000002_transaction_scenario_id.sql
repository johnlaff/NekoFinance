-- Plano 072 (slice A): marca uma linha de `"transaction"` como pertencente a um cenário hipotético.
-- NULL = livro-razão REAL (todo lançamento existente e todo lançamento futuro criado pelas
-- ferramentas normais) — SEM backfill, a coluna nasce NULL em toda linha atual. NOT NULL = linha
-- hipotética "e se", dona de um `scenario`, que NUNCA deve aparecer no forecast/métricas/write-back
-- do livro real. `ON DELETE CASCADE`: apagar o cenário limpa as linhas hipotéticas que ele possui.
ALTER TABLE "transaction" ADD COLUMN scenario_id TEXT REFERENCES scenario(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_transaction_scenario
    ON "transaction" (scenario_id);
