# Plan 081: Cenários — régua "Economia após parcela" (segunda perna do gate de financiamento)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: confirm plan 080 is DONE in `plans/README.md`
> (this plan builds on its target state — the "antes → depois" line and the
> canonical medians). Then
> `git diff --stat ec2aa1b..HEAD -- src-tauri/src/scenarios.rs src-tauri/src/commands/forecast_cmds.rs src/lib/api.ts src/screens/scenarios.tsx`
> — changes from plan 080 are EXPECTED; compare the "Current state" excerpts
> below (which describe the pre-080 code plus 080's declared target) against
> the live code. On any other mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1 (o gate de financiamento tem duas perguntas; o app responde só uma)
- **Effort**: M
- **Risk**: LOW (leitura pura; nenhuma escrita nem migração)
- **Depends on**: plano 080 executado e mergeado (issue #169)
- **Category**: correctness (aderência ao método)
- **Planned at**: commit `ec2aa1b`, 2026-07-14
- **Decisão de origem**: issue #170 (modelagem fixada em grilling; ver comentário de resolução)

## Why this matters

O gate do método para assumir um compromisso novo tem **duas** perguntas: (1) a reserva cai
abaixo de 12 meses? — coberta pela régua "Reserva após financiar" (plano 080) — e (2) a nova
parcela impede de continuar poupando 20–30% das entradas? A pergunta 2 não existe no compare
de cenários: hoje é possível simular um empréstimo cuja parcela engole toda a poupança mensal
e sair com a reserva verde — meia resposta no lugar mais sensível do app.

As fontes do método dão duas formulações complementares da mesma proteção:

- **Piso**: após o impacto da parcela, o percentual poupado (poupança ÷ entradas) não pode
  ficar abaixo de 20% — quem poupa pouco fura primeiro aqui.
- **Regra da metade**: "não assuma uma parcela do tamanho dessa sobra — pelo menos metade
  dessa sobra tem que continuar sobrando, senão você trava seu patrimônio" — quem poupa
  muito fura primeiro aqui (ex.: poupa 50% das entradas; parcela que consome 60% da sobra
  deixa 20%, passa no piso, mas mata mais da metade do ritmo de patrimônio).

Nenhuma sozinha cobre os dois perfis; a régua nova combina as duas numa escada só.

## Semântica decidida (issue #170)

- **Métrica**: `Economia%_pós = (economia_típica − parcela) ÷ entradas_típicas`.
- **economia_típica** = mediana mensal da economia REGISTRADA (eventos `Economia` +
  anotação da aba de economia, com `max(derivado, anotação)` por mês — mesma regra do motor
  mensal) sobre os últimos 6 meses de calendário completos.
- **entradas_típicas** = mediana mensal dos eventos `Income` na mesma janela.
- Mesma janela e estimador do `realized_monthly_baseline` da perna 1: as duas réguas
  descrevem o mesmo "mês típico". A meta 20–30% do método se julga na média anual — isso é
  didática de popover, não estimador.
- **Escada composta (3 faixas)**, julgada sempre sobre o valor bruto (sem clamp):
  - vermelho `Economia%_pós < 20%` (fura o piso);
  - amarelo `Economia%_pós ≥ 20%` **e** `parcela > ½ × economia_típica` (fere a metade);
  - paz: ambas vivas. Fronteiras: 20,00% exato = passa o piso; parcela exatamente igual à
    metade = paz (a regra é "MAIS da metade").
- **Escopo**: só a parcela do empréstimo hipotético desconta (simétrico à perna 1 — o gate
  pergunta sobre A PARCELA; as demais mudanças do cenário já têm leitura na trajetória).
- **Exibição**: linha gêmea da reserva no card "Empréstimo simulado" —
  `Economia após parcela · 27% → 19%` com badge da escada e InfoPopover didático próprio.
- **Casos-limite**: `entradas_típicas == 0` ⇒ linha oculta (sem % possível);
  `economia_típica == 0` ⇒ exibe `0% → 0%` vermelho (informação, não ausência);
  pós-parcela negativa ⇒ EXIBE clampado em `0%` e o popover declara em R$ que a parcela
  excede a economia típica — o estado da escada continua vindo do valor bruto.

## Current state

- `src-tauri/src/commands/forecast_cmds.rs:353-388` — `realized_monthly_baseline`:
  mediana do custo de vida (`FixedOut + Daily + Cartao`) por mês, janela
  `[1º do mês − 6 meses, 1º do mês)` via `load_metric_db_events`; meses sem eventos não
  entram; vetor vazio ⇒ `Ok(0)`. É o molde das duas medianas novas.
- `src-tauri/src/commands/forecast_cmds.rs:294-…` — `load_economia_annotation(pool, &years)`
  (`pub(crate)`): anotação da aba de economia por `(year, month)`, tabela
  `economia_annotation`. O motor mensal usa `economia = max(derivado, anotação)` por mês
  (`src-tauri/src/forecast/mod.rs:388-396`) para não dobrar dinheiro após round-trip.
- `src-tauri/src/forecast/mod.rs:421-426` — convenção de percentual poupado:
  `savings_rate_bps = economia * 10_000 / income` (bps inteiros).
- `src-tauri/src/scenarios.rs:1046-1054` — `LoanBreakdown` com `loan_installment_cents`
  e (pós-plano-080) `reserve_months_before_financing`/`reserve_months_after_financing`.
- Pós-plano-080, `detect_loan` recebe `reserve_balance_cents` e `baseline_cents` e o
  builder do compare já consulta `realized_monthly_baseline` antes de chamá-lo; o
  frontend tem o padrão "antes → depois" (`ReserveMonthsBadge` com `before`/`after`) e o
  popover didático da reserva (`src/screens/scenarios.tsx`, card "Empréstimo simulado").
- O compare é somente-leitura (nenhuma transação de escrita) — as queries novas rodam
  direto no pool sem risco do deadlock de pool-de-1-conexão.

## Target state

**Backend** (`forecast_cmds.rs` + `scenarios.rs`):

1. Nova função `pub(crate) async fn realized_savings_baseline(pool, today_naive)
-> Result<(i64, i64), String>` em `forecast_cmds.rs`, ao lado de
   `realized_monthly_baseline` e no mesmo molde: janela `[1º do mês − 6, 1º do mês)`,
   `load_metric_db_events` uma vez, e:
   - universo de meses = meses com **ao menos um evento de qualquer tipo** na janela
     (mês ativo sem economia registrada conta como economia 0 — é sinal real, não
     ausência de dado; excluir esses meses inflaria a mediana);
   - `income[mês]` = Σ eventos `Income`; `economia[mês]` = `max(Σ eventos Economia,
anotação do mês via load_economia_annotation)` — mesma regra do motor;
   - retorno `(mediana(income), mediana(economia))`; janela sem meses ativos ⇒ `(0, 0)`.
2. `LoanBreakdown` ganha:
   ```rust
   pub savings_rate_before_bps: Option<i64>,
   pub savings_rate_after_bps: Option<i64>,
   pub economia_median_cents: i64,
   ```
   `None` quando `income_median == 0` (linha oculta). `after` é o valor BRUTO
   (`(economia_median − installment) * 10_000 / income_median`) — pode ser negativo; o
   clamp é só de exibição, no frontend. `economia_median_cents` alimenta a regra da
   metade e a frase em R$ do popover.
3. Builder do compare: chamar `realized_savings_baseline` junto das queries existentes
   (antes de `detect_loan`) e passar `(income_median_cents, economia_median_cents)` para
   `detect_loan`, que deriva os três campos do item 2 a partir de
   `loan_installment_cents`.

**Frontend** (`api.ts` + `scenarios.tsx`):

4. `LoanBreakdown` (TS): os três campos novos, com doc-comment declarando a semântica
   (bruto, sem clamp; `null` oculta).
5. Nova função `savingsAfterState(afterBps, installmentCents, economiaMedianCents)` com a
   escada composta: `afterBps < 2000` → "Abaixo do piso" (`--danger-400`, AlertTriangle) ·
   `installmentCents * 2 > economiaMedianCents` → "Mais da metade da sobra"
   (`--warning-400`, AlertTriangle) · senão → "Paz" (`--primary-quiet-text`,
   CheckCircle2). Mesmos tokens/ícones da escada da reserva (linhas gêmeas de verdade).
6. Linha nova no card "Empréstimo simulado", imediatamente abaixo de "Reserva após
   financiar", mesma anatomia: rótulo "Economia após parcela" + InfoPopover + badge
   `label · 27% → 19%`. Percentuais = `bps / 100` arredondado para inteiro (formato do
   restante do app); exibição clampa em 0% quando o bruto é negativo; linha some quando
   `savings_rate_before_bps == null`.
7. Popover didático (redação final pelo crivo de copy do impeccable, sentence case):
   fórmula (mediana da economia registrada menos a parcela, dividida pela mediana das
   entradas — últimos 6 meses completos); o piso de 20% e que a meta 20–30% se julga na
   média anual; a regra da metade; e, quando a parcela excede a economia típica, a frase
   com os valores: "A parcela (R$ X) excede sua economia típica (R$ Y)".

## Steps (TDD)

1. **RED (Rust)** — testes de `realized_savings_baseline` e de `detect_loan`:
   - mediana da economia usa `max(derivado, anotação)` por mês (mês com anotação maior
     que os transfers não subconta);
   - mês ativo sem economia entra como 0 na mediana (não é descartado);
   - janela ignora o mês corrente e meses além de 6 meses atrás;
   - `income_median == 0` ⇒ `savings_rate_before/after_bps == None`;
   - `after` inclui a parcela: `after < before` sempre que `installment > 0`;
   - parcela > economia típica ⇒ `after` negativo (bruto preservado);
   - fronteiras da escada (no que o backend expõe): `after == 2000` bps não é vermelho.
2. **GREEN (Rust)** — itens 1–3 do Target state. `cargo test` verde.
3. **RED (vitest)** — `savingsAfterState` e a linha nova: fronteiras 1999/2000 bps;
   parcela exatamente metade ⇒ paz, um centavo acima ⇒ amarelo; `null` oculta a linha;
   bruto negativo exibe `0%` mas estado vermelho; formato "27% → 19%".
4. **GREEN (frontend)** — itens 4–7 do Target state. `npm run test:run` verde.
5. Rodar impeccable `audit` + `critique` na tela de cenários (entrega de UI — gate
   inegociável) e ajustar o que sair P1.

## Verification

```bash
npm run rust:check > /tmp/rust.log 2>&1; echo $?     # 0
npm run test:run > /tmp/vitest.log 2>&1; echo $?     # 0
npm run check > /tmp/check.log 2>&1; echo $?         # 0
npm run e2e > /tmp/e2e.log 2>&1; echo $?             # 0 — mudança de layout: inspecionar screenshots
```

Manual: cenário com empréstimo cuja parcela consome mais da metade da economia típica mas
mantém o pós ≥ 20% deve sair AMARELO (não verde); parcela pequena com poupança forte deve
sair paz nas DUAS réguas; usuário sem renda nos últimos 6 meses completos não vê a linha.

## STOP conditions

- Plano 080 ainda não executado/mergeado (`plans/README.md` sem DONE na linha 080).
- Excerpts do "Current state" divergirem do código vivo por algo além do delta declarado
  do plano 080.
- `load_economia_annotation` ou `load_metric_db_events` deixarem de ser acessíveis sem
  tornar público algo além de `pub(crate)`.
- Qualquer teste existente de `detect_loan`/compare quebrar por motivo que não seja os
  campos novos do `LoanBreakdown`.
