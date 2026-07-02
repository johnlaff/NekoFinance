# Architecture

Neko Finance is designed as a local-first desktop app. The repo is public-safe; private data and private methodology packs stay outside git.

## Runtime Layers

| Layer            | Responsibility                                                                                                                                                                                                                                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| React UI         | Dashboards, chat/captain panel, approval dialogs, data mapping screens.                                                                                                                                                                                                                                            |
| Tauri shell      | Desktop runtime, local file access, secure OS integrations, command bridge.                                                                                                                                                                                                                                        |
| Forecast core    | Pure Rust projection engine (`src-tauri/src/forecast/`): chained daily balance, month-end,                                                                                                                                                                                                                         |
|                  | future deficit, safe-to-spend, monthly metrics. No IO; commands are thin adapters (spec 003/005).                                                                                                                                                                                                                  |
| Local storage    | SQLite (WAL) for normalized finance data, settings, and sync state. Full-text search was prototyped (migration 0015) and removed (migration 0010-drop) — tables were never populated; search is client-side. Re-add with triggers and rebuild when FTS is actually implemented.                                    |
| Local retrieval  | LanceDB for anonymized methodology chunks and future semantic retrieval (not built yet).                                                                                                                                                                                                                           |
| Google connector | OAuth desktop flow, Google Sheets read/write, sync checkpoints.                                                                                                                                                                                                                                                    |
| Copilot          | `CopilotScreen` is a chat-shaped UI, not an agent yet: one deterministic answer ("quanto posso gastar hoje", from `get_dashboard_summary` + `get_forecast`) plus static seeded/suggestion messages. Free-form input gets a canned "still learning" reply — no tool-calling backend, retrieval, or LLM is wired in. |

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

## Copilot Contract (target — not yet implemented)

The rules below are the contract the copilot must satisfy once it exists. Today `CopilotScreen`
has no tool-calling backend, so none of this is wired in yet (see the Copilot row above).

- Deterministic tools calculate totals, categories, budgets, deltas, and sheet diffs.
- The model can explain, diagnose, rank options, and draft proposed changes.
- The model cannot write material changes to Google Sheets directly.
- Every write requires a structured diff, validation, and explicit approval in the UI.
- Retrieval (once built) should use formal rules first, then any local retrieval layer — no
  full-text or vector index exists in the app today (see Local storage / Local retrieval rows).

## MVP Slices

Done (see `specs/` for the full spec/plan/tasks of each):

1. ✅ Scaffold and privacy guardrails.
2. ✅ Local SQLite schema (spec 001): accounts, transactions, splits, reserve, sheet sync metadata. (An early daily-check-in table had no production writer and was later dropped — see CONTEXT.md.)
3. ✅ Google OAuth desktop flow + Sheets/local-xlsx import with layout detection (spec 002).
4. ✅ Forecast core (spec 003): pure projection engine. O herói do dashboard é "pode gastar até X hoje" (guardrail duplo caixa × poupança anual); o saldo projetado de fim de mês é o aside secundário.
5. ✅ Navigable app shell, PT-BR copy (spec 004); screens grew to nine (Hoje/Dashboard, Lançamentos, Este mês/Totais, O ano/Anual, Calendário, Horizonte, Tags, Mia, Configurações).
6. ✅ Forecast view (spec 005): safe-to-spend, deficit warning, daily projection table.
7. ✅ Account liquidity classes feeding a correct projection seed (spec 007); five first-class movement types in the engine (spec 011).
8. ✅ Motion & interaction polish (spec 006) and the Design System production contract (spec 009): tokens, accessible component contracts, dark-first WCAG AA, reduced-motion.
9. ✅ Robust import + stable identity, three-way-merge reconciliation, conflict gate (specs 010/012/013); tags + categories→tags demotion, recurrence, multi-titular splits, write-back preview, month/annual views (specs 014, 015, 016, 017, 018, 019).
10. ✅ Engine model reopened to make Cartão and Patrimônio explicit `EventKind` buckets (6 total) and classify note line items by spreadsheet section (spec 020).

Next:

11. First-class invoice entity with per-owner splits and net-zero reimbursement links.
12. Copilot (Mia) with deterministic tools first (read-only), then human-approved sheet diffs.
13. Gated bidirectional write-back (per-cell checksum) — flips the system of record to SQLite (ADR-0003).
14. Evals for diagnoses and safe write behavior; what-if scenarios.

## Naming Note

`Neko Finance` is acceptable for the personal MVP. If this becomes a SaaS, naming, domains, trademarks, and app-store conflicts must be reviewed before launch.
