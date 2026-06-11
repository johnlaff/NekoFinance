# Contributing

Neko Finance is a personal, local-first project, but issues and PRs are welcome.

## Ground rules

- Read `AGENTS.md` first — it is the canonical guide for both humans and AI agents working in
  this repo (commands, engineering rules, quality gates, privacy rules).
- Domain vocabulary lives in `CONTEXT.md`; consequential decisions in `docs/adr/`.
- Non-trivial work starts with a spec under `specs/<number>-<slug>/` (spec → plan → tasks).
- TDD is required for finance math, sync, storage migrations, and bug fixes.
- Run `npm run check` before opening a PR — it is the same gate CI enforces.

## Privacy (non-negotiable)

This repo is public and data-free. Never commit personal financial data, OAuth tokens, private
methodology material, or anything matching the privacy rules in `AGENTS.md`. `npm run
privacy:scan` and gitleaks run on every PR.

## Quick start

```bash
npm ci
npm run tauri dev   # desktop app
npm run check       # full local gate
```
