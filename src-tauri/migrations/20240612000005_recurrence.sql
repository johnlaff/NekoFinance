-- Spec 016: recorrências/séries (Repetir: Nunca/Diariamente/Semanalmente/Mensalmente).
-- Um lançamento recorrente gera N ocorrências (transações) compartilhando o mesmo `recurrence_id`.
-- Permite editar/apagar "deste ponto em diante" ou "toda a série", como no método.

CREATE TABLE IF NOT EXISTS recurrence (
    id TEXT PRIMARY KEY NOT NULL,
    frequency TEXT NOT NULL CHECK(frequency IN ('diaria', 'semanal', 'mensal')),
    infinite INTEGER NOT NULL DEFAULT 0,
    repetitions INTEGER,
    start_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Cada transação pode pertencer a uma série. NULL = lançamento avulso (Repetir = Nunca).
ALTER TABLE "transaction" ADD COLUMN recurrence_id TEXT REFERENCES recurrence(id);

CREATE INDEX IF NOT EXISTS idx_transaction_recurrence ON "transaction"(recurrence_id);
