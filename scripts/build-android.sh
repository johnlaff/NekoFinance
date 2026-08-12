#!/usr/bin/env bash
# Builds a signed release APK for Android from Linux/WSL2.
#
# Default: aarch64-only release APK, signed with the keystore referenced by
#          src-tauri/gen/android/keystore.properties (gitignored, never generated
#          by this script — see docs/building-android.md for provisioning it).
#
#   --all-abis   build every ABI (armv7, i686, x86_64 besides aarch64) instead
#                of just the aarch64 target exercised on the reference device
#   --debug      debug-signed APK (no keystore.properties needed); for a quick
#                local install/test cycle only, never for distribution
#
# Toolchain pinned by the spec-042 gate (docs/building-android.md has the full
# table): NDK 28.2.13676358, cargo-ndk 4.1.2, AGP 8.11.0, Gradle 8.14.3 under
# JDK 21 (Gradle 8.14.3 does not run under JDK 25 — "Unsupported class file
# major version 69"). ANDROID_HOME/NDK_HOME are expected to already be exported
# (this machine does it in /etc/profile.d/android.sh); JAVA_HOME is switched to
# a JDK 21 install found on PATH/mise for the duration of this script only —
# never pinned in gradle.properties, the JDK 21 path is per-machine.
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="release"
TARGET_ARGS=(--target aarch64)
for arg in "$@"; do
  case "$arg" in
    --all-abis) TARGET_ARGS=() ;;
    --debug) MODE="debug" ;;
    *)
      echo "unknown option: $arg (supported: --all-abis, --debug)" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${ANDROID_HOME:-}" || -z "${NDK_HOME:-}" ]]; then
  echo "ERROR: ANDROID_HOME/NDK_HOME not set. See docs/building-android.md for the pinned SDK/NDK setup." >&2
  exit 1
fi

# Gradle 8.14.3 refuses to run under JDK 25 (the toolchain image on this repo's dev machines).
# Prefer an explicit JDK 21 if one is discoverable; otherwise fall back to whatever JAVA_HOME
# already points at and let Gradle's own version check fail with its real error.
if command -v mise >/dev/null 2>&1; then
  JDK21_HOME="$(mise where java@temurin-21 2>/dev/null || true)"
  if [[ -n "$JDK21_HOME" ]]; then
    export JAVA_HOME="$JDK21_HOME"
    export PATH="$JAVA_HOME/bin:$PATH"
  fi
fi
echo "Using JAVA_HOME=${JAVA_HOME:-<unset>}"

KEYSTORE_PROPS="src-tauri/gen/android/keystore.properties"
if [[ "$MODE" == "release" && ! -f "$KEYSTORE_PROPS" ]]; then
  cat >&2 <<EOF
ERROR: $KEYSTORE_PROPS not found — a release build must be signed with the
owner's own keystore, never left unsigned or signed with a placeholder.

Provisioning steps (one-time, outside the repo): docs/building-android.md.
For a quick local install without signing, use --debug instead.
EOF
  exit 1
fi

DEBUG_ARGS=()
[[ "$MODE" == "debug" ]] && DEBUG_ARGS=(--debug)

npx tauri android build --apk "${TARGET_ARGS[@]}" "${DEBUG_ARGS[@]}"

OUT_DIR="src-tauri/gen/android/app/build/outputs/apk"
# Gradle names the build-type directory "debug"/"release" regardless of the ABI folder
# above it (per-ABI vs "universal") — filtering on that path segment is deterministic.
# mtime/"-newer" is NOT: a release rebuild with byte-identical output (same source, same
# optimizations) is a legitimate Gradle cache hit that leaves the existing APK's mtime
# untouched, so comparing timestamps against a just-built debug APK picks the wrong file.
APK="$(find "$OUT_DIR" -iname "*.apk" -path "*/$MODE/*" 2>/dev/null | sort | tail -1)"

echo
echo "Built: ${APK:-$OUT_DIR (see this directory for the APK)}"
echo "Install on the paired device: adb install -r \"$APK\""
