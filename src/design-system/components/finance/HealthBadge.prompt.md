Status pill with a radial progress ring showing the financial health level; always pairs color with a word so the state is readable without color perception.

```jsx
<HealthBadge level="strong" label="Sobrou dinheiro" sublabel="3,1 meses de reserva" size="lg" />
<HealthBadge level="steady" score={74} />
<HealthBadge level="watch" score={48} sublabel="gastos +18%" />
<HealthBadge level="risk" label="Em risco" sublabel="limite ultrapassado" />
```

Levels: `strong` (success-tint / success-400) · `steady` (primary-quiet / primary) · `watch` (warning-tint / warning-400) · `risk` (danger-tint / danger-400). The `label` prop overrides the default PT-BR word for the level ("Forte", "Estável", "Atenção", "Em risco") — use it for method-specific copy such as "Sobrou dinheiro" or "Dentro da renda". `score` (0–100) overrides the default ring fill. Use `size="lg"` for the dashboard hero badge. The progress arc animates via `--dur-slow` / `--ease-entrance` and respects `prefers-reduced-motion` automatically through those tokens.

Tokens used: `--radius-pill`, `--font-sans`, `--fs-sm`, `--fs-title`, `--fs-micro`, `--fw-bold`, `--fw-medium`, `--success-tint`, `--success-400`, `--primary-quiet`, `--primary`, `--warning-tint`, `--warning-400`, `--danger-tint`, `--danger-400`, `--dur-slow`, `--ease-entrance`.
