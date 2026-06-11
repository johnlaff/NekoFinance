# Building the Windows .exe

Two supported paths produce the Windows desktop binary. Both yield a **single-file portable
`neko-finance.exe`** (MSVC target): `webview2-com-sys` links the WebView2 loader statically on
`target_env = "msvc"`, and `tauri-build` links the VC runtime statically (`STATIC_VCRUNTIME`,
set by the tauri CLI and exported explicitly in `scripts/build-windows.sh`). The exe needs no
companion DLL and no VC++ Redistributable. _(Full `+crt-static` was evaluated and rejected: it
breaks the bundled SQLite link under cargo-xwin, and the only thing it would add is staticizing
ucrt — already an OS component on Windows 10/11.)_

The only runtime dependency is the **WebView2 Runtime** — a system component preinstalled on
up-to-date Windows 10/11. The NSIS installer bootstraps it automatically on machines that lack it;
the portable exe assumes it is present.

## 1. CI release (canonical for distribution)

`.github/workflows/release.yml` runs on every `v*.*.*` tag (or manual `workflow_dispatch`):

- `windows-latest` (MSVC): NSIS installer + MSI via `tauri-action`, attached to a draft GitHub
  Release, **plus** the portable single-file exe (workflow artifact, and attached to the release
  on tag builds as `neko-finance-<tag>-windows-x64-portable.exe`).
- `ubuntu-24.04`: Linux bundles (deb/AppImage/rpm).

## 2. Local cross-compile from WSL2/Linux (default: MSVC via cargo-xwin)

```bash
# one-time setup
sudo apt install clang lld llvm nsis
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc

# build the single-file exe
npm run build:windows
# → src-tauri/target/x86_64-pc-windows-msvc/release/neko-finance.exe

# optionally also build the NSIS installer locally
npm run build:windows -- --installer
```

Notes:

- The first MSVC build downloads the Windows SDK/CRT through `xwin` (~1.5 GB, cached in
  `~/.cache/cargo-xwin`); the script auto-accepts the Microsoft SDK license
  (`XWIN_ACCEPT_LICENSE=1`).
- `webview2-com-sys` links `WebView2LoaderStatic.lib` on `target_env = "msvc"` and falls back to
  a dynamic `WebView2Loader.dll` import on every other target — that is why the MSVC target is
  the default here and why the gnu build needs a side file.
- Verify portability after a build:
  `x86_64-w64-mingw32-objdump -p …/neko-finance.exe | grep "DLL Name"` — only Windows system
  DLLs should appear: no `WebView2Loader.dll`, no `vcruntime140.dll`. (`api-ms-win-crt-*`
  imports are fine — that is ucrt, shipped with Windows 10/11.)
- From WSL2 you can launch the exe directly on the Windows side thanks to interop.
- The app database lives in the per-user app-data directory
  (`%APPDATA%/app.neko.finance` on Windows), not next to the executable.

### Legacy fallback: MinGW (`--gnu`)

`npm run build:windows -- --gnu` builds `x86_64-pc-windows-gnu` with the system MinGW toolchain
(no SDK download; setup: `sudo apt install gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64` +
`rustup target add x86_64-pc-windows-gnu`). The windows-gnu target cannot static-link the
WebView2 loader, so the script copies `WebView2Loader.dll` next to the exe — **ship both files
together**. Useful when the xwin toolchain is unavailable; otherwise prefer the MSVC default.
