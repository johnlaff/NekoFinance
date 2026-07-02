# Spec 021 — Performance inclui a previsão de diário restante

## Decisão

A Performance do mês corrente passa a descontar também a **previsão de diário restante**
(teto diário × dias ainda não vividos do mês + diários futuros pré-lançados):

```
Performance = Entradas − (Saídas fixas + Diário realizado + Previsão de diário restante
              + Cartão + Economia + Patrimônio)
```

O mês nasce mostrando o cenário cheio (assume o gasto típico até o fim do mês) e melhora
conforme o gasto real fica abaixo do teto. Os meses passados não mudam (previsão = 0); os
meses futuros seguem descontando apenas o que estiver pré-lançado na planilha.

Isto substitui a decisão anterior (2026-06-20) de manter a previsão fora da Performance.
Fundamentação: a estrutura da planilha produz exatamente esse resultado quando usada como
ensinado — a soma da coluna Diário cobre o mês inteiro, incluindo dias futuros
pré-preenchidos com o gasto planejado. Decisão do dono do produto em 2026-07-02.

## O que NÃO muda

- **Custo de vida** segue reportando apenas o realizado (fixas + diário realizado + cartão).
  `total_outflow_cents` continua sendo a variante com projeção para a cobertura de meses.
- **Diário médio** segue `Σ diário realizado ÷ dias decorridos`.
- **Economizado%** (economia ÷ entradas) não é afetado.
- **Guardrail "pode gastar hoje"** já usava figuras anuais realizadas; não é afetado.
- **Cadeia de Saldo/caixa** já incluía a previsão (driver do saldo projetado); não muda.

## Superfícies

- Motor: `MonthMetric.daily_projected_cents` exposto (antes era um acumulador interno);
  fórmula da Performance em `month_metrics`.
- DTO/TS: `MonthMetricDto.daily_projected_cents` → `MonthMetric` em `api.ts`.
- UI: Este mês (Totais) exibe "− Previsão de diário R$ X" no subtexto da Performance e uma
  linha própria no Entrou × Saiu, para a aritmética exibida fechar com o número do motor.

## Aceitação

- Mês corrente com teto ativo: `performance = base_realizada − previsão_restante`
  (teste `daily_ceiling_feeds_performance_not_cost_of_living` e
  `forecast_dual_guardrail_savings_binds_for_owner`).
- Mês passado: valores idênticos aos anteriores (suíte de regressão).
- Equação exibida no Totais fecha com o valor do motor quando `daily_projected_cents > 0`.
