Controlled list editor for breaking a transaction into named sub-parts, each with a positive BRL amount and a free-text description, plus a running total once two or more items are present.

```jsx
<LineItemEditor
  items={[
    { amount_cents: 8500, description: "Supermercado Pão de Açúcar", position: 0 },
    { amount_cents: 3200, description: "Padaria da esquina", position: 1 },
  ]}
  onChange={(next) => console.log(next)}
/>
```

`items` is the full controlled list (`LineItemDraft[]`); omit it to see the built-in demo. `onChange` receives the complete updated array on every add, remove, or edit — the parent is expected to reflect it back. `disabled` makes every control inert. The total row (`Total das partes`) appears only when `items.length >= 2`. Amount inputs use `--font-money` (tabular mono) and `inputMode="decimal"` for mobile keyboards; the section eyebrow uses `--fs-label` / `--ls-label` / `--text-muted`; field backgrounds are `--bg-subtle` with `--border-input` borders; the add-item affordance uses a dashed `--border` outline.
