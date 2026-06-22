Inline BRL amount rendered in tabular mono type with a typographic minus sign and optional sign-based colour — the atomic unit for every monetary figure in the app.

```jsx
<Money cents={524800} size="display" sign="auto" />
<Money cents={-38750} size="lg" sign="auto" />
<Money cents={0} size="md" sign="auto" />
<Money cents={1234567} size="sm" hideCents />
```

`cents` is an integer (e.g. 1234567 = R$ 12.345,67). `size` picks the type scale (`sm` 13 px → `display` 34 px); `lg` and `display` use bold weight, the rest semibold. `sign="auto"` colours positive values with `var(--money-pos)` (jade), negative with `var(--money-neg)` (red), and zero with `var(--money-neutral)`; `sign="none"` (default) inherits the parent colour. `sign="negative"` forces red unconditionally. Uses `var(--font-money)`, `var(--fs-money-sm/md/lg/xl)`, `var(--fw-semibold)`, `var(--fw-bold)`, `var(--money-pos)`, `var(--money-neg)`, `var(--money-neutral)`.
