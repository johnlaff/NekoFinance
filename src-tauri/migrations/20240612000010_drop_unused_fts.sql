-- Remove a infraestrutura FTS5 nunca populada. As tabelas `transaction_fts`/`category_fts` foram
-- criadas (migration 0015) mas nenhum writer de produção as alimenta (sem triggers, sem rebuild) e
-- a busca de Lançamentos é filtrada no cliente. Manter tabelas mortas confunde o schema. Recriar
-- com triggers + rebuild quando a busca full-text for de fato implementada.
DROP TABLE IF EXISTS transaction_fts;
DROP TABLE IF EXISTS category_fts;
