# Plan 074: Didactic scenario compare — verdict first, method states, numbers as evidence

> **Executor instructions**: follow the slices in order (each = one PR: executor → adversarial
> review → CI green → merge). Read the live code — line numbers below were verified at
> `dc0b3cf` and will drift. The approved visual direction is the "074-direcao-didatica"
> mock (see Status); reproduce its hierarchy, not its pixel values.

## Status

- **Priority**: P0 (user-reported: values clip the cards; chart label collides with its line;
  "toda UI/UX está bem pobre e nada didática")
- **Effort**: M total (3 slices: S + M + S)
- **Depends on**: 072 (merged — the compare surface exists)
- **Planned at**: commit `dc0b3cf`, 2026-07-08
- **Inputs**: impeccable critique (Assessment A design review + Assessment B detector/measurements),
  2026 market/SOTA research, method-vocabulary extraction from the raw sources, and a
  code-level defect inventory. Direction mock approved-pending (claude.ai artifact
  `72c43461`, label `074-direcao-didatica`).

## Why this matters

At the exact moment of highest stakes ("posso me comprometer com esse financiamento?") the
compare surface shows clipped digits, a line drawn through its own label, and identical
"• R$ 0,00" pills — it reads as UNFINISHED, the worst failure mode for a brand whose promise
is "precisa, discreta, confiável". And the one sentence that answers "é seguro?" already
exists in code — visible only to screen readers (the aria-live region).

The fix is not decoration; it is hierarchy. Target model (mock): **Nível 1** a one-line
verdict; **Nível 2** each KPI card leads with a METHOD STATE (word + icon + color — the DS
rule "status is never color-only"); **Nível 3** numbers demoted to compact evidence with
full precision preserved.

**Method fidelity (hard constraint)**: every state label below is verbatim method vocabulary
already implemented in the codebase — `performanceStatus`/`custoVidaStatus` (totaisStatus.ts:
"Sobrou dinheiro"/"Faltou dinheiro"; "Dentro da renda"/"Acima da renda") and `saldoBand`
(saldoHeatmap.ts — the canonical ABSOLUTE Termômetro bands; NEVER make them relative). Copy
tone: "vermelho é GPS, não ameaça". Nothing invented.

## Measured defects (Assessment B — the arithmetic that must not regress)

- **Card overflow**: worst-case row = real@13px (~94px) + arrow (12) + gaps (12) + scenario@17px
  (~122) ≈ 240px vs 156px available in a 190px card (190 − 32 padding − 2 border) → 54% over.
  Both `Money` spans are `white-space: nowrap`; no wrap/shrink strategy exists
  (scenarios.css ~:279-315, Money.tsx:75).
- **Label collision**: both chart labels share x-anchor `x(last) − 4` with fixed ±8/+14 y-offsets
  and no collision logic (scenarios.tsx ~:1100-1119); converging lines at the horizon put
  "Simulação" on top of a polyline.
- **Type scale drift**: 24 hardcoded font-sizes on this surface, 5 values off the DS scale
  (12.5px ×5, 17px, 10px); dead rule `.scn-diffchart__worst` (css ~:423).
- **Glyph bug**: delta arrow derives from the RAW SIGN (`deltaCents > 0 ? "▲" …`,
  scenarios.tsx ~:792), not from the computed better/worse — the same ▲ means "good" on one
  card and "bad" on the next, distinguished only by hue (violates the DS color-only rule).
- Contrast: current tokens pass AA (label 5.65:1, chips 5.4–7.2:1; the violet replace-chip is
  marginal at 4.60:1 — do not darken it further).

## Slice A — P0 mechanical fixes (S) · branch `advisor/074a-compare-p0`

1. **Card row restructure (kills the overflow structurally)**: new card anatomy per the mock —
   headline value on its own line (compact format, see 2), evidence line
   `real → scenario` in one small muted row (`--fs-label`, full precision, `flex-wrap: wrap;
min-width: 0` as safety), delta chip on its own line. Never two full values on one
   nowrap line again.
2. **Compact BR headline formatter**: `fmtCompactBRL(cents)` in nkFormat.ts —
   `>= R$ 10.000` → "R$ 30,8 mil"; `>= R$ 1.000.000` → "R$ 1,2 mi"; below, full value without
   trailing ",00" (BR style: words "mil"/"mi", NEVER "k"/"M"; dot thousands, comma decimals).
   Unit-test the bands + rounding. Full precision stays on the evidence line (and in
   aria-labels).
3. **Delta discipline**: `|delta| <= 100` cents → render quiet text "≈ sem mudança"
   (`--text-faint`, no pill) — kills both the "• R$ 0,00" noise and the −R$ 0,09 red alarm.
   Material deltas: glyph from **better/worse** (lucide `TrendingUp`/`TrendingDown`,
   strokeWidth 1.75 — replaces the raw-sign ▲▼• Unicode), color + word together.
4. **Chart label gutter**: reserve a right gutter = label width (measure "Simulação" at 11px
   ≈ 62px → gutter 72px) so end labels start AFTER the last point (`x(last) + 12`,
   `textAnchor="start"`); add an SVG halo (`stroke: var(--surface); stroke-width: 3;
paint-order: stroke`) as second defense; y-offsets keep direction-aware but clamp so the
   two labels never overlap (min 14px separation). Add a legend row (top-right chips) as
   redundancy.
5. Remove dead `.scn-diffchart__worst`; swap the 24 hardcoded font-sizes to `var(--fs-*)`
   tokens (12.5px → `--fs-label`(12) or `--fs-sm`(13) — pick per context and note it;
   17px → `--fs-title`(16); SVG `fontSize="10"` → 11).

- **Done**: repro spec with the REAL magnitudes ("R$ 30.840,59") asserts via boundingBox that
  every value fits its card at 1280px AND with the sheet open; a label-collision e2e assert
  (label x > last polyline x); vitest/lint/tc/format/doctor/e2e/check green.

## Slice B — the didactic layer (M) · branch `advisor/074b-didactic-states`

1. **Verdict banner (Nível 1)**: promote the existing sr-only sentence to a visible banner
   above the KPI grid (keep the aria-live region — the banner is its visual twin). Verdict
   logic (deterministic, unit-tested):
   - scenario `deepest_deficit < 0` → risk: "Fura o caixa em {mês} — faltam {R$ X}." +
     GPS-tone subline (suggest: antecipar entrada / reduzir parcela / cobrir com empréstimo).
   - else → ok: "Este cenário se mantém no azul o ano todo." + subline with menor saldo +
     Termômetro band word.
     Icon + word + color (✓ jade / ▲ danger), border-tinted card (mock).
2. **State-first cards (Nível 2)** — state line is the hero (icon + word + band color).
   **STATE TRANSITIONS are the headline device** (owner-refined after mock review): compute
   the state for BOTH branches; when they differ, the hero is the NEW (scenario) state
   (icon + word + color) with the origin on a small muted line below it — `Antes: Dentro da
renda` — **STACKED, never inline** (states are long words; inline `old → new` wraps
   ugly in a narrow card with an orphaned icon — owner-rejected; the inline `old → new`
   format is reserved for the short numeric evidence line). This is the compare's real "aha"
   and it also resolves the hero-number ambiguity (the hero becomes the state, and the
   compact number is unambiguously the SCENARIO value beneath it). When states are equal,
   render the single state as in the mock. There is ONE card per metric always — the
   transition is a rendering MODE of that card, not an extra card. Unit-test both renderings.
   Per-metric states:
   - Buraco do futuro & Saldo no fim → `saldoBand()` labels/colors (Folga/Ok/Apertado/
     Negativo/Crítico — canonical absolute bands).
   - Performance → `performanceStatus()` ("Sobrou dinheiro"/"Faltou dinheiro"); evidence line
     = real → scenario like every other card. The "nasce no vermelho e esverdeia" narrative
     lives ONLY in the InfoPopover (owner-reviewed decision: verdict + consolation on the same
     card cancel each other out). **Card-copy rule**: text in the card body must be
     DATA-DERIVED for this situation (e.g. Pode gastar's "Limitado pela régua…" comes from
     `binding_guardrail`); CONSTANT concept explanations belong in the popover, never the card.
   - Custo de vida → `custoVidaStatus(cost, income)` ("Dentro da renda"/"Acima da renda").
     **Backend addition**: ScenarioCompareDto gains `real_income_cents` +
     `scenario_income_cents` (the month's Entradas — already computed in the metric events;
     expose, don't re-derive). Mirror in api.ts.
   - Pode gastar hoje → state from value+guardrail: `> 0` → "Livre até {compact}";
     `== 0 && binding_guardrail == "savings"` → "Segure hoje" + evidence "Limitado pela régua
     de poupança (20–30% ao ano), não pelo caixa."; `== 0 && cash` → "Segure hoje" + caixa.
   - **Red discipline (SOTA + método)**: full danger red only on real threshold breaches
     (Termômetro negative bands, "Faltou dinheiro", "Acima da renda", deficit verdict);
     worse-but-within-limits deltas use the quiet worse-tint chip only.
3. **Card order by decision priority** (SOTA Z-pattern): Buraco do futuro, Saldo no fim,
   Pode gastar hoje, Performance, Custo de vida.
4. **Copy guards**: the verdict subline and the first card must not repeat the same literal
   sentence (verdict frames the decision; the card holds the evidence). Known tension to
   MONITOR in dogfooding (not to change now): "Faltou dinheiro" mid-month is method-faithful
   (the official surface shows it too — remaining daily projection counts as outflow) but may
   read alarmist early in the month; if dogfooding confirms, the relief valve is popover copy,
   NEVER softening the verbatim state.

- **Done**: vitest for every state mapping (each band/threshold + both verdicts); the states
  and verdict match `totaisStatus.ts`/`saldoHeatmap.ts` outputs verbatim (assert against the
  helpers, not string copies); a11y — banner is not color-only, aria-live still announces;
  gates + e2e green.

## Slice C — polish (S) · branch `advisor/074c-compare-polish`

1. "O que mudou": color each row's amount by movement kind via `TYPE_META` (imported but
   unused there); keep op-chips as-is.
2. Loan summary: `reserve_months_after_financing` gets the method's semáforo (< 6 below-min
   danger word; 6–8 "amarela"; 8–12 ok; 12+ "paz" jade) — word + icon + color.
3. DiffSparkline title/legend polish; ensure worst-month note uses the same compact format.
4. **Naming collision + BR-style violation found in slice A**: `src/lib/format.ts` already
   exports a DIFFERENT `fmtCompactBRL` in "k"/"M" style ("R$ 5.8k") used by
   `BalanceTrajectory.tsx` axis labels — user-visible English-style abbreviation, violating
   the BR rule ("mil"/"mi", never "k"). Rename the old one (e.g. `fmtAxisBRL`) AND convert
   its output to the BR compact style via the new `nkFormat.fmtCompactBRL` (axis ticks may
   need the shorter no-decimals variant — verify chart density), updating BalanceTrajectory
   and its tests.

- **Done**: gates + e2e green; screenshot review at 1280/1600px with sheet open and closed.

## Cross-slice rules

- Method vocabulary only; Termômetro thresholds ABSOLUTE (never relative — owner-corrected
  rule); tone GPS-não-ameaça; PT-BR labels start with a capital.
- Money via `<Money>`/`<SignedMoney>` (`size="inherit"` where the wrapper owns the font);
  money never animated; states never color-only.
- Update the permanent e2e spec (scenario-sheet.spec.ts) rather than only adding new specs
  when an assertion fits there.

## STOP conditions

- If a state label would need NEW method vocabulary (not in totaisStatus/saldoHeatmap/the
  verbatim set above) → STOP and report; do not invent method terms.
- If exposing income on the DTO requires touching `project_with_metrics`'s signature → STOP
  (compute in the command layer instead).
