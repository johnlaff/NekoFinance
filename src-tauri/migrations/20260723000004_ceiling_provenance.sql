-- Proveniência da cerimônia do teto. A tela do teto REPRODUZ a nota original da célula (a
-- notação do dono é o contrato do write-back), então o texto cru precisa sobreviver ao import:
-- até aqui só o hash normalizado da nota persistia, e uma citação reconstruída dos itens seria
-- paráfrase vendida como reprodução.
ALTER TABLE ceiling_proposal ADD COLUMN raw_note TEXT;

-- `source_note` acompanha o orçamento que a nota sustenta (propagado no aceite da proposta); o
-- rito no app grava um registro novo sem nota — a prova, a partir daí, é a cerimônia do app.
ALTER TABLE daily_budget ADD COLUMN source_note TEXT;

-- Quando a cerimônia foi FEITA (YYYY-MM): o mês da nota, no aceite de proposta; o mês corrente,
-- no rito. Distinto de `start_date`/`calculated_at`, que marcam quando o registro entrou em
-- vigor — uma nota de setembro aceita hoje continua sendo uma cerimônia de setembro.
ALTER TABLE daily_budget ADD COLUMN ceremony_month TEXT;

-- Registros anteriores à coluna: a data de início é a melhor testemunha disponível.
UPDATE daily_budget SET ceremony_month = substr(start_date, 1, 7) WHERE ceremony_month IS NULL;
