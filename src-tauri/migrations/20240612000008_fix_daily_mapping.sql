-- Conserta mappings gerados antes da correção do detector: o Diário variável era emitido como
-- `daily_budget` (target inexistente para o importador) e ficava inativo, então a coluna Diário
-- — a estrela do método — nunca era importada de planilhas reais. Realinha para `amount_daily`
-- e ativa. `daily_budget` continua sendo o nome da TABELA do check-in; aqui só tocamos no
-- target_field de mapeamento de coluna.
UPDATE sheet_mapping
SET target_field = 'amount_daily', is_active = 1
WHERE target_field = 'daily_budget';
