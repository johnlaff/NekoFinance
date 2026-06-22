Linha de lançamento expandível para tabelas de extrato e tela de revisão de importação: data, descrição, pílula de método, procedência, titular, nota e valor — com painel de itens de fatura (lump) recolhível.

```jsx
<TransactionRow
  date="21/06"
  desc="Fatura Nubank"
  amount={-24750}
  method="Crédito"
  provenance="importado"
  note="Uber · Mercado Livre · Netflix"
  lump={[
    { what: "Uber", amount: -3200 },
    { what: "Mercado Livre", amount: -18950 },
    { what: "Netflix", amount: -2600 },
  ]}
  defaultOpen
/>
```

Props principais: `date` (string de data), `desc` (descrição), `amount` (centavos inteiros — positivo = entrada verde, negativo = saída neutro), `method` (pílula de método), `provenance` (`importado` | `manual` | `projetado` | `conciliado` — dot colorido com rótulo PT-BR), `owner` (nó ReactNode, ex. OwnerChip), `note` (nota em itálico), `passthrough` (repasse, dimeia valor), `future` (stripe diagonal brass), `lump` (itens de fatura expansíveis), `selected` (rail jade à esquerda). Tokens: `--prov-imported`, `--prov-app`, `--prov-projected`, `--prov-reconciled`, `--info-400`, `--info-tint`, `--money-pos`, `--brass-500`, `--surface-selected`, `--bg-subtle`, `--border`, `--bw-hair`, `--text`, `--text-muted`, `--text-faint`, `--primary`, `--font-money`, `--font-sans`, `--fs-body`, `--fs-sm`, `--fs-micro`, `--fs-label`, `--fs-money-sm`, `--fw-semibold`, `--fw-bold`, `--radius-pill`, `--dur-fast`, `--ease-standard`.
