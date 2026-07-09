# Note Conventions

These conventions describe new spreadsheet notes that the app can classify deterministically.
Import and reconciliation never rewrite existing notes; an approved write-back may update the
notes of target cells when the itemized breakdown is valid (diff + confirmation first).

## Itemized Notes

Use optional uppercase section headers followed by one item per line:

```text
CONTAS:
R$ 120,00 - assinatura mensal
R$ 80,00 - servico recorrente

CARTOES:
R$ 45,90 - compra no credito
```

The parser accepts common spacing variants, including `R$10,00`, `10,00- descricao`, and doubled
spaces. Values remain magnitudes; the parent transaction keeps the financial sign.

## Sections

Classification is by section only:

| Section header          | Kind       | Use for                          |
| ----------------------- | ---------- | -------------------------------- |
| `CONTAS`                | Saida      | Fixed or bill-like outflows      |
| `OUTROS`                | Saida      | Miscellaneous outflows           |
| `DIARIO`                | Diario     | Variable daily spending          |
| `CARTOES`, `FATURAS`    | Cartao     | Credit-card bucket               |
| `ECONOMIA`              | Economia   | Accessible savings/reserve       |
| `INVESTIMENTO`          | Patrimonio | Long-term or illiquid investment |
| `AJUSTES`, `AJUSTE`     | Ajuste     | Difference/reconciliation item   |
| Missing or unrecognized | Saida      | Safe default                     |

There is no fallback by bank, card issuer, brand, or words in the item description. For example,
an unheaded item whose description mentions a bank still defaults to Saida.

## Totals

Items should sum to the cell value. If the note needs a balancing line, add it under `AJUSTES`:

```text
AJUSTES:
R$ 0,10 - diferenca de arredondamento
```

When the item total does not match the cell total, the importer keeps the parsed items as they
are (no synthetic item is stored). The remainder (cell total − sum of items) is reconciled at
read time by the metrics engine as a signed fixed-outflow adjustment — the same role as a manual
`AJUSTES` line — so the financial buckets always add up to the cell total. Write-back falls back
to writing the plain cell total (no `=SUM()` formula, note untouched) whenever the stored items
do not sum to it. The cell total is always the source of truth and is never changed by the app.
A single classified item under a section header is a valid breakdown on its own; a single line
without a section header is treated as a memo, not a breakdown.
