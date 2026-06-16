# Spec 019 — Views do mês: Totais, Horizonte multi-mês, Anuais

> Renumerada de 013 (colisão: 013 é a Conciliação avançada). Conteúdo inalterado.
>
> Fonte: notas locais privadas (o método: "olhar para frente"). As três telas (Totais, Horizonte
> multi-mês, Visão anual) foram ENTREGUES sobre o functional-core (spec 003/011): aqui é shell + UI.

## Totais (entregue)

4 métricas-herói do mês corrente (Performance / Custo de vida / Economizado / Diário médio) com
status do método via StatusChip (pílula calma com ponto de cor — preferida ao anel radial do
HealthBadge neste layout denso de 4 métricas). Lê `get_forecast` → `months[today]`. Ver `TotaisScreen`.

## Horizonte multi-mês (entregue)

Matriz **dia × mês** do saldo projetado, colorida por faixa (heatmap) — a visão "para frente" do
método. Lê `forecast.daily` (saldo encadeado de hoje até `horizon_end`, já cruzando meses) e
agrupa por ano-mês → uma coluna por mês, uma linha por dia, célula = saldo do dia.

**Faixas de saldo** (`saldoBand`, pura, em `lib/saldoHeatmap.ts`) — o **termômetro** do método, com
limiares **ABSOLUTOS** em reais. Esta é a formatação condicional canônica da coluna Saldo da planilha
de referência (a planilha-de-ensino do método é literalmente nomeada "termometro"; os mesmos limiares
aparecem na planilha viva do usuário). Quanto maior o saldo, mais verde; quanto menor, mais perto do
vermelho — é a leitura "estou bem ou não?" de relance.

```
cents <= -R$ 500   → critical     (vermelho forte)
-500 < cents < 0   → negative     (vermelho claro)
R$ 0 a R$ 1.000    → tight        (âmbar)
R$ 1.000 a 2.000   → ok           (verde claro)
cents >  R$ 2.000  → comfortable  (verde forte)
```

Os limiares ficam num único objeto (`SALDO_BAND_THRESHOLDS_CENTS`), configuráveis por usuário no
futuro (escalas de renda diferentes), com default = planilha. Mapeia para os tokens `--saldo-band-*`
/ `--saldo-band-*-fill` (já no states.css). O termômetro é aplicado no Horizonte e na coluna Saldo da
tabela diária do Dashboard. O dia de hoje é destacado. Sem dado → EmptyState.

> Correção (2026-06-15): a versão anterior desta spec usava faixas RELATIVAS ao baseline e alegava
> que "o método proíbe limiares fixos" — FALSO. A formatação condicional da planilha-de-ensino e da
> planilha viva usa exatamente estes limiares absolutos; o termômetro é canônico do método.

## Visões anuais (entregue)

`AnnualScreen` (rota `anuais`): tabela do ano inteiro com navegação por ano (`MonthNav`). Lê
`months[]` do forecast. Lidera com as colunas da aba Economia da planilha (Entradas | Economia |
Economizado%) e expande com Performance / Custo de vida / Diário médio. Linha **TOTAL** com o
Economizado% ANUAL = ΣEconomia/ΣEntradas (ponderado, o número que a meta 20–30% cobra), em verde
quando ≥20%.

## DoD

- `saldoBand` pura + testes das faixas ABSOLUTAS (limiares e fronteiras da planilha) em
  `lib/saldoHeatmap.test.ts`.
- `HorizonteScreen` agrupa `forecast.daily` por mês e renderiza o heatmap; rota `horizonte`.
- a11y (tabela semântica / aria), responsivo, tema, motion reduzido. React Doctor sem achado novo.
- `npm run check` verde; testes cobrindo `saldoBand` + render.
