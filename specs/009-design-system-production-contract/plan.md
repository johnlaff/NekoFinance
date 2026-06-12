# Plan 009 - Design System Production Contract

## Current State

- Product direction exists in `PRODUCT.md`: dark-first Midnight Ledger, jade/brass, precision,
  privacy, forecast-first finance, PT-BR UI.
- Domain language exists in `CONTEXT.md`: Person/Profile/Device, Account, Transaction, Split,
  Daily Budget, Daily Check-in, Regua 1, Regua 2, Reserve, Sheet Mapping, Sync Log.
- Production TSX components exist in `src/design-system/components/`: `Button`, `Badge`,
  `EmptyState`, `MetricTile`, `SegmentedControl`, `MiaAvatar`, `NekoMark`.
- Generated/reference contracts exist under grouped `components/core`, `components/finance`, and
  `components/copilot` `.d.ts`/prompt files, plus full-screen UI kits.
- Screens still define many local patterns: cards, tables, connection status, forecast table,
  assistant panel, review-like states, settings panels.
- Tokens exist in CSS custom properties, but the production contract does not yet distinguish every
  semantic state needed for sync, privacy, confidence, diff, rollback, and chart accessibility.

## Architecture Decision

Use `src/design-system/components/*.tsx` and colocated tests as the production source of truth.
Generated JSX bundles and UI kit screens remain reference material until a component is migrated into
typed production TSX.

Rationale:

- Existing production code already imports the TSX components.
- Small TSX components are easier to test with Vitest/Testing Library.
- Finance-critical components need explicit props and accessibility behavior, not generated demo
  markup.
- The current repo has no external component library dependency. Keep that until a specific complex
  primitive justifies React Aria/Radix/Ark UI.

## Token Layers

| Layer         | Purpose                                                       | Examples                                             | Rule                                                    |
| ------------- | ------------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------- |
| Primitive     | Raw palette, font families, scale values                      | `--jade-400`, `--ink-900`, `--fs-body`               | Only token files use these directly.                    |
| Semantic      | Product meaning independent of component                      | `--bg`, `--surface`, `--text`, `--border-focus`      | Components prefer these.                                |
| Component     | Component-specific slots                                      | `--button-bg`, `--panel-border`, `--table-row-hover` | Add only when a semantic token is too broad.            |
| State         | Status/review/diff/confidence/connection meaning              | `--state-review-bg`, `--diff-added-text`             | Never color-only; paired with labels/icons.             |
| Chart         | Series, grid, axis, threshold, forecast/manual/source markers | `--chart-1`, `--chart-grid`, `--chart-projected`     | Stable mapping across screens.                          |
| Density       | Compact/comfortable layout and row heights                    | `--density-row-compact`, `--hit-min`                 | Data tables may compact, controls must stay accessible. |
| Accessibility | Forced-colors/high-contrast/reduced-motion hooks              | `--focus-ring`, high-contrast overrides              | Must preserve meaning without motion/color.             |

## Component Inventory

### Core

| Component          | Production status | Required behavior                                                                                    |
| ------------------ | ----------------- | ---------------------------------------------------------------------------------------------------- |
| `Button`           | Exists            | Native `button`, variants, disabled state, visible focus, loading/pending state needed.              |
| `Badge`            | Exists            | Status label not color-only; icon/shape optional; no uppercase-only copy when readability suffers.   |
| `SegmentedControl` | Exists            | Roving/focus behavior verified; clear selected state; labels visible.                                |
| `EmptyState`       | Exists            | Empty/loading/error variants; no fake AI promises.                                                   |
| `Panel`            | Planned           | Shared card shell with title/action/body/footer slots.                                               |
| `DataTable`        | Planned           | Semantic table wrapper, sticky header behavior, row status, caption/summary, keyboard-safe controls. |
| `Money`            | Planned           | BRL formatting, sign semantics, tabular figures, SR context.                                         |
| `FormField`        | Planned           | Label, hint, error, described-by wiring, pending/disabled states.                                    |

### Finance

| Component                 | Purpose                                  | Required behavior                                                                     |
| ------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------- |
| `ForecastHero`            | Projected balance and safe-to-spend hero | Shows basis date, deficit state, source freshness.                                    |
| `ReguaCard`               | Regua 1/Regua 2 side by side             | Never merges daily debit spend with credit pressure.                                  |
| `ReserveCard`             | Reserve months/trend                     | Uses months of coverage, trend label, no color-only health.                           |
| `PocketCard`              | Liquidity/account group                  | Distinguishes liquid/reserve/restricted/illiquid and forecast inclusion.              |
| `InvoiceCard`             | Credit/fatura module                     | Shows status, due date, owner split, residual/paid states.                            |
| `TransactionRow`          | Transaction list/review row              | Date, description, payment method, amount, source, projection/manual/import status.   |
| `OwnerChip`               | Person/household attribution             | Text + shape/icon; owner colors are accents only.                                     |
| `SourceCellBadge`         | Spreadsheet provenance                   | Sheet/year/month/cell/range; formula/manual/projected markers.                        |
| `ReviewQueueItem`         | Ambiguous import/reconciliation item     | Proposed classification, alternatives, learn-rule action, conflict state.             |
| `SheetMappingPanel`       | Column/range mapping                     | Shows source field, domain target, validation, missing/unknown columns.               |
| `ApprovalDiffCard`        | Sheets write proposal                    | Old/new value, old/new note, affected range, checksum, validation, approval/rollback. |
| `SyncStatus`              | Connection/sync freshness                | Connected/paused/expired/offline/conflict states with action.                         |
| `PrivacyDataLocationCard` | Local-first privacy                      | Data location, backup state, cloud boundary, export controls.                         |

### Copilot

| Component         | Purpose                          | Required behavior                                                                      |
| ----------------- | -------------------------------- | -------------------------------------------------------------------------------------- |
| `MiaChatBubble`   | Assistant/user messages          | Distinguish draft, final, error, pending tool, not-live placeholder.                   |
| `MiaCitation`     | Source/tool provenance           | Source cell, local table, deterministic tool, timestamp; no raw private data in tests. |
| `ToolResultBlock` | Deterministic calculation output | Inputs, outputs, assumptions, validation state, link to source details.                |
| `ConfidenceMeter` | Review confidence                | Label + icon + reason; never used as sole approval gate.                               |

## CSS Strategy

- Keep CSS custom properties as the runtime token transport.
- Add cascade layers when touching global CSS: reset/base, tokens, primitives, components,
  utilities, overrides.
- Use container queries for card/table layout changes instead of viewport-only breakpoints.
- Prefer native HTML and CSS before JS; introduce headless primitives only for complex behavior
  such as combobox, select, menu, popover, dialog, and table navigation.
- Avoid screen-local one-off card/table styles once a production component exists.

## Accessibility Strategy

- Base on WCAG 2.2 AA.
- Prefer semantic HTML. Use ARIA only where native semantics are insufficient.
- For tables, keep real `table` markup for finance data unless virtualization becomes necessary.
- For charts, expose a table equivalent and persistent textual summary.
- For approval flows, implement WCAG financial/data error prevention: review, confirm, and
  recover/reverse.
- For React changes touching hooks/state/effects/accessibility, run `npm run doctor` in addition to
  narrow unit tests.

## AI UX Strategy

- No live chat affordance until the feature works.
- Mia surfaces deterministic tool output separately from model text.
- Suggested write-backs produce `ApprovalDiffCard` proposals, not direct actions.
- Suggested diagnoses cite source cells, local entities, or deterministic tool results.
- Copy uses three modes: `Encontrei`, `Sugiro`, `Aprovar para escrever`.

## Data Visualization Strategy

- Pick chart types by task: trend, deviation, ranking, distribution, part-to-whole, flow, or
  forecast uncertainty.
- Prefer small multiples and direct labels for personal finance diagnostics.
- No 3D, chart junk, decorative gradients, or hover-only exact values.
- Stable chart colors: same domain concepts keep the same series color across screens.
- Every chart has a title, plain-language summary, accessible description, and data table.

## Implementation Sequence

1. **Contract docs**: land this spec, plan, tasks, acceptance, and research.
2. **Token audit**: document current token coverage and add missing semantic state tokens for
   diff/write-back, confidence/review, privacy/local, connection, and density.
3. **Core hardening**: remove string-to-style parsing from production components when touched;
   add class/token-based variants, loading states, and focus tests.
4. **Money + DataTable primitives**: create `Money` and `DataTable` before expanding transaction,
   forecast, credit, and review tables.
5. **Finance components**: migrate repeated dashboard/transaction/settings patterns into `Panel`,
   `TransactionRow`, `SourceCellBadge`, `SyncStatus`, and `PrivacyDataLocationCard`.
6. **Approval components**: build `ApprovalDiffCard` before any write-back UI from spec 008.
7. **Copilot components**: build `MiaCitation` and `ToolResultBlock` before live Mia diagnosis.
8. **Chart contract**: add chart tokens, accessible chart wrappers, and table alternatives before
   adding richer dashboards.
9. **Visual/a11y verification**: run unit tests, typecheck, Playwright visual smoke, React Doctor
   where relevant, and privacy scan.

## Risks

- **Overbuilding components before screens need them**: mitigate by implementing components only
  when a feature slice uses them.
- **Duplicating generated design-system code**: mitigate by choosing TSX production components as
  source of truth and treating generated JSX as reference.
- **Accessibility regressions in dense tables**: mitigate with table-focused tests and manual
  keyboard review.
- **AI trust leakage**: mitigate by keeping provenance and deterministic tool output separate from
  model prose.
- **Private data in examples/snapshots**: mitigate with synthetic PT-BR labels and privacy scan.

## Required Checks By Change Type

- Docs/spec only: `npm run format:check` if formatting uncertainty exists; otherwise no app gate is
  required.
- Token/component unit changes: `npm run test:run`, `npm run typecheck`, `npm run lint`.
- Layout/flow changes: add `npm run e2e` and inspect screenshots/traces.
- React hook/state/effect/accessibility changes: add `npm run doctor`.
- Foundation-wide changes: `npm run check`.
