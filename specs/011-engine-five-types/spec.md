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
  **cartão**=expense+credit; **economia**=`transfer` p/ conta com `liquidity ∈ {reserve,restricted,illiquid}`.
  Transfer entre contas `liquid` continua net-zero (skip).
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

## DoD

- `EventKind::Economia` + `signed` + `classify(…, to_liquidity)` com testes.
- `month_metrics` com as fórmulas acima + campo `economia_cents` em `MonthMetric`.
- `project_daily_ceiling` puro + testes (nasce-no-vermelho, pula dia com Daily, só mês corrente).
- `MonthMetricDto` expõe `real_daily_avg_cents` e `economia_cents`; shell injeta o teto via `daily_budget`.
- `cargo test` verde; sem baixar cobertura; método preservado (sem categoria-orçamento).

```

```
