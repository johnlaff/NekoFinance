# Specs

Use this directory for Spec-Driven Development artifacts.

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
