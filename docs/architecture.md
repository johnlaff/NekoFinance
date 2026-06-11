# Architecture

Neko Finance is designed as a local-first desktop app. The repo is public-safe; private data and private methodology packs stay outside git.

## Runtime Layers

| Layer            | Responsibility                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------------- |
| React UI         | Dashboards, chat/captain panel, approval dialogs, data mapping screens.                           |
| Tauri shell      | Desktop runtime, local file access, secure OS integrations, command bridge.                       |
| Local storage    | SQLite for normalized finance data, settings, sync state, and FTS5 text search.                   |
| Local retrieval  | LanceDB for anonymized methodology chunks and future semantic retrieval.                          |
| Google connector | OAuth desktop flow, Google Sheets read/write, sync checkpoints.                                   |
| Copilot          | Tool-using agent that reads local state, calls deterministic finance tools, and proposes actions. |

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

1. Scaffold and privacy guardrails.
2. Local SQLite schema for accounts, transactions, categories, owners, and sheet sync metadata.
3. Google OAuth desktop flow and read-only Sheets import.
4. Dashboard over cached data with owner separation for personal, additional-card, and shared expenses.
5. Private methodology pack loader with schema validation and privacy scan.
6. Copilot with deterministic tools, RAG, and human-approved sheet diffs.
7. Evals for diagnoses, category ownership, and safe write behavior.

## Naming Note

`Neko Finance` is acceptable for the personal MVP. If this becomes a SaaS, naming, domains, trademarks, and app-store conflicts must be reviewed before launch.
