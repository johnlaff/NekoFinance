# Spec 019 — Views do mês: Totais, Horizonte multi-mês, Anuais

> Renumerada de 013 (colisão: 013 é a Conciliação avançada). Conteúdo inalterado.
>
> Fonte: notas locais privadas (o método: "olhar para frente"). GAPs de visão — o motor já
> produz os dados; falta a tela. Functional-core já pronto (spec 003/011); aqui é shell + UI.

## Totais (entregue)

4 métricas-herói do mês corrente (Performance / Custo de vida / Economizado / Diário médio) com
status do método via HealthBadge. Lê `get_forecast` → `months[today]`. Ver `TotaisScreen`.

## Horizonte multi-mês (esta slice)

Matriz **dia × mês** do saldo projetado, colorida por faixa (heatmap) — a visão "para frente" do
método. Lê `forecast.daily` (saldo encadeado de hoje até `horizon_end`, já cruzando meses) e
agrupa por ano-mês → uma coluna por mês, uma linha por dia, célula = saldo do dia.

**Faixas de saldo** (`saldoBand`, pura) — RELATIVAS à escala do usuário (`baseline_outflow_cents`,
o gasto mensal típico). NÃO usar limiares BRL absolutos: a fonte-verdade do método proíbe copiar os
fixos do material de referência (perdem sentido para rendas diferentes).

```
cents < -1 mês de gasto → critical     (vermelho forte)
cents <  0              → negative      (vermelho)
cents <  1 mês de gasto → tight         (âmbar)
cents <  2 meses        → ok            (verde claro)
cents >= 2 meses        → comfortable   (verde forte)
```

Sem baseline ainda (usuário novo) → classifica só pelo sinal. Mapeia para os tokens
`--saldo-band-*` / `--saldo-band-*-fill` (já no states.css). O dia de hoje é destacado. Sem dado →
EmptyState.

## Visões anuais (entregue)

`AnnualScreen` (rota `anuais`): tabela do ano inteiro com as 4 métricas-herói mês a mês, navegação
por ano (`MonthNav`). Lê `months[]` do forecast.

## DoD

- `saldoBand` pura + testes das faixas RELATIVAS (escala muda a faixa do mesmo saldo).
- `HorizonteScreen` agrupa `forecast.daily` por mês e renderiza o heatmap; rota `horizonte`.
- a11y (tabela semântica / aria), responsivo, tema, motion reduzido. React Doctor sem achado novo.
- `npm run check` verde; testes cobrindo `saldoBand` + render.
