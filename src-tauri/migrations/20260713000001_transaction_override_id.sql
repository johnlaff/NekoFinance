-- Substituição de override (`replace`) como SÉRIE dona da identidade: uma linha hipotética por
-- ocorrência suprimida, todas apontando para o `scenario_override` via `transaction.override_id`
-- (FK, `ON DELETE CASCADE`) em vez do marcador textual `" #repl:<override_id>"` na descrição.
-- Apagar a obrigação/recorrência mata o override e a série juntos — "substituir X por Y" nunca
-- degrada para "manter X e adicionar Y" (uma linha órfã). O backfill dos `#repl:` legados roda em
-- Rust no startup (parser ancorado do marcador não cabe em SQL puro); esta migração cria só a
-- coluna. O marcador é aposentado: nenhum caminho novo o escreve; o compare pareia por FK.
ALTER TABLE "transaction" ADD COLUMN override_id TEXT REFERENCES scenario_override(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_transaction_override
    ON "transaction" (override_id);
