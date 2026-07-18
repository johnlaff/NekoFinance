-- Divisor da cerimônia do teto: `per_day = total mensal ÷ divisor_days`. Faz parte da
-- cerimônia documentada pelo dono (a nota real mantém "/ 31 Dias" mesmo em meses de 30 dias),
-- então persiste com o orçamento em vez de ser derivado do calendário. NULL = teto informado
-- direto por dia (sem cerimônia itemizada).
ALTER TABLE daily_budget ADD COLUMN divisor_days INTEGER;
