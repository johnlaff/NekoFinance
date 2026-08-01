# Spec 011 — Engine: 5 tipos de 1ª classe + previsão de diário no forecast

> Atualização 2026-06-23: a decisão travada de 2026-06-21 sobre `economia=Saída` foi reaberta por
> decisão explícita do dono e supersedida pela [Spec 020](../020-classificacao-notas-5-tipos/spec.md).
> O modelo atual separa Cartão, Economia e Patrimônio; custo de vida exclui economia/patrimônio, e
> Performance desconta os outflows reais uma única vez. A tabela de tipos abaixo também ficou para
> trás num segundo ponto: destino `illiquid` (FGTS/previdência) **não** é Economia — é Patrimônio,
> fora da régua dos 20–30% em qualquer condição de reserva. Ver [ADR-0005](../../docs/adr/0005-single-annual-ruler-patrimonio-outside.md).

> Fonte: síntese do método + a planilha (notas locais privadas). Corrige a raiz comum de 3
> divergências do que o método pede.

## Problema (o que está errado hoje)

O `forecast/mod.rs` só conhece 3 tipos (`Income/FixedOut/Daily`); `transfer` (economia) é
descartado silenciosamente (`classify … _ => None`) e a poupança é aproximada por **superávit**
(`savings_rate = performance/income`), não pela economia lançada. A previsão de diário
(`daily_budget`) é um número estático, **não** dirige o forecast — então o saldo projetado e a
Performance ficam **otimistas** (não "nascem no vermelho"). E `real_daily_avg_cents` é calculado
mas **morre no DTO** (`MonthMetricDto` não o carrega).

## Verdade-fonte (fórmulas do método)

- `Performance = Entradas − (Saídas_fixas + Diário + Economia + Cartão + Previsão_de_diário_restante)`
- `Custo de vida = Saídas_fixas + Diário + Cartão` (sem economia, sem previsão)
- `Economizado% = Economia / Entradas` (meta 20–30% **anual**)
- `Diário médio = Σ Diário_realizado ÷ dia_atual` (D/N)

## Modelo (decisões)

- **5 tipos derivados** de `(type, is_fixed, payment_method, liquidez-destino)` — sem migração do
  enum `transaction.type`: entrada=income; saída=expense fixo; diário=expense variável;
  **cartão**=expense+credit; **economia**=`transfer` p/ poupança real: `liquidity ∈ {reserve, illiquid}`
  (reserva + FGTS/previdência = poupança forçada). `restricted` (vale-refeição) é gasto restrito,
  **não** poupança — não conta como Economia. Transfer entre contas `liquid` continua net-zero (skip).
- **`EventKind::Economia`** (novo): saída do saldo de gasto (signed −), porque guardar reduz o que
  está disponível para gastar (fiel à conta única do método). Alimenta o Economizado%
  (savings_rate_bps); **não** desconta a Performance de novo (a poupança já entra como Saída no grid →
  cost_of_living — ver decisão FINAL abaixo) e **não** entra em Custo de vida diretamente como Economia.
- **Cartão** permanece o lump da fatura no vencimento (Régua 2, já modelado como FixedOut) — o
  efeito de caixa do crédito é diferido. O badge/breakout "Cartão" é derivado (UI), não muda o caixa.
- **Previsão de diário dirige o forecast**: `project_daily_ceiling` injeta eventos `Daily`
  projetados (`realized=false`) de `today+1` até o fim do **mês corrente**, no valor do teto/dia,
  pulando dias que já têm um Daily. Reduz o saldo projetado (não-otimista) e entra na Performance
  como `previsão restante`; não entra em `real_daily_avg` (só realizado) nem em Custo de vida.

## Métricas (engine, por mês)

```
daily_realized   = Σ Daily realizado
daily_projected  = Σ Daily projetado (teto + futuros pré-lançados)
economia         = Σ Economia (mês)
cost_of_living   = fixed_out + daily_realized            (cartão já em fixed_out)
performance      = income − cost_of_living                (economia NÃO é descontada de novo — ver decisão FINAL abaixo)
savings_rate_bps = economia × 10000 / income             (0 se income ≤ 0)
real_daily_avg   = daily_realized / dias_decorridos      (inalterado)
```

## Revisão da fórmula de Performance — decisão FINAL (2026-06-21) — 🔒 DECISÃO TRAVADA

**Decisão do dono (2026-06-21, FINAL — supersede planos 040 e 046)**:

```
performance = income − cost_of_living   # = Entradas − (Saídas + Diário)
```

🔒 **DECISÃO TRAVADA — NÃO re-adicionar `− economia`.** Esta é a forma definitiva, fiel à
planilha. Os planos 040 e 046 oscilaram (040 chegou a esta fórmula; 046 adicionou `− economia`
por engano); o plano 051 é a correção definitiva que fecha a oscilação. Qualquer auditoria futura
que considere re-subtrair a economia DEVE parar: a subtração extra é dupla contagem. Só uma nova
decisão explícita do dono (não uma "correção de fidelidade") pode reabrir isto.

Por que **não** subtrair `− economia` separadamente:

- A poupança é lançada como Saída (expense) no grid mensal do método → ao ser importada como
  `type='expense'`, torna-se `EventKind::FixedOut` ou `EventKind::Daily` → **já está em
  `cost_of_living_cents`**. É contada UMA vez, aqui.
- A aba Economia importa o mesmo valor como `type='transfer'` para a conta reserva →
  `EventKind::Economia` → alimenta `savings_rate_bps` (Economizado%). Este é um registro de
  **taxa de poupança**, não um segundo movimento de dinheiro para a Performance.
- Subtrair `economia` além de `cost_of_living` seria dupla contagem (o erro do plano 046).

O termo `− daily_projected` continua EXCLUÍDO: a projeção de diário afeta o saldo de caixa
mas não tem linha correspondente na Performance da planilha (só o realizado aparece lá).

`economia_cents` permanece em `MonthMetric` como numerador do Economizado% e é reportado no DTO.
`realized_annual_economia` (`forecast_cmds.rs`) alimenta o guardrail de poupança —
completamente independente de `performance_cents`.

**Invariante-chave**: a poupança entra no engine OU como expense row em `cost_of_living`
(Saída do grid → FixedOut/Daily) OU como transfer em `EventKind::Economia` (anotação da aba
Economia), mas NÃO conta nas duas pontas da Performance. Ver também a nota da parte 2 abaixo.

### Parte 2 — risco de dupla contagem no **Saldo** (não na Performance) — DOCUMENTADO, follow-up

Investigação (plano 051) do fluxo da aba Economia (`parse_economia_sheet` →
`store_economia_entries`):

- `store_economia_entries` grava uma linha `type='transfer'`, `to_account_id=reserva`,
  id determinístico `economia:YYYY-MM` (uma por mês, upsert idempotente). Em re-import o mesmo
  id é sobrescrito — **não** duplica linhas da própria aba Economia. ✅
- Essa transfer é classificada por `classify("transfer", …, Some("reserve"))` →
  `EventKind::Economia`. Em `signed()` (`forecast/mod.rs`), `Economia` retorna `-amount`, ou
  seja, **sai do Saldo** (espelha a conta única do método: guardar reduz o disponível).
- **Risco real e pré-existente** (ortogonal à Performance, NÃO piorado pelo plano 051):
  se o dono lançar a MESMA poupança nas DUAS pontas — uma Saída no grid mensal (expense →
  hits Saldo via `FixedOut`/`Daily`) **e** uma entrada na aba Economia (transfer → hits Saldo
  via `Economia`) — o valor sai do Saldo **duas vezes**. Hoje isso não se manifesta porque a
  aba Economia está vazia (`economia = 0`).
- **Guardrail de poupança permanece coerente**: `safe_to_spend_today` usa
  `annual_savings_cents` (de `realized_annual_economia`, que soma SÓ os transfers→reserva/illiquid,
  `forecast_cmds.rs`) — não lê `performance_cents` nem a Saída do grid. A mudança da fórmula de
  Performance (plano 051) não o afeta.

**Decisão sobre a Parte 2 — DEFERIDA para follow-up** (não implementada aqui): o conserto
correto (fazer a aba Economia ser só anotação de taxa que NÃO toca o Saldo, mantendo a Saída do
grid como o único movimento) repercute pelos planos 003/005/045 e pela semântica de `signed()`
para `EventKind::Economia` em todo o Saldo chain. Mudá-lo no escopo do plano 051 (que é só a
fórmula de Performance) violaria o "vertical slice pequeno". Arquivos a tocar no follow-up:
`forecast/mod.rs` (`signed` p/ `EventKind::Economia`), e os testes do Saldo chain
(`economia_reduces_spending_balance`). **Não** deixar silencioso: o risco está documentado aqui
e na nota de manutenção do plano 051. Acionar quando o dono começar a preencher a aba Economia.

## DoD

- `EventKind::Economia` + `signed` + `classify(…, to_liquidity)` com testes.
- `month_metrics` com as fórmulas acima + campo `economia_cents` em `MonthMetric`.
- `project_daily_ceiling` puro + testes (nasce-no-vermelho, pula dia com Daily, só mês corrente).
- `MonthMetricDto` expõe `real_daily_avg_cents` e `economia_cents`; shell injeta o teto via `daily_budget`.
- `cargo test` verde; sem baixar cobertura; método preservado (sem categoria-orçamento).

```

```
