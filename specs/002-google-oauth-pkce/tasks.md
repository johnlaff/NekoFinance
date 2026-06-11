# Tasks: Google OAuth PKCE + Sheets Read

## Phase 1 — Rust dependencies

- [ ] T1.1 Add `oauth2`, `reqwest`, `keyring` crates to Cargo.toml
- [ ] T1.2 Add Google OAuth scopes: `openid`, `profile`, `email`, `https://www.googleapis.com/auth/spreadsheets.readonly`
- [ ] T1.3 Write unit test: code_verifier generation (length 43-128, valid chars)

## Phase 2 — PKCE flow (TDD)

- [ ] T2.1 Create `src/oauth/pkce.rs`: generate code_verifier, compute S256 challenge
- [ ] T2.2 Test: challenge matches expected SHA256+BASE64URL output
- [ ] T2.3 Create `src/oauth/mod.rs`: struct `OAuthState` (code_verifier, state param, redirect port)
- [ ] T2.4 Create `start_oauth_flow` command: build auth URL, open browser, start listener
- [ ] T2.5 Test: auth URL contains correct params (client_id, redirect_uri, scope, code_challenge, S256)
- [ ] T2.6 Test: loopback listener captures redirect and extracts code

## Phase 3 — Token management

- [ ] T3.1 Create `src/oauth/token_store.rs`: keyring store/load/delete
- [ ] T3.2 Test: token store round-trip with mock keyring
- [ ] T3.3 Exchange authorization code for tokens (POST to Google token endpoint)
- [ ] T3.4 Test: token exchange request body contains correct fields
- [ ] T3.5 Implement refresh token flow
- [ ] T3.6 Test: expired token triggers refresh, refresh success updates stored token
- [ ] T3.7 Create `check_auth_status` command
- [ ] T3.8 Create `disconnect_google` command

## Phase 4 — Google Sheets client

- [ ] T4.1 Create `src/google_sheets/mod.rs`: authenticated HTTP client
- [ ] T4.2 Implement `get_spreadsheet_values` (range read)
- [ ] T4.3 Implement `list_spreadsheets` (Drive API)
- [ ] T4.4 Test: mock server returns sheet data, client parses correctly

## Phase 5 — Import pipeline

- [ ] T5.1 Create `import_sheet_data` command: read sheet, map via sheet_mapping, insert into transaction table
- [ ] T5.2 Implement date_direction logic: past rows → transaction, future rows → projection
- [ ] T5.3 Create `sync_log` entry on import
- [ ] T5.4 Dedup: skip rows already imported (checksum on sheet range)
- [ ] T5.5 Create `fetch_sheet_preview` command: first 20 rows only

## Phase 6 — Frontend

- [ ] T6.1 Add "Conectar Google" button + auth status indicator to App.tsx
- [ ] T6.2 Wire Tauri commands via `invoke()`
- [ ] T6.3 Sheet picker: list spreadsheets, select sheet, show preview table
- [ ] T6.4 Import button with progress indicator
- [ ] T6.5 Disconnect button with confirmation

## Phase 7 — Environment & docs

- [ ] T7.1 Update `.env.example` with `GOOGLE_CLIENT_ID=`
- [ ] T7.2 Document Google Cloud Console setup steps in `docs/oauth-setup.md`
- [ ] T7.3 Run `npm run check` — full gate green
