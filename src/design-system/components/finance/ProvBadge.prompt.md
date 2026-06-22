Transaction-provenance badge — a pill with a colored dot paired with a plain-language label (Da planilha, Do app, or Previsto) and an educational hover/focus tooltip explaining how the transaction arrived.

```jsx
<ProvBadge provenance="importado" />
<ProvBadge provenance="manual" />
<ProvBadge provenance="projetado" />
```

`provenance` drives the dot color and label: `"importado"` uses `--text-faint` (neutral — from the spreadsheet, not yet reconciled), `"manual"` uses `--info-400` (entered in the app and written back), `"projetado"` uses `--secondary` / brass (future or projected, not yet confirmed). The badge is always a colored dot + word — color is never the sole signal. Hovering or focusing reveals an inline tooltip with a short PT-BR educational note. Use it next to a transaction row's date or amount in the Livro-razão or recent-transactions list.
