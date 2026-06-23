# Plan 061: Show classified items in Lançamentos + document the note convention

> **Executor instructions**: Follow step by step; run every verification command. If a "STOP
> condition" occurs, stop and report. When done, update `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src/screens/TransactionsScreen.tsx src/lib/api.ts`
> Compare excerpts to live code before proceeding.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW
- **Depends on**: plans/059-line-item-classification-core.md, plans/060-item-attribution-and-divergence.md
- **Category**: ux / docs
- **Planned at**: commit `da2d3e9`, 2026-06-22

## Why this matters

Plans 059/060 make the app _know_ each note item's kind (Saída / Cartão / Diário / Economia /
Patrimônio / Ajuste) and the new metric buckets (Cartão total, Economia%, Patrimônio). This plan
surfaces them to the user: in **Lançamentos**, expanding a Saída lump shows its items with a kind
badge + a "itens não batem" marker when the note sum diverges (warn-only — the app never
fabricates a value, per the owner's decision); and the metric screens (Este mês / O ano) show the
new **Gastos com cartão** bucket, the automatic **Economia%**, and **Patrimônio** as a separate
long-term line (excluded from custo de vida and from the accessible Economia%). It also documents
the lightweight note **convention** so the user's _new_ notes parse cleanly — without rewriting
the existing spreadsheet.

## Current state

- `src/screens/TransactionsScreen.tsx` — the ledger. Rows can expand; line-item detail comes
  from the per-transaction items read (plan 035). Locate how items are fetched/rendered:
  `grep -n "getLineItems\|line_item\|itens\|LineItem" src/screens/TransactionsScreen.tsx src/lib/api.ts`.
  - `src/lib/api.ts` has a `getLineItems(transactionId)` binding (used by Compose edit-mode).
    After plan 060 the items carry a derived `kind` field — extend the TS type accordingly.
- Type/movement vocab + colors: `src/lib/nkFormat.ts` exposes `TYPE_META` (entrada/saida/diario/
  economia/cartao → color token + glyph) and the design tokens `--type-*`. Reuse these for
  badges — do NOT invent new colors. Cartão = `--type-cartao`, etc. "Diferença"/Ajuste can reuse
  a neutral/warning token already in the design system (grep `--warning` in `src/design-system`).
- Design constraints (`PRODUCT.md` / the design system): dark-first, calm, no clutter; money is
  never animated; badges should be quiet. Match the existing chip/badge component style already
  used in the redesigned screens (grep for an existing badge/chip class in `src/redesign.css` or
  the screen CSS).
- Convention docs live under `docs/` (method-neutral; the repo must stay data-free — no real
  names/values). The methodology pack (`.methodology-pack/`) is gitignored and must not be
  referenced by path in committed docs.

## Commands you will need

| Purpose   | Command                | Expected |
| --------- | ---------------------- | -------- |
| Typecheck | `npm run typecheck`    | exit 0   |
| Lint      | `npm run lint`         | exit 0   |
| Unit test | `npm run test:run`     | all pass |
| E2E       | `npm run e2e`          | all pass |
| Privacy   | `npm run privacy:scan` | passes   |
| UI audit  | `npm run ui:audit`     | exit 0   |

## Scope

**In scope:**

- `src/lib/api.ts` — extend the line-item type with the derived `kind` (from plan 060).
- `src/screens/TransactionsScreen.tsx` (+ `src/screens/lancamentos.css`) — kind badge per item
  in the expanded row; "itens não batem" marker when divergence-flagged.
- `src/screens/TransactionsScreen.test.tsx` — assert badges render per kind.
- `docs/note-conventions.md` (create) — the lightweight, method-neutral note convention.

**Out of scope (do NOT touch):**

- The Rust engine/import (done in 059/060), the forecast math, other screens.
- Any change that alters the spreadsheet or suggests rewriting existing notes.
- New colors/tokens — reuse `TYPE_META` / `--type-*` / existing warning token.

## Git workflow

- Branch: `advisor/061-ledger-item-badges-and-convention`
- Message: `feat(lancamentos): kind badges on note items + document note convention`

## Steps

### Step 1: Type the derived kind

In `src/lib/api.ts`, extend the line-item type returned by `getLineItems` (and the month-metric
DTO from plan 060) with `kind: "saida" | "cartao" | "diario" | "economia" | "patrimonio" | "ajuste"`,
plus the new metric fields `cartao_cents` and `patrimonio_cents` on `MonthMetric`.

**Verify**: `npm run typecheck` → exit 0.

### Step 2: Render kind badges in the expanded row

In `TransactionsScreen.tsx`, where the expanded row lists items, render a small badge per item
using `TYPE_META` for cartao/saida/economia and a neutral/warning style for `conta`/`ajuste`
(reuse an existing chip class). When the transaction is divergence-flagged (plan 060), show a
quiet "itens não batem" note near the breakdown. Keep it calm and uncluttered (PRODUCT.md).

**Verify**: `npm run typecheck && npm run lint && npm run ui:audit` → exit 0.

### Step 3: Test the badges

In `TransactionsScreen.test.tsx`, with `mockCommands` returning a transaction whose items
include a `kind:"cartao"` and a `kind:"conta"` item (+ a divergence-flagged case), assert both
badges render and the "itens não batem" marker shows only when flagged. Pattern:
`DashboardScreen.test.tsx`.

**Verify**: `npm run test:run` → all pass, including the new badge tests.

### Step 4: Document the note convention

Create `docs/note-conventions.md` describing the supported grammar (method-neutral, no private
data):

- Section headers in ALL-CAPS: `CONTAS` (saída fixa), `CARTÕES`/`FATURAS` (cartão), `ECONOMIA`
  (poupança acessível), `INVESTIMENTO` (patrimônio/previdência — longo prazo, separado da
  economia), `OUTROS` (saída), `AJUSTES` (Diferença).
- One item per line: `R$ <valor> - <descrição>` (the parser tolerates `R$10,00`, `10,00- desc`,
  double spaces).
- Items should sum to the cell value; use an `AJUSTES` line `R$ <x> - Diferença` to close it.
- Note that classification is by **section first**, with a card/bank-name fallback only for
  unheaded notes, and that "Fatura <telecom>" is a bill, not a card.
- State explicitly: this is for **new** notes; existing notes are never rewritten by the app.

**Verify**: `npm run privacy:scan` → passes (no real names/values).

## Test plan

- Badge per kind renders (cartao/conta at minimum); divergence marker conditional.
- Convention doc exists, is private-data-free.
- e2e still green (Lançamentos nav + content unaffected by additive badges).

## Done criteria

- [ ] Line-item type carries `kind`; badges render per kind in the expanded ledger row
- [ ] Divergence-flagged transactions show a quiet "itens não batem" marker
- [ ] `docs/note-conventions.md` exists, method-neutral, data-free
- [ ] `npm run typecheck`, `npm run lint`, `npm run ui:audit` exit 0
- [ ] `npm run test:run` exits 0; new badge tests pass
- [ ] `npm run e2e` exits 0
- [ ] `npm run privacy:scan` passes
- [ ] `plans/README.md` status row updated

## STOP conditions

- The redesigned `TransactionsScreen` has no item-expansion surface at all and adding one
  balloons scope beyond badges → STOP and report (may need a small dedicated plan to restore
  the itemized expansion from plan 035 first).
- `TYPE_META` / `--type-*` tokens are gone → STOP (don't invent colors).

## Maintenance notes

- Badges read the derived `kind` from the backend (plan 060). If the classifier (plan 059)
  gains kinds, extend `TYPE_META`/the badge switch.
- Reviewer: confirm no new color tokens, calm styling, and that the convention doc contains no
  real financial data or personal names.
- Deferred (not here): a dedicated "Cartão" analytics view using plan 060's per-period kind
  totals — raise separately if the user wants a card-spend dashboard.
