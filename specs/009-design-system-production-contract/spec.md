# Spec 009 - Design System Production Contract

## Summary

Turn the existing Midnight Ledger design system into a production contract for Neko Finance:
tokens, components, accessibility behavior, chart rules, approval surfaces, and AI/copilot
provenance. This is not a redesign. It hardens the current direction so future screens stop
reimplementing cards, tables, status chips, write-back diffs, and privacy states ad hoc.

## Problem

The repo has a strong product/design direction (`PRODUCT.md`) and useful generated design-system
assets under `src/design-system/`, but production code currently uses only a small TSX subset and
some screen-local patterns. Rich domain components such as approval diffs, source-cell badges,
owner chips, transaction rows, citations, confidence states, and chart language are not yet a
shared implementation contract.

Without a contract, upcoming slices (`007-pockets-liquidity`, `008-auto-import`, Mia, credit,
review queue, write-back rollback) will duplicate visual and accessibility behavior at exactly the
places where financial mistakes are most expensive.

## Goals

- Define the production design-system contract for tokens, components, states, and accessibility.
- Align the contract with WCAG 2.2 AA, local-first privacy, spreadsheet reconciliation, and explicit
  human approval for writes.
- Keep Midnight Ledger: dark-first graphite, jade primary, brass warmth, Hanken Grotesk, Geist Mono
  for money, Newsreader for methodology/editorial surfaces.
- Make finance-critical surfaces auditable: money values, tables, source cells, diffs, validation,
  rollback, chart alternatives, deterministic tool provenance.
- Give future implementation slices a concrete component inventory and acceptance standard.

## Non-goals

- No visual rebrand.
- No broad component rewrite in this spec artifact.
- No new cloud or AI provider integration.
- No private methodology material, real personal finance data, screenshots, tokens, OAuth state, or
  spreadsheet rows in public files.
- No generic fintech dashboard language, USD examples, gamification, or fake AI chat behavior.

## Product Principles

1. **Forecast first**: the projected balance, safe-to-spend, reserve health, and credit pressure are
   more important than decorative historical dashboards.
2. **Spreadsheet aware**: the UI must show sheet/year/month/range/cell provenance, formula safety,
   and before-after diffs whenever Sheets could change.
3. **Local-first visible**: storage location, offline readiness, sync status, and data-leaves-device
   moments must be visible without alarmism.
4. **AI as proposal, not authority**: Mia explains and drafts proposals from deterministic tools;
   she never performs free-form financial math or writes without validation and approval.
5. **Finance without shame**: warnings are precise and actionable, not moralizing.
6. **Dense, not cramped**: desktop tables may be compact, but focus, target size, keyboard access,
   and screen-reader labels remain first-class.

## User Stories

### US1 - Shared production components

**As** a developer building finance screens
**I want** a documented, tested TSX component contract
**So that** Dashboard, Transacoes, Credito, Caixa de revisao, Ajustes, and Mia approval flows use
consistent visual, accessibility, and state behavior.

**Acceptance**: The component inventory in `plan.md` has props, required states, keyboard behavior,
and test requirements. Production work uses TSX components under `src/design-system/components/` as
the source of truth; generated bundle examples are reference only.

### US2 - Token architecture that survives theme/accessibility variants

**As** a maintainer
**I want** token layers for primitive, semantic, component, state, chart, density, and accessibility
variants
**So that** dark/light/high-contrast and compact/comfortable layouts can evolve without rewriting
screens.

**Acceptance**: Tokens are documented with naming rules, source layer, intended usage, contrast role,
and fallback behavior. Components consume semantic/component tokens, not raw palette tokens, except
inside token definitions.

### US3 - Approval surfaces for material writes

**As** the person using the app
**I want** every material Google Sheets write to show exactly what will change
**So that** I can catch mistakes before they affect my spreadsheet.

**Acceptance**: `ApprovalDiffCard` and related patterns show old/new value, old/new note, sheet/range,
cell checksum/conflict state, validation status, source transaction IDs, and rollback batch. Approval
requires an explicit action with action-specific copy, not a generic confirm.

### US4 - Accessible finance tables and charts

**As** a keyboard or screen-reader user
**I want** tables and charts to expose exact data and relationships
**So that** I can audit transactions, projections, and trends without depending on color, hover, or
image-only charts.

**Acceptance**: Tables have semantic headers, keyboard-operable controls, money labels, tabular
figures, non-color-only statuses, and persistent focus. Charts have titles, descriptions, direct
labels where useful, table equivalents, no hover-only tooltips, and non-color encodings.

### US5 - Mia provenance and human control

**As** the person using Mia
**I want** every suggestion to show its data source and deterministic tool basis
**So that** I can decide whether to approve, edit, reject, or ignore it.

**Acceptance**: Copilot UI distinguishes `I found`, `I suggest`, and `Approve to write`; cites source
cells/tool results; shows uncertainty/review states; and never presents an unimplemented chat as live.

## Functional Requirements

- **FR1**: Define token layers for color, typography, spacing, radius, elevation, motion, density,
  chart series, owner/person roles, confidence/review states, diff/write-back states, and
  local/privacy/connection states.
- **FR2**: Document production TSX component contracts for existing components: `Button`, `Badge`,
  `SegmentedControl`, `EmptyState`, `MetricTile`, `MiaAvatar`, `NekoMark`.
- **FR3**: Document planned component contracts for finance-critical surfaces: `Panel`, `DataTable`,
  `Money`, `ForecastHero`, `ReguaCard`, `ReserveCard`, `PocketCard`, `InvoiceCard`,
  `TransactionRow`, `OwnerChip`, `SourceCellBadge`, `ReviewQueueItem`, `SheetMappingPanel`,
  `ApprovalDiffCard`, `SyncStatus`, `PrivacyDataLocationCard`, `MiaChatBubble`, `MiaCitation`,
  `ToolResultBlock`, chart blocks, loading/error states.
- **FR4**: Each status component must pair color with visible text and/or icon/shape.
- **FR5**: Every money value must use tabular figures and expose sign, currency, value, and context
  to assistive technology when visual abbreviation could be ambiguous.
- **FR6**: Every approval dialog must expose validation failures before the approve action and keep
  the focused control visible.
- **FR7**: Every material write proposal must show source, deterministic validation, before/after
  diff, conflict/checksum state, approval state, rejection state, and rollback visibility.
- **FR8**: Copilot components must separate model prose from deterministic tool results and
  citations.
- **FR9**: Chart patterns must include a chart-type decision guide and accessible data equivalent.
- **FR10**: PT-BR copy rules must cover empty, loading, warning, error, conflict, approval,
  rollback, success, privacy, and AI states.

## Accessibility Requirements

- WCAG 2.2 AA minimum for production UI.
- Focus indicators must be visible and not obscured by sticky headers, dialogs, or sidebars.
- Interactive targets must be at least 24 px or have equivalent spacing; primary desktop controls
  should continue to use the existing `--hit-min` pattern where possible.
- Reduced motion must preserve meaning and avoid delayed financial data.
- Financial/data-changing flows must support review, confirmation, and reversal/recovery.
- Dialogs must trap focus, return focus on close, expose accessible names/descriptions, and avoid
  destructive default actions.
- Authentication/connection flows must support paste/password managers and avoid memory/cognitive
  tests beyond standard auth.
- Charts must not encode critical status with color alone.

## Privacy Requirements

- Public examples use synthetic labels only: `Pessoa A`, `Pessoa B`, `Conta principal`,
  `Cartao principal`, `Instituicao`, `Compromisso fixo`, `Documento de origem`,
  `Reembolso previsto`.
- UI copy must say when data stays local and when data may leave the device.
- No private names, domains, emails, source names, course names, raw methodology quotes, OAuth
  tokens, API keys, embeddings, or realistic personal finance rows.
- Copilot provenance logs must be source-neutral and avoid printing sensitive row contents in public
  test snapshots.

## Acceptance

See `acceptance.md` for the design review and verification checklist.
