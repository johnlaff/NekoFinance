-- Plano 072 (slice A): fundação do "what-if" — cenários hipotéticos isolados do livro-razão real.
-- `scenario` é só o rótulo/dono do cenário; as linhas hipotéticas em si vivem em `"transaction"`
-- marcadas com `scenario_id` (ver a migração seguinte). Nenhum comando de CRUD nesta slice — só o
-- modelo de dados e o isolamento; a UI/CRUD chegam nas slices B/C.
CREATE TABLE IF NOT EXISTS scenario (
    id         TEXT PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL,
    person_id  TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_scenario_person_id
    ON scenario (person_id);
