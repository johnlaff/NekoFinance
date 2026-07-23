-- A tag é um interruptor de contabilidade — quatro réguas independentes
-- (Performance · Custo de vida · Economia · Diário médio) no lugar do "fora de tudo" único.
-- Cada flag = 1 tira o lançamento SÓ da régua homônima; o Saldo (cadeia de caixa) nunca tem
-- máscara. O flag antigo significava "fora de tudo", então o backfill liga os quatro — uma tag
-- que ignorava tudo continua ignorando tudo. A coluna antiga morre: o modelo de tags é local
-- do Neko, nenhum caminho de escrita externo depende dela.
ALTER TABLE tag ADD COLUMN exclude_from_performance    INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_cost_of_living INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_savings        INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_daily_avg      INTEGER NOT NULL DEFAULT 0;
UPDATE tag SET exclude_from_performance    = exclude_from_totals,
               exclude_from_cost_of_living = exclude_from_totals,
               exclude_from_savings        = exclude_from_totals,
               exclude_from_daily_avg      = exclude_from_totals;
ALTER TABLE tag DROP COLUMN exclude_from_totals;
