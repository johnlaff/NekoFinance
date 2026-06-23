# Plan 059: Spec (5-type alignment) + classify note line-items by section

> **Executor instructions**: Follow step by step; run every verification command and confirm
> the expected result. Write the spec FIRST (Step 0). If a "STOP condition" occurs, stop and
> report. When done, update this plan's row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da2d3e9..HEAD -- src-tauri/src/google_sheets/import.rs`
> On any change, compare excerpts to live code before proceeding.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none (foundation for plans 060, 061, 062)
- **Category**: methodology / spec
- **Planned at**: commit `da2d3e9`, 2026-06-22 (design confirmed with the owner via interview)
- **Completed**: 2026-06-23 via PR #90 (`8fd40a960cdbb7ac21af79e45bc9153b858c7585`)

## Why this matters

The owner confirmed (from the methodology reference material and the real spreadsheet) that the
method has **5 first-class movement types** and that the Neko app currently honors only 4 (it
folds **Cartão** into fixed Saída and
keeps **Economia** inside cost-of-living). This plan writes the spec for aligning Neko to the
canonical 5-type model and builds the pure classifier that maps each note line-item to a type —
the foundation the engine (060), the Economia-tab write-back (062), and the UI (061) build on.

## 🔓 Owner decision recorded (reopens locked model)

The owner **explicitly authorized** reopening the locked finance model (plans 040↔046↔051/052,
`specs/011-engine-five-types`). The README "considered and rejected" note said only a new
explicit owner decision may reopen it — this is that decision. The spec (Step 0) must record it.

**Canonical model to implement (from the methodology reference material + the spreadsheet, cross-checked):**

- **5 types**: Entrada, Saída, Diário, **Cartão**, **Economia** (App enum strings, unaccented:
  `Entrada/Saida/Diario/Cartao/Economia`).
- **Custo de vida = Saídas fixas + Diário + Cartão (+ previsão de diário). EXCLUI economia e
  patrimônio.** (Verbatim from the app's own help text: "a soma de tudo que você gasta para se
  manter, sem contar o que foi separado para economias".)
- **Economia** is an outflow that lowers Saldo/Performance (money left the account) **but is
  excluded from custo de vida**, and feeds **Economia% = economia ÷ entradas** (target 20–30%).
- **Cartão** is a distinct bucket ("Gastos com cartão"), **inside** custo de vida, shown
  separately — NOT merged into fixed Saída.
- **Previdência / long-term locked investment = Patrimônio** (illiquid): **out of custo de
  vida AND out of the accessible Economia%** — a separate long-term bucket. (Owner's call:
  previdência is locked until retirement, so it is not "accessible savings".)
- **Invariant (regression)**: **Saldo and Performance do NOT change** (the money leaves the
  account in both models). Only custo-de-vida (stops being inflated by economia), Economia%
  (becomes automatic), and the now-visible Cartão/Patrimônio buckets change. Plan 060 owns the
  math + regression tests.

## Current state

- `src-tauri/src/google_sheets/import.rs`:
  - `parse_itemized_note(note) -> Vec<NoteLineItem>` (line ~906) already splits items and records
    the most-recent ALL-CAPS header as `section: Option<String>` per item.
  - `NoteLineItem` (line ~870): `amount_cents, description, position, section`.
  - `line_item` table has `section TEXT` (migration `20260621000003_line_item_section.sql`).
- Section vocabulary actually found in the real notes (use these, accent/case-insensitive):
  - `CONTAS` (and `Contas:`) → fixed Saída
  - `CARTÕES`/`CARTOES`, `FATURAS`/`FATURAS:`/`Fatura:` → **Cartão**
  - `INVESTIMENTO`/`Investimento:` → **Patrimônio** (previdência etc.)
  - `ECONOMIA` (does NOT exist yet — owner will start using it) → **Economia**
  - `OUTROS` → Saída; `AJUSTES`/`Ajuste` → reconciliation marker (Diferença)
  - ad-hoc one-offs (e.g. a person name, "Juros", "Anuidade Sicoob") → default Saída
- **Decided (owner):** classification is **by SECTION only — NO bank-name fallback** (too
  error-prone: "Amazon Prime" vs the Amazon card, "Inter" in "internet"). A line with no
  recognized section defaults to **Saída**.
- Convention: pure functions, no I/O, unit-tested in the file's `#[cfg(test)]` module (see the
  tests around `parse_itemized_note`).
- Project rule (`AGENTS.md`): spec under `specs/` first; TDD for methodology rules.

## Commands you will need

| Purpose         | Command                                                    | Expected |
| --------------- | ---------------------------------------------------------- | -------- |
| Rust check+test | `npm run rust:check`                                       | exit 0   |
| Targeted test   | `cargo test --manifest-path src-tauri/Cargo.toml classify` | pass     |
| Privacy scan    | `npm run privacy:scan`                                     | passes   |

## Scope

**In scope:**

- `specs/<NNN>-classificacao-notas-5-tipos/spec.md` (create; `ls specs/` for the next number) —
  record the canonical 5-type model, the owner decision reopening 051/052, the section→type
  table, the previdência=patrimônio rule, divergence=warn-only, and the Saldo/Performance
  invariant.
- `src-tauri/src/google_sheets/import.rs` (or a sibling `classify.rs` matching the module
  layout) — `ItemKind` enum + `classify_line_item` pure fn + tests.

**Out of scope (this plan):**

- Engine math / metrics / cost-of-living / Economia% (plan 060).
- Economia-tab write-back (plan 062). UI (plan 061).
- The bank-name fallback (explicitly dropped).
- Editing the spreadsheet or real data in spec/tests (synthetic examples only).

## Git workflow

- Branch: `advisor/059-classification-core`
- Message: `feat(import): 5-type note classifier (section→kind) + spec`

## Steps

### Step 0: Write the spec

Create `specs/<NNN>-classificacao-notas-5-tipos/spec.md` capturing the "Owner decision" +
"Canonical model" sections above (method-neutral, no private data, no real names). Explicitly
state it supersedes the economia=Saída framing of plans 051/052 per owner decision, and list the
downstream plans (060 engine, 061 UI, 062 Economia-tab write-back).

**Verify**: file exists; `npm run privacy:scan` → passes.

### Step 1: `ItemKind`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind { Saida, Cartao, Diario, Economia, Patrimonio, Ajuste }
```

(`Saida` = fixed Saída/CONTAS/OUTROS; `Ajuste` = AJUSTES/Diferença line.)

**Verify**: `cargo check --manifest-path src-tauri/Cargo.toml` → exit 0.

### Step 2: `classify_line_item(section, description) -> ItemKind`

Section-only mapping (normalize: trim, strip trailing `:`, ASCII-fold, lowercase):
`contas → Saida`; `cartoes`/`cartao`/`faturas`/`fatura → Cartao`; `investimento → Patrimonio`;
`economia → Economia`; `ajustes`/`ajuste → Ajuste`; `outros → Saida`; none/unknown → `Saida`.
No description/bank-name logic.

**Verify**: `cargo check ...` → exit 0.

### Step 3: Unit tests (TDD)

Cover: each section → its kind (accent/case variants `CARTÕES`/`CARTOES`/`FATURAS:`);
`INVESTIMENTO`/`Investimento:` → Patrimonio; `ECONOMIA` → Economia; `AJUSTES` → Ajuste;
no section → Saida; a bank name in the description with no section (e.g. "R$ 10 - Nubank") →
**Saida** (proves the fallback is intentionally absent).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml classify` → all pass.

## Test plan

- All Step-3 cases, named. Pattern: existing tests near `parse_itemized_note`.
- `npm run privacy:scan` passes.

## Done criteria

- [x] `specs/<NNN>-classificacao-notas-5-tipos/spec.md` exists, records the owner decision, data-free
- [x] `ItemKind` + `classify_line_item` compile; section-only (no bank fallback)
- [x] `cargo test ... classify` passes incl. the "no fallback" and Patrimonio/Economia cases
- [x] `npm run rust:check` exits 0; `npm run privacy:scan` passes
- [x] No files outside scope modified; `plans/README.md` row updated

## STOP conditions

- `NoteLineItem`/`parse_itemized_note` no longer records `section` → STOP.
- The spec format conflicts with existing `specs/*/spec.md` conventions in a way you can't match → STOP and ask.

## Maintenance notes

- kind→bucket mapping lives in plan 060 (engine), not here.
- If the owner adds new section names, extend the section map only.
- Reviewer: confirm there is NO bank-name fallback and that unknown sections default to Saída.
