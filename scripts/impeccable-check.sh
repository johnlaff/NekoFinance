#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DETECTOR=".agents/skills/impeccable/scripts/detect.mjs"
# All hand-written UI sources (App.tsx is composition-only since spec 004; the
# screens/shell/features dirs carry the actual UI). Vendored design-system
# files are upstream artifacts and stay out of scope.
mapfile -t FILES < <(
  find src/App.tsx src/App.css src/screens src/shell src/features \
    \( -name "*.tsx" -o -name "*.css" \) ! -name "*.test.tsx" 2>/dev/null
)

if [[ ! -f "$DETECTOR" ]]; then
  printf 'Impeccable detector not found. Skipping UI audit.\n' >&2
  exit 0
fi

printf 'Running Impeccable UI audit...\n'
node "$DETECTOR" --json "${FILES[@]}"

EXIT=$?
if [[ $EXIT -eq 2 ]]; then
  printf '\nUI anti-patterns found. Run `npm run ui:audit` locally for details.\n' >&2
  exit 1
elif [[ $EXIT -ne 0 ]]; then
  printf '\nImpeccable detector error (exit %d).\n' "$EXIT" >&2
  exit "$EXIT"
fi

printf 'UI audit passed.\n'
