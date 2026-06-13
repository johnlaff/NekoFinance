-- Saldo corrente diário lido da planilha (coluna "Saldo" do método).
--
-- No método, o Saldo é o saldo encadeado que "bate com o banco": cada dia carrega o
-- anterior + Entrada − (Saída + Diário), e o ano puxa o fim do ano anterior. É a única
-- coluna que codifica o histórico acumulado + o carry-over de anos anteriores.
--
-- A projeção do Neko (forecast core) precisa de uma SEMENTE = saldo de partida. Sem esta
-- série a semente era 0 (nenhum bolso criado) e a projeção começava do zero, mostrando
-- "saldo zerado" e déficits falsos. Com ela, a semente = Saldo do dia mais recente ≤ hoje,
-- e a projeção continua a própria linha da planilha (spec 010, slice "seed pela planilha").
--
-- Replace-all por aba a cada import (igual às transações): re-importar substitui a série.
CREATE TABLE IF NOT EXISTS sheet_daily_balance (
    sheet_name    TEXT    NOT NULL,
    date          TEXT    NOT NULL,
    balance_cents INTEGER NOT NULL,
    is_projection INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (sheet_name, date)
);

-- A seleção da semente busca o saldo mais recente até hoje, varrendo por data.
CREATE INDEX IF NOT EXISTS idx_sheet_daily_balance_date ON sheet_daily_balance (date);
