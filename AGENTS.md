# AGENTS.md

## Project

Neko Finance is a local-first Tauri desktop app for personal finance workflows around Google Sheets, dashboards, and an AI copilot.

The public repo must stay source-neutral and data-free. Do not commit private source material, transcripts, embeddings, OAuth tokens, API keys, spreadsheet data, or personal finance cache files.

## Repo Map

- `src/` — React/TypeScript frontend; screens consume only `*View.ts`/hooks (never `lib/api` directly).
- `src/design-system/` — "Midnight Purr" tokens and components.
- `src-tauri/` — Rust shell and functional core (`reading/`, `forecast/`, `mia/`, `commands/`); has its own `AGENTS.md`.
- `specs/NNN-slug/` — Spec-Driven Development artifacts (spec → plan → tasks).
- `docs/adr/` — architectural decisions; `CONTEXT.md` — domain glossary and shared language.
- `evals/` — Mia copilot evaluation harness.

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

## Commits, Branches, and PRs

- Never push to `main` directly: branch → PR → all checks green → squash-merge.
- Commit subject: a descriptive sentence in pt-BR stating what changed and why (no Conventional Commits prefixes); body carries the rationale when the subject is not enough.
- Branch names are short and purpose-prefixed (e.g. `impl/issue-123`, `ci/<topic>`).
- PRs are didactic: what and why in the first paragraph, how to verify, and `Closes #N` on its own line to auto-close the issue.
- No agent/model self-references in commits, PRs, code comments, or docs.

## Privacy

- Keep `.methodology-pack/`, `.neko-data/`, `.lancedb/`, and OAuth/token files local and gitignored.
- Keep `.private-forbidden-patterns` local. It should include private names/domains that must not appear in public files.
- Do not print secrets in logs, test snapshots, manifests, or docs.

## Quality Gates

- Before finishing code work, run the narrowest relevant checks. For foundation changes, run `npm run check` — the full local gate. `npm run build:windows` cross-compiles a single-file Windows exe.
- Single-test commands for TDD loops: `npx vitest run <path> -t "<name>"` (TS) and `cargo test <name> --manifest-path src-tauri/Cargo.toml` (Rust).
- Keep ESLint, TypeScript strict mode, Prettier, rustfmt, clippy, tests, privacy scan, and npm audit green.
- Do not lower coverage or loosen rules to make a task pass without documenting a concrete reason.
- For frontend layout or flow changes, run Playwright visual smoke and inspect screenshots or traces.
- React Doctor gates pull requests (`blocking: warning`, scope `changed`): anything the PR introduces must be fixed, not waived. It is not part of `npm run check`, and no local command reproduces its verdict in full — so a frontend change is only cleared once the PR check is green. `npm run doctor` (whole project) and `npm run doctor:changed` (against the base) run it locally with telemetry disabled, as an early signal rather than proof.
- A local scan reports FEWER findings than CI, and the gap is structural: the security rules walk the working directory and read at most 2500 files per bucket, while build output and tool caches (`src-tauri/target/`, agent state directories) push a working tree well past that. Files under `src/` fall outside the budget and their security findings never surface locally. To reproduce the CI verdict, scan a clean checkout: `git clone --no-hardlinks . /tmp/nk-scan && cd /tmp/nk-scan && npx react-doctor@<pinned> . --json --json-out /tmp/scan.json`.

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
