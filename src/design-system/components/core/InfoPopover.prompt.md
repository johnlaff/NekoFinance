Inline didactic explainer that wraps a finance term and opens a positioned tooltip panel with the Neko-method definition when the user clicks, hovers, or presses Enter/Space — closes on Esc, outside click, or mouse-leave.

```jsx
{
  /* Built-in glossary key — most common usage */
}
<InfoPopover term="reserva">Reserva</InfoPopover>;

{
  /* Inline custom entry */
}
<InfoPopover
  term={{
    title: "Diário",
    body: "Gasto variável do dia a dia — mercado, café, transporte.",
  }}
>
  Diário
</InfoPopover>;

{
  /* Hide the 'i' badge when the trigger is already distinct */
}
<InfoPopover term="caixa" hideMarker>
  <strong>Caixa atual</strong>
</InfoPopover>;

{
  /* Wider panel for longer copy */
}
<InfoPopover term="performance" width={320}>
  Performance
</InfoPopover>;
```

Pass a string `term` key to pull from the 12-entry built-in PT-BR glossary (`pode_gastar`, `reserva`, `caixa`, `performance`, etc.); pass a `{title?, body}` object for one-off explanations. The `children` prop sets the clickable label text. `hideMarker` suppresses the circular "i" badge. `width` (default 280 px) controls panel width. The popover uses `position: fixed` so it escapes overflow-clipped containers, flips above/below the trigger based on available viewport space, and clamps horizontally with a 12 px margin. Respects `prefers-reduced-motion` — the fade-slide animation is suppressed. Keyboard: Enter/Space open; Esc closes and returns focus to the trigger.

Tokens used: `--surface-elevated`, `--border-strong`, `--elev-overlay`, `--radius-md`, `--radius-xs`, `--radius-circle`, `--radius-pill`, `--primary`, `--primary-quiet`, `--text-strong`, `--text`, `--text-faint`, `--surface-2`, `--font-sans`, `--lh-relaxed`, `--ls-snug`, `--t-hover`, `--shadow-focus`, `--dur-fast`, `--ease-entrance`.
