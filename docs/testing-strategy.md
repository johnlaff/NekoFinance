# Testing Strategy

## Coverage Policy

Coverage is a risk signal, not proof of quality. The policy follows the pragmatic market pattern: high thresholds where correctness matters, lower tolerance for UI shell churn, and explicit exclusions for files that do not have meaningful executable behavior.

Current automated threshold:

- TypeScript coverage gate: 90% lines, statements, branches, and functions for included source.
- Excluded from coverage: app bootstrap, test setup, type-only files, generated files, future fixtures, and files without meaningful runtime behavior.
- **Known gap (2026-06-09)**: overall TS coverage is ~55% (statements). Design system components are at ~98%, but `App.tsx` (919 lines, Google Sheets panel + dashboard + transaction list) is at ~44%. The App component's UI flows (OAuth connect, sheet picker, preview, mapping editor, import) are complex interactive paths — per the strategy below they should be covered by Playwright E2E smoke, not line-by-line Vitest. This gap will be addressed when Playwright E2E is integrated into the CI gate.

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
- Screenshots captured as test artifacts for visual inspection.
- Traces/videos retained on failure.
- Pixel-perfect visual snapshot testing is deferred until the design system stabilizes.

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
- Decision: include Playwright now as visual smoke; keep it out of `npm run check`; run manually/local or via the manual GitHub Actions workflow until the UI stabilizes.

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
