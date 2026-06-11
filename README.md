# Neko Finance

Neko Finance is a local-first desktop MVP for personal finance workflows around Google Sheets, dashboards, and an AI copilot.

Current status: scaffold only. The app uses synthetic placeholder data and does not include private methodology, scraped source material, OAuth tokens, embeddings, or financial data.

## Stack

- Tauri 2 desktop shell
- React 19 + TypeScript 6 + Vite 8
- Rust 1.96 toolchain locally
- Planned: SQLite/FTS5, LanceDB, Google Sheets API, Vercel AI SDK providers

See `docs/version-matrix.md` for the exact latest-compatible versions checked before project creation.

## Commands

```bash
npm install
npm run check
npm run typecheck
npm run coverage
npm run e2e
npm run doctor
npm run build
npm run privacy:scan
npm run tauri dev
```

Linux desktop builds require the Tauri system prerequisites. On this Ubuntu/WSL environment, `cargo check` also needs `libdbus-1-dev` and `pkg-config`:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev pkg-config
```

## Privacy Rules

- Keep raw source material, transcripts, videos, embeddings, OAuth tokens, and financial data out of git.
- Keep methodology packs outside git under `METHODOLOGY_PACK_DIR`.
- Use only anonymized, source-neutral methodology rules in public docs and code.
- Require a human-approved diff before any material write to Google Sheets.
- Keep `.private-forbidden-patterns` local and gitignored for private names/domains.

## Docs

- `docs/architecture.md`: MVP architecture and implementation slices.
- `docs/engineering-standards.md`: coding standards, SDD workflow, and quality gates.
- `docs/ai-development.md`: agent-ready workflow and product AI guardrails.
- `docs/testing-strategy.md`: coverage, Playwright, React Doctor, and eval policy.
- `docs/claude-design-prompt.md`: ready-to-use prompt for Claude Design.
- `docs/release-and-distribution.md`: release train, Windows builds, and updater plan.
- `docs/multiuser-model.md`: future multiuser/domain ownership model.
- `docs/methodology-pack.md`: private methodology pack contract.
- `docs/version-matrix.md`: versions researched before scaffolding.
- `AGENTS.md`: canonical instructions for AI coding agents.
- `CLAUDE.md`: Claude Code entrypoint importing `AGENTS.md`.
