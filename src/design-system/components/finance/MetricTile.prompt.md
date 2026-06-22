Headline KPI tile for dashboards — a money value in tabular mono with a signed, color-coded delta (trending icon + figure) and an optional SVG polyline sparkline.

```jsx
<MetricTile label="Saldo do mês" value="R$ 4.820,00" delta="+12,4%" deltaDir="up" sublabel="vs. mês anterior" spark={[40,55,48,70,62,88,100]} />
<MetricTile label="Gastos" value="R$ 3.142,18" delta="6,1%" deltaDir="down" sublabel="abaixo do orçamento" />
<MetricTile label="Reserva" value="R$ 12.000,00" deltaDir="neutral" sublabel="sem variação" />
```

`value` is pre-formatted and rendered as-is in `var(--font-money)` at `var(--fs-money-xl)`. `deltaDir` (`up`/`down`/`neutral`) drives a trending icon and the `var(--money-pos)` / `var(--money-neg)` / `var(--text-muted)` color. The optional `spark` array provides numeric values for a polyline sparkline in `var(--primary)`. Keep tiles in a 3–4 column grid at desktop widths.
