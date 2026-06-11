# Spec: Google OAuth PKCE + Sheets Read

## Summary

Implement the desktop OAuth PKCE flow to connect Neko Finance to Google Sheets. The app must authenticate via system browser, store tokens in the OS keychain, and read spreadsheet data into the local SQLite schema.

## Motivation

The app currently has a full local schema but no data. The user's financial data lives in Google Sheets (file `Finanças.xlsx` exported to Google Sheets). Import requires OAuth consent, token management, and sheet parsing.

## User Stories

### US1 — First-time OAuth consent

**As** the user
**I want** to click "Conectar Google" and be redirected to the system browser for Google OAuth consent
**So that** the app gets read access to my Google Sheets without ever seeing my password.

**Acceptance**: Clicking "Conectar Google" opens the default browser with Google's consent screen. After consent, the browser redirects to a local loopback URL. The app captures the auth code, exchanges it for tokens, and stores them. UI shows "Conectado ✓".

### US2 — Token persistence and refresh

**As** the user
**I want** tokens to be stored securely in the OS keychain and refreshed automatically
**So that** I don't need to re-authenticate every time I open the app.

**Acceptance**: Access token and refresh token are stored in the OS keychain. On app startup, if a token exists, it's validated. If expired, the refresh token is used to obtain a new access token silently. If refresh fails, prompt re-authentication.

### US3 — Read Google Sheets

**As** the user
**I want** to select a spreadsheet and sheet, preview the data, and import it using sheet_mapping rules
**So that** my financial data flows from Google Sheets into the local database.

**Acceptance**: After OAuth, the app lists the user's spreadsheets. User selects one and a specific sheet. The app fetches the sheet data via Google Sheets API v4, displays a preview of rows (first 20), and offers "Import" with the configured sheet_mapping.

### US4 — Cache imported data

**As** the user
**I want** imported data cached locally
**So that** dashboards work offline and I'm not rate-limited by the Google Sheets API.

**Acceptance**: After import, transaction rows are written to SQLite with a `sync_log` entry tracking source spreadsheet, sheet name, and import timestamp. Subsequent imports skip already-imported rows (dedup by spreadsheet range + checksum).

### US5 — Revoke access

**As** the user
**I want** to disconnect Google and wipe stored tokens
**So that** I can revoke the app's access at any time.

**Acceptance**: "Disconnect Google" button deletes tokens from keychain and clears the in-memory token state. Any cached data remains (offline access) but new imports require re-authentication.

## Non-functional requirements

- Zero secrets in git. OAuth client ID is the only public piece and goes in `.env.example` (placeholder), actual value in gitignored `.env`.
- Token stored in OS keychain, never in SQLite, never logged.
- PKCE flow (no client secret in the binary). Uses S256 challenge method.
- Loopback redirect on a random port. No custom URI scheme registration needed.
- All Google API calls run in Rust (Tauri commands), not in the frontend.
- TDD: every OAuth state machine and token lifecycle must have unit tests.
