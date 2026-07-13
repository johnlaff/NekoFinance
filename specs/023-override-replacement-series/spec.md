# Spec 023 — Semântica do override de cenários (série de substituição + FK + fronteira)

## Decisão

O override de cenário (`suppress`/`replace`) ganha três invariantes que a modelagem antiga não
sustentava (decididos em [#154](https://github.com/johnlaff/NekoFinance/issues/154)):

1. **Substituição é uma SÉRIE, não uma linha única.** Um `replace` gera **uma linha hipotética por
   ocorrência suprimida** (datas derivadas dos itens da obrigação, ou das linhas reais da
   recorrência, com `date >= from_date`) — nunca mais "suprime N meses, repõe 1". Todas as linhas
   são donas do mesmo `override_id`; a série é função determinística do override, então
   regenerá-la produz o mesmo resultado.
2. **Identidade por FK, morte em cascata.** As linhas de substituição apontam para o override por
   uma coluna `transaction.override_id` (FK, `ON DELETE CASCADE`) — não mais pelo marcador textual
   `#repl:<override_id>` na descrição. Apagar a obrigação/recorrência mata o override e a série
   juntos: "substituir X por Y" nunca degrada para "manter X e adicionar Y". O marcador é
   aposentado (nenhum caminho novo o escreve); o compare pareia por FK.
3. **Fronteira cirúrgica contra o caso destrutivo.** Criar override sobre uma obrigação é recusado
   **apenas** quando a supressão zeraria uma célula que ainda tem itens irmãos não suprimidos — o
   único caso em que suprimir derrubaria contribuição alheia. Células sub-explicadas continuam
   simuláveis.

E uma quarta, de exposição:

4. **Recorrências nativas no seletor de alvo.** A UI passa a oferecer as recorrências criadas no app
   (grupo "Recorrências", ao lado das "Obrigações"), com prévia equivalente. O backend já tratava
   `recurrence_id` de ponta a ponta; a geração de série da decisão 1 generaliza sem caso especial.

## Modelo de dados

```sql
ALTER TABLE "transaction" ADD COLUMN override_id TEXT REFERENCES scenario_override(id) ON DELETE CASCADE;
CREATE INDEX idx_transaction_override ON "transaction" (override_id);
```

- `override_id` NOT NULL = linha de substituição de um `replace` (some com o override em cascata).
  NULL = qualquer outra linha (real, hipotética solta, ou parcela de empréstimo).
- O marcador `#repl:` **deixa de ser escrito**. `parse_repl_marker` sobrevive só para o backfill
  do legado. O sufixo `#loan:`/`loan_id` (spec 022) não muda.

## Geração da série (comando)

`set_scenario_override(op="replace", replacement)`:

1. Valida a fronteira (abaixo) e insere o `scenario_override` — tudo numa transação única.
2. Deriva as **datas das ocorrências** `>= from_date` do alvo:
   - **obrigação** → datas dos `obligation_items` casados;
   - **recorrência** → datas das linhas reais da série (`recurrence_id = ? AND scenario_id IS NULL`).
3. Insere **uma linha hipotética por ocorrência**, todas com `override_id = <id>`, valor
   `replacement.amount_cents`, sem marcador. Falha em qualquer linha → rollback (sem par órfão).

`ReplacementInput` perde o campo `date` (as datas vêm do alvo, não de uma entrada única). Mantém
`amount_cents` + descritores (`description`, `txn_type`, `payment_method`, `is_fixed`).

## Fronteira (só alvo-obrigação, `suppress` e `replace`)

Por célula (transação) afetada pelo override — matched = itens da obrigação com `date >= floor`,
`floor = max(from_date, mês corrente)`, agrupados por `transaction_id`:

- `T` = total da célula (`transaction.amount`, magnitude);
- `S` = soma das magnitudes dos itens suprimidos na célula;
- irmãos = `line_item`s da célula fora do matched.

**Recusa** (mensagem didática) quando, para alguma célula, `S >= T` **e** existe irmão de magnitude
positiva — suprimir zeraria a célula e derrubaria a contribuição do irmão (nota sobre-explicada).
Todos os outros casos passam: `S < T` deixa residual (irmão preservado); `S >= T` sem irmãos zera a
célula corretamente (a célula É a obrigação); célula sub-explicada (`itens <= T`) nunca dispara.
Só células que a projeção realmente toca (`>= mês corrente`) entram: uma célula histórica é inerte
(`load_real_rows` começa no mês corrente), então bloquear por causa dela seria falso-positivo — o
mesmo piso da geração de série. Alvo-recorrência não tem fronteira — a série é de propósito único,
suprimir derruba a linha inteira sem irmão a preservar.

## Backfill de legado (startup, idempotente)

`backfill_scenario_override_replacements`, logo após `backfill_scenario_loans`: processa só linhas
de cenário cuja descrição ainda termina no marcador `#repl:<id>` (parser ancorado). Para cada uma:

- override existe → seta `override_id = <id>` e remove o sufixo da descrição;
- override órfão (id morto) → só remove o sufixo (a linha vira uma adição comum, como já degradava).

Não expande a linha única legada em série (preserva o dado existente, como o backfill do #165);
só novos overrides geram série. Re-rodar é no-op.

## Compare (`get_scenario_forecast`)

- `HypoTxnRow`/`ScenarioTransactionRow` expõem `override_id`.
- Pareamento por FK: linhas com `override_id` casando um override existente fundem na entrada
  `replace` de `changes` (e somem da lista de "add"). `parse_repl_marker` não participa mais.
- A entrada `replace` reporta **valores por ocorrência** (mensal): `old` = magnitude da ocorrência
  suprimida representativa — a mais antiga `>= max(from_date, mês corrente)` (só ocorrências que a
  projeção toca), com desempate determinístico por `(data, transaction_id)` para não depender do
  seed de hash — `new` = `replacement.amount_cents`. `suppress` segue reportando o total suprimido
  no `old`. Isso reflete o input "Novo valor/mês" e evita comparar total-do-horizonte com valor-mensal.

## Contrato de interação (UI — `scenarios.tsx`)

- **Seletor de alvo**: um `<select>` com dois `<optgroup>` — "Obrigações" e "Recorrências". As
  recorrências ganham rótulo derivado (descrição da ocorrência mais antiga + frequência).
- **Prévia equivalente, numa caixa só**: `Isto afeta N ocorrências a partir de {data DD/MM/AAAA}`
  para os dois tipos de alvo (recorrência usa as ocorrências reais da série; obrigação, os
  `obligation_items`). `N` conta só ocorrências `>= max(from_date, mês corrente)` — o mesmo piso do
  backend, então bate com as linhas efetivamente criadas. No `replace`, a mesma frase acrescenta
  `Serão criadas N linhas de R$ X (uma por ocorrência)` — o usuário vê que a substituição repõe cada
  mês. Prévia ilegível continua bloqueando o Confirmar (sem "0 ocorrências" por erro de leitura).
- **Confirmar bloqueado** quando `N = 0` (não cria override morto ocupando o slot único do alvo) ou,
  no `replace`, quando o novo valor é vazio/ilegível. A rejeição da fronteira chega verbatim (a
  mensagem didática, não o fallback genérico).
- **Série na lista editável**: as N linhas iguais colapsam num `Disclosure` ("N× de R$ X"), como o
  empréstimo; a lixeira por linha nomeia a data no `aria-label` (as descrições são idênticas).

## O que NÃO muda

- Empréstimo (`scenario_loan`/`loan_id`), o marcador `#loan:` e seu backfill (spec 022).
- Isolamento do cenário (`scenario_id IS NULL` no forecast real) e o não-toque em `account.balance`.
- A subtração line-item de `apply_suppression` (preserva irmãos e residual não documentado).
- Não há comando de editar/remover override individual: a "morte da série" é a cascata da FK
  (obrigação/recorrência/cenário apagados); a "regeneração" é a natureza determinística da geração.

## Aceitação

1. **Série**: `replace` sobre obrigação com 12 ocorrências futuras cria 12 linhas com o mesmo
   `override_id`; a projeção não fica otimista (paga o novo valor todo mês). Idem recorrência.
2. **FK/cascata**: apagar a obrigação apaga override + série; nada de linha de substituição órfã.
3. **Fronteira**: célula sobre-explicada com irmão vivo recusa (mensagem didática); célula
   sub-explicada e célula sem irmão passam. Vale para `suppress` e `replace`.
4. **Backfill**: banco com `#repl:` deriva `override_id` + perde o sufixo; órfão vira adição comum;
   re-rodar é no-op.
5. **Compare**: série funde numa entrada `replace` por FK; nenhuma linha da série aparece como
   "add"; `old`/`new` por ocorrência.
6. **UI**: grupo "Recorrências" no seletor, prévia de ocorrências para os dois alvos, prévia da
   série no `replace`; react-doctor sem novas violações; `impeccable` audit + critique na entrega.
7. Gates: `npm run check` verde + E2E; rollback comprovado em teste (falha no meio da série não
   deixa estado intermediário).
