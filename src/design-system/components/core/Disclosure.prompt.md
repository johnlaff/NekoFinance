Collapsible disclosure panel with an always-visible trigger and an animated body that expands on demand using the grid-template-rows jank-free technique.

```jsx
{
  /* Bare mode — nested inside a card (default) */
}
<Disclosure
  title="Lançamentos de julho"
  summary="R$ 4.820,00 · 12 lançamentos"
  defaultOpen={false}
>
  <p>Conteúdo detalhado aparece aqui ao expandir o painel.</p>
</Disclosure>;

{
  /* Card mode — autonomous surface with accent */
}
<Disclosure
  title="Alerta de teto diário"
  summary="Você ultrapassou o limite ontem"
  accent="warn"
  bare={false}
  defaultOpen
>
  <p>Detalhes do alerta e ações sugeridas.</p>
</Disclosure>;
```

`title` (required in practice) and `children` drive content. `summary` provides a one-line preview visible when collapsed. `bare` (default `true`) strips card chrome for use inside existing surfaces — set `bare={false}` for a standalone card. `accent` (`ok` / `warn` / `brass`) adds a coloured left border in card mode or tints the title in bare mode. `defaultOpen` controls initial state; the chevron rotates on open. Respects `prefers-reduced-motion` on the expand animation and chevron rotation. Uses `var(--surface)`, `var(--border)`, `var(--shadow-1)`, `var(--surface-hover)`, `var(--text-strong)`, `var(--text-muted)`, `var(--text-faint)`, `var(--success-500)`, `var(--success-400)`, `var(--warning-500)`, `var(--warning-400)`, `var(--secondary)`, `var(--radius-md)`, `var(--radius-sm)`, `var(--dur-fast)`, `var(--dur-base)`, `var(--ease-standard)`, `var(--shadow-focus)`, `var(--font-sans)`, `var(--fs-body)`, `var(--fs-sm)`, `var(--fw-semibold)`, `var(--lh-snug)`, `var(--bw-default)`, `var(--bw-hair)`, `var(--hit-min)`.
