# Spec 013 — Views do mês: Totais, Horizonte multi-mês, Anuais

> Fonte: notas locais privadas (o método: "olhar para frente"). GAPs de visão — o motor já
> produz os dados; falta a tela. Functional-core já pronto (spec 003/011); aqui é shell + UI.

## Totais (entregue)
4 métricas-herói do mês corrente (Performance / Custo de vida / Economizado / Diário médio) com
status do método via HealthBadge. Lê `get_forecast` → `months[today]`. Ver `TotaisScreen`.

## Horizonte multi-mês (esta slice)
Matriz **dia × mês** do saldo projetado, colorida por faixa (heatmap) — a visão "para frente" do
método. Lê `forecast.daily` (saldo encadeado de hoje até `horizon_end`, já cruzando meses) e
agrupa por ano-mês → uma coluna por mês, uma linha por dia, célula = saldo do dia.

**Faixas de saldo** (`saldoBand`, pura) — thresholds em centavos:
```
cents < -50000  → critical     (vermelho forte)
cents <      0  → negative      (vermelho)
cents < 100000  → tight         (âmbar)
cents < 200000  → ok            (verde claro)
cents >= 200000 → comfortable   (verde forte)
```
Mapeia para os tokens `--saldo-band-*` / `--saldo-band-*-fill` (já no states.css). O dia de hoje
é destacado. Sem dado → EmptyState.

## Visões anuais (próxima slice)
As 4 métricas agregadas por ano (tendência mês a mês). Lê `months[]` do ano. Sparkline opcional.

## DoD
- `saldoBand` pura + testes das faixas (fronteiras −500/0/1000/2000).
- `HorizonteScreen` agrupa `forecast.daily` por mês e renderiza o heatmap; rota `horizonte`.
- a11y (tabela semântica / aria), responsivo, tema, motion reduzido. React Doctor sem achado novo.
- `npm run check` verde; testes cobrindo `saldoBand` + render.
