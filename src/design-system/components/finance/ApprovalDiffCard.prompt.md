The human-approval gate for an AI-proposed Google Sheets write-back. Shows exactly which cells change (antes → depois), the rationale note, and requires an explicit action. Never auto-applies — this is a core trust surface.

```jsx
<ApprovalDiffCard
  title="Recategorizar 3 transações"
  sheet="Gastos 2025"
  range="D1204:E1206"
  changes={[
    { field: "Categoria", before: "Sem categoria", after: "Alimentação" },
    { field: "Dono", after: "Compartilhado" },
  ]}
  note={
    <span>
      Estabelecimento <b>"Whole Foods"</b> identificado pela regra de Alimentação (3
      linhas anteriores).
    </span>
  }
  status="pending"
  actions={
    <>
      <Button variant="primary" size="sm">
        Aprovar e escrever
      </Button>
      <Button variant="ghost" size="sm">
        Editar
      </Button>
      <span style={{ flex: 1 }} />
      <Button variant="danger" size="sm">
        Recusar
      </Button>
    </>
  }
/>
```

`status` controls the header pill: `pending` → "Precisa de aprovação" (amber), `approved` → "Aprovado" (jade), `rejected` → "Recusado" (red). A `before` omitted or empty renders as a pure addition (only the jade `after` chip). Pass action buttons via `actions`. Key tokens: `--diff-add` / `--diff-add-bg`, `--diff-remove` / `--diff-remove-bg`, `--warning-tint`, `--success-tint`, `--danger-tint`, `--bw-hair`, `--radius-pill`, `--fs-label`, `--fw-bold`, `--fw-semibold`, `--font-money`, `--bg-subtle`.
