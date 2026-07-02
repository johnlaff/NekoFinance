# Note Conventions

These conventions describe new spreadsheet notes that the app can classify deterministically.
Existing notes are never rewritten by the app.

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

When the item total does not match the cell total (beyond 1 cent of rounding), the importer keeps
the parsed items and appends a synthetic `AJUSTES` item named `Diferença` worth the remainder
(cell total − sum of items), mirroring the manual convention above. The cell total is always the
source of truth: the app never changes the spreadsheet total, and the synthetic item only exists
so the stored breakdown always sums to it. A single classified item under a section header is a
valid breakdown on its own.
