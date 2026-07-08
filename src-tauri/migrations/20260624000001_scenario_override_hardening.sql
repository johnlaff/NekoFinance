-- Revisão adversarial do plano 072 (slice A.1): endurece `scenario_override`. Sem CRUD em
-- produção ainda (tabela sempre vazia), então reconstruir é seguro — SQLite não suporta ALTER de
-- CHECK/FK. Dois defeitos corrigidos:
-- 1) CHECK antigo (`obligation_id IS NOT NULL OR recurrence_id IS NOT NULL`) permitia os DOIS
--    setados ao mesmo tempo, quebrando "exatamente um alvo" — agora é XOR de verdade.
-- 2) `recurrence_id` não tinha FK — apagar uma recorrência deixava overrides "pendurados"
--    apontando pra um id morto. Agora tem `REFERENCES recurrence(id) ON DELETE CASCADE`, igual
--    ao `obligation_id`.
DROP TABLE IF EXISTS scenario_override;

CREATE TABLE scenario_override (
    id            TEXT PRIMARY KEY NOT NULL,
    scenario_id   TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    op            TEXT NOT NULL CHECK(op IN ('suppress','replace')),
    from_date     TEXT NOT NULL,
    obligation_id TEXT REFERENCES obligation(id) ON DELETE CASCADE,
    recurrence_id TEXT REFERENCES recurrence(id) ON DELETE CASCADE,
    CHECK ((obligation_id IS NULL) != (recurrence_id IS NULL))
);

CREATE INDEX idx_scenario_override_scenario_id ON scenario_override (scenario_id);
CREATE INDEX idx_scenario_override_obligation_id ON scenario_override (obligation_id);
CREATE INDEX idx_scenario_override_recurrence_id ON scenario_override (recurrence_id);
