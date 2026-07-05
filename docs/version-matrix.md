# Version Matrix

Checked on 2026-06-08 from npm and crates.io before scaffolding.

## Installed In This Scaffold

| Area               | Package                            | Version              | Notes                                                                                                                                                                                                                                           |
| ------------------ | ---------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tauri CLI          | `@tauri-apps/cli`                  | `2.11.2`             | Latest npm.                                                                                                                                                                                                                                     |
| React              | `react`, `react-dom`               | `19.2.7`             | Latest npm.                                                                                                                                                                                                                                     |
| React types        | `@types/react`, `@types/react-dom` | `19.2.17`, `19.2.3`  | Latest npm.                                                                                                                                                                                                                                     |
| Vite               | `vite`                             | `8.0.16`             | Requires Node `^20.19.0 &#124;&#124; >=22.12.0`; local Node is `24.16.0`.                                                                                                                                                                       |
| Vite React plugin  | `@vitejs/plugin-react`             | `6.0.2`              | Latest npm.                                                                                                                                                                                                                                     |
| TypeScript         | `typescript`                       | `6.0.3`              | Compatible with checked `typescript-eslint` range `<6.1.0`.                                                                                                                                                                                     |
| Tauri crate        | `tauri`                            | `2.11.2`             | Latest crates.io.                                                                                                                                                                                                                               |
| Tauri build crate  | `tauri-build`                      | `2.6.2`              | Latest crates.io.                                                                                                                                                                                                                               |
| Tauri opener crate | `tauri-plugin-opener`              | `2.5.4`              | Latest crates.io.                                                                                                                                                                                                                               |
| Tauri dialog crate | `tauri-plugin-dialog`              | `2.7.1`              | Latest crates.io; native open/save dialogs (backup, local file import).                                                                                                                                                                         |
| Tauri notify crate | `tauri-plugin-notification`        | `2.3.3`              | Latest crates.io 2.x (matches `tauri = 2.11.2`). Background read-side sync fires a native OS notification when the stored Google token is revoked / can't refresh (plan 026). Permission `notification:default` in `capabilities/default.json`. |
| Serde              | `serde`, `serde_json`              | `1.0.228`, `1.0.150` | Latest crates.io.                                                                                                                                                                                                                               |
| OS keychain        | `keyring`                          | `3.6.3`              | `keyring 4.x` dropped the `sync-secret-service` feature flag needed on Linux; `3.x` (locked at `3.6.3`) is used instead. Features: `apple-native`, `windows-native`, `sync-secret-service`.                                                     |

## Quality Tooling

| Area                   | Package/action                | Version  | Notes                                                                                                   |
| ---------------------- | ----------------------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| ESLint                 | `eslint`                      | `10.4.1` | Latest npm, Node 24-compatible.                                                                         |
| ESLint JS config       | `@eslint/js`                  | `10.0.1` | Explicit dependency required by ESLint flat config.                                                     |
| TypeScript ESLint      | `typescript-eslint`           | `8.61.0` | Supports ESLint 10 and TypeScript `<6.1.0`; compatible with `typescript@6.0.3`.                         |
| React Hooks lint       | `eslint-plugin-react-hooks`   | `7.1.1`  | Latest npm.                                                                                             |
| Prettier               | `prettier`                    | `3.8.4`  | Latest npm.                                                                                             |
| Test runner            | `vitest`                      | `4.1.8`  | Compatible with Vite 8 and Node 24.                                                                     |
| Coverage               | `@vitest/coverage-v8`         | `4.1.8`  | Matches Vitest version.                                                                                 |
| DOM test env           | `jsdom`                       | `29.1.1` | Latest npm, Node 24-compatible.                                                                         |
| React testing          | `@testing-library/react`      | `16.3.2` | Supports React 19.                                                                                      |
| Jest DOM matchers      | `@testing-library/jest-dom`   | `6.9.1`  | Latest npm.                                                                                             |
| User events            | `@testing-library/user-event` | `14.6.1` | Latest npm.                                                                                             |
| Node types             | `@types/node`                 | `25.9.3` | Latest npm.                                                                                             |
| Playwright             | `@playwright/test`            | `1.60.0` | Latest npm; Chromium-only smoke for MVP.                                                                |
| React Doctor           | `npx react-doctor@latest`     | latest   | Advisory; sempre a última via npx (sem devDependency fixa). CI: Action `millionco/react-doctor@v2.1.0`. |
| Checkout action        | `actions/checkout`            | `v6.0.3` | Latest GitHub release checked.                                                                          |
| Setup Node action      | `actions/setup-node`          | `v6.4.0` | Latest GitHub release checked.                                                                          |
| Cache action           | `actions/cache`               | `v5.0.5` | Latest GitHub release checked.                                                                          |
| Upload artifact action | `actions/upload-artifact`     | `v7.0.1` | Latest checked; not used yet.                                                                           |
| Gitleaks action        | `gitleaks/gitleaks-action`    | `v3.0.0` | Node 24 runtime.                                                                                        |
| Tauri action           | `tauri-apps/tauri-action`     | `v0.6.2` | Latest tag checked.                                                                                     |
| React Doctor action    | `millionco/react-doctor`      | `v2.1.0` | Latest tag checked; advisory workflow.                                                                  |

## Installed In `src-tauri/Cargo.toml` (Rust runtime, beyond the Tauri crates above)

OAuth and Google Sheets access are Rust-native, not a Node dependency: there is no `googleapis` or
`google-auth-library` in `package.json`, and no AI SDK (`ai`, `@ai-sdk/*`, `@openai/agents`) either
— the copilot backend has not been built yet (see `docs/architecture.md`).

| Area                    | Crate         | Version pin   | Notes                                                                                               |
| ----------------------- | ------------- | ------------- | --------------------------------------------------------------------------------------------------- |
| Async runtime           | `tokio`       | `1`, full     | Backs the Tauri commands and the OAuth loopback listener.                                           |
| OAuth desktop flow      | `oauth2`      | `5`           | PKCE desktop flow (spec 002) — replaces any Node `google-auth-library` path.                        |
| HTTP client             | `reqwest`     | `0.13`        | Google Sheets API calls (read + write-back) and general HTTP — replaces any Node `googleapis` path. |
| SQL toolkit             | `sqlx`        | `0.9`         | SQLite runtime, migrations. Requires Rust `1.94`; local Rust is `1.96`.                             |
| Local browser launch    | `open`        | `5`           | Opens the OAuth consent URL in the system browser.                                                  |
| xlsx import             | `calamine`    | `=0.35.0`     | Local `.xlsx` import path (no Google account required).                                             |
| xlsx zip container      | `zip`         | `=7.2.0`      | Plan 068: reads `xl/comments*.xml` (cell notes) from the `.xlsx` zip directly — calamine 0.35 exposes values but no comments/annotations API. Pinned to the version already resolved transitively via `calamine` so only one `zip` major version is in the tree. |
| xlsx comment XML parser | `quick-xml`   | `=0.41.0`     | Plan 068: parses `xl/workbook.xml`, the `.rels` chain, and `xl/comments*.xml` to recover cell notes. Pinned to the fixed line (>=0.41) — the transitive `quick-xml 0.39.4` pulled in by `calamine`/`plist` is under RUSTSEC-2026-0194/0195 (ignored for that path in `.cargo/audit.toml` since it only parses Tauri's own build-time plist, not third-party XML at runtime); this path DOES parse untrusted `.xlsx` XML at runtime, so it stays on the patched line even though it duplicates a second `quick-xml` version in the dep tree. |
| OS keychain             | `keyring`     | `3`           | Token storage; features `apple-native`, `windows-native`, `sync-secret-service`.                    |
| At-rest encryption      | `aes-gcm`     | `0.10`        | Encrypts cached OAuth tokens.                                                                       |
| Hashing                 | `sha2`, `hex` | `0.11`, `0.4` | Checksums for sync log / reconciliation.                                                            |
| Standalone reminder CLI | `notify-rust` | `=4.18.0`     | Cross-platform OS notification for the `--remind` CLI path when the app is closed (plan 039).       |

## Installed In `package.json` (frontend, beyond React/Vite/tooling above)

| Area            | Package                     | Version  | Notes                                                 |
| --------------- | --------------------------- | -------- | ----------------------------------------------------- |
| Tauri JS API    | `@tauri-apps/api`           | `2.11.0` | Frontend invokes Tauri commands via `src/lib/api.ts`. |
| Tauri dialog JS | `@tauri-apps/plugin-dialog` | `2.7.1`  | Native open/save dialogs (backup, local file import). |
| Icons           | `lucide-react`              | `1.17.0` | Icon set used across screens and the design system.   |

## Not Adopted / Superseded (kept for historical context)

These were candidates considered early on; none are installed, and the paths they would have
enabled were built differently. Check `package.json`/`Cargo.toml` for the authoritative installed
set before reviving any of these.

| Area              | Package                                       | Why not adopted                                                                                  |
| ----------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Tauri opener JS   | `@tauri-apps/plugin-opener`                   | Rust plugin (`tauri-plugin-opener`) is installed; no frontend opener bindings needed so far.     |
| SQLite plugin     | `@tauri-apps/plugin-sql` / `tauri-plugin-sql` | Storage uses `sqlx` directly from Rust commands instead of the JS SQL plugin.                    |
| Rust SQLite       | `rusqlite`                                    | `sqlx` was chosen instead (async, migrations built in).                                          |
| Google APIs       | `googleapis`                                  | OAuth + Sheets read/write are Rust-native (`oauth2` + `reqwest`), not a Node dependency.         |
| Google auth       | `google-auth-library`                         | Same — superseded by the `oauth2` Rust crate.                                                    |
| AI SDK            | `ai`, `@ai-sdk/deepseek`, `@ai-sdk/google`    | The copilot (Mia) backend/tool-calling has not been built yet; `CopilotScreen` is UI-only today. |
| OpenAI Agents     | `@openai/agents`                              | Same — no agent orchestration exists yet.                                                        |
| Schema validation | `zod`                                         | No AI SDK / runtime-validated LLM output yet to justify it.                                      |
| LanceDB Node      | `@lancedb/lancedb`                            | No vector/local retrieval layer is built; see `docs/architecture.md` (Local retrieval row).      |
| Apache Arrow      | `apache-arrow`                                | Only needed alongside LanceDB, which is not installed.                                           |
| Query cache       | `@tanstack/react-query`                       | Data fetching uses the project's own `useCommand` hook (`src/lib/useCommand.ts`) instead.        |
| Charts            | `recharts`                                    | No charting library is in use yet; screens render custom SVG/CSS visualizations.                 |

## Rule

Before adding new dependencies, check the registry again and choose the newest version that satisfies peer dependencies, engine requirements, and Tauri compatibility. If latest is incompatible, document the reason here.

## Security-audit exceptions (cargo-audit)

Tracked in `.cargo/audit.toml` (read by CI's `cargo audit` step). Current entries:

| Advisory                              | Crate             | Why ignored                                                                                                                                                                           | Remove when                                         |
| ------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| RUSTSEC-2026-0194 / RUSTSEC-2026-0195 | `quick-xml` <0.41 | Only remaining path is `plist 1.9.0` ← `tauri-utils 2.9.2` (pins 0.39; no newer release). Surface is Tauri's own plist parsing of first-party config, not third-party XML at runtime. | `cargo tree -i quick-xml` shows ≥0.41 (Tauri bump). |
