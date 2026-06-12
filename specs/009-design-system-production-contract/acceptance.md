# Acceptance 009 - Design System Production Contract

## Documentation Acceptance

- [ ] The production source of truth is explicit: TSX components in `src/design-system/components/`.
- [ ] Generated JSX/UI kit assets are marked as reference, not production contracts.
- [ ] Component contracts include purpose, props/states, accessibility behavior, and tests.
- [ ] Token layers are documented and distinguish primitive vs semantic vs component/state tokens.
- [ ] Public examples are source-neutral, synthetic, PT-BR/BRL where product-facing.
- [ ] No private source names, personal finance rows, OAuth/token material, emails, domains, or raw
      methodology quotes appear in public docs/examples.

## Token Acceptance

- [ ] Components consume semantic/component tokens, not raw palette tokens, except in token files.
- [ ] Dark and light themes preserve WCAG AA contrast for normal text and finance-critical labels.
- [ ] Focus, danger, warning, success, review, conflict, and approval states remain distinguishable
      without color alone.
- [ ] Reduced-motion users get immediate data state changes without delayed financial values.
- [ ] High-contrast/forced-colors behavior is documented for focus, status, and approval surfaces.

## Component Acceptance

- [ ] `Button` exposes correct native semantics for click/submit, disabled, pending/loading, and
      destructive variants.
- [ ] `Badge` and status components pair color with label/icon/shape.
- [ ] `Money` formats BRL consistently, uses tabular figures, and exposes accessible context.
- [ ] `DataTable` uses semantic table markup, captions/summaries, visible focus, and keyboard-safe
      row actions.
- [ ] Finance components do not collapse Regua 1 and Regua 2 into one blended metric.
- [ ] `SourceCellBadge` can show sheet/year/month/cell/range and source type.
- [ ] `ApprovalDiffCard` can show value diff, note diff, validation, checksum conflict, approval,
      rejection, applied, and rollback states.
- [ ] Copilot components separate deterministic tool output, model prose, and citations.

## Financial Safety Acceptance

- [ ] No material write can be represented as approved without structured diff, validation, and
      explicit human action.
- [ ] Approval copy names the action, such as `Aplicar 14 alteracoes na planilha`, instead of a
      generic `OK`.
- [ ] Destructive/reversible states show what recovery or rollback means before approval.
- [ ] Cell checksum conflicts block apply and explain how to regenerate the diff.
- [ ] Validation failures are announced and keep the approve action disabled.

## Accessibility Acceptance

- [ ] All interactive elements are reachable by keyboard.
- [ ] Focus remains visible and not obscured by sticky headers, dialogs, sidebars, or overlays.
- [ ] Approval dialogs have accessible name, description, focus trap, Escape/close behavior, and
      focus return.
- [ ] Target sizes meet WCAG 2.2 target-size minimum or have equivalent spacing.
- [ ] Tables and charts do not require hover to understand exact values.
- [ ] Charts include a textual summary and data-table equivalent.
- [ ] Screen-reader labels include currency, sign, context, and relevant date/source where visual
      abbreviation could be ambiguous.

## Local-First And Privacy Acceptance

- [ ] Sync/connection states distinguish local data freshness from cloud/provider availability.
- [ ] UI states say when data stays on this device and when an action may send data elsewhere.
- [ ] Copilot/citation examples avoid raw private rows and use synthetic source-neutral labels.
- [ ] Logs, snapshots, and docs avoid secrets and realistic personal finance data.

## Verification Gates

- [ ] Docs-only edits: format reviewed; no app test gate required unless formatting changes are broad.
- [ ] Token/component edits: `npm run test:run`, `npm run typecheck`, and `npm run lint` pass.
- [ ] Layout/flow edits: `npm run e2e` passes and screenshots/traces are inspected.
- [ ] React hook/state/effect/accessibility edits: `npm run doctor` runs with telemetry disabled.
- [ ] Foundation-wide edits: `npm run check` passes.
- [ ] Public docs/examples involving AI, sheets, methodology, or finance data pass
      `npm run privacy:scan`.
