# Plan 077: Cenários — validar a fronteira do empréstimo e bloquear confirmação com prévia falha

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5cb24d1..HEAD -- src/screens/scenarios.tsx src/lib/scenarioHelpers.ts src-tauri/src/scenarios.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug (validação de fronteira + estado de erro)
- **Planned at**: commit `5cb24d1`, 2026-07-10

## Why this matters

Dois buracos na fronteira da UI de cenários. (1) A taxa do empréstimo aceita qualquer texto:
`parseFloat(ratePct) || 0` transforma entrada inválida silenciosamente em financiamento a 0%,
e taxa negativa passa (`validInputs` só checa principal e prazo) — o backend calcula PRICE para
qualquer `i64` e o helper de agrupamento não reconhece marcador com taxa negativa, então o
empréstimo "some" da lista agrupada. (2) No override de obrigação, falha na prévia vira
`affectedCount = 0` ("0 ocorrências") e o botão Confirmar continua habilitado — o usuário salva
um override sem ver o que ele afeta. A regra do repo é "parse and validate data at boundaries" e
erro visível, nunca silencioso.

## Current state

- `src/screens/scenarios.tsx:761-765` (LoanSection) — parsing frouxo:
  ```tsx
  const principalCents = parseBRLToCents(principal) ?? 0;
  const term = Math.max(1, parseInt(termMonths, 10) || 0);
  const rateBps = Math.round((parseFloat(ratePct.replace(",", ".")) || 0) * 100);
  const validInputs = principalCents > 0 && term > 0;
  ```
- `src/screens/scenarios.tsx:640-745` (ObligationOverrideSection) — prévia e botão:
  ```tsx
  const previewQ = useCommand(
    selectedId ? `obligation_items:${selectedId}` : "obligation_items:none",
    () => (selectedId ? obligationItems(selectedId) : Promise.resolve([])),
  );
  const affectedCount = (previewQ.data ?? []).filter(
    (it) => it.date >= fromDate,
  ).length;
  ...
  <Button ... disabled={
    busy || previewQ.loading || (action === "replace" && !newAmount.trim())
  }>
  ```
  Não existe render de `previewQ.error` nem gate do botão nesse estado.
- `src/lib/scenarioHelpers.ts:30` — o parser do marcador `#loan:<uuid>:<rateBps>` não casa
  bps negativo (confira o regex ao editar).
- Convenções: mensagens de erro em pt-BR sentence case; o padrão de exibição de erro na
  própria seção já existe (`error` state + `setError(scenarioErrorMessage(err))` no
  LoanSection, `src/screens/scenarios.tsx:759` e `:826`) — siga-o.

## Commands you will need

| Purpose   | Command             | Expected on success |
| --------- | ------------------- | ------------------- |
| Typecheck | `npm run typecheck` | exit 0              |
| Lint      | `npm run lint`      | exit 0              |
| Tests     | `npm run test:run`  | all pass            |
| Doctor    | `npm run doctor`    | 0 violações novas   |

## Scope

**In scope**:

- `src/screens/scenarios.tsx`
- `src/screens/scenarios.test.tsx` (testes novos)

**Out of scope** (do NOT touch):

- `src-tauri/src/scenarios.rs` — validação backend do PRICE/payload entra no plano 079
  (comando atômico), não aqui.
- `src/lib/scenarioHelpers.ts` — com a validação de fronteira, bps negativo não é mais
  alcançável pela UI; mudar o regex sem necessidade arrisca o agrupamento existente.
- Semântica do override (o que "substituir" significa) — em decisão no issue #154.

## Git workflow

- Branch: `advisor/077-cenarios-fronteira`
- PR ao final; merge somente com CI verde.

## Steps

### Step 1: Validação estrita do empréstimo

Em `LoanSection`, derivar um estado de validade explícito: taxa = número finito ≥ 0 (aceitar
vírgula decimal), prazo inteiro entre 1 e 480, principal > 0. Entrada de taxa não-numérica
NÃO vira 0: torna o formulário inválido com mensagem inline (ex.: "Taxa inválida — use um
número, ex.: 1,8"). Desabilitar o botão Confirmar enquanto inválido. Manter `rateBps = 0`
válido apenas quando o campo contém explicitamente `0`.

**Verify**: `npm run typecheck` → exit 0.

### Step 2: Prévia falha bloqueia confirmação

Em `ObligationOverrideSection`: quando `previewQ.error` estiver setado, (a) renderizar o erro
no padrão da seção (mesmo visual do `error` do LoanSection) com um botão/ação de tentar de
novo (re-disparar o `useCommand` — siga o mecanismo de refetch existente no arquivo), e
(b) incluir `previewQ.error != null` na condição `disabled` do botão Confirmar. O texto
"0 ocorrências" nunca deve aparecer quando a causa é erro de leitura.

**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 3: Testes

Ver Test plan.

**Verify**: `npm run test:run` → all pass, incluindo os novos.

## Test plan

Em `src/screens/scenarios.test.tsx` (modelar nos testes existentes do arquivo — ele já mocka
os comandos Tauri):

1. Taxa "abc" → botão do empréstimo desabilitado + mensagem de taxa inválida visível.
2. Taxa "-2" → botão desabilitado (nunca cria empréstimo com bps negativo).
3. Taxa "1,8" → botão habilitado (parse com vírgula funciona).
4. `obligationItems` rejeitando → mensagem de erro visível, botão Confirmar desabilitado,
   e o texto "0 ocorrências" ausente.

## Done criteria

- [ ] `npm run typecheck`, `npm run lint`, `npm run test:run` exit 0
- [ ] `npm run doctor` sem violações novas
- [ ] Os 4 testes novos existem e passam
- [ ] `git status` — nenhum arquivo fora do escopo
- [ ] Linha 077 atualizada em `plans/README.md`

## STOP conditions

- Os excertos de "Current state" não batem (o arquivo foi refatorado).
- O mecanismo de retry do `useCommand` não existir/for diferente — reporte em vez de
  inventar um cache-buster.
- Corrigir o gate exigir mudar `src/lib/api.ts` ou o hook `useCommand` em si.

## Maintenance notes

- Quando o issue #154 decidir a semântica do override, os testes daqui (4) continuam válidos —
  eles testam o gate de erro, não a semântica.
- Revisor: conferir que taxa `0` explícita continua permitida (empréstimo sem juros é caso real).
