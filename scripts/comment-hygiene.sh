#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

file_list="$(mktemp "${TMPDIR:-/tmp}/neko-comment-hygiene.XXXXXX")"
trap 'rm -f "$file_list"' EXIT

if ! find src src-tauri/src -type f \
  \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' -o -name '*.css' \) \
  ! -path 'src/design-system/_ds_bundle.js' \
  -print0 > "$file_list"; then
  printf 'Não foi possível enumerar os arquivos-fonte para a higiene de comentários.\n' >&2
  exit 2
fi

source_files=()
while IFS= read -r -d '' source_file; do
  source_files+=("$source_file")
done < "$file_list"

if (( ${#source_files[@]} == 0 )); then
  printf 'Nenhum arquivo-fonte encontrado para a higiene de comentários.\n' >&2
  exit 2
fi

if awk '
  function check_comment(text, lower) {
    lower = tolower(text)
    if (lower ~ /(plan|plano)s?[ -]?0?[0-9][0-9][0-9]?/ ||
      lower ~ /spec ?0[0-9][0-9]/ ||
      lower ~ /review (adversarial|p[0-3])/ ||
      lower ~ /adversarial review|\(review\)/ ||
      lower ~ /dogfooding|descoberto (no|em) /) {
      printf "%s:%d:%s\n", FILENAME, FNR, text
      found = 1
    }
  }

  FNR == 1 { in_block = 0 }

  {
    rest = $0
    while (length(rest) > 0) {
      if (in_block) {
        block_end = index(rest, "*/")
        if (block_end == 0) {
          check_comment(rest)
          break
        }
        check_comment(substr(rest, 1, block_end - 1))
        rest = substr(rest, block_end + 2)
        in_block = 0
        continue
      }

      line_start = index(rest, "//")
      block_start = index(rest, "/*")

      if (line_start > 0 && (block_start == 0 || line_start < block_start)) {
        check_comment(substr(rest, line_start + 2))
        break
      }

      if (block_start > 0) {
        rest = substr(rest, block_start + 2)
        in_block = 1
        continue
      }

      break
    }
  }

  END { if (found) exit 1 }
' "${source_files[@]}"; then
  printf 'comment-hygiene ok\n'
else
  awk_status=$?
  if (( awk_status == 1 )); then
    printf 'Comentários com meta-referência de processo. Preserve apenas o racional técnico.\n' >&2
  else
    printf 'A verificação de comentários falhou com status %d.\n' "$awk_status" >&2
  fi
  exit "$awk_status"
fi
