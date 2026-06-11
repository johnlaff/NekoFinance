# Spec: App Shell — Real Navigation, Screens, and PT-BR Copy

## Summary

Turn the single-screen prototype into a navigable five-screen desktop app: **Dashboard**,
**Transações**, **Mia (copiloto)**, **Metodologia**, and **Configurações e privacidade**. The
sidebar items today render as buttons but do nothing; every screen except the dashboard does not
exist. This spec adds a typed in-app navigation state, extracts the 900-line `App.tsx` monolith
into screen/shell/lib modules, fixes the missing PT-BR diacritics across the entire UI, and gives
the local `.xlsx` import (backend command already shipped in `002`) its missing UI.

No new finance math: this is shell/UI work on top of the engine from `003`.

## Motivation

The app is functionally a dashboard demo: clicking "Transações" or "Configurações" silently does
nothing, the search input is decorative, and the only data-entry path (Google OAuth) is buried in
the sidebar footer and requires a configured client id. A first usable release needs working
navigation, a full transaction list, a settings surface that exposes both import paths (Google
Sheets and local `.xlsx`), and honest placeholders where features are not built yet. The UI copy
also ships without Portuguese diacritics ("Transacoes", "Financas", "Diario"), which reads as
broken product in the target locale.

## User Stories

### US1 — Navigate between screens

**As** the primary user
**I want** the sidebar items to actually switch screens
**So that** the app behaves like an application instead of a static page.

**Acceptance**: Clicking a sidebar item renders the corresponding screen and marks it
`aria-current="page"`. Navigation is plain in-app state (no URL routing): a typed `Screen` union
drives which screen renders. Screen state survives switching (no remount-induced data loss for
already-fetched data is required in this slice; refetch on mount is acceptable). Covered by
component tests.

### US2 — Transações screen

**As** the primary user
**I want** a dedicated screen listing my transactions with filter and search
**So that** I can inspect everything that was imported, not just the 20 most recent.

**Acceptance**: Screen lists transactions (recent-first, up to a explicit fetch limit documented in
code), with the same Todas/Crédito/Futuro filter the dashboard card has, plus a client-side text
search over the description. Empty/loading/error states use the design-system `EmptyState`.
Currency renders via the shared BRL formatter (INTEGER cents in, localized string out).

### US3 — Header search that does something

**As** the primary user
**I want** the header search to take me to the filtered transaction list
**So that** the most prominent input in the app is not decorative.

**Acceptance**: Submitting a query in the header search navigates to Transações with the query
applied. The input has an accessible label. (Full-text FTS5 search via SQL is a later slice; this
slice filters the fetched list.)

### US4 — Configurações e privacidade screen

**As** the primary user
**I want** a settings screen with my data connections and privacy facts
**So that** I can manage Google Sheets, import a local spreadsheet copy, and see where my data lives.

**Acceptance**: The screen hosts (a) the Google Sheets connection/import flow (moved out of the
sidebar footer; the sidebar keeps only a compact connection status), (b) **local `.xlsx` import**
via a native file dialog wired to the existing `import_local_xlsx` command, reporting per-sheet
import counts, and (c) a read-only "seus dados" block showing the local database location and app
version (new `get_app_info` command), stating that data stays on-device. Import results and errors
surface in the UI, not only in logs.

### US5 — Metodologia screen

**As** the primary user
**I want** a short in-app explanation of the method the numbers follow
**So that** the hero metrics (saldo projetado, diário, reserva) are self-explaining.

**Acceptance**: A static screen describes, in original neutral copy (no third-party course
material), the concepts the app already models: previsibilidade (forecast-first), saldo projetado
de fim de mês, custo de vida vs diário, Régua 1 (débito) vs Régua 2 (crédito/fatura), and reserva
em meses. Content sourced from `CONTEXT.md` vocabulary only.

### US6 — Mia placeholder that is honest

**As** the primary user
**I want** the copilot screen to say clearly what exists and what is coming
**So that** the app never fakes an AI feature.

**Acceptance**: The Mia screen shows the copilot identity (DS avatar), states that chat is under
development, and lists what Mia will do (read-only diagnosis first; any spreadsheet write always
behind explicit approval). No fake input box pretending to work.

### US7 — PT-BR diacritics everywhere

**As** the primary user
**I want** correct Portuguese accents in every UI string
**So that** the product reads as finished software in its target language.

**Acceptance**: All user-facing strings carry correct diacritics ("Transações", "Finanças",
"Configurações", "Diário hoje", "Crédito no mês", "Descrição", "Método", "Nenhuma transação",
"Salário projetado"…). Tests assert the accented strings. No source/data files require encoding
changes (UTF-8 already).

### US8 — Maintainable frontend layout

**As** a developer (and any AI agent working on this repo)
**I want** the monolithic `App.tsx` split into cohesive modules
**So that** screens can evolve independently and diffs stay reviewable.

**Acceptance**: `App.tsx` becomes composition + navigation state only. New layout:
`src/lib/` (formatters, typed Tauri API wrappers + shared DTO types), `src/shell/` (AppShell,
ThemeToggle), `src/features/sheets/` (Google Sheets panel), `src/screens/` (one file per screen).
Existing design-system imports unchanged. All existing tests keep passing (updated for accents);
new tests cover navigation and formatters.

## Non-functional requirements

- **No new heavyweight deps**: navigation is typed React state (5 fixed screens, no URL semantics
  in a desktop webview). The only new dependency is `tauri-plugin-dialog` (+ its JS guest binding)
  for the native file picker, pinned exact like the rest of the manifest, with the matching
  capability permission.
- **Design system**: screens use existing DS tokens/components (`EmptyState`, `Button`, `Badge`,
  `SegmentedControl`, `MetricTile`, avatars) and the ui-kit CSS conventions; dark-first, WCAG AA.
- **A11y carry-overs from the first UI review**: accessible search label, `aria-current` on nav,
  `prefers-reduced-motion` respected for spinners.
- **Honesty rule**: no dead controls — every visible interactive element does something or is
  removed (the decorative notifications bell is removed until a notification source exists).
- **Privacy**: methodology screen copy is original and source-neutral; no personal data in
  fixtures, tests, or copy (Constitution P1).
- **Testing**: component tests for navigation, screen smoke states, and formatters (TDD optional
  for visual-only parts per AGENTS, required for any logic — formatters, filter functions).
