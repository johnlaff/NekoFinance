# AI-Ready Development

## Agent Instructions

- `AGENTS.md` is the canonical cross-agent instruction file.
- `CLAUDE.md` imports `AGENTS.md` for Claude Code compatibility.
- Keep both concise. Long procedures belong in `docs/` or future skills, not always-loaded agent context.

## Spec-First Workflow

Use `specs/<number>-<slug>/` for non-trivial features. Each feature should contain:

- `spec.md`: user stories, requirements, acceptance criteria, non-goals.
- `plan.md`: architecture, data model changes, risks, dependencies, tests.
- `tasks.md`: ordered implementation checklist.
- `acceptance.md`: manual and automated verification.

## AI Coding Guardrails

- Agents may edit code and docs but must not introduce private source material or secrets.
- Agents must prefer deterministic tools/tests over generated assertions.
- Agents must not add cloud infrastructure unless a spec justifies it.
- Agents must leave a clear reason when skipping tests or broader checks.
- Generated code must pass the same quality gates as human code.
- Frontend agents should run Playwright visual-smoke tests when changing layouts or flows and inspect screenshots/traces before declaring UI work complete.
- React-specific agent changes should run `npm run doctor` when the change touches hooks, state, effects, accessibility, or component architecture.

## Product AI Guardrails

- The Neko copilot reads local state and anonymized methodology packs only.
- Retrieval order is formal rules first, then FTS/vector context, then model reasoning.
- Model output that affects money or Sheets writes must be structured, validated, and reviewed by a deterministic reviewer.
- The app should keep an audit trail of proposed actions and approved writes.

## Visual Verification For Agents

Playwright is the UI verification harness for agents. Agents should prefer role/text locators, avoid CSS selectors, capture screenshots for visual review, and use traces to debug failures. Pixel snapshots are intentionally deferred until the design system is stable.
