The human-approval gate for an AI-proposed Google Sheets write. Shows exactly which cells change (before → after), the rationale, and requires an explicit action. Never auto-apply — this is a core trust surface.

```jsx
<ApprovalDiffCard
  title="Re-categorize 3 transactions"
  sheet="Expenses 2025"
  range="D1204:E1206"
  changes={[
    { field: "Category", before: "Uncategorized", after: "Groceries" },
    { field: "Owner", after: "Household (shared)" },
  ]}
  note={
    <span>
      Matched merchant <b>“Whole Foods”</b> to your Groceries rule (3 prior rows).
    </span>
  }
  status="pending"
  actions={
    <>
      <Button variant="primary" size="sm">
        Approve & write
      </Button>
      <Button variant="ghost" size="sm">
        Edit
      </Button>
      <span style={{ flex: 1 }} />
      <Button variant="danger" size="sm">
        Reject
      </Button>
    </>
  }
/>
```

`status` controls the header pill (`pending`/`approved`/`rejected`). A `before` that is omitted/empty renders as an addition (only the jade `after`). Pass real `<Button>`s in `actions`.
