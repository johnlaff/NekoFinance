-- Base do merge de 3 vias do total declarado da fatura: o valor como veio da
-- planilha no último import. NULL = nunca importado (fatura nascida no app).
ALTER TABLE invoice ADD COLUMN source_stated_total_cents INTEGER;
