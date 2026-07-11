# Plan 075: Corrigir drift de docs e dar camada didática à régua "Reserva após financiar"

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 5cb24d1..HEAD -- SESSION-CONTEXT.md docs/version-matrix.md src/screens/scenarios.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs + dx (didática de UI)
- **Planned at**: commit `5cb24d1`, 2026-07-10
- **Issue**: https://github.com/johnlaff/NekoFinance/issues/151

## Why this matters

`SESSION-CONTEXT.md` é o prompt de retomada colado no início de sessões novas; hoje ele
subestima o schema (diz 36 migrações; são 41), as specs (diz 20; são 21) e silencia sobre a
feature de cenários "e se" inteira (migrações + módulo Rust + tela já entregues) — exatamente o
tipo de omissão que faz uma sessão nova retrabalhar. `docs/version-matrix.md` declara `aes-gcm 0.10`
enquanto o `Cargo.toml` fixa `0.11` — a matriz existe para decisões de upgrade, e um número errado
nela é pior que ausente. Por fim, no compare de cenários a linha "Reserva após financiar" é o único
termo com carga de método sem o `InfoPopover` didático que todos os KPIs vizinhos do mesmo card têm —
o usuário vê "Confortável · 9,4 meses" sem saber o que a régua significa.

## Current state

- `SESSION-CONTEXT.md:33` — texto atual (errado nos dois números):

  ```
  - 36 migrações SQL em `src-tauri/migrations/`. 20 specs em `specs/001` a `specs/020`.
  ```

  Contagens reais: `ls src-tauri/migrations/*.sql | wc -l` → 41; specs vão até
  `specs/021-performance-previsao-diario/`. O arquivo não contém a palavra "cenário" em lugar
  nenhum (`grep -c "cenário\|scenario" SESSION-CONTEXT.md` → 0), embora existam
  `src-tauri/src/scenarios.rs`, `src/screens/scenarios.tsx`, `src/lib/scenarioHelpers.ts` e as
  migrações `20260623000001_scenario.sql`, `...002_transaction_scenario_id.sql`,
  `...003_scenario_override.sql`, `20260624000001_scenario_override_hardening.sql`.
  `docs/architecture.md` já descreve a feature ("What-if scenarios ... surfaced in Horizonte
  with didactic compare") — use-o como referência de fraseado.

- `docs/version-matrix.md:66` — célula errada:

  ```
  | At-rest encryption      | `aes-gcm`     | `0.10`        | Encrypts cached OAuth tokens. ... |
  ```

  Real: `src-tauri/Cargo.toml:47` → `aes-gcm = "0.11"`.

- `src/screens/scenarios.tsx` — o padrão didático dos KPIs do compare (linhas ~1147–1151):

  ```tsx
  <span className="scn-kpi__label">
    <InfoPopover term={term} hideMarker>
      {label}
    </InfoPopover>
  </span>
  ```

  `InfoPopover` já está importado no arquivo (linha 71,
  `import { InfoPopover } from "../design-system/components/InfoPopover";`) e recebe
  `term={{ title, body }}`.

- `src/screens/scenarios.tsx` — o bloco do empréstimo SEM a camada didática (linhas ~1467–1474):

  ```tsx
  {
    compare.loan.reserve_months_after_financing != null && (
      <div className="scn-loan-summary__row">
        <span>Reserva após financiar</span>
        <ReserveMonthsBadge months={compare.loan.reserve_months_after_financing} />
      </div>
    );
  }
  ```

- A régua que o badge aplica (`reserveMonthsState`, linhas ~1019–1050): `< 6` → "Abaixo do
  mínimo"; `6–8` → "Zona amarela"; `8–12` → "Confortável"; `> 12` → "Paz".

- Convenção de copy do produto: sentence case, primeira letra maiúscula; tom didático mas nunca
  moralizante (`PRODUCT.md`, princípio "Friendly guidance").

## Commands you will need

| Purpose   | Command             | Expected on success   |
| --------- | ------------------- | --------------------- |
| Typecheck | `npm run typecheck` | exit 0                |
| Lint      | `npm run lint`      | exit 0                |
| Tests     | `npm run test:run`  | all pass              |
| E2E       | `npm run e2e`       | all pass (ver Step 3) |

## Scope

**In scope** (the only files you should modify):

- `SESSION-CONTEXT.md`
- `docs/version-matrix.md`
- `src/screens/scenarios.tsx`
- `tests/e2e/scenario-visual.spec.ts-snapshots/*` (somente se o Step 3 exigir regeneração)

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/scenarios.rs` — a SEMÂNTICA da régua (numerador/denominador) está em decisão
  no issue #150; este plano só adiciona a explicação do que a régua significa, não muda cálculo.
- `README.md`, `docs/architecture.md` — já corretos.
- Qualquer outro texto de `SESSION-CONTEXT.md` além dos pontos citados.

## Git workflow

- Branch: `advisor/075-precisao-didatica`
- Commits no estilo do repo (pt-BR, imperativo, sem referência a processo): ex.
  `docs: contagens e cenários no SESSION-CONTEXT; aes-gcm 0.11 na matriz`
- Abrir PR ao final (nunca push direto na main); merge somente com CI verde.

## Steps

### Step 1: Corrigir SESSION-CONTEXT.md

Na linha 33, trocar por:

```
- 41 migrações SQL em `src-tauri/migrations/`. 21 specs em `specs/001` a `specs/021`.
```

E adicionar ao bloco "O que o app é hoje" (logo após o bullet do motor de forecast) um bullet
sobre cenários, no espírito de `docs/architecture.md` — por exemplo:

```
- Cenários "e se" (what-if): lançamentos hipotéticos, override de obrigações e simulação de
  financiamento, isolados por `scenario_id` (`src-tauri/src/scenarios.rs`), com compare didático
  na tela Horizonte (`src/screens/scenarios.tsx`).
```

**Verify**: `grep -n "41 migrações" SESSION-CONTEXT.md && grep -c "scenario" SESSION-CONTEXT.md` → linha encontrada; contagem ≥ 2.

### Step 2: Corrigir docs/version-matrix.md

Na linha 66, trocar `` `0.10` `` por `` `0.11` `` (só a célula de versão da linha `aes-gcm`).

**Verify**: `grep -n "aes-gcm" docs/version-matrix.md` → mostra `0.11`; `grep -n '`0.10`' docs/version-matrix.md` → sem match na linha do aes-gcm.

### Step 3: InfoPopover em "Reserva após financiar"

Em `src/screens/scenarios.tsx`, envolver o rótulo no mesmo padrão dos KPIs:

```tsx
<span>
  <InfoPopover
    hideMarker
    term={{
      title: "Reserva após financiar",
      body: "Quantos meses de custo de vida a sua reserva cobriria depois de assumir o financiamento. A régua: abaixo de 6 meses é abaixo do mínimo; de 6 a 8, zona amarela; de 8 a 12, confortável; acima de 12, paz — folga de sobra para financiar sem ansiedade.",
    }}
  >
    Reserva após financiar
  </InfoPopover>
</span>
```

IMPORTANTE: o body explica o SIGNIFICADO da régua (a escada 6/8/12), não a fórmula de cálculo —
a derivação está em revisão no issue #150 e o texto não pode fixá-la.

**Verify**: `npm run typecheck && npm run lint && npm run test:run` → exit 0.
Depois `npm run e2e`: se APENAS os snapshots `compare-*.png` de
`tests/e2e/scenario-visual.spec.ts` falharem por diferença visual no rótulo do empréstimo,
regenerar com `npx playwright test scenario-visual --update-snapshots`, reexecutar `npm run e2e`
(verde) e inspecionar visualmente o novo `compare-dark` confirmando que só o rótulo mudou.
Qualquer outra falha de e2e é STOP.

## Test plan

- Sem teste unitário novo: Steps 1–2 são docs; Step 3 é apresentação pura (sem lógica),
  coberta pelo snapshot e2e do compare — o critério de regressão visual já existente.
- Rodar a suíte inteira (`npm run test:run`) para garantir zero quebras.

## Done criteria

- [ ] `npm run typecheck` exit 0
- [ ] `npm run lint` exit 0
- [ ] `npm run test:run` exit 0
- [ ] `npm run e2e` exit 0 (com snapshots regenerados somente se o Step 3 exigiu)
- [ ] `grep -n "36 migrações" SESSION-CONTEXT.md` → sem match
- [ ] `git status` — nenhum arquivo fora do escopo modificado
- [ ] Linha do plano 075 atualizada em `plans/README.md`

## STOP conditions

- O excerto do bloco do empréstimo em `scenarios.tsx` não bate com o código atual (issue #150
  pode ter mudado a régua antes deste plano rodar).
- `npm run e2e` falha em qualquer spec que não seja o snapshot do compare.
- O `InfoPopover` exigir prop diferente de `term={{title, body}}` (a assinatura mudou).

## Maintenance notes

- Quando o issue #150 fixar a semântica da régua, revisar o body do popover para citar a
  derivação correta (hoje ele deliberadamente não a menciona).
- Revisor do PR: conferir que o texto do popover não moraliza e segue sentence case.
- SESSION-CONTEXT.md continua propenso a drift de contagens; considerar (fora deste plano)
  remover números absolutos dele.
