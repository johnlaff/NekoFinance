#!/usr/bin/env bash
# Cross-compiles the Windows .exe from Linux/WSL2 (MinGW + x86_64-pc-windows-gnu).
# The windows-gnu target links WebView2Loader dynamically, so the loader DLL must
# sit next to the exe — this script copies it from the webview2-com-sys package.
# See docs/building-windows.md.
set -euo pipefail

cd "$(dirname "$0")/.."

npx tauri build --no-bundle --target x86_64-pc-windows-gnu

OUT="src-tauri/target/x86_64-pc-windows-gnu/release"
LOADER=$(find "$HOME/.cargo/registry/src" -path "*webview2-com-sys*/x64/WebView2Loader.dll" 2>/dev/null | head -1)

if [[ -z "$LOADER" ]]; then
  echo "ERROR: WebView2Loader.dll not found in the cargo registry (webview2-com-sys)." >&2
  echo "Run a build first so cargo downloads the crate, then re-run." >&2
  exit 1
fi

cp "$LOADER" "$OUT/"
echo "Built: $OUT/neko-finance.exe (+ WebView2Loader.dll)"
echo "Ship both files together; WebView2 runtime is preinstalled on Windows 10/11."
