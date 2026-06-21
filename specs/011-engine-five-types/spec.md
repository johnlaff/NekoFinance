# Spec 011 — Engine: 5 tipos de 1ª classe + previsão de diário no forecast

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
  está disponível para gastar (fiel à conta única do método). Conta como economia em Performance e em
  Economizado%; **não** entra em Custo de vida.
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
performance      = income − cost_of_living − economia − daily_projected
savings_rate_bps = economia × 10000 / income             (0 se income ≤ 0)
real_daily_avg   = daily_realized / dias_decorridos      (inalterado)
```

## Revisão da fórmula de Performance (2026-06-20)

**Decisão do dono**: a Performance exibida foi alterada para paridade com a planilha.

```
performance = income − cost_of_living          # = Entradas − (Saídas + Diário)
```

Os termos `− economia` e `− daily_projected` foram removidos da fórmula de exibição.
Economia continua sendo o numerador do Economizado% e continua alimentando o guardrail de
poupança anual via `realized_annual_economia` (independente de `performance_cents`).
A previsão de diário restante continua reduzindo o saldo de caixa projetado (correto para o
forecast); ela não afeta mais a Performance.

Esta é uma reversão deliberada em relação à fórmula do spec original. O motivo: o usuário
compara a Performance do Neko com a linha correspondente na sua planilha — qualquer divergência
quebra a confiança. Paridade de planilha prevalece sobre a fidelidade ao comportamento do App
nesta métrica específica.

## DoD

- `EventKind::Economia` + `signed` + `classify(…, to_liquidity)` com testes.
- `month_metrics` com as fórmulas acima + campo `economia_cents` em `MonthMetric`.
- `project_daily_ceiling` puro + testes (nasce-no-vermelho, pula dia com Daily, só mês corrente).
- `MonthMetricDto` expõe `real_daily_avg_cents` e `economia_cents`; shell injeta o teto via `daily_budget`.
- `cargo test` verde; sem baixar cobertura; método preservado (sem categoria-orçamento).

```

```
