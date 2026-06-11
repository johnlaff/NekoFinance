# Release And Distribution

## Current Position

Neko Finance is a personal MVP. Releases should be reliable, but not overbuilt. The repo now has a manual/tag-based release workflow that builds Linux and Windows bundles. Auto-update is planned but not enabled until signing keys exist.

## Release Train

- `main` should stay releasable.
- Use SemVer tags: `v0.1.0`, `v0.1.1`, `v0.2.0`.
- Use patch releases for fixes, minor releases for new user-visible slices, major releases only after public API/data compatibility matters.
- Until beta, releases are draft/prerelease by default.
- For personal use, trigger release manually when a useful slice is stable.
- For SaaS/public use later, move to a predictable stable train, for example one stable release every two weeks plus hotfixes.

## GitHub Actions Cost

- Public repo: standard GitHub-hosted runners are free.
- Private repo: GitHub Free includes a monthly quota; Windows runners cost more minutes than Linux, and macOS costs much more.
- Keep full release builds manual or tag-triggered, not on every push.

## Windows Builds

The release workflow builds on `windows-latest` using `tauri-apps/tauri-action@v0.6.2`. For personal use, unsigned installers are acceptable but Windows SmartScreen warnings are expected.

Before public distribution, decide:

- NSIS vs MSI as primary installer.
- Windows code signing certificate.
- Whether to publish via GitHub Releases, Microsoft Store, or a dedicated updater CDN.

## Auto-Update Plan

Tauri updater is the intended path to avoid manual `.exe` downloads.

Do not enable it until update signing keys are generated and stored safely:

```bash
npm run tauri signer generate -- -w ~/.tauri/neko-finance.key
```

Store only the public key in `tauri.conf.json`. Store the private key outside git and provide it to CI as:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Then enable:

- `@tauri-apps/plugin-updater`
- `tauri-plugin-updater`
- `tauri-plugin-process`
- `bundle.createUpdaterArtifacts = true`
- updater endpoint pointing to GitHub Releases `latest.json` or a future dynamic update server.

For personal MVP, GitHub Releases static `latest.json` is enough. For SaaS, use a dynamic update server only if channels, staged rollout, or rollback control becomes necessary.
