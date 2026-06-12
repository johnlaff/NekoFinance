# Tasks 009 - Design System Production Contract

## Phase 0 - Spec Artifact

- [x] T001 Create spec folder and source-neutral research-backed documentation.
- [x] T002 Capture production vs generated design-system boundary.
- [x] T003 Define acceptance checklist and verification gates.

## Phase 1 - Token Contract

- [ ] T004 Inventory existing tokens in `src/design-system/tokens/*.css` by layer: primitive,
      semantic, component, state, chart, density, accessibility.
- [ ] T005 Add missing semantic tokens for review/confidence, diff/write-back, rollback, privacy,
      local/cloud boundary, connection freshness, and table density.
- [ ] T006 Add token comments documenting intended usage and contrast role.
- [ ] T007 Add high-contrast/forced-colors and reduced-motion checks to token acceptance notes.

## Phase 2 - Core Components

- [ ] T008 Replace production component string-style parsing with explicit class/token-based variant
      styles when each component is next touched.
- [ ] T009 Harden `Button`: loading/pending state, `aria-busy` where needed, focus-visible test,
      disabled semantics, danger confirmation usage notes.
- [ ] T010 Harden `Badge`: require visible label for all statuses; add icon/shape option for
      color-blind-safe status encoding.
- [ ] T011 Harden `SegmentedControl`: verify keyboard behavior, selected state announcement, and
      minimum target size.
- [ ] T012 Add `Panel` for shared card shells and migrate one low-risk screen section.
- [ ] T013 Add `Money` for BRL values, tabular figures, sign display, and screen-reader context.
- [ ] T014 Add `DataTable` wrapper with caption/summary, semantic headers, empty/error slots, and
      keyboard-safe row actions.

## Phase 3 - Finance Components

- [ ] T015 Build `SourceCellBadge` with sheet/year/month/range/cell and source-state variants.
- [ ] T016 Build `TransactionRow` on top of `Money`, `Badge`, `OwnerChip`, and `SourceCellBadge`.
- [ ] T017 Build `SyncStatus` for connected/paused/expired/offline/conflict states.
- [ ] T018 Build `PrivacyDataLocationCard` for local storage, export, backup, and cloud-boundary
      messaging.
- [ ] T019 Build `ReguaCard` so Regua 1 and Regua 2 stay visually adjacent but semantically
      separate.
- [ ] T020 Build `ReserveCard` and `PocketCard` using liquidity classes from spec 007.
- [ ] T021 Build `InvoiceCard` for spec 008 credit/fatura surfaces.

## Phase 4 - Approval And Review

- [ ] T022 Build `ReviewQueueItem` for ambiguous import/reconciliation states.
- [ ] T023 Build `SheetMappingPanel` for source column/range to domain-field validation.
- [ ] T024 Build `ApprovalDiffCard` with value diff, note diff, source range, checksum state,
      validation messages, and approval/reject controls.
- [ ] T025 Add rollback-batch display pattern with conflict copy for cells edited after apply.
- [ ] T026 Add tests for approval flows: accessible name/description, keyboard path, disabled approve
      when validation fails, conflict state announcement.

## Phase 5 - Copilot Provenance

- [ ] T027 Build `MiaCitation` for deterministic tool, source cell, local entity, and timestamp
      provenance.
- [ ] T028 Build `ToolResultBlock` to separate deterministic calculation output from assistant prose.
- [ ] T029 Build `MiaChatBubble` states: placeholder/not-live, pending tool, suggestion, proposal,
      error, rejected, approved.
- [ ] T030 Add copy tests or snapshots using synthetic PT-BR data only.

## Phase 6 - Charts

- [ ] T031 Add chart token documentation for series, threshold, projection, manual/source, grid,
      and axis colors.
- [ ] T032 Add accessible chart wrapper requirements: title, summary, long description, table
      equivalent, keyboard-accessible details.
- [ ] T033 Create first chart only when a product screen needs it; include table alternative in the
      same slice.

## Phase 7 - Verification

- [ ] T034 Run `npm run test:run` for component changes.
- [ ] T035 Run `npm run typecheck` and `npm run lint` for TSX/token integration changes.
- [ ] T036 Run `npm run e2e` and inspect screenshots/traces for layout/flow changes.
- [ ] T037 Run `npm run doctor` for React hook/state/effect/accessibility changes.
- [ ] T038 Run `npm run privacy:scan` before landing public docs/examples involving AI, sheets,
      methodology, or finance data.
