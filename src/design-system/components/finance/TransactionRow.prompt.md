A single dense transaction line for tables and the import-review screen. Five columns: date · merchant+category · owner · status/confidence · amount.

```jsx
<TransactionRow
  date="08 Jun"
  merchant="Whole Foods Market"
  category="Groceries"
  categoryColor="var(--chart-1)"
  owner={<OwnerChip name="Household" type="shared" bare />}
  amount="642.18"
  status="needs-owner"
  confidence="low"
/>
```

During import, pass `confidence` to render a 3-bar meter (high/medium/low) instead of a status dot. `status="needs-owner"` adds a warning rail on the left. Set `positive` for income (green, +). Use `selected` for the active row in a master/detail layout.
