#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

blocked_paths=(
  ".circle-auth"
  ".circle-data"
  "private-data"
  "raw-scrape"
  "transcripts"
  "videos"
  "embeddings"
  "indexes"
)

for path in "${blocked_paths[@]}"; do
  if [[ -e "$path" ]]; then
    printf 'Blocked private artifact path exists: %s\n' "$path" >&2
    exit 1
  fi
done

if [[ ! -f ".private-forbidden-patterns" ]]; then
  printf 'No .private-forbidden-patterns file found. Skipping private-name scan.\n' >&2
  exit 0
fi

while IFS= read -r pattern || [[ -n "$pattern" ]]; do
  [[ -z "$pattern" || "$pattern" =~ ^[[:space:]]*# ]] && continue

  if rg --hidden --no-ignore-vcs --fixed-strings --line-number \
    --glob '!node_modules/**' \
    --glob '!dist/**' \
    --glob '!src-tauri/target/**' \
    --glob '!package-lock.json' \
    --glob '!.private-forbidden-patterns' \
    --glob '!SESSION-CONTEXT.md' \
    --glob '!.methodology-pack/**' \
    "$pattern" .; then
    printf 'Private forbidden pattern matched: %s\n' "$pattern" >&2
    exit 1
  fi
done < ".private-forbidden-patterns"

printf 'Privacy scan passed.\n'
