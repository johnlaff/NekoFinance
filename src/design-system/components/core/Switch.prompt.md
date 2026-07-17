On/off toggle (36×20 px track, jade when on, neutral when off) for immediate state changes in settings panels — matches the `gs-toggle` visual idiom used in GoogleSheetsPanel.

```jsx
const [on, setOn] = React.useState(true);
<Switch checked={on} onChange={setOn} label="Atualização automática" />;
```

Controlled via `checked` + `onChange(next: boolean)`. Accent (`--primary`) when on; `--border-input` when off — passes WCAG 1.4.11 ≥3:1 contrast against the app surface in both themes. Knob is always white (`#fff`). Focus ring uses `--shadow-focus`. Use `label` prop for a trailing text label; prefer `aria-label` on the `<input>` for screen-reader–only descriptions. For explicit two-sided labels ("Ligado / Desligado"), use SegmentedControl instead. Tokens: `--primary`, `--border-input`, `--shadow-1`, `--shadow-focus`, `--t-hover`, `--font-sans`, `--fs-body`, `--text`.
