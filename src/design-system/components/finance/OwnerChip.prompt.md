Identifies the owner of a transaction or account — personal ("Eu"), partner ("Parceiro(a)"), or shared ("Compartilhado") — with a colored dot or avatar ring.

```jsx
<OwnerChip who="personal" />
<OwnerChip who="partner" note="paga" />
<OwnerChip who="shared" avatar />
<OwnerChip who="personal" name="Ana" avatar note="responsável" />
<OwnerChip who="partner" bare />
```

`who` selects the owner category and sets the accent color via `--owner-personal`, `--owner-partner`, or `--owner-shared`. `name` overrides the default label; `note` appends a secondary qualifier in `--text-faint`. Set `avatar` to replace the 7 px dot with a 20 px circular ring (border colored by owner) showing two-letter initials. Use `bare` inside dense table rows to strip the pill border and background (`--surface-2`, `--border`).
