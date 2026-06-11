# Specs

Use this directory for Spec-Driven Development artifacts.

Implemented so far:

- `001-sqlite-local-schema` — normalized local schema (18 migrations) + FTS5.
- `002-google-oauth-pkce` — OAuth desktop flow, Sheets/local-xlsx import, layout detection.
- `003-forecast-core` — pure projected-running-balance engine (TDD).
- `004-app-shell-navigation` — five navigable screens, typed API layer, PT-BR copy.
- `005-forecast-view` — safe-to-spend, deficit warning, daily projection table.

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
