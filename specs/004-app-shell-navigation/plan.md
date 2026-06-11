# Plan: App Shell — Real Navigation, Screens, and PT-BR Copy

## Architecture

```
src/
  App.tsx                  → navigation state only: { screen, searchQuery } + composition
  lib/
    format.ts              → fmtBRL(cents), fmtDate(iso)  (pure, tested)
    api.ts                 → DTO interfaces (DashboardSummary, TransactionRow, …) +
                             typed invoke wrappers + isTauri guard
  shell/
    AppShell.tsx           → sidebar (nav + compact Sheets status), header (search, theme), body
    ThemeToggle.tsx
  features/sheets/
    GoogleSheetsPanel.tsx  → OAuth → pick → preview → mapping → import (moved, behavior intact)
    LocalXlsxImport.tsx    → native file dialog → import_local_xlsx → result/error panel
  screens/
    DashboardScreen.tsx    → current dashboard content (hero, 4 tiles, recent txns, Mia aside)
    TransactionsScreen.tsx → full list + SegmentedControl filter + text search
    CopilotScreen.tsx      → honest placeholder (MiaAvatar + roadmap copy)
    MethodologyScreen.tsx  → static method explainer (CONTEXT.md vocabulary)
    SettingsScreen.tsx     → GoogleSheetsPanel + LocalXlsxImport + "seus dados" (get_app_info)
```

Navigation: `type Screen = "dashboard" | "transactions" | "copilot" | "methodology" | "settings"`.
`App` owns `screen` + `pendingSearch`; `AppShell` receives `activeNav` + `onNavigate` and renders
nav buttons with `aria-current`. Decision: **no router library** — five fixed views in a desktop
webview have no URL/back-button semantics; typed state keeps the bundle small and the flow obvious.
Revisit only if deep-linking or >1 window appears.

## Backend deltas (small)

- `get_app_info` command → `{ version, db_path }` from `CARGO_PKG_VERSION` + the managed
  `AppDataDir`. Read-only, no schema change.
- `tauri-plugin-dialog` registered in `lib.rs`; `dialog:default` permission added to
  `capabilities/default.json`; exact-pinned in `Cargo.toml`/`package.json`.

## Data flow

- Dashboard/Transações fetch via typed wrappers in `lib/api.ts`; screens own their
  loading/error/empty states (DS `EmptyState` variants).
- Transações fetch limit: 500 (explicit constant; FTS/pagination deferred — documented seam).
- Header search submit → `onNavigate("transactions", query)`; TransactionsScreen applies it as the
  initial filter value (controlled input thereafter).

## Risks

1. **Behavior drift while extracting `GoogleSheetsPanel`** — move file verbatim first, restyle
   second; existing OAuth flow has no tests (network), so manual-path review + types must carry it.
2. **Test fragility on copy changes** — tests updated in the same commit as the accent fixes
   (assert accented strings).
3. **Plugin/capability mismatch** (dialog) — runtime permission errors only appear in the real
   shell; verify via `tauri build --no-bundle` + dev run; keep permission minimal (`dialog:open`).
4. **Sidebar footer removal** changes connection visibility — compact status chip (dot + label
   "Sheets") stays in the sidebar, full panel lives in Configurações.

## Testing strategy

- `lib/format.test.ts`: BRL formatting (positive/negative/zero), date formatting.
- `App.navigation.test.tsx`: clicking each nav item renders its screen; `aria-current` moves;
  header search submit lands on Transações with query applied.
- Screen smoke tests: Transações (filter + search narrow the list), Settings (renders import
  blocks; `get_app_info` mocked), Methodology/Copilot render static content.
- Existing `App.test.tsx` assertions updated to accented copy.
- Rust: `get_app_info` unit test (version non-empty; path formatting); `npm run rust:check`.

## Release implications

This is the first slice where the packaged `.exe` becomes navigable; the Windows build
(`tauri build --no-bundle --target x86_64-pc-windows-gnu`) must be re-run after merge so the
embedded frontend picks up the new shell.
