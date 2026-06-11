Unified placeholder for the four non-content states — keeps empty / loading / skeleton / error consistent everywhere.

```jsx
<EmptyState variant="empty" title="No transactions yet"
  description="Connect a Google Sheet and Mia will import and categorize your activity."
  action={<Button variant="primary">Connect Google Sheets</Button>} />

<EmptyState variant="skeleton" skeletonRows={6} />
<EmptyState variant="loading" title="Reading your sheet…" />
<EmptyState variant="error" title="Couldn’t reach that sheet"
  description="Check it’s shared with your connected account."
  action={<Button variant="secondary">Retry</Button>} />
```

Use `skeleton` for table/list loading (shimmer rows), `loading` for a centered spinner with a status line, `error` for recoverable failures with a retry action.
