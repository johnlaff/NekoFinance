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
  ".neko-data"
  ".lancedb"
)

for path in "${blocked_paths[@]}"; do
  if [[ -e "$path" ]]; then
    printf 'Blocked private artifact path exists: %s\n' "$path" >&2
    exit 1
  fi
done

if [[ ! -f ".private-forbidden-patterns" ]]; then
  # A deny-list é gitignored (ela própria contém os nomes privados), então uma máquina nova/CT
  # pode não tê-la. Por padrão, pulamos (contribuidor sem o arquivo passa). Em contextos onde a
  # lista é OBRIGATÓRIA (CI do mantenedor), defina PRIVACY_SCAN_REQUIRE_DENYLIST=1 para FALHAR em
  # vez de pular em silêncio — assim o gate nunca passa verde sem realmente ter escaneado.
  if [[ "${PRIVACY_SCAN_REQUIRE_DENYLIST:-0}" == "1" ]]; then
    printf 'PRIVACY_SCAN_REQUIRE_DENYLIST=1 but .private-forbidden-patterns is missing — failing.\n' >&2
    exit 1
  fi
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

scan_tree_for_pattern() {
  local pattern="$1"
  local entry="$2"
  local files=()

  mapfile -t files < <(
    rg --hidden --no-ignore-vcs --fixed-strings --ignore-case --files-with-matches \
      --glob '!.git/**' \
      --glob '!node_modules/**' \
      --glob '!dist/**' \
      --glob '!src-tauri/target/**' \
      --glob '!package-lock.json' \
      --glob '!.private-forbidden-patterns' \
      --glob '!SESSION-CONTEXT.md' \
      --glob '!.methodology-pack/**' \
      --glob '!.claude/skills/neko-fontes-brutas/**' \
      --glob '!Docs/**' \
      --glob '!.playwright-mcp/**' \
      -- "$pattern" . || true
  )

  if [[ "${#files[@]}" -eq 0 ]]; then
    return 1
  fi

  for file in "${files[@]}"; do
    awk -v pat="$pattern" -v entry="$entry" '
      BEGIN { needle = tolower(pat) }
      index(tolower($0), needle) {
        printf "Private forbidden pattern matched: denylist entry #%d at %s:%d\n", entry, FILENAME, FNR > "/dev/stderr"
      }
    ' "$file"
  done

  return 0
}

entry_index=0
while IFS= read -r pattern || [[ -n "$pattern" ]]; do
  [[ -z "$pattern" || "$pattern" =~ ^[[:space:]]*# ]] && continue
  entry_index=$((entry_index + 1))

  # (1) Arquivos da árvore de trabalho (exclui .git/: binário/histórico, lento e ruidoso). O
  #     histórico de MENSAGENS é coberto pelo passo (2). O histórico de CONTEÚDO (versões antigas
  #     de arquivos) é intencionalmente fora de escopo: senão REMOVER um vazamento já versionado
  #     faria o próprio diff de remoção disparar o scan para sempre. O que importa para o repo
  #     público é a árvore atual + as mensagens — ambas cobertas aqui.
  if scan_tree_for_pattern "$pattern" "$entry_index"; then
    exit 1
  fi

  # (2) Mensagens de commit do range publicável (case-insensitive, como o passo 1).
  if [[ -n "$commit_msgs" ]] && printf '%s' "$commit_msgs" | grep -qiF -- "$pattern"; then
    printf 'Private forbidden pattern in commit message: denylist entry #%d\n' "$entry_index" >&2
    exit 1
  fi
done < ".private-forbidden-patterns"

printf 'Privacy scan passed.\n'
