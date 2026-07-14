# Plan 080: Cenários — régua "Reserva após financiar" canônica (reserva ÷ (baseline + parcela))

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 61875c6..HEAD -- src-tauri/src/scenarios.rs src-tauri/src/commands/forecast_cmds.rs src/lib/api.ts src/screens/scenarios.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1 (gate financeiro mais sensível do app com número enganoso)
- **Effort**: M
- **Risk**: LOW (leitura pura; nenhuma escrita nem migração)
- **Depends on**: nada pendente
- **Category**: correctness (aderência ao método)
- **Planned at**: commit `61875c6`, 2026-07-14
- **Decisão de origem**: issue #150 (semântica fixada em grilling; ver comentário de resolução)

## Why this matters

A linha "Reserva após financiar" do compare de cenários responde hoje uma pergunta que o
método não faz: divide o **colchão de caixa projetado** (menor saldo do horizonte − piso de
reserva) pelo **custo de vida só do mês corrente**. Dois defeitos:

1. **Número instável**: o custo de vida do mês corrente só conta diário **realizado**
   (`forecast/mod.rs:399` — `fixed_out + daily_realized + cartao`); no início do mês o
   denominador despenca e a régua infla — justamente no gate "posso assumir esta parcela?".
2. **Nome errado**: "reserva em meses" no método é `saldo guardado ÷ custo de vida típico`
   — a régua canônica que o dashboard já calcula (`forecast_cmds.rs:1506`). O cálculo atual
   mede folga de caixa, que o compare **já mostra** no guardrail (menor saldo em R$).
   Mesmo nome, duas semânticas: a comparação dashboard × cenário é ilegítima.

Agrava: o semáforo do frontend pinta 8–12 meses de **verde "Confortável"** — mas o gate do
método para assumir financiamento usa **12** como limiar de paz (compromisso novo sobe o
alvo). Verde com 9 meses e parcela nova é a mensagem errada no pior lugar.

Semântica decidida (issue #150): numerador = saldo das contas de reserva (idêntico ao
dashboard); denominador = `realized_monthly_baseline` (mediana de 6 meses completos)
**+ parcela do empréstimo**. Só a parcela entra como compromisso novo — as demais mudanças
hipotéticas do cenário já têm leitura própria na trajetória e no guardrail. Exibição
"antes → depois" com escada de 3 faixas: `< 6` vermelho · `6–12` amarelo · `≥ 12` paz.

## Current state

- `src-tauri/src/scenarios.rs:1670-1726` — `detect_loan` recebe o custo de vida do cenário e
  o colchão, e deriva a régua atual:
  ```rust
  fn detect_loan(
      hypo_rows: &[HypoTxnRow],
      rate_by_loan: &HashMap<String, i64>,
      scenario_cost_of_living_cents: i64,
      scenario_reserve_after_cents: i64,
  ) -> Option<(String, LoanBreakdown)> {
      ...
      let reserve_months_after_financing = (scenario_cost_of_living_cents > 0)
          .then(|| scenario_reserve_after_cents as f64 / scenario_cost_of_living_cents as f64);
  ```
- `src-tauri/src/scenarios.rs:1965-1968` — derivação do colchão (morre neste plano):
  ```rust
  let scenario_reserve_after_cents = scenario_fc
      .deepest_deficit
      .map(|p| p.balance_cents)
      .unwrap_or(0)
      - reserve_floor_cents;
  ```
- `src-tauri/src/scenarios.rs:1053` — DTO: `pub reserve_months_after_financing: Option<f64>`.
- `src-tauri/src/commands/forecast_cmds.rs:1500-1510` — a régua canônica do dashboard
  (numerador + denominador a reutilizar):
  ```rust
  let reserve_balance: (i64,) =
      sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
          .fetch_one(pool).await...;
  let reserve_baseline = realized_monthly_baseline(pool, today_naive).await?;
  ```
  `realized_monthly_baseline` é `pub(crate)` (`forecast_cmds.rs:353`) — mediana do custo de
  vida dos meses realizados completos na janela de 6 meses.
- `src/lib/api.ts:974-982` — interface `LoanBreakdown`, campo com doc de "aproximação".
- `src/screens/scenarios.tsx:1554-1599` — `reserveMonthsState` com **4 faixas**
  (`<6` vermelho · `6–8` amarelo · `8–12` verde "Confortável" · `>12` jade "Paz") e
  `ReserveMonthsBadge` exibindo `label · X,X meses`.
- `src/screens/scenarios.tsx:2001-2018` — render da linha no card "Empréstimo simulado",
  com `InfoPopover` cujo body descreve a escada 6/8/12 em 4 faixas.
- O compare é somente-leitura (nenhuma transação de escrita) — as duas queries novas podem
  rodar direto no pool sem risco do deadlock de pool-de-1-conexão.

## Target state

**Backend** (`scenarios.rs`):

1. `LoanBreakdown` ganha `pub reserve_months_before_financing: Option<f64>` e o campo
   existente muda de semântica (documentar: régua canônica, mediana + parcela).
2. No builder do compare (junto das outras queries, antes de `detect_loan`):
   - `reserve_balance_cents`: mesma query do dashboard (`SUM(balance) WHERE liquidity='reserve'`).
   - `baseline_cents`: `forecast_cmds::realized_monthly_baseline(pool, today).await?`.
3. `detect_loan` troca os parâmetros `scenario_cost_of_living_cents`/`scenario_reserve_after_cents`
   por `reserve_balance_cents: i64` e `baseline_cents: i64`, e deriva:
   ```rust
   let reserve_months_before_financing =
       (baseline_cents > 0).then(|| reserve_balance_cents as f64 / baseline_cents as f64);
   let reserve_months_after_financing = (baseline_cents > 0)
       .then(|| reserve_balance_cents as f64 / (baseline_cents + installment_cents) as f64);
   ```
   `baseline == 0` (sem meses completos) ⇒ ambos `None` ⇒ linha oculta. Reserva zerada ⇒
   `Some(0.0)` — reserva vazia é informação, o gate reprova.
4. Apagar a derivação de `scenario_reserve_after_cents` (linhas 1965-1968). Os campos
   `*_cost_of_living_cents` do DTO do compare e o helper `current_month_cost_of_living`
   ficam — outros consumidores os usam.

**Frontend** (`api.ts` + `scenarios.tsx`):

5. `LoanBreakdown` (TS): adicionar `reserve_months_before_financing: number | null` e
   atualizar o doc-comment do campo existente (fim da "aproximação").
6. `reserveMonthsState` → **3 faixas**: `< 6` → "Abaixo do mínimo" (`--danger-400`,
   AlertTriangle) · `6 ≤ m < 12` → "Zona amarela" (`--warning-400`, AlertTriangle) ·
   `m ≥ 12` → "Paz" (`--primary-quiet-text`, CheckCircle2). **12,0 exato = Paz** (a fonte
   define "12+"; diverge deliberadamente da convenção limite-superior-inclusivo do
   Termômetro — anotar no comentário da função). "Confortável" morre.
7. `ReserveMonthsBadge` passa a receber `before` e `after` e exibe
   `label · 8,1 → 5,2 meses` (uma casa decimal via `toLocaleString("pt-BR")`, como hoje);
   cor e rótulo derivam do **after**. Se `before == null`, exibe só o after (defensivo —
   com a regra do item 3 os dois são sempre `Some`/`None` juntos).
8. Popover: body passa a declarar fórmula + escada nova, na direção de:
   "Saldo das contas de reserva dividido pelo custo de vida típico (mediana dos últimos
   6 meses completos) somando a parcela nova. Abaixo de 6 meses: abaixo do mínimo; de 6 a
   12: zona amarela; 12 ou mais: paz — assumir financiamento sobe o alvo para 12 meses."
   Redação final passa pelo crivo de copy do impeccable (sentence case, capitalizar
   primeira letra).

## Steps (TDD)

1. **RED (Rust)** — no módulo de testes de `scenarios.rs`, cobrir com pool de 1 conexão
   (regressão do deadlock) ou o helper de teste existente do arquivo:
   - régua independente do dia do mês: mesmo dataset, `today` no dia 2 vs dia 28 ⇒ mesmo
     resultado (mata a inflação de início de mês);
   - denominador inclui a parcela: `after < before` sempre que `installment > 0`;
   - `baseline == 0` ⇒ ambos `None`;
   - reserva zerada ⇒ `Some(0.0)`;
   - numerador vem de contas `liquidity='reserve'` (conta corrente não conta).
2. **GREEN (Rust)** — implementar itens 1–4 do Target state. `cargo test` verde.
3. **RED (vitest)** — testes de `reserveMonthsState`/`ReserveMonthsBadge`: fronteiras
   5.9/6.0/11.9/12.0, formato "antes → depois", `null` oculta a linha.
4. **GREEN (frontend)** — itens 5–8 do Target state. `npm run test:run` verde.
5. Rodar impeccable `audit` + `critique` na tela de cenários (entrega de UI — gate
   inegociável) e ajustar o que sair P1.

## Verification

```bash
npm run rust:check > /tmp/rust.log 2>&1; echo $?     # 0
npm run test:run > /tmp/vitest.log 2>&1; echo $?     # 0
npm run check > /tmp/check.log 2>&1; echo $?         # 0
npm run e2e > /tmp/e2e.log 2>&1; echo $?             # 0 — mudança de layout: inspecionar screenshots
```

Manual: com dados em que a mediana de 6 meses ≫ custo realizado do mês corrente (início de
mês), a régua NÃO pode inflar; dashboard "X meses" e cenário "antes" devem coincidir.

## STOP conditions

- Excerpts do "Current state" não batem com o código vivo (drift).
- `realized_monthly_baseline` deixar de ser acessível de `scenarios.rs` sem tornar público
  algo além de `pub(crate)`.
- Qualquer teste existente de `detect_loan`/compare quebrar por motivo que não seja a
  mudança de semântica descrita aqui.
- A linha "antes" divergir do `reserve_months` do dashboard com os mesmos dados.
