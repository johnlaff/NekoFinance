# Building the Windows .exe

Two supported paths produce the Windows desktop binary.

## 1. CI release (canonical)

`.github/workflows/release.yml` builds Windows (NSIS installer) and Linux bundles via
`tauri-action` on every `v*.*.*` tag (or manual `workflow_dispatch`) and attaches the artifacts to
a draft GitHub Release. This is the path for distributable, versioned builds.

## 2. Local cross-compile from WSL2/Linux (development)

The repo cross-compiles a runnable `neko-finance.exe` from Linux using the MinGW-w64 toolchain and
the `x86_64-pc-windows-gnu` Rust target:

```bash
# one-time setup
sudo apt install gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu

# build (frontend + Rust, no bundler) + copy WebView2Loader.dll next to the exe
npm run build:windows
# → src-tauri/target/x86_64-pc-windows-gnu/release/neko-finance.exe
#   src-tauri/target/x86_64-pc-windows-gnu/release/WebView2Loader.dll
```

Notes:

- `--no-bundle` skips installer generation (NSIS is not available cross-OS here). Frontend assets,
  migrations, and icons are embedded in the `.exe` at compile time; the **only** companion file is
  `WebView2Loader.dll` — the windows-gnu target cannot static-link the loader (the static lib is
  MSVC-only), so `scripts/build-windows.sh` copies it from the `webview2-com-sys` package. Ship
  the two files together.
- The MinGW runtime itself links statically (verified with `objdump -p … | grep "DLL Name"`:
  besides `WebView2Loader.dll`, only Windows system DLLs appear — no `libgcc_s_seh-1.dll` /
  `libwinpthread-1.dll`).
- WebView2 is required at runtime; it ships with Windows 10/11 by default. The NSIS installer from
  the CI path bootstraps it on machines that lack it.
- From WSL2 you can launch the result directly on the Windows side
  (`./src-tauri/target/x86_64-pc-windows-gnu/release/neko-finance.exe`) thanks to interop.
- The app database lives in the per-user app-data directory
  (`%APPDATA%/app.neko.finance` on Windows), not next to the executable.
- Official Tauri guidance for Windows-on-Linux builds is the experimental `cargo-xwin` (MSVC)
  route; the MinGW route above is simpler in this repo's WSL2 environment and is validated by the
  checks in this document. If MSVC-specific issues appear, switch to `cargo-xwin` or the CI path.
