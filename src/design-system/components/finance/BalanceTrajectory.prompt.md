SVG area chart of the 30-day projected balance trajectory with stroke-draw mount animation, an interactive hover crosshair and tooltip, a "hoje" marker, a minimum-balance label, and a dashed zero-line band when the balance goes negative.

```jsx
{
  /* Full variant — Horizonte screen */
}
<BalanceTrajectory daily={forecastDays} today="2026-06-21" variant="full" />;

{
  /* Compact variant — embedded in the hero forecast tile */
}
<BalanceTrajectory daily={forecastDays} today="2026-06-21" variant="compact" />;

{
  /* Standalone demo — no props required */
}
<BalanceTrajectory />;
```

`daily` is an array of `{ date: string, balance_cents: number }` in chronological order; omit it to see the built-in 30-day demo. `today` (YYYY-MM-DD) places the green "hoje" dot; it defaults to the current date. `variant` switches between `"full"` (260 px, axis labels, max label) and `"compact"` (120 px, no labels). The stroke-draw line animation (`nk-btraj__line`) runs once on mount and is suppressed by `prefers-reduced-motion`. Hover fires a crosshair and a floating HTML tooltip; the tooltip shifts left when near the right edge. When the balance dips below zero, a dashed `--danger-400` line marks R$ 0 and the minimum-balance dot turns red — ensuring the deficit is communicated beyond color alone. Tokens used: `--primary`, `--danger-400`, `--border-strong`, `--surface`, `--surface-elevated`, `--text-muted`, `--text-faint`, `--text-strong`, `--font-sans`, `--font-money`, `--radius-sm`, `--shadow-2`, `--ls-label`, `--dur-deliberate`, `--ease-entrance`.
