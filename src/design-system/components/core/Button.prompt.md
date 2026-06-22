Primary action control — calm jade fill, brass quiet tint, ghost with visible border, or danger tint. Restrained press feedback. Use for the single most important action in a view; pair with `secondary`/`ghost` for the rest.

```jsx
<Button variant="primary" onClick={save}>Salvar alteração</Button>
<Button variant="secondary" iconLeft={<PlusIcon/>}>Adicionar conta</Button>
<Button variant="ghost" size="sm">Cancelar</Button>
<Button variant="danger">Desconectar</Button>
```

Variants: `primary` (jade fill, `--primary`), `secondary` (brass quiet tint, `--secondary-quiet` + `--secondary` text), `ghost` (transparent with `--border`, `--text`), `danger` (tint fill `--danger-tint` + `--danger-400` text — destructive only). Sizes: `sm` 28px, `md` 36px, `lg` 44px. Props: `iconLeft`, `iconRight`, `disabled`. Never use more than one `primary` per visual group.
