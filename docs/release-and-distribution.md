# Release And Distribution

## Current Position

Neko Finance is a personal MVP. Releases should be reliable, but not overbuilt. The repo has an
automated release train (release-please) that turns merged PRs into a tag and a draft release, plus a
tag-triggered build workflow that builds Linux and Windows bundles. Auto-update is planned but not
enabled until signing keys exist.

## Release Train

- Every PR title must carry a Conventional Commit prefix in English (`feat:`/`fix:`/`chore:`…, body
  in any language) — the `PR Title` check enforces it, including PRs opened by automation. Because
  merges are squashed, the PR title _is_ the commit release-please reads.
- The `Release Please` workflow (`.github/workflows/release-please.yml`) runs on every push to
  `main` and keeps a living Release PR, accumulating `CHANGELOG.md` entries from those titles and
  deciding the next SemVer bump (`fix:` → patch, `feat:` → minor, `!`/`BREAKING CHANGE:` → major).
  Config lives in `release-please-config.json` / `.release-please-manifest.json`; the version is
  kept in lockstep across `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
- Merging the Release PR is the only manual gesture: release-please commits the version bump,
  creates the `v*.*.*` tag and publishes a GitHub Release as a **draft** (not pre-release). That tag
  push is what triggers the existing `Release` workflow (`.github/workflows/release.yml`), which
  builds the Linux/Windows bundles and attaches them to the same draft.
- **Release only ever comes from CI.** `windows-latest` in `release.yml` is the only path that
  produces a release artifact; the local `cargo-xwin` cross-compile (`scripts/build-windows.sh`) is
  a fast local test cycle and never publishes anything — see "Windows Builds" below.
- A maintainer publishes the draft by hand after the build matrix is green (and can smoke-test the
  installer first). Publishing is the moment `/releases/latest/download/latest.json` starts serving
  the update, so releases are **not** marked pre-release — a published pre-release is invisible to
  `/releases/latest` and would fail silently.
- Use patch releases for fixes, minor releases for new user-visible slices, major releases only
  after public API/data compatibility matters — release-please derives this from PR title prefixes,
  it is never decided by hand.

## GitHub Actions Cost

- Public repo: standard GitHub-hosted runners are free.
- Private repo: GitHub Free includes a monthly quota; Windows runners cost more minutes than Linux, and macOS costs much more.
- Keep full release builds manual or tag-triggered, not on every push.

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

Tauri updater is the intended path to avoid manual `.exe` downloads. The full design (signing key
custody, in-app update state machine, key rotation runbook) is decided in
[ADR-0012](adr/0012-release-train-and-updater.md) — this section only tracks the current state.

Not enabled yet: `uploadUpdaterJson` stays `false` in `release.yml` and the release body still says
so, until the signing keys exist and are wired as GitHub Actions secrets
(`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) per ADR-0012.
