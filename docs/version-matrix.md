# Version Matrix

Checked on 2026-06-08 from npm and crates.io before scaffolding.

## Installed In This Scaffold

| Area               | Package                            | Version              | Notes                                                                     |
| ------------------ | ---------------------------------- | -------------------- | ------------------------------------------------------------------------- |
| Tauri CLI          | `@tauri-apps/cli`                  | `2.11.2`             | Latest npm.                                                               |
| React              | `react`, `react-dom`               | `19.2.7`             | Latest npm.                                                               |
| React types        | `@types/react`, `@types/react-dom` | `19.2.17`, `19.2.3`  | Latest npm.                                                               |
| Vite               | `vite`                             | `8.0.16`             | Requires Node `^20.19.0 &#124;&#124; >=22.12.0`; local Node is `24.16.0`. |
| Vite React plugin  | `@vitejs/plugin-react`             | `6.0.2`              | Latest npm.                                                               |
| TypeScript         | `typescript`                       | `6.0.3`              | Compatible with checked `typescript-eslint` range `<6.1.0`.               |
| Tauri crate        | `tauri`                            | `2.11.2`             | Latest crates.io.                                                         |
| Tauri build crate  | `tauri-build`                      | `2.6.2`              | Latest crates.io.                                                         |
| Tauri opener crate | `tauri-plugin-opener`              | `2.5.4`              | Latest crates.io.                                                         |
| Serde              | `serde`, `serde_json`              | `1.0.228`, `1.0.150` | Latest crates.io.                                                         |

## Quality Tooling

| Area                   | Package/action                | Version  | Notes                                                                           |
| ---------------------- | ----------------------------- | -------- | ------------------------------------------------------------------------------- |
| ESLint                 | `eslint`                      | `10.4.1` | Latest npm, Node 24-compatible.                                                 |
| ESLint JS config       | `@eslint/js`                  | `10.0.1` | Explicit dependency required by ESLint flat config.                             |
| TypeScript ESLint      | `typescript-eslint`           | `8.61.0` | Supports ESLint 10 and TypeScript `<6.1.0`; compatible with `typescript@6.0.3`. |
| React Hooks lint       | `eslint-plugin-react-hooks`   | `7.1.1`  | Latest npm.                                                                     |
| Prettier               | `prettier`                    | `3.8.3`  | Latest npm.                                                                     |
| Test runner            | `vitest`                      | `4.1.8`  | Compatible with Vite 8 and Node 24.                                             |
| Coverage               | `@vitest/coverage-v8`         | `4.1.8`  | Matches Vitest version.                                                         |
| DOM test env           | `jsdom`                       | `29.1.1` | Latest npm, Node 24-compatible.                                                 |
| React testing          | `@testing-library/react`      | `16.3.2` | Supports React 19.                                                              |
| Jest DOM matchers      | `@testing-library/jest-dom`   | `6.9.1`  | Latest npm.                                                                     |
| User events            | `@testing-library/user-event` | `14.6.1` | Latest npm.                                                                     |
| Node types             | `@types/node`                 | `25.9.2` | Latest npm.                                                                     |
| Playwright             | `@playwright/test`            | `1.60.0` | Latest npm; Chromium-only smoke for MVP.                                        |
| React Doctor           | `react-doctor`                | `0.5.1`  | Latest npm; advisory with telemetry disabled locally.                           |
| Checkout action        | `actions/checkout`            | `v6.0.3` | Latest GitHub release checked.                                                  |
| Setup Node action      | `actions/setup-node`          | `v6.4.0` | Latest GitHub release checked.                                                  |
| Cache action           | `actions/cache`               | `v5.0.5` | Latest GitHub release checked.                                                  |
| Upload artifact action | `actions/upload-artifact`     | `v7.0.1` | Latest checked; not used yet.                                                   |
| Gitleaks action        | `gitleaks/gitleaks-action`    | `v3.0.0` | Node 24 runtime.                                                                |
| Tauri action           | `tauri-apps/tauri-action`     | `v0.6.2` | Latest tag checked.                                                             |
| React Doctor action    | `millionco/react-doctor`      | `v2.1.0` | Latest tag checked; advisory workflow.                                          |

## Planned Dependencies

These are candidates for future slices. Note: several have since been installed as their slices landed (e.g. `@tauri-apps/api`, `sqlx`, `google-auth-library`/`googleapis` for OAuth+import, `ai` for the copilot groundwork) — check `package.json`/`Cargo.toml` for the authoritative installed set.

| Area              | Package                                       | Latest checked | Compatibility note                                                                               |
| ----------------- | --------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------ |
| Tauri JS API      | `@tauri-apps/api`                             | `2.11.0`       | Add when the frontend starts invoking Tauri commands.                                            |
| Tauri opener JS   | `@tauri-apps/plugin-opener`                   | `2.5.4`        | Add only if the frontend needs opener bindings. Rust plugin remains installed.                   |
| SQLite plugin     | `@tauri-apps/plugin-sql` / `tauri-plugin-sql` | `2.4.0`        | Use with SQLite feature when storage slice starts.                                               |
| Rust SQLite       | `rusqlite`                                    | `0.40.1`       | Alternative if direct Rust storage is preferred.                                                 |
| SQL toolkit       | `sqlx`                                        | `0.9.0`        | Requires Rust `1.94`; local Rust is `1.96`.                                                      |
| OS keychain       | `keyring`                                     | `4.0.1`        | For OAuth refresh tokens/API keys if needed.                                                     |
| Google APIs       | `googleapis`                                  | `173.0.0`      | Node `>=18`; local Node is `24.16`.                                                              |
| Google auth       | `google-auth-library`                         | `10.7.0`       | Node `>=18`.                                                                                     |
| AI SDK            | `ai`                                          | `6.0.198`      | Peer `zod ^3.25.76 &#124;&#124; ^4.1.8`.                                                         |
| DeepSeek provider | `@ai-sdk/deepseek`                            | `2.0.35`       | Pair with AI SDK 6.                                                                              |
| Google provider   | `@ai-sdk/google`                              | `3.0.80`       | Pair with AI SDK 6.                                                                              |
| OpenAI Agents     | `@openai/agents`                              | `0.11.6`       | Candidate if Vercel AI SDK is not enough for agent orchestration.                                |
| Schema validation | `zod`                                         | `4.4.3`        | Satisfies AI SDK peer range.                                                                     |
| LanceDB Node      | `@lancedb/lancedb`                            | `0.30.0`       | Correct package name for Node.                                                                   |
| Apache Arrow      | `apache-arrow`                                | `18.1.0`       | Latest compatible with `@lancedb/lancedb`; global latest `21.1.0` is outside LanceDB peer range. |
| Query cache       | `@tanstack/react-query`                       | `5.101.0`      | Supports React 18/19.                                                                            |
| Charts            | `recharts`                                    | `3.8.1`        | Supports React 19; requires `react-is` if used.                                                  |

## Rule

Before adding new dependencies, check the registry again and choose the newest version that satisfies peer dependencies, engine requirements, and Tauri compatibility. If latest is incompatible, document the reason here.
