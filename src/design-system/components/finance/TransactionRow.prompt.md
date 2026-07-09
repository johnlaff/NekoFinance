Linha de lançamento expandível para tabelas de extrato e tela de revisão de importação: data, descrição, pílula de método, procedência, titular, nota e valor — com painel de itens de fatura (lump) recolhível.

```jsx
<TransactionRow
  date="21/06"
  desc="Fatura do cartão"
  amount={-24750}
  method="Crédito"
  provenance="importado"
  note="Transporte · Marketplace · Streaming"
  lump={[
    { what: "Transporte", amount: -3200 },
    { what: "Marketplace", amount: -18950 },
    { what: "Streaming", amount: -2600 },
  ]}
  defaultOpen
/>
```

Props principais: `date` (string de data), `desc` (descrição), `amount` (centavos inteiros — positivo = entrada verde, negativo = saída neutro), `method` (pílula de método), `provenance` (`importado` | `manual` | `projetado` | `conciliado` — dot colorido com rótulo PT-BR), `owner` (nó ReactNode, ex. OwnerChip), `note` (nota em itálico), `passthrough` (repasse, dimeia valor), `future` (stripe diagonal brass), `lump` (itens de fatura expansíveis), `selected` (rail jade à esquerda). Tokens: `--prov-imported`, `--prov-app`, `--prov-projected`, `--prov-reconciled`, `--info-400`, `--info-tint`, `--money-pos`, `--brass-500`, `--surface-selected`, `--bg-subtle`, `--border`, `--bw-hair`, `--text`, `--text-muted`, `--text-faint`, `--primary`, `--font-money`, `--font-sans`, `--fs-body`, `--fs-sm`, `--fs-micro`, `--fs-label`, `--fs-money-sm`, `--fw-semibold`, `--fw-bold`, `--radius-pill`, `--dur-fast`, `--ease-standard`.
