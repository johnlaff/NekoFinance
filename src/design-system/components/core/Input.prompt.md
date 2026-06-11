Labeled text / number / money input with optional leading icon, prefix/suffix affixes, hint and error states.

```jsx
<Input label="Account name" placeholder="e.g. Joint checking" />
<Input label="Opening balance" money prefix="$" suffix="USD" defaultValue="12408.52" />
<Input label="Sheet ID" error="That sheet isn't shared with Neko." />
```

Set `money` for any monetary value — it switches to tabular mono and right-aligns. `error` paints the danger border and shows the message in place of `hint`. `required` adds a danger asterisk.
