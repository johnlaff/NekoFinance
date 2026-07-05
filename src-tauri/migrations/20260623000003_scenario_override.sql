-- Plano 072 (slice A): como um cenário MODIFICA o real sem editar o real. Além de acrescentar
-- linhas hipotéticas (`transaction.scenario_id`), um cenário precisa poder AGIR sobre uma obrigação
-- ou recorrência real a partir de uma data — "suprimir" (ex.: simular cancelar uma assinatura) ou
-- "substituir" (ex.: simular um aumento de aluguel). `op` decide a semântica; qual delas junto com
-- `from_date` é lido/aplicado pelo motor de projeção de cenário (fora desta slice — só o modelo).
-- Exatamente um alvo (`obligation_id` XOR/OR `recurrence_id`, nunca os dois ausentes) — o CHECK
-- garante que toda override aponta para algo real a modificar.
CREATE TABLE IF NOT EXISTS scenario_override (
    id            TEXT PRIMARY KEY NOT NULL,
    scenario_id   TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    op            TEXT NOT NULL CHECK(op IN ('suppress','replace')),
    from_date     TEXT NOT NULL,
    obligation_id TEXT REFERENCES obligation(id) ON DELETE CASCADE,
    recurrence_id TEXT,
    CHECK (obligation_id IS NOT NULL OR recurrence_id IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_scenario_override_scenario_id
    ON scenario_override (scenario_id);
