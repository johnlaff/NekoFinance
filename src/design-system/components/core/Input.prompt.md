Labeled text / number / money input field used throughout the app's transaction forms, matching the NewTransactionForm field style: --bg-subtle background, --border-input border (WCAG 1.4.11 compliant), --radius-xs corners, --hit-min height, and an uppercase muted label.

```jsx
<Input label="Descrição" placeholder="Ex.: Mercado, salário, aluguel…" />
<Input label="Valor" money prefix="R$" inputMode="decimal" placeholder="0,00" defaultValue="1234,50" />
<Input label="Data" type="date" defaultValue="2026-06-21" />
<Input label="Valor" money readOnly defaultValue="3500,00" hint="Total calculado a partir das partes detalhadas." />
<Input label="Conta" error="Nenhuma conta de reserva encontrada." />
```

Set `money` for monetary amounts — switches to `var(--font-money)` tabular mono and right-aligns. Set `readOnly` when the value is auto-calculated (e.g. sum of line-item parts): dims the background to `var(--surface-2)` and mutes the text. `error` paints the danger border and replaces `hint`. `required` adds a danger asterisk. `prefix`/`suffix` accept any node (text or inline SVG). `icon` places a 16×16 leading node inside the field border.

Key tokens: `--bg-subtle` (field background), `--border-input` (border — WCAG 1.4.11), `--border-focus` + `--focus-ring` (focus ring), `--radius-xs` (4px corner), `--hit-min` (36px height), `--bw-hair` (1px border), `--fs-label` + `--ls-label` (label), `--fs-body` (input text), `--font-money` (tabular mono), `--danger-500` / `--danger-tint` (error state), `--surface-2` (readonly background).
