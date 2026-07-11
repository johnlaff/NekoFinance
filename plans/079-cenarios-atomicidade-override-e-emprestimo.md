# Plan 079: Cenários — override e empréstimo atômicos (transação única + unicidade no schema)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5cb24d1..HEAD -- src-tauri/src/scenarios.rs src-tauri/migrations/ src/screens/scenarios.tsx src/lib/api.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (migração de schema + mudança de recuperação de falha)
- **Depends on**: recomendado após 078 (mesmo arquivo; evita conflito de merge)
- **Category**: tech-debt (atomicidade de escrita financeira)
- **Planned at**: commit `5cb24d1`, 2026-07-10

## Why this matters

Duas escritas financeiras dos cenários são multi-passo sem transação. (1) O override:
dup-check via `COUNT(*)` seguido de `INSERT` (check-then-act sem transação nem índice único) e,
no `op=replace`, a linha de substituição é criada DEPOIS, com compensação best-effort
(`let _ = DELETE ...` que ignora o próprio erro) — uma falha no meio pode transformar
"alterar valor" em "remover". (2) O empréstimo: o frontend cria principal + N parcelas em IPCs
sequenciais; falha no meio deixa grupo parcial persistido — hoje MITIGADO deliberadamente
(mensagem com contagem + linhas visíveis com botão de excluir), mas a regra do repo é
"missing transactions around multi-write operations" ser defeito: a mitigação manual vira
desnecessária com um comando único transacional. A regra de engenharia do repo: operações
financeiras multi-write são atômicas.

## Current state

- `src-tauri/src/scenarios.rs:~340-358` — check-then-act do override:
  ```rust
  let (dup,): (i64,) = sqlx::query_as(
      "SELECT COUNT(*) FROM scenario_override \
       WHERE scenario_id = ?1 AND (obligation_id = ?2 OR recurrence_id = ?3)",
  )...;
  if dup > 0 { return Err(...); }
  let id = uuid::Uuid::new_v4().to_string();
  sqlx::query("INSERT INTO scenario_override (...) VALUES (...)")...;
  ```
- `src-tauri/src/scenarios.rs:~377-408` — substituição pareada criada fora de transação, com
  compensação que ignora erro:
  ```rust
  if let Err(e) = created {
      let _ = sqlx::query("DELETE FROM scenario_override WHERE id = ?1")...;
      return Err(format!("linha de substituição inválida: {e}"));
  }
  ```
- `src-tauri/migrations/20260624000001_scenario_override_hardening.sql` — tem CHECK (XOR do
  alvo) e FKs, mas NÃO tem índice único por `(scenario_id, alvo)`.
- `src/screens/scenarios.tsx:~795-830` — loop sequencial de IPCs do empréstimo com o
  banner explicando a mitigação atual (catch sempre invalida; mensagem
  "O empréstimo ficou incompleto (N de M parcelas criadas)...").
- Convenções: comandos Tauri em `src-tauri/src/commands/` delegam para módulos de domínio;
  migrações nomeadas `YYYYMMDDNNNNNN_slug.sql`; erros de domínio como `Err(String)` em pt-BR.
  Padrão de transação sqlx já usado no repo: procure `pool.begin()` em
  `src-tauri/src/` e siga um exemplo existente (ex.: o import atômico).

## Commands you will need

| Purpose    | Command                                 | Expected on success |
| ---------- | --------------------------------------- | ------------------- |
| Rust tests | `cd src-tauri && cargo test scenarios`  | all pass            |
| Rust gate  | `npm run rust:check`                    | exit 0              |
| Frontend   | `npm run typecheck && npm run test:run` | exit 0 / all pass   |
| Gate final | `npm run check`                         | exit 0              |

## Scope

**In scope**:

- `src-tauri/src/scenarios.rs`
- `src-tauri/src/commands/` (registrar o comando novo `create_scenario_loan`)
- `src-tauri/src/lib.rs` (handler list, se for onde os comandos são registrados)
- `src-tauri/migrations/` (UMA migração nova de índices únicos parciais)
- `src/lib/api.ts` (binding do comando novo)
- `src/screens/scenarios.tsx` (LoanSection passa a chamar o comando único)
- `src/screens/scenarios.test.tsx` (ajustar mocks)

**Out of scope** (do NOT touch):

- Semântica do override (substituição por ocorrência, células divergentes, FK do `#repl:`) —
  em decisão no issue #154. Este plano NÃO muda o formato do marcador `#repl:`/`#loan:` nem o
  significado de nada; só torna as escritas atômicas.
- Migrações existentes (nunca editar migração aplicada).

## Git workflow

- Branch: `advisor/079-cenarios-atomicidade`
- TDD nos dois fixes (RED colado no PR); PR ao final, CI verde.

## Steps

### Step 1: Índices únicos parciais + unicidade testada

Nova migração: `CREATE UNIQUE INDEX ... ON scenario_override(scenario_id, obligation_id)
WHERE obligation_id IS NOT NULL;` e o equivalente para `recurrence_id`. Antes do CREATE, a
migração precisa lidar com duplicatas pré-existentes (improvável, mas migração não pode
falhar): mantenha a mais antiga (`MIN(rowid)`) e delete as demais do mesmo alvo.

**Verify**: `cd src-tauri && cargo test` → migrações aplicam em pool de teste sem erro.

### Step 2: Override numa transação única

Reescrever o caminho do `set_scenario_override`: `let mut tx = pool.begin()`; INSERT do
override e INSERT da substituição (quando houver) dentro da mesma `tx`; `tx.commit()` no fim.
O dup-check vira tratamento do erro de violação de unicidade (mapear para as mesmas mensagens
pt-BR atuais — "já existe uma alteração para esta obrigação neste cenário"). Remover o DELETE
compensatório (a transação torna a compensação obsoleta).

Teste RED primeiro: substituição com payload inválido (ex.: data malformada) NÃO pode deixar
override órfão persistido — assert `COUNT(scenario_override) == 0` após a falha. (Hoje passa
por compensação; o teste deve continuar verde após a reescrita — o RED aqui é para o caso de
violação de unicidade concorrente: dois INSERTs do mesmo alvo, o segundo recebe o erro de
domínio, nunca um duplicado.)

**Verify**: `cd src-tauri && cargo test scenarios` → all pass, incluindo os novos.

### Step 3: Comando único `create_scenario_loan`

Novo comando Tauri que recebe `{scenario_id, principal_cents, term_months, rate_bps,
first_installment_date, description}` e cria principal + parcelas numa única transação,
reutilizando o cálculo PRICE existente e produzindo EXATAMENTE as mesmas linhas (mesmas
descrições e marcador ` #loan:<uuid>:<rateBps>`) que o loop do frontend produz hoje —
`detect_loan` e `scenarioHelpers.ts` dependem desse formato. Validar payload na entrada
(principal > 0, 1 ≤ term ≤ 480, rate_bps ≥ 0 finito). Falha em qualquer parcela → rollback
total (teste: injete data inválida na parcela do meio; assert zero linhas do grupo).

**Verify**: `cd src-tauri && cargo test scenarios` → all pass.

### Step 4: Frontend usa o comando único

`LoanSection.confirm()` troca o loop por UMA chamada `createScenarioLoan(...)` (binding novo
em `src/lib/api.ts`). Remover o banner de mitigação e a mensagem de grupo parcial — com
atomicidade, o erro vira só `scenarioErrorMessage(err)`. Ajustar os mocks dos testes.

**Verify**: `npm run typecheck && npm run test:run` → exit 0 / all pass.

## Test plan

- Rust: (a) rollback total do loan com falha injetada no meio; (b) violação de unicidade do
  override → erro de domínio, zero duplicatas; (c) substituição inválida → zero órfãos;
  (d) golden: linhas geradas pelo comando novo batem byte a byte (descrição/marcador) com o
  formato antigo.
- Frontend: teste do LoanSection atualizado — falha do comando único mostra erro simples,
  sem mensagem de parcial.

## Done criteria

- [ ] Saídas RED registradas no PR
- [ ] `npm run check` exit 0
- [ ] `grep -n "ficou incompleto" src/screens/scenarios.tsx` → sem match
- [ ] Migração nova aplicada e idempotente nos testes
- [ ] `git status` — só arquivos do escopo
- [ ] Linha 079 atualizada em `plans/README.md`

## STOP conditions

- Os excertos não batem (078 ou #154 mexeram antes — re-derive o inventário e reporte).
- O golden-test do formato do marcador falhar de forma irreconciliável (formato ambíguo) —
  NÃO invente formato novo; reporte.
- A migração de dedup encontrar duplicatas REAIS no banco de dev — pare e liste-as.

## Maintenance notes

- O issue #154 pode substituir o marcador `#repl:` por FK persistida; o Step 2 já deixa a
  escrita transacional, então essa migração futura fica mais simples.
- Revisor: escrutinar o mapeamento de erro de unicidade → mensagem pt-BR (não vazar erro cru
  de SQLite pro usuário) e o golden do formato `#loan:`.
