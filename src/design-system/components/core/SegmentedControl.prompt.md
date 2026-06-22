Compact single-select toggle for 2–4 mutually exclusive views — time ranges, filter scopes, period selectors — with radiogroup semantics and full keyboard navigation.

```jsx
const [periodo, setPeriodo] = React.useState("mes");
<SegmentedControl
  ariaLabel="Período"
  value={periodo}
  onChange={setPeriodo}
  options={[
    { value: "dia", label: "Dia" },
    { value: "semana", label: "Semana" },
    { value: "mes", label: "Mês" },
  ]}
/>;
```

Key props: `options` (array of `{value, label}`), `value`, `onChange`, `size` ("sm" | "md"), `disabled`, `ariaLabel`. The active segment uses `var(--surface-selected)` background and `var(--primary)` text; the container sits on `var(--bg-subtle)`. Arrow keys, Home, and End navigate between segments (roving tabindex — only the selected radio is in the tab order).
