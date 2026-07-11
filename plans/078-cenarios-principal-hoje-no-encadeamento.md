# Plan 078: Cenários — eventos hipotéticos de HOJE entram no encadeamento de saldo

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5cb24d1..HEAD -- src-tauri/src/scenarios.rs`
> If the file changed since this plan was written, compare the "Current state"
> excerpts against the live code before proceeding; on a mismatch, treat it as
> a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (mexe na projeção; TDD obrigatório)
- **Depends on**: none (mas coordene com 079 se rodarem juntos — mesmo arquivo)
- **Category**: bug (correção do motor de cenários)
- **Planned at**: commit `5cb24d1`, 2026-07-10

## Why this matters

O principal do empréstimo simulado é criado com a data de HOJE
(`disbursementDate = todayISO()` em `src/screens/scenarios.tsx:~800`), mas o ramo de
ENCADEAMENTO de saldo do cenário carrega hipotéticos com `date > today` (exclusivo) — então o
principal aparece nas métricas do mês (que carregam desde o início do mês, inclusivo) e na
detecção do empréstimo, mas NUNCA no saldo projetado: a trajetória não sobe com o dinheiro
recebido, o menor ponto (déficit) fica mais fundo do que deveria e o guardrail de caixa do
"Pode gastar" fica artificialmente apertado. Para linhas REAIS o `date > today` é correto —
o movimento de hoje já está refletido no saldo-semente da conta. Para linhas HIPOTÉTICAS não
existe semente: o evento de hoje simplesmente se perde. O compare fica internamente
inconsistente (métricas veem o principal; a trajetória não).

## Current state

- `src-tauri/src/scenarios.rs:~618-640` — `load_hypothetical_rows` tem os dois SQLs:
  ```rust
  // INCLUSIVE:  WHERE t.date >= ?2 AND t.date <= ?3 AND t.scenario_id = ?1
  // EXCLUSIVE:  WHERE t.date >  ?2 AND t.date <= ?3 AND t.scenario_id = ?1
  let sql = if inclusive_start { INCLUSIVE } else { EXCLUSIVE };
  ```
- `src-tauri/src/scenarios.rs:~940-951` — o call-site que monta o compare:
  ```rust
  let real_chain_raw = load_real_rows(pool, &today_str, false, &horizon_str).await?;
  let real_metric_raw = load_real_rows(pool, &month_start_str, true, &horizon_str).await?;
  let hypo_chain_rows =
      load_hypothetical_rows(pool, scenario_id, &today_str, false, &horizon_str).await?;
  let hypo_metric_rows =
      load_hypothetical_rows(pool, scenario_id, &month_start_str, true, &horizon_str).await?;
  ```
  O terceiro argumento `false` = exclusivo (`date > today`) — este é o ponto do bug para o
  ramo hipotético.
- Racional da assimetria correta: linhas reais de hoje já estão no saldo-semente (a mesma
  razão documentada no comentário próximo a `load_all_hypothetical_rows`,
  `src-tauri/src/scenarios.rs:~1085`: "um principal desembolsado HOJE fica fora do
  encadeamento (`date > today`)"). Linhas hipotéticas nunca tocam o saldo real — precisam
  entrar pelo fluxo.
- Convenção de teste: os testes do módulo ficam em `#[cfg(test)]` no próprio
  `src-tauri/src/scenarios.rs` — siga o padrão dos existentes (criam pool SQLite em memória,
  inserem linhas e chamam o builder do compare).

## Commands you will need

| Purpose    | Command                                    | Expected on success |
|------------|--------------------------------------------|---------------------|
| Rust tests | `cd src-tauri && cargo test scenarios`     | all pass            |
| Rust gate  | `npm run rust:check`                       | exit 0 (fmt+clippy+test) |
| Frontend   | `npm run test:run`                         | all pass            |

## Scope

**In scope**:
- `src-tauri/src/scenarios.rs` (fix + testes de regressão)

**Out of scope** (do NOT touch):
- `load_real_rows` e o seed real — o `date > today` das linhas REAIS está correto.
- `src/screens/scenarios.tsx` — a data do desembolso continua hoje; o fix é no carregamento.
- Semântica de override/substituição — issue #154.

## Git workflow

- Branch: `advisor/078-principal-hoje-encadeamento`
- TDD: commit do teste RED antes (ou junto) do fix; PR ao final, CI verde.

## Steps

### Step 1 (RED): teste de regressão

Novo teste no `#[cfg(test)]` de `scenarios.rs`: cenário com uma única transação hipotética
`income` datada de HOJE (simulando o principal). Assert: o saldo do fim do mês corrente no
compare do cenário = saldo real + principal; e o `deepest_deficit` do cenário não é mais fundo
que o real. Rode e confirme que FALHA (prova de que o teste testa o bug).

**Verify**: `cd src-tauri && cargo test scenarios` → o teste novo FALHA; cole a saída do RED
no relatório/PR.

### Step 2 (GREEN): incluir hipotéticos de hoje no ramo de encadeamento

Trocar o carregamento do ramo hipotético de encadeamento para incluir a data de hoje
(`inclusive_start = true` com `start = today`), SEM tocar no ramo real. Atenção ao
double-count: verifique que nenhum outro ponto soma o hipotético de hoje ao encadeamento
(o ramo de métricas é um pipeline separado — métricas ≠ encadeamento; conferir no builder).

**Verify**: `cd src-tauri && cargo test scenarios` → todos passam, incluindo o novo.

### Step 3: gates

**Verify**: `npm run rust:check` → exit 0; `npm run test:run` → all pass (o frontend consome
o DTO; nada deve quebrar).

## Test plan

- Teste 1 (Step 1): principal hipotético hoje eleva o saldo encadeado do cenário.
- Teste 2: transação hipotética `expense` datada de hoje REDUZ o saldo encadeado (simetria).
- Teste 3 (anti-double-count): hipotético datado de AMANHÃ produz o mesmo resultado antes e
  depois do fix (garante que só a fronteira de hoje mudou).

## Done criteria

- [ ] Saída RED do Step 1 registrada no PR
- [ ] `cd src-tauri && cargo test scenarios` all pass
- [ ] `npm run rust:check` exit 0
- [ ] `npm run test:run` exit 0
- [ ] `git status` — somente `src-tauri/src/scenarios.rs` (+ plans/README.md) modificados
- [ ] Linha 078 atualizada em `plans/README.md`

## STOP conditions

- O teste do Step 1 PASSAR sem fix (o bug não existe como descrito — o call-site pode ter
  mudado; reporte).
- O fix exigir mudar `load_real_rows`/seed real ou o forecast core (`src-tauri/src/forecast/`).
- Algum teste existente de `scenarios.rs` quebrar de um jeito que exija mudar a EXPECTATIVA
  do teste antigo (pode ser sintoma de double-count — reporte em vez de ajustar o assert).

## Maintenance notes

- Se um dia o cenário ganhar "semente própria" (saldo inicial hipotético), esta fronteira
  precisa ser revisitada — a inclusão de hoje pressupõe que hipotéticos nunca entram em
  semente nenhuma.
- Revisor: o ponto a escrutinar é double-count do dia de hoje entre métricas e encadeamento.
