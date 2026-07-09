# Engineering Standards

## Research Summary

Checked current primary sources before choosing standards:

- `AGENTS.md` is the open, cross-agent convention for repository instructions.
- Claude Code reads `CLAUDE.md`; the recommended pattern is to import `AGENTS.md` to avoid duplicate instructions.
- GitHub Spec Kit is a mature Spec-Driven Development toolkit and provides a useful workflow even if the CLI is not installed yet.
- Tauri updater supports signed self-updates, static `latest.json`, and GitHub Releases.
- `tauri-apps/tauri-action` builds Linux and Windows desktop bundles in GitHub Actions.
- GitHub Actions standard runners are free for public repos; private repos use account quotas.

## Chosen Methodology

Use Spec-Driven Development for non-trivial features, implemented as a lightweight local workflow first:

1. Clarify the requirement.
2. Write the spec.
3. Write the technical plan.
4. Break the plan into tasks.
5. Implement with tests where correctness matters.
6. Validate with local gates.

Full Spec Kit CLI adoption is deferred until the first large feature where slash commands/templates save more time than they add process.

## Coding Style

- TypeScript: strict mode, explicit boundary validation, React function components, no implicit global state.
- Rust: `edition = "2024"`, rustfmt, clippy with warnings denied in CI.
- Formatting: Prettier for web/docs/config files, rustfmt for Rust.
- Linting: ESLint flat config with type-aware TypeScript rules and React Hooks rules.
- Architecture: functional core, imperative shell. Keep pure finance calculations away from UI and IO.

## Testing Policy

- TDD required: finance math, categorization, ownership splitting, sync, migrations, methodology rules, agent tools, and bug fixes.
- TDD optional: visual-only layout changes and short-lived prototypes.
- Coverage target: 90% for included TypeScript source today, with explicit exclusions for bootstrap, generated, type-only, and no-runtime-behavior files. Critical finance/agent modules should target 95-100% meaningful branch coverage when created.
- Evals are required before enabling agent actions that diagnose finances or propose sheet writes.
- Playwright visual-smoke tests are available for agent UI validation but are not part of the default local gate until the UI stabilizes.
- The UI anti-pattern audit (`npm run ui:audit`, `scripts/impeccable-check.sh`) **is part of the blocking gate** — `npm run check` fails when it finds UI anti-patterns.
- React Doctor is advisory for React-specific code analysis and must run with telemetry disabled locally.

## Project-Specific Calisthenics

These replace generic object-calisthenics dogma with rules that fit TypeScript/Rust/Tauri:

- No hidden IO inside domain functions.
- No direct Sheets writes from UI or LLM paths.
- No unvalidated external data crossing into domain logic.
- No raw strings for critical IDs once the domain module exists.
- No broad service objects that mix storage, network, UI, and business rules.
- Prefer small modules named by domain capability, not by technical layer alone.
- Keep adapters boring: Google Sheets adapter, SQLite adapter, methodology-pack loader, AI provider adapter.

## Quality Gates

Local foundation gate:

```bash
npm run check
```

CI should run the same gate on Ubuntu. Release builds run only on tags or manual workflow dispatch to avoid wasting private-repo minutes.

Optional visual and React-specific checks:

```bash
npm run e2e
npm run doctor
```
