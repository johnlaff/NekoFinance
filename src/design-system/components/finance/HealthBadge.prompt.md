Overall financial-health status with a radial ring. Color is always paired with a word (Strong / Steady / Watch / At risk).

```jsx
<HealthBadge level="strong" sublabel="3.1 months runway" size="lg" />
<HealthBadge level="watch" score={48} sublabel="spending up 18%" />
```

Levels: `strong` (success) · `steady` (jade) · `watch` (warning) · `risk` (danger). `score` (0–100) overrides the default ring fill. Use `size="lg"` for the dashboard hero.
