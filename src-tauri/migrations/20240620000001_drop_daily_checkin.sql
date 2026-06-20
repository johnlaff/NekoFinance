-- Remove a tabela `daily_checkin`, vestigial e nunca populada em produção. Ela foi criada
-- (migration 0010) mas nenhum writer de produção a alimenta — o ritual diário é gravado como uma
-- transação Diário comum, e a fatura do cartão é um único lançamento de saída na data de
-- vencimento. A coluna `credit_spend` era o último resíduo da ideia abandonada de "crédito
-- acumulando por dia". Os dois únicos leitores eram fallbacks sobre uma tabela sempre vazia (no-op),
-- já removidos. Forward-only: instalações existentes apenas descartam a tabela (vazia) no próximo boot.
DROP TABLE IF EXISTS daily_checkin;
