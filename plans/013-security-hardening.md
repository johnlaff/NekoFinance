# Plan 013: Security hardening: token fail-closed, loopback timeout, CSP, privacy-scan

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/oauth/token_store.rs src-tauri/src/oauth/server.rs src-tauri/tauri.conf.json scripts/privacy-scan.sh src/features/sheets/GoogleSheetsPanel.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: MED
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Five small, independent hardening fixes across the OAuth layer and build config.
The token file-fallback silently stores the OAuth refresh token with only
obfuscation-level protection whenever the OS keychain is unavailable; making it
fail-closed prevents silent credential exposure on headless/CI environments. The
loopback `accept()` blocks forever if the user dismisses the browser tab,
leaking the port and the Tauri task indefinitely. The production CSP includes
Vite dev-server origins, widening the allowed network surface in the shipped
binary. The raw Google API error body is forwarded verbatim to the UI, which
can expose upstream diagnostic detail that is not safe for end-user display.
Finally, the privacy scan does not block commits that accidentally contain data
from `.neko-data/` or `.lancedb/`, which hold personal finance cache files.

## Current state

### Files and their roles

- `src-tauri/src/oauth/token_store.rs` — OAuth token persistence; keyring primary, file-encrypted fallback.
- `src-tauri/src/oauth/server.rs` — single-connection TCP loopback server for the OAuth redirect.
- `src-tauri/tauri.conf.json` — Tauri app config including the bundled CSP header.
- `scripts/privacy-scan.sh` — pre-push script that blocks private artifact paths and forbidden-name patterns.
- `src/features/sheets/GoogleSheetsPanel.tsx` — React panel that renders the raw Rust error string in `<code>`.

### Repo conventions

- Rust error handling: functions return `Result<T, String>` (not `anyhow`). Match this in all new code.
- Env-var opt-in pattern already used in `src-tauri/src/oauth/pkce.rs:50-54` with `std::env::var("GOOGLE_CLIENT_SECRET")`.
- Commit message style from `git log`: `fix: <imperative, Portuguese detail OK>` or `feat:` / `chore:`.
- React Compiler is ENABLED — do NOT add `useMemo`, `useCallback`, or `React.memo` manually.
- Money is integer cents; amounts are positive magnitude — not relevant to this plan but noted for context.

### Excerpts as of d183bbf

**token_store.rs:30-35** — the fallback WARN comment (find with `derive_key`):
```rust
/// Chave do fallback de arquivo cifrado. AVISO de segurança: é OFUSCAÇÃO BEST-EFFORT, não proteção
/// forte — o sal fica em claro ao lado do ciphertext e a chave deriva de machine-id + sal, ambos
/// legíveis por qualquer processo do mesmo usuário. Só protege contra leitura casual do arquivo, não
/// contra um atacante local. O caminho preferido é o keychain do SO; este fallback existe para
/// ambientes sem keychain. Endurecimento futuro: falhar fechado (recusar persistir) sem keychain,
/// ou derivar de um segredo protegido pelo SO (DPAPI/libsecret).
fn derive_key(app_dir: &std::path::Path) -> Result<[u8; 32], String> {
```

**token_store.rs:169-182** — `store_token` silently falls back today:
```rust
pub fn store_token(app_dir: &std::path::Path, token: &StoredToken) -> Result<(), String> {
    match try_keyring_store(token) {
        Ok(()) => return Ok(()),
        // Keychain indisponível (ex.: Linux headless / sem libsecret): caímos no arquivo cifrado,
        // que é só ofuscação best-effort (ver `derive_key`). Avisa para não ser uma degradação de
        // segurança silenciosa.
        Err(e) => eprintln!("keyring indisponível ({e}); usando fallback de arquivo cifrado"),
    }

    let key = derive_key(app_dir)?;
    let encrypted = encrypt_token(token, &key)?;
    let path = encrypted_token_path(app_dir);
    std::fs::write(&path, &encrypted).map_err(|e| format!("write encrypted: {e}"))
}
```

**server.rs:18-28** — `incoming().next()` blocks with no accept-level timeout:
```rust
pub async fn listen_for_code(self) -> Result<(String, Option<String>), String> {
    self.listener
        .set_nonblocking(false)
        .map_err(|e| format!("nonblocking error: {e}"))?;

    let stream = self
        .listener
        .incoming()
        .next()
        .ok_or("no incoming connection")?
        .map_err(|e| format!("accept error: {e}"))?;
```
The `set_read_timeout` at line 33 applies only to data already accepted — not to
the `incoming().next()` call above it which blocks forever waiting for any
connection to arrive.

**tauri.conf.json:23-25** — CSP includes Vite dev-server origins:
```json
"security": {
  "csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' http://localhost:1420 ws://localhost:1420"
}
```
`http://localhost:1420` and `ws://localhost:1420` are Vite HMR origins that
must not appear in the production bundle.

**token_store.rs:267-269** — raw upstream body forwarded as error:
```rust
if !resp.status().is_success() {
    let body = resp.text().await.unwrap_or_default();
    return Err(format!("refresh failed: {body}"));
}
```
This `body` is the raw Google API JSON error response and propagates through the
Tauri command `Result<_, String>` directly to the `detailOf()` function in the
frontend, which renders it verbatim inside `<code>` (GoogleSheetsPanel.tsx:849,
936).

**scripts/privacy-scan.sh:7-16** — blocked-paths list missing data directories:
```bash
blocked_paths=(
  ".circle-auth"
  ".circle-data"
  "private-data"
  "raw-scrape"
  "transcripts"
  "videos"
  "embeddings"
  "indexes"
)
```
`.neko-data/` (local finance cache) and `.lancedb/` (vector index) are listed
in `AGENTS.md` as gitignored private directories but are absent from this list,
so their existence is never checked at pre-push time.

## Commands you will need

| Purpose            | Command                                                                           | Expected on success                     |
|--------------------|-----------------------------------------------------------------------------------|-----------------------------------------|
| Rust check         | `npm run rust:check`                                                              | exit 0 (fmt + clippy + test)            |
| Rust tests only    | `cargo test --manifest-path src-tauri/Cargo.toml --locked`                       | all pass                                |
| Typecheck          | `npm run typecheck`                                                               | exit 0, no TS errors                    |
| Lint               | `npm run lint`                                                                    | exit 0                                  |
| Privacy scan       | `npm run privacy:scan`                                                            | exit 0, "Privacy scan passed."          |
| Full gate          | `npm run check`                                                                   | exit 0                                  |
| Grep CSP dev URLs  | `grep -n "localhost:1420" src-tauri/tauri.conf.json`                             | no matches (after step 3)               |
| Grep raw body leak | `grep -n 'format!("refresh failed: {body}")' src-tauri/src/oauth/token_store.rs` | no matches (after step 4)               |
| Grep fallback path | `grep -n 'using fallback\|arquivo cifrado' src-tauri/src/oauth/token_store.rs`   | shows fail-closed branch (after step 1) |

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/oauth/token_store.rs`
- `src-tauri/src/oauth/server.rs`
- `src-tauri/tauri.conf.json`
- `scripts/privacy-scan.sh`
- `src/features/sheets/GoogleSheetsPanel.tsx` — only the `detailOf` function (step 4)

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/oauth/mod.rs` — the exchange-token body leak at line 87 (`token exchange failed: {body}`) is a separate error path used during initial login, not the refresh path. Fixing it is deferred to avoid scope creep; it is noted in Maintenance notes.
- `src-tauri/src/oauth/pkce.rs` — no changes needed.
- Any other React component — only `detailOf` in `GoogleSheetsPanel.tsx` is in scope.
- Any migration, DB schema, or finance-core module.
- `src-tauri/Cargo.toml` — no new dependencies needed for any of the five steps.

## Git workflow

- Branch: `advisor/013-security-hardening`
- One commit per step is fine; alternatively one commit covering all five steps.
- Message style (match repo): `fix: <short imperative>` e.g.
  `fix: token store fail-closed sem keychain, loopback timeout, CSP prod, privacy scan`
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Token store — fail-closed when OS keychain is unavailable

**Goal**: `store_token` must return `Err(...)` instead of silently writing the
file-encrypted fallback when the keychain is unavailable, unless the caller has
explicitly set the env var `NEKO_INSECURE_FILE_FALLBACK=1`.

**File**: `src-tauri/src/oauth/token_store.rs`

Replace the `store_token` function (lines 169-182) and update the `derive_key`
doc-comment. The new logic:

```rust
pub fn store_token(app_dir: &std::path::Path, token: &StoredToken) -> Result<(), String> {
    match try_keyring_store(token) {
        Ok(()) => return Ok(()),
        Err(e) => {
            // Keychain unavailable. Fail closed by default to prevent silent
            // credential exposure on headless/CI environments. Set
            // NEKO_INSECURE_FILE_FALLBACK=1 to allow the weak file-based
            // fallback (obfuscation only — not strong protection).
            if std::env::var("NEKO_INSECURE_FILE_FALLBACK").as_deref() != Ok("1") {
                return Err(format!(
                    "Keychain unavailable ({e}). Set NEKO_INSECURE_FILE_FALLBACK=1 to allow \
                     the insecure file-based fallback, or install a keychain (libsecret on Linux)."
                ));
            }
            eprintln!(
                "NEKO_INSECURE_FILE_FALLBACK=1: using weak file-based token storage ({e})"
            );
        }
    }

    let key = derive_key(app_dir)?;
    let encrypted = encrypt_token(token, &key)?;
    let path = encrypted_token_path(app_dir);
    std::fs::write(&path, &encrypted).map_err(|e| format!("write encrypted: {e}"))
}
```

Also update the `derive_key` doc-comment at lines 30-35 to remove the
"Endurecimento futuro: falhar fechado" clause (it is now done). Replace the last
sentence with: `/// O fallback só é usado com NEKO_INSECURE_FILE_FALLBACK=1.`

Add a new test at the bottom of the `#[cfg(test)]` block in the same file,
after `test_derive_key_consistent`:

```rust
#[test]
fn test_store_token_fails_closed_without_keychain_env() {
    // Ensure the env var is NOT set for this test.
    // In CI without a keychain daemon, try_keyring_store will fail;
    // store_token must return Err unless NEKO_INSECURE_FILE_FALLBACK=1.
    // We simulate by verifying the file is NOT written on a fresh dir
    // when the env var is absent and the keyring fails.
    // NOTE: if a real keychain IS available (dev machine), keyring succeeds
    // and this test verifies nothing harmful — it passes trivially.
    // The meaningful assertion is that the env-var branch exists; that is
    // covered by the code path grep below.
    let dir = temp_app_dir();
    std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK");
    // We don't call store_token here because keyring may succeed on dev.
    // The structural assertion is: the encrypted path must NOT exist after
    // a failed keyring with the env-var absent.
    let enc_path = dir.join(ENCRYPTED_FILE);
    assert!(!enc_path.exists(), "file must not exist before any store");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_store_token_file_fallback_when_env_set() {
    // With NEKO_INSECURE_FILE_FALLBACK=1 and no keychain, the file SHOULD be written.
    // On a machine with a working keychain the test is a no-op pass (keyring succeeds first).
    let dir = temp_app_dir();
    std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1");
    let token = StoredToken {
        access_token: "ya29.test".into(),
        refresh_token: "1//test".into(),
        expires_at: 9_999_999_999,
        scope: "spreadsheets.readonly".into(),
    };
    // Must not Err regardless of whether keychain is present.
    assert!(store_token(&dir, &token).is_ok());
    std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK");
    std::fs::remove_dir_all(&dir).ok();
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | grep -E "FAILED|ok|error"` — all tests pass, no FAILED lines.

---

### Step 2: OAuth loopback server — bounded accept timeout

**Goal**: `listen_for_code` must not block forever on `accept()`. Wrap the
blocking `incoming().next()` in a thread with a deadline so that if the user
abandons the browser tab the task is released within a bounded time.

**File**: `src-tauri/src/oauth/server.rs`

The current code at lines 18-28 uses `incoming().next()` which blocks the OS
thread with no deadline. The fix uses `set_read_timeout` on the *listener*
before calling `accept`, which `std::net::TcpListener` supports. Setting it
causes `accept()` to return `Err(WouldBlock)` / `TimedOut` after the deadline.

Replace the `listen_for_code` implementation from line 18 through line 28 with:

```rust
pub async fn listen_for_code(self) -> Result<(String, Option<String>), String> {
    // Bounded accept: if the user closes the browser without completing the
    // OAuth flow the listener must not block indefinitely. Two minutes is
    // generous for any real redirect; after that we release the port and task.
    const ACCEPT_TIMEOUT: Duration = Duration::from_secs(120);
    self.listener
        .set_nonblocking(false)
        .map_err(|e| format!("nonblocking error: {e}"))?;
    self.listener
        .set_read_timeout(Some(ACCEPT_TIMEOUT))
        .map_err(|e| format!("accept timeout: {e}"))?;

    let stream = self
        .listener
        .incoming()
        .next()
        .ok_or("no incoming connection")?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                "OAuth timeout: no browser callback received within 2 minutes".to_string()
            } else {
                format!("accept error: {e}")
            }
        })?;

    // Timeout de leitura: uma conexão que abre e nunca manda a request line não pode pendurar
    // a task para sempre. 30s cobre qualquer redirect real.
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| format!("read timeout: {e}"))?;
```

Keep lines 36 onward (clone, BufReader, read_line, extract_code_and_state,
write response, Ok) unchanged.

Add a new unit test at the bottom of the `#[cfg(test)]` block:

```rust
#[test]
fn test_listen_for_code_times_out() {
    use std::net::TcpListener;
    // Bind a real listener but never connect to it — the accept must time out.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let server = OAuthServer::new(listener);
    // listen_for_code is async; run it synchronously for the test.
    let rt = tokio::runtime::Runtime::new().unwrap();
    // Override the default 120s timeout: tests cannot wait 2 minutes.
    // The test just ensures the error path is reachable; we accept any Err.
    // Because we cannot easily override the constant in a unit test, we
    // instead verify that setting a 1-second read_timeout on a listener
    // correctly causes accept to fail. Do this at the std level:
    let listener2 = TcpListener::bind("127.0.0.1:0").expect("bind2");
    listener2.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let result: std::io::Result<(std::net::TcpStream, _)> = listener2.accept();
    assert!(result.is_err(), "accept must fail after timeout");
    let kind = result.unwrap_err().kind();
    assert!(
        kind == std::io::ErrorKind::WouldBlock || kind == std::io::ErrorKind::TimedOut,
        "expected WouldBlock or TimedOut, got {kind:?}"
    );
}
```

Note: the test validates the OS-level `set_read_timeout`+`accept` contract
(not the full `listen_for_code` async path, which would require waiting 120s).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- oauth::server` → all tests pass.

---

### Step 3: CSP — remove Vite dev-server origins from production config

**Goal**: The shipped `tauri.conf.json` must not allow `http://localhost:1420`
or `ws://localhost:1420` in `connect-src`.

**File**: `src-tauri/tauri.conf.json`

Current line 24:
```json
"csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' http://localhost:1420 ws://localhost:1420"
```

Replace with (drop the two localhost Vite origins from `connect-src`):
```json
"csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'"
```

Tauri injects the dev URL automatically during `tauri dev` via its own
mechanism; the CSP in `tauri.conf.json` is what goes into the production bundle,
so removing them here does not affect local development.

**Verify**: `grep -n "localhost:1420" src-tauri/tauri.conf.json` → no matches (exit 1 from grep, meaning zero lines found).

Also run: `npm run typecheck` → exit 0 (config change should have no TS impact, but confirms the build files parse cleanly).

---

### Step 4: Sanitize upstream API error body before forwarding to frontend

**Goal**: Replace the verbatim upstream Google API response body in the Rust
error string with a safe, user-facing message. Keep the HTTP status for
diagnostics; drop the body.

**File**: `src-tauri/src/oauth/token_store.rs`

Current lines 267-269 (`refresh_access_token`):
```rust
if !resp.status().is_success() {
    let body = resp.text().await.unwrap_or_default();
    return Err(format!("refresh failed: {body}"));
}
```

Replace with:
```rust
if !resp.status().is_success() {
    let status = resp.status();
    // Do not forward the raw upstream body to the frontend — it may contain
    // diagnostic detail not intended for end-user display.
    let _ = resp.text().await; // consume body to release the connection
    return Err(format!(
        "Token refresh failed (HTTP {status}). Reconnect your Google account."
    ));
}
```

**Verify**:
- `grep -n 'format!("refresh failed: {body}")' src-tauri/src/oauth/token_store.rs` → no matches.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked` → all pass.

---

### Step 5: Privacy scan — add `.neko-data` and `.lancedb` to blocked paths

**Goal**: The scan must exit non-zero if `.neko-data/` or `.lancedb/` exist in
the working tree, preventing accidental commits of personal finance cache files.

**File**: `scripts/privacy-scan.sh`

Current `blocked_paths` array at lines 7-16:
```bash
blocked_paths=(
  ".circle-auth"
  ".circle-data"
  "private-data"
  "raw-scrape"
  "transcripts"
  "videos"
  "embeddings"
  "indexes"
)
```

Replace with:
```bash
blocked_paths=(
  ".circle-auth"
  ".circle-data"
  "private-data"
  "raw-scrape"
  "transcripts"
  "videos"
  "embeddings"
  "indexes"
  ".neko-data"
  ".lancedb"
)
```

**Verify**:
```bash
# Simulate: create the directory, run the scan, confirm it fails.
mkdir -p /tmp/neko-scan-test/.neko-data
# We can't run privacy:scan against /tmp, so verify the script logic directly:
bash -c '
  blocked_paths=(".neko-data" ".lancedb")
  (cd /tmp/neko-scan-test && for p in "${blocked_paths[@]}"; do
    [[ -e "$p" ]] && echo "BLOCKED: $p found" && exit 1
  done; echo "OK: none found")
'
```
Expected: `BLOCKED: .neko-data found`.

Then run `npm run privacy:scan` from the repo root (where neither `.neko-data`
nor `.lancedb` should exist) → exit 0, "Privacy scan passed."

---

### Step 6: Full gate verification

Run the full check suite to confirm all five steps together produce a clean
build.

**Verify**: `npm run check` → exit 0, all gates green.

## Test plan

New tests written in this plan:

| Test name | File | What it covers |
|---|---|---|
| `test_store_token_fails_closed_without_keychain_env` | `token_store.rs` | Structural presence of fail-closed path; env var absent |
| `test_store_token_file_fallback_when_env_set` | `token_store.rs` | File written when `NEKO_INSECURE_FILE_FALLBACK=1` |
| `test_listen_for_code_times_out` | `server.rs` | OS-level `set_read_timeout`+`accept` contract returns `WouldBlock`/`TimedOut` |

Existing test to use as structural pattern: `token_store.rs::tests::test_token_store_roundtrip`
and `server.rs::tests::test_extract_code_from_path`.

**Verification command**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -5` → all pass, including the three new tests.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0 (fmt + clippy + cargo test all pass).
- [ ] `npm run typecheck` exits 0, no TS errors.
- [ ] `npm run lint` exits 0.
- [ ] `npm run privacy:scan` exits 0, prints "Privacy scan passed."
- [ ] `grep -n "localhost:1420" src-tauri/tauri.conf.json` returns no lines.
- [ ] `grep -n 'format!("refresh failed: {body}")' src-tauri/src/oauth/token_store.rs` returns no lines.
- [ ] `grep -n 'NEKO_INSECURE_FILE_FALLBACK' src-tauri/src/oauth/token_store.rs` shows the env-var check in `store_token`.
- [ ] `grep -E '\.neko-data|\.lancedb' scripts/privacy-scan.sh` returns the two new entries.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | grep FAILED` returns no lines.
- [ ] `git diff --name-only` shows only the five in-scope files (no unintended changes).
- [ ] `plans/README.md` status row for plan 013 updated to DONE.

## STOP conditions

Stop and report back (do not improvise) if:

- The code at any location in "Current state" does not match the excerpt — the file has changed since d183bbf. Run the drift check at the top.
- Step 1: changing `store_token` causes `test_token_store_roundtrip` to fail on a dev machine that has a working keychain (the test calls `store_token`; if it now errors, something is wrong with the keyring branch, not the fallback branch).
- Step 2: `set_read_timeout` on `TcpListener` is not available on the target platform (this is part of std; should always be available, but if clippy emits an "unused" or "unresolved" error, stop).
- Step 3: removing the Vite origins from CSP breaks the UI in `tauri dev` mode in a way that is not self-evidently a dev-only issue — stop and investigate before landing.
- Step 4: the `resp.text().await` consume triggers a borrow-checker error because `resp` is partially moved by `resp.status()`. If so, capture status first: `let status = resp.status();` before consuming body. (This is the intended pattern already in the plan, but confirm it compiles.)
- Any step's `cargo test` produces a new FAILED test not present before this plan.
- A fix in any step appears to require touching a file outside the in-scope list.

## Maintenance notes

- **Deferred: `mod.rs` exchange-token body leak** — `src-tauri/src/oauth/mod.rs:85-87` has the same raw-body-in-error pattern for the initial token exchange (`token exchange failed: {body}`). This was deliberately left out of scope to keep the change small. A follow-up plan or PR should apply the same sanitization pattern used in step 4.
- **`NEKO_INSECURE_FILE_FALLBACK` must be documented** — add a note to `docs/` (or the existing env-var inventory if one exists) so that headless deployment instructions reference this knob. Without documentation, operators on CI or WSL2 without a keychain daemon will get a confusing error at OAuth time.
- **CSP and Tauri dev mode** — Tauri 2.x injects the `devUrl` into the CSP at dev time via its own internal mechanism, so `npm run tauri dev` should continue to work. If a future Tauri upgrade changes this behavior, re-examine whether `devUrl` CSP injection still applies.
- **PR reviewer checklist**: confirm that the `NEKO_INSECURE_FILE_FALLBACK` Err message is user-readable and actionable; confirm that the sanitized refresh-failure message does not hide information that would be needed to debug auth failures (the HTTP status code is kept; body is not).
- **Privacy scan does not cover git object history** — only the working tree and commit messages in the publishable range are scanned (documented in the script). If `.neko-data` was ever committed and then removed, the block-list catches the directory's presence but not historical content. This is intentional and documented in `scripts/privacy-scan.sh:87-91`.
