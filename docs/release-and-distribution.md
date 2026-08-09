# Release And Distribution

## Current Position

Neko Finance is a personal MVP. Releases should be reliable, but not overbuilt. The repo has an
automated release train (release-please) that turns merged PRs into a draft release, plus a build
workflow — dispatched by the train — that builds Linux and Windows bundles and attaches them to the
draft. The tag is born when the maintainer publishes the draft.

## Release Train

- Every PR title must carry a Conventional Commit prefix in English (`feat:`/`fix:`/`chore:`…, body
  in any language) — the `PR Title` check enforces it, including PRs opened by automation. Because
  merges are squashed, the PR title _is_ the commit release-please reads.
- The `Release Please` workflow (`.github/workflows/release-please.yml`) runs on every push to
  `main` and keeps a living Release PR, accumulating `CHANGELOG.md` entries from those titles and
  deciding the next SemVer bump (`fix:` → patch, `feat:` → minor, `!`/`BREAKING CHANGE:` → major).
  Config lives in `release-please-config.json` / `.release-please-manifest.json`; the version is
  kept in lockstep across `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
- Merging the Release PR is one of only two manual gestures: release-please commits the version
  bump and creates a GitHub Release **draft** named `v*.*.*` (not pre-release). A draft holds the
  tag name but the git tag itself only exists once the draft is published. Because events created
  with `GITHUB_TOKEN` never trigger other workflows (GitHub's anti-recursion rule, with
  `workflow_dispatch` as a documented exception), the `Release Please` workflow explicitly
  dispatches the `Release` workflow (`.github/workflows/release.yml`) with the tag name and the
  bump commit, and that run builds the Linux/Windows bundles and attaches them to the same draft.
- **Release only ever comes from CI.** `windows-latest` in `release.yml` is the only path that
  produces a release artifact; the local `cargo-xwin` cross-compile (`scripts/build-windows.sh`) is
  a fast local test cycle and never publishes anything — see "Windows Builds" below.
- The second manual gesture: a maintainer publishes the draft by hand after the build matrix is
  green (and can smoke-test the installer first). Publishing creates the git tag and is the moment
  `/releases/latest/download/latest.json` starts serving the update, so releases are **not** marked
  pre-release — a published pre-release is invisible to `/releases/latest` and would fail silently.
- Use patch releases for fixes, minor releases for new user-visible slices, major releases only
  after public API/data compatibility matters — release-please derives this from PR title prefixes,
  it is never decided by hand.

## GitHub Actions Cost

- Public repo: standard GitHub-hosted runners are free.
- Private repo: GitHub Free includes a monthly quota; Windows runners cost more minutes than Linux, and macOS costs much more.
- Keep full release builds manual or train-dispatched, not on every push.

## Windows Builds

The release workflow builds on `windows-latest` using `tauri-apps/tauri-action` (SHA-pinned) and
publishes NSIS/MSI installers plus a portable single-file exe, all with SLSA build-provenance
attestations (`gh attestation verify <file> --repo <owner>/<repo>`). This CI-native runner is the
only supported path for `tauri-action` to produce a release build — the action does not document or
support cross-compiling Windows from Linux. Local cross-compiled builds via `cargo-xwin`
(`scripts/build-windows.sh`, WSL2) remain a fast local test cycle only — see
`docs/building-windows.md` — and never produce a release artifact. For personal use, unsigned
installers are acceptable but Windows SmartScreen warnings are expected.

Before public distribution, decide:

- NSIS vs MSI as primary installer.
- Windows code signing certificate.
- Whether to publish via GitHub Releases, Microsoft Store, or a dedicated updater CDN.

## Auto-Update Plan

Tauri updater is the intended path to avoid manual `.exe` downloads. The full design (in-app update
state machine, capability wiring) is decided in [ADR-0012](adr/0012-release-train-and-updater.md) —
this section tracks the signing pipeline and the key custody/rotation procedure.

`release.yml` builds with `uploadUpdaterJson: true` and `updaterJsonPreferNsis: true`, and
`src-tauri/tauri.conf.json` sets `bundle.createUpdaterArtifacts: true`. The build only produces a
signed NSIS bundle and a populated `latest.json` once the signing key exists as GitHub Actions
secrets — see the checklist below. Until a maintainer completes it, `TAURI_SIGNING_PRIVATE_KEY` is
empty at build time, `tauri-action` skips signing, and `plugins.updater.pubkey` in
`tauri.conf.json` stays the empty-string placeholder it ships with.

### Signing key custody checklist (maintainer, one-time)

1. Generate the minisign key pair with the Tauri CLI, keeping the original under `~/.tauri/`
   (outside the repo, never committed):

   ```bash
   npm run tauri signer generate -- -w ~/.tauri/neko-finance.key
   ```

2. Set a passphrase when prompted — an unprotected key is a single leaked file away from full
   compromise.
3. Back up the private key file and its passphrase as **two separate entries** in the password
   manager. Splitting them means one leaked factor is not enough to sign a malicious update.
4. Add the private key and passphrase as GitHub Actions repository secrets (Settings → Secrets and
   variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/neko-finance.key`.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the passphrase from step 2.
5. Copy the **public** key content (`~/.tauri/neko-finance.key.pub`) into
   `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`, replacing the empty-string placeholder.
   Commit this file — the public key is not a secret.
6. Cut a release and confirm the draft carries a signed NSIS bundle and a `latest.json` with a
   non-empty `signature` field before publishing.

### Key rotation runbook (bridge release)

The updater plugin has no native key rotation (tracked upstream in
[tauri-apps/tauri#7585](https://github.com/tauri-apps/tauri/issues/7585)): an installed app only
trusts the pubkey it currently knows and cannot validate an update signed with a brand-new key it
has never seen. Rotating requires one intermediate **bridge release** that keeps both keys valid
for one version:

1. Generate the new key pair (checklist above, new file name) without deleting the old one yet.
2. Ship a bridge release **still signed with the old private key**, but with
   `plugins.updater.pubkey` already updated to the **new** public key. Installed apps on the old
   pubkey accept this release because the signature still matches what they trust.
3. Replace the `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets with the
   new key pair's values.
4. Ship the next release signed with the new key. Only apps that already installed the bridge
   release (step 2) can validate it — this is why the bridge step cannot be skipped.
5. Retire the old private key and passphrase from the password manager once telemetry (or a
   reasonable adoption window) shows the installed base has moved past the bridge release.

Losing the private key with no rotation in progress permanently strands every installed app: there
is no server-side revocation, so the only recovery is asking users to reinstall from a fresh
download.
