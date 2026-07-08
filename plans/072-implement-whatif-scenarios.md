# Plan 072: Implement what-if scenarios (successor to the 067 spike) — sliced

> **Successor to the design-only spike.** The full design (schema, engine call shape,
> compare DTO, UI, resolved open questions, regression tests) lives in
> **`plans/067-spike-design.md`** — READ IT FIRST; this plan only sequences the build into
> vertical PR-sized slices and states each slice's scope/done-criteria. Do not re-derive the
> design; follow the doc.
>
> **Planned at**: commit `232a2eb`, 2026-07-05 (068/069/070 merged).
> **Drift check (per slice)**: `git diff --stat 232a2eb..HEAD -- src-tauri/src/forecast/mod.rs src-tauri/src/commands/forecast_cmds.rs src-tauri/migrations/ src/lib/api.ts`

## Status

- **Priority**: direction / P1
- **Effort**: L+ (three slices)
- **Depends on**: 068 + 069 (merged) — `.xlsx` itemization + the `obligation` resolver the override consumes.
- **Category**: feature
- **Adherence**: verified against the method's RAW sources (original teaching transcripts + the owner's live spreadsheet, both local-only, 2026-07-05) — see `067`'s Status. Method-neutral copy (public repo).

## Slice order (each = one PR, executor → review → CI green → merge)

### Slice A — Data model + `scenario_id` isolation (FOUNDATION, highest-risk) · Effort M
The one that MUST be perfect: a single missed read leaks hypothetical rows into the real forecast.
- Migrations: `scenario` table (067-spike-design §2.1); `ALTER TABLE "transaction" ADD COLUMN scenario_id` (§2.2, nullable, FK→scenario, index); `scenario_override` table (§2.3: nullable `obligation_id` FK→obligation, `recurrence_id`, `op IN ('suppress','replace')`, `from_date`).
- Add `AND scenario_id IS NULL` (or the equivalent) to **every** read over `"transaction"` enumerated in §2.4 (`projection_seed`, `forecast_horizon_end`, `load_cashflow_events`, `load_forecast_events`, `load_metric_events`, `load_realized_month_events`, `realized_annual_savings`, `load_economia_annotation`, `reserve_floor`, …) **and every write-back path** in §2.5 (scenario rows NEVER reach Sheets).
- **Done**: `cargo test` green; a NEW regression test proving a seeded `scenario_id`-tagged row does NOT change `get_forecast`/the real projection or any metric; `npm run check` green. NO new UI, NO CRUD yet.

### Slice B — CRUD + `get_scenario_forecast` + override loader · Effort M/L
- CRUD commands (§6 IN): `create_scenario`, `list_scenarios`, `delete_scenario`, `add_scenario_transaction`, `delete_scenario_transaction`, `set_scenario_override`; register in `lib.rs`.
- `get_scenario_forecast(scenario_id)` (§3): build `real_events ∪ hypothetical_events`, call `project_with_metrics` twice (same seed), return `ScenarioCompareDto` (§4d: month_end, deepest_deficit, performance, safe_to_spend, binding_guardrail, **cost_of_living**, the `diff` deltas, the `changes` list, and the loan/CET breakdown).
- The **line-item-scoped raw-row override loader** (§3.3): superset-select variant of `load_cashflow_events` that also selects `t.id/description/recurrence_id`; for each override, reduce the day's event by the matched `line_item` (069's resolver) — drop only if it hits 0; never drop a sibling; `suppress` drops the series. Deterministic loan finance tool (PRICE + CET), principal Entrada + n parcelas.
- **BEFORE shipping any CRUD that can create a scenario row, extend the `scenario_id IS NULL` isolation** (slice A deferred these because they are dormant with no scenario rows in production — they become live the moment CRUD exists):
  - The write-back **audit/rekey path** in `write_back_cmds.rs` — `record_write_back_audit`, `rekey_manual_row_to_deterministic`, `realign_saida_cell`, `realign_credit_lump` read/update `"transaction"` by date+type without a scenario filter. Unfiltered, a scenario row sharing a date/type with a real cell write could have its `source_amount` overwritten OR be rekeyed into a deterministic sheet-row id and registered in `sync_log` (masquerading as an imported real row). Filter all of them.
- **Done**: the §8 Rust regression tests (determinism, idempotency, no-double-count, siblings-unaffected, PRICE table, principal-raises-seed, scenario isolation); **extend slice A's isolation test to also snapshot the annual metrics** (`realized_annual_savings`, `realized_annual_economia`, `projected_annual_savings`) so a leak there is caught (slice A's test snapshots forecast/dashboard/month_grid/write-back but not the annual/Totais reads); DTO mirrored in `src/lib/api.ts`; `npm run check` green.

### Slice C — Frontend: side-sheet in Horizonte + teaching layer · Effort M/L
- A side-sheet (not a modal) in `HorizonteScreen.tsx` (§5): pick/create scenario; add hypothetical rows; the **override** action on a real recurring row (alterar valor = replace / remover = suppress) with the mandatory affected-occurrences preview; the **decomposed loan control** (valor / parcelas / juros a.m. / data 1ª parcela; PRICE + custo do crédito/CET; principal Entrada that lifts the balance).
- The compare surface: the 5 canonical KPI cards (Buraco do futuro, Saldo no fim, **Custo de vida**, Performance, Pode gastar) with real→scenario + deltas; the dual-line chart (solid jade real vs dashed violet scenario); the **difference-AREA sparkline**; the **"O que mudou"** list. Port the visuals + interactions from the published prototype (`scratchpad/neko-scenario.html`).
- The **teaching layer** (§5.7): the method terms carry an on-demand explanation (dotted-underline term → popover, `aria-describedby`, dismissible), method-neutral copy, verified definitions (Performance includes economia + previsão; reserva = custo de vida × N meses).
- **Extend `scenario_id IS NULL` isolation to the real-ledger UI reads** (slice A deferred these — they're the surfaces that would otherwise show scenario rows to the user): `recent_transactions` (Livro-razão listing, transactions.rs) and `upcoming_bills_inner` (dashboard "contas a vencer", transactions.rs). The scenario side-sheet is the ONLY place scenario rows appear.
- Accessibility (§7.9): chart text/table equivalent + ARIA live region on recompute; reduced-motion.
- **Done**: `npm run check` green; Playwright smoke green; React Doctor 0 new; read-only (no write-back, no `account.balance` change).

## Cross-slice rules
- Read-only: a scenario NEVER writes to Google Sheets and never mutates `account.balance` or realized data.
- Match repo conventions (functional-core/imperative-shell; parse at boundaries; deterministic finance tools with tests — no LLM math).
- CET/PRICE labeled a BR-market aid, not a method rule. Method-neutral copy.
- Per slice: executor in a worktree → tech-lead review + adversarial review → PR → **CI green** → merge (never push to main directly).

## STOP conditions
- Slice A: if any read over `"transaction"` cannot be cleanly filtered by `scenario_id` (e.g. a raw join that would need restructuring), STOP and report — do not ship a partial filter that leaks.
- If the override cannot be made line-item-scoped without dropping siblings (069 resolver gap), STOP.
- If `project_with_metrics` needs a signature change for the second call, STOP (the spike says it must not).
