# Spec 007 — Bolsos & Liquidez (pockets)

## Problema

O método trata o **saldo projetado** como o caixa que realmente paga o mês. Hoje o seed da
projeção soma `bank + wallet + savings` — ou seja, **poupança/reserva infla o caixa projetado**,
e dinheiro que existe fora da planilha (vale alimentação/refeição, previdência corporativa,
FGTS) não é modelado de forma nenhuma.

## Decisão

Toda conta declara **o quão líquido o dinheiro é** via coluna `account.liquidity`:

| liquidity    | exemplo                        | entra no saldo projetado?     | entra na reserva? | patrimônio?            |
| ------------ | ------------------------------ | ----------------------------- | ----------------- | ---------------------- |
| `liquid`     | conta corrente, carteira       | ✅ (seed do forecast)         | —                 | ✅                     |
| `reserve`    | poupança/reserva de emergência | ❌                            | ✅                | ✅                     |
| `restricted` | vale alimentação/refeição      | ❌                            | ❌                | ❌ (ledger à parte)    |
| `illiquid`   | previdência privada, FGTS      | ❌                            | ❌                | ✅                     |
| `NULL`       | cartão de crédito              | ❌ (vira Saída no vencimento) | —                 | passivo (slice futuro) |

Novos `account.type`: `meal_voucher`, `pension`, `fgts` (além dos 5 existentes).

**Patrimônio (net worth)** = `liquid + reserve + illiquid` (restricted fica fora; passivo do
cartão é slice futuro, junto com a entidade `invoice`).

## User stories

- **US1** — Como usuário, meu saldo projetado considera **apenas contas líquidas**, para que a
  poupança não mascare um mês no vermelho. _(correção de método)_
- **US2** — Como usuário, cadastro meus bolsos (vale, previdência, FGTS, poupança, conta) com
  saldo, sem precisar de planilha, em Ajustes.
- **US3** — Como usuário, vejo no Dashboard um cartão **Bolsos & patrimônio** com os grupos
  (caixa, reserva, restrito, ilíquido) e o patrimônio total.

## Contratos

- Migração `account`: rebuild (SQLite não altera CHECK) com `liquidity TEXT CHECK(... )` +
  backfill `bank|wallet|business → liquid`, `savings → reserve`, `credit_card → NULL`.
  Trigger `account_liquidity_default` deriva a liquidez por tipo quando o INSERT não informa
  (parse na borda: dados antigos/import nunca ficam sem classificação).
- `liquid_seed` (forecast): `WHERE liquidity = 'liquid'`.
- Comando `get_pockets() -> Pockets { liquid_cents, reserve_cents, restricted_cents,
illiquid_cents, net_worth_cents, accounts[] }`.
- Comando `create_account(name, account_type, balance_cents, institution?)` — valida tipo,
  deriva liquidez (determinístico, testado), cria a `person` padrão "Eu" se não existir.

## TDD

Finance math ⇒ testes obrigatórios: derivação de liquidez por tipo, agregação dos grupos e
net worth, seed do forecast ignorando `savings`/`restricted`/`illiquid`, trigger de backfill.

## Fora de escopo (slices seguintes)

Ledger de gastos do vale, entidade `invoice`/fatura, reembolso linkado, passivo do cartão no
patrimônio, edição/remoção de contas pela UI.
