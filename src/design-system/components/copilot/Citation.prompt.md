Provenance for Mia's numbers — keeps every AI figure auditable back to sheet rows.

```jsx
{/* inline numbered chip after a figure */}
You spent <span className="nk-chat__money">$486.20</span> <Citation index={1} source="row 1,204" />

{/* deterministic calculation block */}
<Citation
  variant="tool"
  fn="sum(Dining, May 2025)"
  lines={[
    { label: "Bottega · 12 May", value: "78.00" },
    { label: "Tonkotsu · 19 May", value: "44.20" },
    { label: "+ 9 more", value: "364.00" },
  ]}
  total={{ label: "Total", value: "$486.20" }}
  source="Sheet ‘Expenses 2025’ · 11 rows"
/>
```

Use `inline` chips for quick attribution; use `tool` blocks when the answer is a computed number the user may want to verify. Tool blocks are deterministic — they show the actual rows summed, not a model guess.
