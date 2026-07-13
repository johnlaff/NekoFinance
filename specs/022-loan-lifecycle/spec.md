# Spec 022 — Ciclo de vida do empréstimo hipotético (entidade `scenario_loan`)

## Decisão

O empréstimo simulado deixa de ser uma convenção de strings (sufixo `" #loan:<groupId>:<rateBps>"`
na descrição de cada linha) e passa a ser uma **entidade de domínio**: a tabela `scenario_loan`
guarda os parâmetros; as linhas hipotéticas apontam para ela via `transaction.loan_id`
(FK, `ON DELETE CASCADE`). Com identidade própria, o empréstimo ganha ciclo de vida completo —
criar, **editar** (regenera a série) e **remover** (grupo inteiro) — cada operação numa única
transação SQL, com a série sempre derivada da tabela PRICE (`price_installment`), nunca de
matemática livre.

## Modelo de dados

```sql
CREATE TABLE scenario_loan (
    id                     TEXT PRIMARY KEY NOT NULL,
    scenario_id            TEXT NOT NULL REFERENCES scenario(id) ON DELETE CASCADE,
    principal_cents        INTEGER NOT NULL CHECK (principal_cents > 0),
    rate_bps               INTEGER NOT NULL CHECK (rate_bps >= 0),
    term_months            INTEGER NOT NULL CHECK (term_months BETWEEN 1 AND 480),
    disbursement_date      TEXT NOT NULL,
    first_installment_date TEXT NOT NULL,
    description            TEXT NOT NULL,
    created_at             TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE "transaction" ADD COLUMN loan_id TEXT REFERENCES scenario_loan(id) ON DELETE CASCADE;
```

- `term_months`/`rate_bps` são os **parâmetros** do empréstimo; o que efetivamente pesa na
  projeção são as linhas presentes (o usuário pode apagar parcelas finais para simular quitação
  antecipada — ajuste fino legítimo, preservado).
- O sufixo `#loan:` **deixa de ser escrito**. O sufixo `#repl:` (substituição de override) não
  muda nesta entrega.

## Migração de legado (backfill)

A migração SQL cria só o schema. O backfill roda em Rust no startup, logo após
`sqlx::migrate!` (mesmo caminho de erro visível do setup), e é **idempotente** — processa apenas
linhas de cenário cuja descrição ainda termina com o marcador `#loan:` (parser ancorado idêntico
ao `parse_loan_marker`):

1. Agrupa por `groupId` do marcador.
2. **Derivação limpa** → cria `scenario_loan` + aponta `loan_id` + remove o sufixo das descrições,
   tudo numa transação por grupo:
   - taxa: do marcador (idêntica em todas as linhas do grupo);
   - principal e data do desembolso: da única linha `income` do grupo;
   - prazo `N`: do rótulo `"parcela i/N"` (mesmo `N` em todas as parcelas, `i` distintos, valores
     iguais);
   - data da 1ª parcela: data da parcela de menor `i`, recuada `i−1` meses.
3. **Sem derivação limpa** (sem principal, rótulos inconsistentes, valores divergentes…) → só
   remove o sufixo: as linhas viram lançamentos soltos comuns, nada é apagado.

## Comandos (todos atômicos)

| Comando                       | Contrato                                                                                                                                                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `create_scenario_loan`        | Grava a entidade + principal (na data do desembolso) + N parcelas com `loan_id`, sem marcador; devolve o `loan_id` criado (a UI foca o grupo novo).                                                                                                    |
| `update_scenario_loan`        | Valida que o empréstimo pertence ao cenário; atualiza os parâmetros e **regenera a série inteira** (DELETE das linhas + re-INSERT) na mesma transação. Ajustes finos (parcelas removidas à mão) não sobrevivem — invariante de determinismo vale mais. |
| `delete_scenario_loan`        | `DELETE FROM scenario_loan` — o CASCADE leva principal + parcelas junto.                                                                                                                                                                               |
| `delete_scenario_transaction` | Passa a ser transacional: apagada a última linha de um `loan_id`, o registro `scenario_loan` morre na mesma transação — **sem fantasma**; o estado "Sem parcelas restantes" deixa de existir.                                                          |
| `list_scenario_transactions`  | Cada linha expõe `loan_id` (a UI agrupa por ele, não mais por parse de descrição).                                                                                                                                                                     |
| `list_scenario_loans`         | Novo: entidades do cenário (parâmetros para o formulário de edição e para o summary do grupo).                                                                                                                                                         |

`get_scenario_forecast` (compare): `detect_loan` agrupa por `loan_id` e lê a taxa da entidade;
principal/parcela/prazo continuam derivados das **linhas presentes** (quitação antecipada simulada
reduz o total pago reportado). Paridade de comportamento: só o primeiro grupo (ordem data, id)
vira `LoanBreakdown`; linhas de um segundo empréstimo seguem como "add" em `changes`.

## Contrato de interação (UI)

- **Desembolso explícito**: o formulário ganha o campo "Data do desembolso" (default: hoje),
  visível na criação e na edição — elimina o artefato do principal "no passado" sair da
  trajetória enquanto as parcelas pesam.
- **Grupo com ações**: o Disclosure do empréstimo ganha "Editar" e "Remover". Remover pede
  confirmação nomeando o que morre ("Principal + 36 parcelas saem do cenário"); a lixeira por
  linha permanece sem confirmação (ajuste fino).
- **Editar**: reabre o formulário de empréstimo pré-preenchido (título e botão mudam para o modo
  edição). Se o grupo tem linhas removidas à mão, a confirmação avisa que serão restauradas.
- **Sucesso da criação**: anúncio em região live + scroll/foco no grupo recém-criado com realce
  breve (desligado sob `prefers-reduced-motion`); o summary "Recebe X · Paga N× de Y" é o recibo.
  A side-sheet permanece aberta para o próximo lançamento.
- O estado "Incompleto — i de N parcelas criadas" morre: com criação atômica + entidade, grupo
  parcial não existe mais.

## O que NÃO muda

- Overrides (`suppress`/`replace`) e o marcador `#repl:`.
- Isolamento do cenário (`scenario_id IS NULL` no forecast real) e o não-toque em `account.balance`.
- A tabela PRICE (`price_installment`) e seus limites (prazo 1–480, taxa ≥ 0).
- A remoção por linha individual (sem confirmação).

## Aceitação

1. Migração + backfill: banco legado com grupos `#loan` deriváveis ganha entidades e perde os
   sufixos; grupo não-derivável vira linhas soltas sem perda de dados. Re-rodar o backfill é no-op.
2. Criar: uma transação; linhas com `loan_id`; descrição sem marcador; desembolso na data escolhida.
3. Editar: parâmetros novos regeneram a série inteira sob o mesmo `loan_id`, numa transação;
   falha no meio não deixa estado intermediário (rollback comprovado em teste).
4. Remover grupo: uma transação; entidade + todas as linhas somem.
5. Apagar a última linha de um empréstimo apaga a entidade na mesma transação (sem fantasma).
6. Compare: `LoanBreakdown` equivalente ao de hoje para um cenário recém-criado; taxa vem da
   entidade.
7. UI: agrupamento por `loan_id`; ações Editar/Remover com os diálogos do contrato; anúncio
   live e foco no grupo criado; react-doctor sem novas violações; `impeccable` audit +
   critique na entrega.
