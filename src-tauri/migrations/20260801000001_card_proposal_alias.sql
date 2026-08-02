-- Apelidos que a planilha usa para o MESMO cartão proposto.
--
-- A planilha distingue ciclos do mesmo cartão no próprio nome ("Nubank (26/09)", "Nubank
-- (26/12)"). Isso é anotação humana, não identidade: tratada como identidade, cada anotação
-- virava um cartão diferente para cadastrar. A identidade da proposta passa a ser a RAIZ do
-- nome, e as variantes viram apelidos — assim o cadastro já nasce reconhecendo todas as formas.
CREATE TABLE IF NOT EXISTS card_proposal_alias (
    id          TEXT PRIMARY KEY NOT NULL,
    proposal_id TEXT NOT NULL REFERENCES card_proposal(id) ON DELETE CASCADE,
    alias       TEXT NOT NULL UNIQUE
);
CREATE INDEX IF NOT EXISTS idx_card_proposal_alias_proposal
    ON card_proposal_alias (proposal_id);

-- Cada proposta pendente passa a conhecer o próprio alias como apelido.
INSERT INTO card_proposal_alias (id, proposal_id, alias)
SELECT lower(hex(randomblob(16))), id, alias
FROM card_proposal
WHERE status = 'pending';

CREATE TEMP TABLE proposal_root AS
SELECT id,
       source_month,
       CASE WHEN instr(alias, '(') > 1
            THEN rtrim(substr(alias, 1, instr(alias, '(') - 1))
            ELSE alias END AS root
FROM card_proposal
WHERE status = 'pending';

-- Sobrevivente de cada raiz: a de mês de origem mais antigo, com o id como desempate estável.
CREATE TEMP TABLE proposal_keep AS
SELECT root,
       MIN(source_month) AS source_month,
       (SELECT candidate.id FROM proposal_root candidate
        WHERE candidate.root = grouped.root
        ORDER BY candidate.source_month, candidate.id LIMIT 1) AS keep_id
FROM proposal_root grouped
GROUP BY root;

-- Os apelidos das propostas absorvidas passam para a sobrevivente.
UPDATE card_proposal_alias
SET proposal_id = (
    SELECT k.keep_id FROM proposal_root r JOIN proposal_keep k ON k.root = r.root
    WHERE r.id = card_proposal_alias.proposal_id
)
WHERE proposal_id IN (SELECT id FROM proposal_root);

DELETE FROM card_proposal
WHERE id IN (
    SELECT r.id FROM proposal_root r JOIN proposal_keep k ON k.root = r.root
    WHERE r.id <> k.keep_id
);

-- A sobrevivente assume a raiz como identidade e o mês mais antigo do grupo. A guarda de
-- colisão preserva uma proposta já resolvida que porventura ocupe esse mesmo nome.
UPDATE card_proposal
SET alias = (SELECT k.root FROM proposal_keep k WHERE k.keep_id = card_proposal.id),
    source_month = (SELECT k.source_month FROM proposal_keep k WHERE k.keep_id = card_proposal.id),
    display_name = CASE WHEN instr(display_name, '(') > 1
                        THEN rtrim(substr(display_name, 1, instr(display_name, '(') - 1))
                        ELSE display_name END
WHERE id IN (SELECT keep_id FROM proposal_keep)
  AND NOT EXISTS (
    SELECT 1 FROM card_proposal other
    WHERE other.id <> card_proposal.id
      AND other.alias = (SELECT k.root FROM proposal_keep k WHERE k.keep_id = card_proposal.id)
  );

DROP TABLE proposal_root;
DROP TABLE proposal_keep;
