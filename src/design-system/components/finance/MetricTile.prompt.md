Headline KPI tile for dashboards — a money value in tabular mono with a signed, color-coded delta and optional sparkline.

```jsx
<MetricTile label="Net cashflow" value="$4,820.00" delta="+12.4%" deltaDir="up" sublabel="vs. last month" spark={[40,55,48,70,62,88,100]} />
<MetricTile label="Spending" value="$3,142.18" delta="6.1%" deltaDir="down" sublabel="under budget" />
```

`value` is pre-formatted; cents after the decimal dim automatically. `deltaDir` (`up`/`down`/`flat`) drives the arrow + money color. Keep tiles in a 3–4 col grid on desktop.
