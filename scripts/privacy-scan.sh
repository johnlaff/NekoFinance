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

# Mensagens de commit também vão para o GitHub público — um termo privado numa mensagem vazaria
# mesmo sem aparecer em arquivo. Escaneia o range que seria enviado (origin/main..HEAD); sem o
# remoto (CI/clone raso), cai para todo o histórico de HEAD. Substitui o frágil scan do .git/.
commit_range=""
if git rev-parse --verify --quiet origin/main >/dev/null 2>&1; then
  commit_range="origin/main..HEAD"
fi
commit_msgs="$(git log ${commit_range} --format='%B' 2>/dev/null || true)"

while IFS= read -r pattern || [[ -n "$pattern" ]]; do
  [[ -z "$pattern" || "$pattern" =~ ^[[:space:]]*# ]] && continue

  # (1) Arquivos da árvore de trabalho (exclui .git/: binário/histórico, lento e ruidoso — o
  #     histórico de mensagens é coberto pelo passo (2)).
  if rg --hidden --no-ignore-vcs --fixed-strings --ignore-case --line-number \
    --glob '!.git/**' \
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

  # (2) Mensagens de commit do range publicável (case-insensitive, como o passo 1).
  if [[ -n "$commit_msgs" ]] && printf '%s' "$commit_msgs" | grep -qiF -- "$pattern"; then
    printf 'Private forbidden pattern in commit message: %s\n' "$pattern" >&2
    exit 1
  fi
done < ".private-forbidden-patterns"

printf 'Privacy scan passed.\n'
