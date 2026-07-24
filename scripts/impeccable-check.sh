#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# All hand-written UI sources (App.tsx is composition-only since spec 004; the
# screens/shell/features dirs carry the actual UI). Vendored design-system
# files are upstream artifacts and stay out of scope.
mapfile -t FILES < <(
  find src/App.tsx src/App.css src/screens src/shell src/features \
    \( -name "*.tsx" -o -name "*.css" \) ! -name "*.test.tsx" 2>/dev/null
)

printf 'Running Impeccable UI audit...\n'

# The published CLI is the supported automation entry point, so the gate runs
# identically on a workstation and on a CI runner with no editor plugin present.
# Detector settings live in .impeccable/config.json and are picked up implicitly.
set +e
npx --yes impeccable@latest detect --json "${FILES[@]}"
EXIT=$?
set -e

# Detector contract: 0 = clean, 2 = findings, anything else = the detector itself
# failed and must not read as a passing gate.
if [[ $EXIT -eq 2 ]]; then
  printf '\nUI anti-patterns found. Run `npm run ui:audit` locally for details.\n' >&2
  exit 1
elif [[ $EXIT -ne 0 ]]; then
  printf '\nImpeccable detector error (exit %d).\n' "$EXIT" >&2
  exit "$EXIT"
fi

printf 'UI audit passed.\n'
