# UI Kit — Neko App (Dashboard, Transactions, Copilot)

High-fidelity, click-through recreations of Neko Finance's three core MVP surfaces. They compose the design-system components (never re-implement them) on a shared `AppShell`.

## Files

- `shared/icons.jsx` — Lucide-style 1.75px icon set (`window.Icon`, `name` prop). Shared across screens.
- `dashboard/AppShell.jsx` — sidebar + topbar + optional copilot dock (`window.AppShell`). Props: `active`, `onNav`, `title`, `crumb`, `right`, `dock`, `flush`.
- `dashboard/DashboardScreen.jsx` — health hero, KPI tiles, cashflow chart, category donut, accounts, responsibility split, recent activity.
- `transactions/TransactionsScreen.jsx` — import banner, owner-scope filter, master table + detail panel with category/owner assignment and Google Sheets column mapping.
- `copilot/CopilotScreen.jsx` — Mia chat with inline citations + deterministic tool results, a live `ApprovalDiffCard` (Approve → writes), privacy dock, composer.
- `*/index.html` — mounts each screen; load order is React → ReactDOM → Babel → `_ds_bundle.js` → `icons.jsx` → `AppShell.jsx` → screen.

## How to run

Open any `index.html`. Each links the compiled `_ds_bundle.js` and reads components from `window.NekoFinanceDesignSystem_9bd1cd`. `AppShell.jsx` and `icons.jsx` are plain scripts (no JSX); screen files are `text/babel`.

## What's faked

Data is static. Filters re-slice in-memory arrays; the copilot Approve button flips the diff to `approved` and appends a confirmation. No real Sheets/SQLite/AI calls.

## Composition notes

- Charts (cashflow area+bars, category donut) are hand-built SVG using `--chart-*` tokens — they are data viz, not iconography.
- All money uses tabular mono; all owners use `OwnerChip`; all statuses pair color with a word.
- Layouts collapse responsively (`grid4 → 2col`, master/detail → stacked) under ~1080–1180px.
