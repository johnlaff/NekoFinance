# Plan 076: Tornar os comentários de código atemporais (remover meta-referências de processo)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5cb24d1..HEAD -- src/ src-tauri/src/`
> Drift aqui é esperado (o repo evolui); ele NÃO é STOP para este plano — os greps do
> Step 1 recomputam o inventário na hora. O STOP é só para os padrões não baterem em nada.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: LOW/MED
- **Depends on**: none
- **Category**: tech-debt (legibilidade)
- **Planned at**: commit `5cb24d1`, 2026-07-10
- **Issue**: https://github.com/johnlaff/NekoFinance/issues/152

## Why this matters

A regra do projeto é que comentário de código traga só o racional técnico, atemporal — sem
meta-referências de processo (plano, spec, review, PR, datas de descoberta). A varredura de
2026-07-10 achou ~290 ocorrências de `(plano NNN)`/`(Plan NNN)` em ~35 arquivos, 49 de
`spec NNN`, 24 narrando "review adversarial"/"review P0–P3" e TODOs rotulados
`plan-039-phase2`. Esses rótulos exigem contexto externo que não existe no repo público
(não há `plans/` rastreável a partir do código para um leitor de fora) e emaranham a
invariante técnica — que geralmente já está na mesma frase — com o episódio em que ela foi
descoberta. Removê-los deixa cada comentário sustentável por si só.

## Current state

Amostras representativas (o inventário completo vem dos greps do Step 1):

- `src-tauri/src/obligations.rs:1` — `//! Plan 069: user-confirmed "obligation" identity...`
- `src-tauri/src/conflicts.rs:1` — `//! Gate de conflito de import (spec 013): lista os conflitos pendentes...`
- `src-tauri/src/conflicts.rs:39` — `// edições mais novas — o bug que a review adversarial pegou.`
- `src/screens/dashboard/WriteBackPending.tsx:74` — `* Configurações (plano 028), sem reimplementar o diff/apply.`
- `src-tauri/src/os_scheduler.rs:118` — `// TODO plan-039-phase2: macOS launchd plist`
- `src-tauri/src/oauth/mod.rs:10-13` — racional válido de runtime aninhado terminando em
  `...(descoberto no 1º dogfooding, 2026-06-12)`.

Padrão da reescrita — três casos:

1. **Rótulo redundante** (a maioria): o racional técnico já está na frase; basta remover o
   parêntese/prefixo. Ex.: `// Parseia as linhas itemizadas de uma nota de célula (Plan 035).`
   → `// Parseia as linhas itemizadas de uma nota de célula.`
2. **Rótulo é o único conteúdo**: o comentário só diz `(spec 0NN)` — reescrever com a decisão
   em si. Ex.: `//! Tags livres (spec 014): nome + cor...` → `//! Tags livres: nome + cor...`
   e, se a spec era o único "porquê", uma frase com o porquê (ler a spec correspondente em
   `specs/` para extraí-lo).
3. **Narrativa de episódio**: manter a invariante, cortar o episódio. Ex.:
   `// ... — o bug que a review adversarial pegou.` → terminar na invariante.
   `(descoberto no 1º dogfooding, 2026-06-12)` → remover o parêntese inteiro.
   `TODO plan-039-phase2: macOS launchd plist` → `TODO(os_scheduler): macOS launchd plist`.

O que NÃO é violação (não tocar): strings de UI; testes citando fixtures; nomes de arquivos de
migração (datas ali são identidade); referências a specs/ADRs em DOCS (`docs/`, `specs/`,
`plans/`); comentários que citam `ADR-0001` (ADRs são docs de arquitetura versionadas e
rastreáveis — referência legítima, diferente de plano de sessão); anos-rótulo de abas da
planilha (ex.: `"2025"`, `"2026"` como nome de aba é vocabulário de domínio).

## Commands you will need

| Purpose   | Command             | Expected on success |
|-----------|---------------------|---------------------|
| Typecheck | `npm run typecheck` | exit 0              |
| Lint      | `npm run lint`      | exit 0              |
| Tests     | `npm run test:run`  | all pass            |
| Rust      | `npm run rust:check`| exit 0              |
| Gate      | `npm run check`     | exit 0 (rodar 1× no final) |

## Scope

**In scope**:
- Comentários (`//`, `///`, `//!`, `/* */`, `{/* */}`) em `src/**` e `src-tauri/src/**`,
  incluindo os blocos `#[cfg(test)]` (comentários de teste também devem ser atemporais).
- `scripts/` — apenas se o Step 4 (guarda de CI) for implementado como script.

**Out of scope** (do NOT touch):
- Qualquer CÓDIGO executável — este plano muda somente comentários. Nenhuma linha fora de
  comentário pode mudar (nem formatação).
- `docs/`, `specs/`, `plans/`, `README*`, `CHANGELOG` — referências de processo em docs são
  legítimas.
- Strings literais (UI, mensagens de erro, fixtures).
- `src/design-system/_ds_bundle.js` e artefatos gerados.

## Git workflow

- Branch: `advisor/076-comentarios-atemporais`
- Um commit por diretório lógico (ex.: `chore: comentários atemporais em google_sheets/`)
  facilita revisão; mensagem sem meta-referência de processo.
- PR ao final; merge somente com CI verde.

## Steps

### Step 1: Inventário

Gerar a lista viva (números podem divergir da varredura de 2026-07-10; a lista manda):

```bash
grep -rnE "(plan|plano)[ -]?0?[0-9]{2,3}" src/ src-tauri/src/ --include="*.rs" --include="*.ts" --include="*.tsx" | grep -vE "specs?/|plans/" > /tmp/inv-plan.txt
grep -rnE "spec ?0[0-9]{2}" src/ src-tauri/src/ --include="*.rs" --include="*.ts" --include="*.tsx" > /tmp/inv-spec.txt
grep -rniE "review (adversarial|P[0-3])|adversarial review|\(review\)" src/ src-tauri/src/ > /tmp/inv-review.txt
grep -rnE "dogfooding|descoberto (no|em) " src/ src-tauri/src/ > /tmp/inv-episodio.txt
wc -l /tmp/inv-*.txt
```

**Verify**: os quatro arquivos existem e a soma é > 200 linhas (ordem de grandeza da varredura).
Se a soma for < 20, alguém já fez a limpeza — STOP e reporte.

### Step 2: Reescrita, arquivo a arquivo

Para cada arquivo do inventário, aplicar os três casos do "Current state". Regras duras:
- NUNCA find-and-replace cego; ler o comentário inteiro e preservar/extrair o racional.
- Se remover o rótulo deixa o comentário vazio de porquê, buscar o porquê na spec/plan citado
  (estão em `specs/` e `plans/` no repo) e escrevê-lo em 1 frase.
- Comentário cuja única função era histórica ("antes era X") e que não protege nenhuma
  invariante atual: deletar o comentário inteiro.
- Rodar `npm run typecheck` (ou `npm run rust:check` p/ Rust) a cada ~10 arquivos.

**Verify** (ao final): re-rodar os greps do Step 1 → `wc -l` = 0 em inv-plan, inv-spec,
inv-review e inv-episodio (exceto matches legítimos documentados: `ADR-0001` e anos-rótulo
de aba; se sobrarem, listar cada um no PR com justificativa de 1 linha).

### Step 3: Confirmar zero mudança de código

```bash
git diff --ignore-all-space main...HEAD -- . ':!*.md' | grep -E "^[+-]" | grep -vE "^[+-]{3}" | grep -vE "^[+-]\s*(//|///|//!|\*|/\*|\{?/\*)" | head -20
```

**Verify**: saída vazia (toda linha alterada é comentário). Qualquer linha de código no diff
é STOP.

### Step 4: Guarda contra reintrodução

Adicionar ao gate existente (onde `npm run check` agrega os passos — ver `package.json`,
script `check`) um passo grep que falha se os padrões voltarem, ex. script
`scripts/comment-hygiene.sh` chamado pelo `check`:

```bash
#!/usr/bin/env bash
set -uo pipefail
hits=$(grep -rnE "(plano|plan) ?0?[0-9]{2,3}\)|spec ?0[0-9]{2}\)|review adversarial" src/ src-tauri/src/ --include="*.rs" --include="*.ts" --include="*.tsx" | grep -vE "specs?/|plans/" || true)
if [ -n "$hits" ]; then echo "Comentários com meta-referência de processo:"; echo "$hits"; exit 1; fi
echo "comment-hygiene ok"
```

Atenção ao exit code do grep sem match (1 é o caso BOM — por isso o `|| true` + teste de
string, nunca `grep -q` direto em condicional invertida).

**Verify**: `bash scripts/comment-hygiene.sh` → `comment-hygiene ok`, exit 0; e
`npm run check` → exit 0.

## Test plan

- Nenhum teste novo: mudanças são 100% em comentários (Step 3 prova isso mecanicamente).
- Gate integral no final: `npm run check` → exit 0.

## Done criteria

- [ ] Greps do Step 1 → 0 matches não-justificados
- [ ] Step 3 → diff contém apenas linhas de comentário
- [ ] `npm run check` exit 0
- [ ] `scripts/comment-hygiene.sh` integrado ao gate e verde
- [ ] Linha do plano 076 atualizada em `plans/README.md`

## STOP conditions

- Inventário do Step 1 com soma < 20 (trabalho já feito por outra sessão).
- Qualquer linha de código (não-comentário) aparecer no diff do Step 3.
- Um comentário cuja reescrita exigiria decidir semântica de produto (não só extrair o
  racional) — liste-o no relatório e siga para o próximo.

## Maintenance notes

- A guarda do Step 4 é grep, não parser: comentários multi-linha criativos podem escapar.
  Aceitável — o objetivo é impedir o padrão dominante, não perfeição.
- Revisor do PR: amostrar ~20 reescritas comparando com o original — o risco real deste plano
  é perder um "porquê" na remoção do rótulo, não quebrar build.
