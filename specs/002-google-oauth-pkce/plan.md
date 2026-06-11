# Plan: Google OAuth PKCE + Sheets Read

## Architecture

```
┌──────────────┐    ┌──────────────┐    ┌────────────────┐
│  React UI    │───▶│ Tauri cmd    │───▶│ Google OAuth   │
│ "Conectar"   │    │ start_oauth  │    │ PKCE flow      │
└──────────────┘    └──────┬───────┘    └───────┬────────┘
                           │                    │
                           ▼                    ▼
                    ┌──────────────┐    ┌────────────────┐
                    │ OS Keychain  │    │ Google APIs    │
                    │ (keyring)    │    │ Sheets v4      │
                    └──────────────┘    └───────┬────────┘
                                                │
                                                ▼
                                         ┌──────────────┐
                                         │ SQLite       │
                                         │ (cached)     │
                                         └──────────────┘
```

## Components

### 1. OAuth PKCE Module (Rust)

- `src/oauth/mod.rs` — PKCE flow state machine
- `src/oauth/pkce.rs` — code_verifier, code_challenge (S256)
- `src/oauth/token_store.rs` — keyring read/write/delete

**Flow:**

1. Generate `code_verifier` (cryptographically random 43-128 chars)
2. Compute `code_challenge = base64url(sha256(code_verifier))`
3. Open browser: `https://accounts.google.com/o/oauth2/v2/auth?...&code_challenge=...&code_challenge_method=S256`
4. Start local HTTP listener on random port (0)
5. Google redirects to `http://127.0.0.1:{port}?code=...&state=...`
6. Exchange code for tokens: POST `https://oauth2.googleapis.com/token`
7. Store `refresh_token` in keychain
8. Return success to frontend

### 2. Token Store (Rust)

Uses `keyring` crate:

- **Service**: `neko-finance`
- **Username**: `google-oauth`
- **Stored value**: JSON `{ "access_token", "refresh_token", "expires_at", "scope" }`

### 3. Google Sheets Client (Rust)

- `src/google_sheets/mod.rs` — sheets API wrapper
- Uses `reqwest` with bearer token
- Endpoint: `GET https://sheets.googleapis.com/v4/spreadsheets/{id}/values/{range}`

### 4. Tauri Commands

| Command               | Direction | Purpose                                                         |
| --------------------- | --------- | --------------------------------------------------------------- |
| `start_oauth_flow`    | UI → Rust | Begin PKCE, returns `client_id` needed URL (or starts directly) |
| `check_auth_status`   | UI → Rust | Returns `connected`/`expired`/`disconnected`                    |
| `list_spreadsheets`   | UI → Rust | Drive API to list user's sheets                                 |
| `fetch_sheet_preview` | UI → Rust | Read first 20 rows of a sheet                                   |
| `import_sheet_data`   | UI → Rust | Full import using sheet_mapping rules                           |
| `disconnect_google`   | UI → Rust | Delete tokens from keychain                                     |

## Dependencies

### Rust crates

- `oauth2` v5 — PKCE flow primitives (code_verifier, challenge, auth URL, token exchange)
- `reqwest` v0.12 — HTTP client with TLS
- `keyring` v3 — cross-platform OS keychain access
- `tokio` — async runtime (already present)
- `serde` / `serde_json` — JSON (already present)
- `url` — URL construction (already present for some crates)

### Frontend

- No new npm deps. Tauri `invoke` handles commands.

## Data Boundaries

| Data                        | Location                   | Git                  |
| --------------------------- | -------------------------- | -------------------- |
| OAuth client ID             | `.env` (gitignored)        | Forbidden            |
| Access/refresh tokens       | OS keychain                | Forbidden            |
| Google Sheets data (cached) | SQLite `transaction` table | Forbidden            |
| Sheet preview in UI         | Memory only                | Allowed if synthetic |

## Risks

1. **Loopback redirect on Windows**: May be blocked by firewall. Use a random high port (49152-65535). Fallback: custom URI scheme `neko-finance://`.
2. **keyring on WSL**: WSL may not have a keychain daemon. Test with `gnome-keyring` or fall back to encrypted file.
3. **Google API quotas**: Sheets API has 300 req/min per user, 60 req/min per user per project. Cache aggressively.
4. **Token expiry**: Access tokens last 1 hour. Refresh tokens may be revoked if unused for 6 months.

## Testing Strategy

- **Unit tests**: PKCE code_verifier generation, challenge computation, token JSON serialization, keyring mock.
- **Integration tests**: OAuth flow with a mock HTTP server (no real Google). Token refresh cycle.
- **E2E**: Manual only (requires real Google account). Not part of CI.
- **TDD required**: yes for OAuth state machine and token lifecycle.

## Release Implications

- New Rust crates added to `Cargo.toml`.
- `.env.example` updated with `GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com`.
- Google Cloud Console setup required: project, OAuth consent screen, desktop client credentials.
