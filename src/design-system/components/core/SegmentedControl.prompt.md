Compact single-select toggle for 2–4 mutually exclusive views — filters, ownership scope, time ranges.

```jsx
const [scope, setScope] = React.useState("all");
<SegmentedControl
  value={scope}
  onChange={setScope}
  options={[
    { value: "all", label: "All" },
    { value: "personal", label: "Personal", dot: "var(--owner-personal)" },
    { value: "partner", label: "Partner", dot: "var(--owner-partner)" },
    { value: "shared", label: "Shared", dot: "var(--owner-shared)" },
  ]}
/>;
```

Pass `size="sm"` for toolbar density. Options accept an optional `dot` color for ownership scopes.
