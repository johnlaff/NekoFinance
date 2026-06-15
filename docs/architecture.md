# Architecture

Neko Finance is designed as a local-first desktop app. The repo is public-safe; private data and private methodology packs stay outside git.

## Runtime Layers

| Layer            | Responsibility                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------------- |
| React UI         | Dashboards, chat/captain panel, approval dialogs, data mapping screens.                           |
| Tauri shell      | Desktop runtime, local file access, secure OS integrations, command bridge.                       |
| Forecast core    | Pure Rust projection engine (`src-tauri/src/forecast/`): chained daily balance, month-end,        |
|                  | future deficit, safe-to-spend, monthly metrics. No IO; commands are thin adapters (spec 003/005). |
| Local storage    | SQLite for normalized finance data, settings, sync state, and FTS5 text search.                   |
| Local retrieval  | LanceDB for anonymized methodology chunks and future semantic retrieval (not built yet).          |
| Google connector | OAuth desktop flow, Google Sheets read/write, sync checkpoints.                                   |
| Copilot          | Tool-using agent that reads local state, calls deterministic finance tools, and proposes actions  |
|                  | (not built yet; the deterministic tools it will call exist in the forecast core).                 |

## Data Boundaries

| Data                                   | Location                             | Git status                    |
| -------------------------------------- | ------------------------------------ | ----------------------------- |
| App code and generic docs              | Repo                                 | Allowed.                      |
| Synthetic fixtures                     | Repo                                 | Allowed if clearly synthetic. |
| OAuth tokens/API keys                  | OS keychain or local ignored files   | Forbidden.                    |
| User financial cache                   | `.neko-data/` or app data dir        | Forbidden.                    |
| Private methodology pack               | `.methodology-pack/` or external dir | Forbidden.                    |
| Raw source material/transcripts/videos | External private archive             | Forbidden.                    |
| Vector indexes/embeddings              | Local ignored dirs                   | Forbidden.                    |

## Copilot Contract

- Deterministic tools calculate totals, categories, budgets, deltas, and sheet diffs.
- The model can explain, diagnose, rank options, and draft proposed changes.
- The model cannot write material changes to Google Sheets directly.
- Every write requires a structured diff, validation, and explicit approval in the UI.
- Retrieval uses formal rules first, then FTS/vector context from anonymized methodology chunks.

## MVP Slices

Done (see `specs/` for the full spec/plan/tasks of each):

1. ✅ Scaffold and privacy guardrails.
2. ✅ Local SQLite schema (spec 001): accounts, transactions, splits, daily check-ins, reserve, sheet sync metadata, FTS5.
3. ✅ Google OAuth desktop flow + Sheets/local-xlsx import with layout detection (spec 002).
4. ✅ Forecast core (spec 003): pure projection engine; the dashboard hero is the projected month-end balance.
5. ✅ Navigable app shell, PT-BR copy (spec 004); screens grew to nine (Dashboard, Totais, Anual, Horizonte, Transações, Tags, Mia, Metodologia, Configurações).
6. ✅ Forecast view (spec 005): safe-to-spend, deficit warning, daily projection table.
7. ✅ Account liquidity classes feeding a correct projection seed (spec 007); five first-class movement types in the engine (spec 011).
8. ✅ Motion & interaction polish (spec 006) and the Design System production contract (spec 009): tokens, accessible component contracts, dark-first WCAG AA, reduced-motion.
9. ✅ Robust import + stable identity, three-way-merge reconciliation, conflict gate (specs 010/012/013); tags + categories→tags demotion, recurrence, multi-titular splits, write-back preview, month/annual views (specs 014, 015, 016, 017, 018, 019).

Next:

10. First-class invoice entity with per-owner splits and net-zero reimbursement links.
11. Copilot (Mia) with deterministic tools first (read-only), then human-approved sheet diffs.
12. Gated bidirectional write-back (per-cell checksum) — flips the system of record to SQLite (ADR-0003).
13. Evals for diagnoses and safe write behavior; what-if scenarios.

## Naming Note

`Neko Finance` is acceptable for the personal MVP. If this becomes a SaaS, naming, domains, trademarks, and app-store conflicts must be reviewed before launch.
