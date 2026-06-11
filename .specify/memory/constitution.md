# Neko Finance Constitution

## Principles

1. Local-first privacy is mandatory. Private finance data, private methodology sources, tokens, embeddings, and raw source material must never enter git.
2. Specifications drive non-trivial implementation. The feature spec defines what and why before code defines how.
3. Deterministic tools own financial correctness. LLMs explain, diagnose, and draft proposals but do not perform authoritative calculations or direct writes.
4. Human approval gates material writes. Google Sheets mutations require a diff, validation result, and explicit user approval.
5. Multi-person ownership is a first-class domain concept. The data model must distinguish account owner, payer, beneficiary, and responsible person.
6. Quality gates are part of done. Typecheck, lint, test, build, Rust checks, and privacy scan must stay green for foundation changes.
7. Prefer small, reversible vertical slices. Avoid premature framework work, backend services, or cloud dependencies until a concrete need exists.

## Governance

- New features should create or update `specs/<number>-<slug>/spec.md`, `plan.md`, and `tasks.md` when the work is non-trivial.
- Plans must list data-boundary, privacy, test, and release implications.
- Exceptions to this constitution must be explicit in the spec or PR notes.
