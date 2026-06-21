-- Plano 045: data de vencimento opcional (lembrete de conta a pagar) por lançamento.
-- NULL = sem vencimento explícito (padrão de toda linha existente). Formato ISO "YYYY-MM-DD",
-- igual à coluna `date`. A coluna `date` continua sendo a data de CAIXA (quando o dinheiro sai);
-- `due_date` é a data de VENCIMENTO mostrada no calendário de contas próximas. NÃO entra no
-- encadeamento do Saldo nem no forecast — é metadado consultivo. Aditiva: nenhuma migração editada.
ALTER TABLE "transaction" ADD COLUMN due_date TEXT;
