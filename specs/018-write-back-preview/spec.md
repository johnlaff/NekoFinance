# Spec 018 — Write-back: pré-visualização do caminho inverso (transação → célula), atrás de flag

> Fonte: notas locais privadas + invariante de segurança do AGENTS.md ("toda escrita material no
> Sheets passa por diff + validação + aprovação humana"). Esta fatia entrega só o PREVIEW read-only;
> o envio real ao Google Sheets fica atrás de uma flag DESLIGADA até a fase bidirecional (ADR-0003).

## Problema

O import traz a planilha para o SQLite. O caminho inverso — uma transação voltar a ser a célula que
a originaria — precisa existir para a fase bidirecional, mas não pode escrever no Sheets sem diff e
aprovação. Entregamos primeiro o núcleo puro que descreve o que escreveria, sem escrever.

## Núcleo (puro, read-only, TDD)

- `plan_write_back(rows, layout, mappings, txns) -> Vec<CellWrite>`: para cada transação, encontra a
  célula-alvo (bloco do mês × coluna do tipo × linha do dia) e produz `{a1, current, proposed,
changed}`. PURO: não escreve nada.
- Geometria fiel: `col_to_a1` (base-26 bijetiva), `parse_day_cell` (o dia vem como float "1.0000"
  em planilhas reais → parse `f64`), `cents_to_ptbr` (round-trip com `parse_number`).
- Transações fora do ano da aba, mês sem bloco, tipo sem coluna mapeada ou dia sem linha são
  silenciosamente puladas (não há onde escrever).

## Trava de segurança

- `WRITE_BACK_ENABLED: bool = false`. `ensure_write_back_enabled()` falha cedo em toda rota que
  ESCREVE. O preview/diff funciona desligado (read-only).
- Quando ligado (fase futura): cada escrita material gera diff before→after, validação e aprovação
  humana (`ApprovalDiffCard`); `sync_log` por checksum detecta edição concorrente da planilha.

## UI

`WriteBackPreview` mostra o diff estruturado das células que seriam tocadas. Sem botão de envio
enquanto a flag estiver desligada.

## DoD

- Núcleo com testes (geometria, formatação pt-BR, dia em float, flag desligada nunca escreve).
- Nenhuma escrita real no Sheets nesta fatia.
- `npm run check` verde.
