# Testing Strategy

## Coverage Policy

Coverage is a risk signal, not proof of quality. The policy follows the pragmatic market pattern: high thresholds where correctness matters, lower tolerance for UI shell churn, and explicit exclusions for files that do not have meaningful executable behavior.

Coverage thresholds (configured in `vite.config.ts`, enforced when you run `npm run coverage`):

- 90% lines, statements, branches, and functions for included source. This is a **manual/optional**
  signal — the blocking gate (`npm run check`, CI) runs `test:run` (no coverage threshold) plus lint,
  typecheck, Playwright typecheck (`e2e:typecheck`), build, Rust checks (clippy, rustfmt, Rust
  tests), the privacy scan, and the UI anti-pattern audit (`npm run ui:audit`,
  `scripts/impeccable-check.sh` — fails the gate when it finds UI anti-patterns; run it locally
  for details when a UI change breaks `npm run check`). The Playwright browser smoke (`npm run e2e`) is a separate
  non-blocking command — run it manually or via the dedicated CI workflow; it is not part of
  `npm run check`. Multi-step interactive flows (OAuth connect, sheet picker/preview/mapping,
  import) are covered by the E2E smoke rather than line-by-line Vitest, so the global
  `npm run coverage` number sits below 90% for those UI-shell modules by design; the
  design-system and domain modules stay high.
- Excluded from coverage: app bootstrap, test setup, type-only files, generated files, future fixtures, and files without meaningful runtime behavior.
- **Update (2026-06-14)**: the old monolithic `App.tsx` (~919 lines) was decomposed into `src/shell/`, `src/features/` and `src/screens/`; `App.tsx` is now ~130 lines of wiring. The complex interactive flows (OAuth connect, sheet picker, preview, mapping editor, import) moved into feature modules with their own component tests, and the remaining end-to-end paths are covered by the Playwright E2E smoke (`npm run e2e`). Design system components remain at ~98%. Continue to prefer Playwright smoke over line-by-line Vitest for multi-step interactive flows.

Future domain threshold:

- Finance math, ownership splitting, categorization, Sheets diff generation, methodology rule evaluation, and agent tools should target 95-100% meaningful branch coverage.
- UI composition should be tested through user-visible behavior, not line-by-line implementation details.
- Bug fixes should include regression tests unless the test would be lower value than the code path; document exceptions.

Rationale:

- Google coverage guidance frames 90% as exemplary, while warning that no universal number fits all code.
- Martin Fowler's guidance is aligned: upper 80s/90s can be healthy, but numeric targets can create low-value tests if treated as the goal.
- For AI-first repositories, a hard global number can incentivize agents to write shallow tests. The stronger pattern is coverage plus strict lint, typecheck, E2E smoke, code review, and product evals.

## Unit And Integration Tests

- Use Vitest for TypeScript unit/component tests.
- Use React Testing Library for user-visible component behavior.
- Use Rust unit tests for Tauri/domain logic when Rust modules become non-trivial.
- Use deterministic fixtures only. Never use real financial data or private methodology material.

## Playwright E2E Visual Smoke

Playwright is included because UI-capable agents need a deterministic way to open the app, exercise flows, capture screenshots, and inspect traces.

Current scope:

- Chromium only for speed and cost.
- Desktop and mobile-width smoke paths.
- Versioned visual baselines (`tests/e2e/*-snapshots/`) for every screen in both
  themes, plus state variants (spending mode, missing-data states, accent palette).
- Traces/videos retained on failure.

Baseline discipline:

- An intentional visual change regenerates baselines from scratch: delete the snapshot
  directories, run the suite once to record, run it again to verify stability. Visual
  snapshots enforce `maxDiffPixels: 100` (absolute tolerance); copy is locked via text
  assertion (`.aria.yml` versioned files). `--update-snapshots` alone does not rewrite
  sub-threshold drift; pass `--update-snapshots=all` when regenerating deliberately.
- Specs freeze the clock (`page.clock.install`) so greetings/dates render
  deterministically, and the dev server env is pinned so local and CI render alike.

Commands:

```bash
npm run e2e:install
npm run e2e
npm run e2e:ui
npm run e2e:report
```

Tradeoff:

- Value: catches broken layout, routing, accessibility locator regressions, responsive overflow, and interaction failures that unit tests miss.
- Cost: browser downloads, slower CI, some flake risk, and screenshot churn if used for pixel snapshots too early.
- Decision: include Playwright now as visual smoke; keep it out of `npm run check`. It runs in CI on pull requests that touch UI paths (see `.github/workflows/e2e.yml`) and can also be run manually/local.

## React Doctor

React Doctor is included as an advisory React-specific static analysis layer for AI-generated code. It complements ESLint by scanning for React state/effect, performance, architecture, accessibility, and security issues.

Current scope:

- Local command uses `--no-telemetry`.
- Config disables score/share behavior.
- GitHub workflow is advisory and non-blocking.
- It is not part of `npm run check` yet.

Commands:

```bash
npm run doctor
npm run doctor:changed
```

Tradeoff:

- Value: strong fit for AI-ready React repos because it targets common agent mistakes.
- Cost: young tool, possible false positives, extra CI noise, and telemetry defaults that must be disabled locally.
- Decision: keep advisory until we have enough baseline experience; later promote selected rules to blocking if signal is consistently high.

## Evals For Product AI

Before enabling finance advice or Sheets writes, add evals for:

- Green/yellow/red diagnosis.
- Additional-card and partner ownership separation.
- Debt/reserve/grocery/card-spend scenarios.
- Safe tool calling.
- Rejection of direct writes without approval.
- Retrieval over anonymized methodology rules without leaking source references.
