#!/usr/bin/env bash
# Builds the Windows executable from Linux/WSL2.
#
# Default: MSVC target via cargo-xwin → SINGLE-FILE neko-finance.exe
#          (WebView2 loader and VC runtime statically linked; no companion
#          DLL and no VC++ Redistributable needed).
#
#   --installer   also produce the NSIS setup (requires makensis)
#   --gnu         legacy MinGW build (exe + WebView2Loader.dll side file)
#
# First MSVC run downloads the Windows SDK/CRT via xwin (~1.5 GB, cached under
# ~/.cache/cargo-xwin; Microsoft SDK license auto-accepted). Details and
# verification steps: docs/building-windows.md.
set -euo pipefail
cd "$(dirname "$0")/.."

export XWIN_ACCEPT_LICENSE=1
# tauri-build statically links the VC runtime when this is set (the tauri CLI
# sets it on `tauri build`; exported here so the behavior is explicit). The
# remaining CRT imports are ucrt (api-ms-win-crt-*), an OS component on Win10/11.
export STATIC_VCRUNTIME=true

# Bake the OAuth client secret into the binary so the BACKGROUND sync token refresh has it.
# The frontend bundle already bakes VITE_GOOGLE_CLIENT_SECRET; the Rust side reads it at compile
# time via option_env!("GOOGLE_CLIENT_SECRET"). Without it, the background refresh 400s and the
# Google connection "drops after ~1h". Sourced from the local gitignored .env (desktop-client
# secret — not confidential; it already ships inside the frontend bundle).
if [[ -z "${GOOGLE_CLIENT_SECRET:-}" && -f .env ]]; then
  GOOGLE_CLIENT_SECRET="$(grep -E '^VITE_GOOGLE_CLIENT_SECRET=' .env | head -1 | sed -E "s/^[^=]*=//; s/\r$//; s/^[\"']//; s/[\"']$//")"  # gitleaks:allow -- value read from the local gitignored .env at build time; nothing secret is committed here
  export GOOGLE_CLIENT_SECRET
  [[ -n "$GOOGLE_CLIENT_SECRET" ]] && echo "Baked GOOGLE_CLIENT_SECRET for background token refresh."
fi

MODE="msvc"
BUNDLE_ARGS=(--no-bundle)
for arg in "$@"; do
  case "$arg" in
    --gnu) MODE="gnu" ;;
    --installer) BUNDLE_ARGS=(--bundles nsis) ;;
    *)
      echo "unknown option: $arg (supported: --installer, --gnu)" >&2
      exit 2
      ;;
  esac
done

if [[ "$MODE" == "msvc" ]]; then
  npx tauri build "${BUNDLE_ARGS[@]}" --runner cargo-xwin --target x86_64-pc-windows-msvc
  OUT="src-tauri/target/x86_64-pc-windows-msvc/release"
  echo "Built: $OUT/neko-finance.exe (single file — loader and VC runtime statically linked)"
  if [[ "${BUNDLE_ARGS[0]}" != "--no-bundle" ]]; then
    echo "Installer: $OUT/bundle/nsis/"
  fi
else
  npx tauri build --no-bundle --target x86_64-pc-windows-gnu
  OUT="src-tauri/target/x86_64-pc-windows-gnu/release"
  LOADER=$(find "$HOME/.cargo/registry/src" -path "*webview2-com-sys*/x64/WebView2Loader.dll" 2>/dev/null | head -1)
  if [[ -z "$LOADER" ]]; then
    echo "ERROR: WebView2Loader.dll not found in the cargo registry (webview2-com-sys)." >&2
    exit 1
  fi
  cp "$LOADER" "$OUT/"
  echo "Built: $OUT/neko-finance.exe (+ WebView2Loader.dll — ship both files together)"
fi
