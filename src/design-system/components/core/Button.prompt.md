Primary action control — calm jade fill, restrained press feedback, no bounce. Use for the single most important action in a view; pair with `secondary`/`ghost` for the rest.

```jsx
<Button variant="primary" onClick={save}>Approve change</Button>
<Button variant="secondary" iconLeft={<PlusIcon/>}>Add account</Button>
<Button variant="ghost" size="sm">Cancel</Button>
<Button variant="danger">Disconnect</Button>
```

Variants: `primary` (jade), `secondary` (bordered surface), `ghost` (quiet), `danger` (destructive only — disconnect, delete). Sizes: `sm` 30px, `md` 36px, `lg` 44px. Props: `fullWidth`, `iconLeft`, `iconRight`, `disabled`. Never use more than one `primary` per visual group.
