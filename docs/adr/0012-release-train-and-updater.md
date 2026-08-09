# ADR-0012: Automated release train and signed auto-update

The desktop app ships two Windows artifacts: an NSIS installer and a portable single-file exe.
Releases are cut by hand (tag push) and installed builds never learn about newer versions.

## Decision

**The NSIS-installed app is the auto-update target.** The Tauri v2 updater plugin only updates
through an installer, so the installed app is the supported daily driver; the portable exe stays
published as a convenience artifact with no update channel.

**release-please drives the train.** It maintains a living Release PR that accumulates the
changelog from Conventional Commit PR titles (`feat:`/`fix:`/`chore:` prefixes in English, body in
any language); merging that PR creates a **draft** release and dispatches the `release.yml` build
matrix (a draft holds the tag name, but the git tag itself is only created when the draft is
published — and `GITHUB_TOKEN`-created events never trigger tag workflows). A PR-title check (semantic-pull-request action, SHA-pinned)
turns the commit discipline into a red/green gate — squash-merge means the PR title _is_ the
commit release-please reads.

**Publishing is a human gate.** The build attaches the signed NSIS bundle, `latest.json`, and the
portable exe to the draft; a maintainer publishes after the matrix is green (and can smoke-test the
installer first). Publishing is the moment `/releases/latest/download/latest.json` starts serving
the update — the draft flag alone is the gate, so releases are **not** marked pre-release: a
published pre-release is invisible to `/releases/latest` and would fail silently.

**Updater key custody.** The minisign private key is passphrase-protected; the original lives in
`~/.tauri/` outside the repo, key and passphrase are backed up as separate password-manager
entries, and the CI only sees them as GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`,
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). The public key ships in `tauri.conf.json`
(`plugins.updater.pubkey`).

**In-app flow: silent check, explicit consent.** The app checks for updates in the background at
launch; when one exists it shows a single calm invitation (Configurações also shows the current
version and update state). Download + install (`installMode: passive`) and `relaunch()` run only
after the user opts in — on Windows the installer kills the running app, so the restart must never
be a surprise. `check()` is handled defensively (it may return a non-null object with no update),
and network failures stay silent: a local-first app must not complain about being offline.

**`cargo-xwin` stays local-only.** `scripts/build-windows.sh` remains the fast local test cycle;
release artifacts are born only in CI on `windows-latest` (the tauri-action-supported path that
signs updater artifacts and emits `latest.json`).

## Why

- The updater has no mechanism for a portable exe to replace itself; building one would be custom
  self-swap code against the grain of the official plugin.
- release-please is the only option among {release-please, git-cliff, manual tag} that closes the
  version→changelog→tag→build loop without a forgettable manual step; git-cliff still leaves the
  version decision and the trigger to a human.
- Losing the private key permanently strands every installed app — the plugin has no native key
  rotation; the only path is a bridge release signed with the old key that announces the new
  pubkey (tauri-apps/tauri#7585). Passphrase + split backup means one leaked factor is not enough.
- A human publish gate matches the repo rule that material writes require human approval, and
  costs one click per release.

## Consequences

- Every PR title must carry a Conventional Commit prefix in English; the CI title check enforces
  it, including PRs opened by automation.
- `release.yml` drops `prerelease: true` and enables `uploadUpdaterJson`; the release body no
  longer claims updater artifacts are disabled.
- Rotating the updater key requires a bridge release signed with the outgoing key; the runbook
  lives with the release docs, not in institutional memory.
- The portable exe receives no update notifications; anyone on it upgrades by downloading the next
  release manually.
