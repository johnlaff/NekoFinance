Small status / category label. Always pairs a tone with a word (color is never the only signal).

```jsx
<Badge tone="success" dot>Reconciliado</Badge>
<Badge tone="warning" dot>Aguardando dono</Badge>
<Badge tone="danger">Pendente</Badge>
<Badge tone="info">Importado</Badge>
<Badge tone="primary">Saída</Badge>
<Badge tone="secondary" square>Cartão</Badge>
```

Tones: `success · warning · danger · info · primary · secondary`. Default is `primary`. `dot` adds a leading status dot (currentColor); `square` switches to 4 px radius for counts/codes. Typography is forced uppercase micro (var(--fs-micro), var(--fw-bold), var(--ls-caps)). Tokens: `--success-tint`, `--success-400`, `--warning-tint`, `--warning-400`, `--danger-tint`, `--danger-400`, `--info-tint`, `--info-400`, `--primary-quiet`, `--primary`, `--secondary-quiet`, `--secondary`, `--fs-micro`, `--fw-bold`, `--ls-caps`.
