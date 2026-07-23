-- Vínculo estrutural de pessoa nos lançamentos derivados de nota (#reembolso:/#dividir:).
-- O import resolve/cria a pessoa ao processar o marcador, mas só gravava o vínculo no `split`; a
-- Entrada derivada carregava a pessoa apenas na descrição gerada ("Reembolso: {nome}"). A coluna
-- torna o vínculo consultável sem varrer texto — a agregação de dinheiro de terceiros lê a chave,
-- não a descrição.
--
-- O backfill em SQL puro é seguro porque o formato da descrição é DETERMINÍSTICO do próprio import
-- ("{Tipo}: {nome}"): tudo após o primeiro ':' é o nome da pessoa. `person` já existe — nenhuma
-- entidade nova. Idempotente: reexecutar reescreve o mesmo id (case-insensitive por nome).
ALTER TABLE "transaction" ADD COLUMN counterparty_person_id TEXT REFERENCES person(id);
UPDATE "transaction" SET counterparty_person_id = (
  SELECT p.id FROM person p
  WHERE LOWER(p.name) = LOWER(TRIM(substr(description, instr(description, ':') + 1))))
WHERE id LIKE 'derived:reembolso:%' OR id LIKE 'derived:dividir:%';
