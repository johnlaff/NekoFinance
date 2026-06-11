# Tasks: App Shell — Real Navigation, Screens, and PT-BR Copy

> UI-heavy slice: TDD where there is logic (formatters, navigation, filters); visual structure may
> land with tests in the same change.

## Phase 1 — Extraction (no behavior change)

- [x] T1.1 Create `src/lib/format.ts` + `format.test.ts` (move `fmtBRL`/`fmtDate`; tests first)
- [x] T1.2 Create `src/lib/api.ts` (DTO interfaces + typed invoke wrappers + `isTauri`)
- [x] T1.3 Extract `src/shell/ThemeToggle.tsx`, `src/shell/AppShell.tsx`
- [x] T1.4 Extract `src/features/sheets/GoogleSheetsPanel.tsx` (verbatim move)
- [x] T1.5 Extract `src/screens/DashboardScreen.tsx`; `App.tsx` becomes composition only
- [x] T1.6 All existing tests still green (`npm run test:run`)

## Phase 2 — Navigation (US1, US3)

- [x] T2.1 Test: clicking each nav item renders its screen and moves `aria-current`
- [x] T2.2 Test: header search submit navigates to Transações with the query applied
- [x] T2.3 Implement `Screen` union + nav state in `App.tsx`; wire `AppShell` `onNavigate`
- [x] T2.4 Accessible search label; remove dead notifications bell

## Phase 3 — Screens (US2, US4, US5, US6)

- [x] T3.1 Tests: TransactionsScreen filter (Todas/Crédito/Futuro) + text search narrow the list
- [x] T3.2 Implement `TransactionsScreen` (fetch limit constant, DS EmptyState states)
- [x] T3.3 Implement `MethodologyScreen` (static, CONTEXT.md vocabulary, original copy)
- [x] T3.4 Implement `CopilotScreen` placeholder (MiaAvatar, roadmap, no fake chat)
- [x] T3.5 Add `get_app_info` command (Rust test) + register
- [x] T3.6 Add `tauri-plugin-dialog` (Cargo + npm guest binding + capability `dialog:open`)
- [x] T3.7 Implement `LocalXlsxImport` (dialog → `import_local_xlsx` → result/error panel)
- [x] T3.8 Implement `SettingsScreen` (Sheets panel moved in, xlsx import, "seus dados" block)
- [x] T3.9 Sidebar: compact Sheets status chip replaces footer panel

## Phase 4 — PT-BR + polish (US7)

- [x] T4.1 Fix diacritics across all UI strings; update test assertions in the same change
- [x] T4.2 `prefers-reduced-motion` for spinner animations (App.css)
- [x] T4.3 Visual pass against design-system ui_kits (dashboard/transactions/settings/copilot)

## Phase 5 — Gates

- [x] T5.1 `npm run typecheck && npm run lint && npm run test:run` green
- [x] T5.2 `npm run build` green; `cargo` gates green (`npm run rust:check`)
- [x] T5.3 Privacy scan green
