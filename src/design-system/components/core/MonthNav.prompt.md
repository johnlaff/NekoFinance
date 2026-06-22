Compact temporal navigation row with left/right arrow buttons, a centred month-year label, and a "Hoje" shortcut pill that appears whenever the active period differs from the current month.

```jsx
<MonthNav
  label="Junho de 2026"
  onPrev={() => setMonth((m) => m - 1)}
  onNext={() => setMonth((m) => m + 1)}
  onToday={() => setMonth(currentMonth)}
  canPrev={true}
  canNext={false}
  atToday={false}
/>
```

`label` is a pre-formatted string (the parent owns all date state). `canPrev`/`canNext` disable the respective arrow when the boundary of the navigable range is reached. `atToday` hides the "Hoje" pill when already on the current period. `prevLabel`/`nextLabel` let callers supply accessible override strings for year-level navigation. Uses `--hit-min`, `--radius-sm`, `--border`, `--surface`, `--text`, `--text-faint`, `--dur-fast`, `--ease-standard`, `--space-2`, `--space-3`, `--space-4`, `--radius-pill`, `--primary-quiet`, `--primary-quiet-text`, `--fs-sm`, `--fw-semibold`, `--fs-title`, `--fw-bold`, `--ls-tight`, `--text-strong`, `--font-sans`.
