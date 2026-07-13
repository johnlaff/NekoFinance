-- Empréstimo hipotético como entidade de domínio: parâmetros numa tabela própria em vez do
-- sufixo " #loan:<groupId>:<rateBps>" nas descrições. `term_months`/`rate_bps` são os PARÂMETROS
-- (fonte do formulário de edição e da regeneração via PRICE); o que pesa na projeção são as
-- linhas presentes em `"transaction"` — apagar parcelas finais simula quitação antecipada sem
-- tocar aqui. O backfill dos grupos legados roda em Rust no startup (parser ancorado do marcador
-- + derivação dos parâmetros não cabem em SQL puro); esta migração cria só o schema.
CREATE TABLE IF NOT EXISTS scenario_loan (
    id                     TEXT PRIMARY KEY NOT NULL,
    scenario_id            TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    principal_cents        INTEGER NOT NULL CHECK (principal_cents > 0),
    rate_bps               INTEGER NOT NULL CHECK (rate_bps >= 0),
    term_months            INTEGER NOT NULL CHECK (term_months BETWEEN 1 AND 480),
    disbursement_date      TEXT NOT NULL,
    first_installment_date TEXT NOT NULL,
    description            TEXT NOT NULL,
    created_at             TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_scenario_loan_scenario_id
    ON scenario_loan (scenario_id);

-- NULL = linha hipotética solta (ou linha real); NOT NULL = principal/parcela de um empréstimo.
-- Apagar o empréstimo leva as linhas junto (CASCADE); apagar a última linha de um empréstimo
-- apaga o registro na mesma transação (invariante "sem fantasma", garantida no comando).
ALTER TABLE "transaction" ADD COLUMN loan_id TEXT REFERENCES scenario_loan(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_transaction_loan
    ON "transaction" (loan_id);
