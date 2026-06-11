Identifies who owns or is responsible for a transaction or budget line — the personal / partner / shared distinction central to Neko.

```jsx
<OwnerChip name="Alex Tan" type="personal" />
<OwnerChip name="Sam Okafor" type="partner" role="Payer" />
<OwnerChip name="Household" type="shared" role="Responsible" />
```

`type` sets the avatar accent (`shared` renders a split avatar). `role` appends a qualifier — `Payer`, `Beneficiary`, or `Responsible` — after a divider, to disentangle who paid from whose budget it lands in. Use `bare` inside dense table rows.
