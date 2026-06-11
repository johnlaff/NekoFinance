On/off toggle for settings and immediate state changes — not for form choices (use Radio for those).

```jsx
const [on, setOn] = React.useState(true);
<Switch checked={on} onChange={setOn} label="Require approval for sheet writes" />;
```

Controlled via `checked` + `onChange(next)`. Jade when on, neutral when off; calm 200ms slide.
