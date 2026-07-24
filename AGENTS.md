# AGENTS.md

## Project

Neko Finance is a local-first Tauri desktop app for personal finance workflows around Google Sheets, dashboards, and an AI copilot.

The public repo must stay source-neutral and data-free. Do not commit private source material, transcripts, embeddings, OAuth tokens, API keys, spreadsheet data, or personal finance cache files.

## Engineering Rules

- Use the newest dependency versions that satisfy peer dependencies, engine requirements, Rust MSRV, and Tauri compatibility. Document exceptions in `docs/version-matrix.md`.
- Project principles for Spec-Driven Development: `.specify/memory/constitution.md`.
- Prefer small vertical slices over broad framework work.
- Keep domain logic deterministic and testable. UI and Tauri shell should orchestrate, not own finance rules.
- Use a functional-core, imperative-shell style: pure calculations in core modules; IO at explicit adapters.
- Parse and validate data at boundaries. Do not trust Sheets rows, local packs, LLM output, or scraped-derived files.
- Avoid primitive obsession for finance-critical concepts. Introduce explicit types/schemas for money, account IDs, owner IDs, transaction IDs, and sheet ranges when those modules are created.
- No LLM free-form financial math. Financial calculations must be deterministic tools with tests.
- Any material Google Sheets write must produce a structured diff, pass validation, and require human approval.

## AI-First Workflow

- Non-trivial work starts with a spec under `specs/<number>-<slug>/` before implementation.
- Use the sequence: clarify requirements, write spec, write plan, write tasks, implement, verify.
- TDD is required for finance math, sync, storage migrations, methodology rules, agent tools, and bug fixes. It is optional for visual-only prototypes.
- Every bug fix should add a regression test unless it is impossible or not useful; document the reason if omitted.

## Privacy

- Keep `.methodology-pack/`, `.neko-data/`, `.lancedb/`, and OAuth/token files local and gitignored.
- Keep `.private-forbidden-patterns` local. It should include private names/domains that must not appear in public files.
- Do not print secrets in logs, test snapshots, manifests, or docs.

## Quality Gates

- Before finishing code work, run the narrowest relevant checks. For foundation changes, run `npm run check` — the full local gate. `npm run build:windows` cross-compiles a single-file Windows exe.
- Keep ESLint, TypeScript strict mode, Prettier, rustfmt, clippy, tests, privacy scan, and npm audit green.
- Do not lower coverage or loosen rules to make a task pass without documenting a concrete reason.
- For frontend layout or flow changes, run Playwright visual smoke and inspect screenshots or traces.
- For React hook/state/effect changes, run React Doctor advisory scan with telemetry disabled.

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues. Use `gh` CLI for all operations. See `docs/agents/issue-tracker.md`.

### Triage labels

All five canonical triage labels use their default names (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: `CONTEXT.md` at repo root + `docs/adr/` at repo root. See `docs/agents/domain.md`.

## Design Context

See `PRODUCT.md` for the full design strategy, and the `neko-finance-design` skill for register, audience, brand voice, references and anti-references.

- **Design system**: "Midnight Purr" — zinc neutrals, configurable brand accent (jade default) hard-separated from method-status colors, Geist type, dark-first. Tokens in `src/design-system/`. Skill at `.agents/skills/neko-finance-design/SKILL.md`.
- **UI standards**: `docs/ui-standards.md` is MANDATORY reading before any screen work — hard rules for copy (didactics behind a question, one invitation per state, formula copy must match the engine), layout (independent columns, DOM = reading order, token values before prototype mapping), components (Meter for every bar, EmptyState for load/error), calm density (whitespace over borders, typographic hierarchy, accent spent not sprayed), per-environment ergonomics (mobile first screen, thumb vs mouse density) and baseline regeneration.
