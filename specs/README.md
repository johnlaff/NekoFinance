# Specs

Use this directory for Spec-Driven Development artifacts.

Implemented so far:

- `001-sqlite-local-schema` — normalized local schema (41 migrations today). An FTS5 table was
  added (migration 0015) then dropped (migration 0010-drop) once it turned out to be unpopulated;
  search is client-side.
- `002-google-oauth-pkce` — OAuth desktop flow, Sheets/local-xlsx import, layout detection.
- `003-forecast-core` — pure projected-running-balance engine (TDD).
- `004-app-shell-navigation` — navigable screens, typed API layer, PT-BR copy.
- `005-forecast-view` — safe-to-spend, deficit warning, daily projection table.
- `006-motion-polish`, `007-pockets-liquidity`, `009-design-system-production-contract`.
- `010-import-manual-robusto`, `011-engine-five-types`, `012-stable-identity-reconciliation`.
- `013-advanced-reconciliation` — three-way merge + conflict gate.
- `014-tags`, `015-categories-to-tags`, `016-recurrence`.
- `017-multi-titular` (splits read-side), `018-write-back-preview` (shipped preview-first; the
  write gate has since been enabled — sends require diff approval + confirmation, see ADR-0003).
- `019-month-views` — Totais, Horizonte multi-month, Anual.
- `020-classificacao-notas-5-tipos` — reopens the engine model to make all 5 method buckets
  (entrada/saída/diário/cartão/economia) plus patrimônio explicit, and classifies note line items
  by spreadsheet section instead of free text.
- `021-performance-previsao-diario` — the current month's Performance also subtracts the
  remaining daily forecast (ceiling × days left + pre-launched future dailies), so the month
  starts at the full-scenario value and improves as real spending stays under the ceiling.

`008-auto-import` is specced but deferred. Numbers are unique identifiers, not strict order.

Recommended shape:

```text
specs/
  001-google-sheets-import/
    spec.md
    plan.md
    tasks.md
    acceptance.md
    research.md
```

Flow:

1. Clarify requirements and write `spec.md` focused on user stories and acceptance criteria.
2. Write `plan.md` with architecture, risks, dependencies, data boundaries, and tests.
3. Write `tasks.md` with small implementation steps and parallelizable tasks marked clearly.
4. Implement from tasks, keeping tests and privacy checks green.

This repo is compatible with GitHub Spec Kit concepts, but the CLI is optional until a feature benefits from the full workflow.
