# Plan 044: Parity views — full-year 12-month grid + dual-year Economia side-by-side

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
>
> ```
> git diff --stat d3922d2..HEAD -- \
>   src/screens/AnnualScreen.tsx \
>   src/screens/dashboard/MonthLedgerCard.tsx \
>   src/lib/api.ts \
>   src/lib/saldoHeatmap.ts \
>   src/shell/AppShell.tsx \
>   src/App.tsx
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: LOW
- **Depends on**: none
- **Category**: feature
- **Package**: E
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

The spreadsheet has two views that Neko currently does not replicate, causing
direct function-loss for the user:

1. **Full-year 12-month grid** — the primary spreadsheet view shows all 12
   months as side-by-side Data/Entrada/Saída/Diário/Saldo blocks on one
   scrollable sheet ("ano inteiro de uma vez"). Neko only offers a single-month
   `MonthLedgerCard` on the Dashboard. Without the year-at-a-glance grid, the
   user must click through 12 months to see the yearly day-level picture they
   had in the spreadsheet.

2. **Dual-year Economia comparison** — the spreadsheet's savings tab shows two
   consecutive years (e.g. 2025 | 2026) of Entradas/Economia/% side-by-side so
   the user can compare savings rates year-over-year at a glance. `AnnualScreen`
   only shows one year at a time; the multi-year comparison is completely absent.

Both views use data that is already in the database (`get_month_grid` per month,
`get_annual_metrics` per year). This plan adds the two views and wires them into
the existing navigation, restoring full parity with the spreadsheet's diagnostic
value.

## Current state

### Feature 1 — full-year 12-month grid

**What exists**: `src/screens/dashboard/MonthLedgerCard.tsx` (224 lines) renders
the day-level grid for ONE month at a time with a `MonthNav` selector:

```tsx
// MonthLedgerCard.tsx:41-57
export function MonthLedgerCard({
  today,
  reloadKey = 0,
}: {
  today: string;
  reloadKey?: number;
}) {
  const todayYm = today.slice(0, 7);
  const [ym, setYm] = useState(todayYm);
  const [year, month] = ym.split("-").map(Number);
  const monthName = monthNamePtBR(`${ym}-01`);
  const monthCap = monthName.charAt(0).toUpperCase() + monthName.slice(1);
  const gridQ = useCommand(`month_grid:${ym}:${reloadKey}`, () =>
    getMonthGrid(year!, month!),
  );
```

It calls `getMonthGrid(year, month)` — the existing Tauri command — and renders
the `fc-table` with the termômetro coloring on each day's Saldo cell (lines
141–157):

```tsx
// MonthLedgerCard.tsx:141-157
{
  /* Saldo com o termômetro da planilha (dias não importados ficam neutros). */
}
{
  d.balance_cents == null ? (
    <td className="money" style={{ color: "var(--text-faint)" }}>
      —
    </td>
  ) : (
    <td
      className="money"
      style={{
        background: SALDO_BAND_FILL[saldoBand(d.balance_cents)],
        color: "var(--text)",
      }}
      title={`Saldo ${SALDO_BAND_LABEL[saldoBand(d.balance_cents)]}`}
    >
      <Money cents={d.balance_cents} size="sm" sign="none" />
    </td>
  );
}
```

The `saldoHeatmap` exports used here (`src/lib/saldoHeatmap.ts`, lines 58–76):

```ts
// saldoHeatmap.ts:58-76
export const SALDO_BAND_FILL: Record<SaldoBand, string> = {
  critical: "var(--saldo-band-critical-fill)",
  negative: "var(--saldo-band-negative-fill)",
  tight: "var(--saldo-band-tight-fill)",
  ok: "var(--saldo-band-ok-fill)",
  comfortable: "var(--saldo-band-comfortable-fill)",
};

export const SALDO_BAND_LEGEND: { band: SaldoBand; label: string }[] = [
  { band: "comfortable", label: "folga" },
  { band: "ok", label: "ok" },
  { band: "tight", label: "apertado" },
  { band: "negative", label: "negativo" },
  { band: "critical", label: "crítico" },
];

export const SALDO_BAND_LABEL: Record<SaldoBand, string> = Object.fromEntries(
  SALDO_BAND_LEGEND.map((l) => [l.band, l.label]),
) as Record<SaldoBand, string>;
```

**What does NOT exist**: a screen that renders all 12 months in one view.
Confirmed by: `grep -rn "YearGrid\|year_grid\|year-grid" src/` → no results.

**Rust command** (already registered, no new command needed):

```rust
// forecast_cmds.rs:934-941
/// Grade do mês `year-month` (visão fiel à planilha). Ver [`month_grid`].
#[tauri::command]
pub async fn get_month_grid(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<Vec<MonthGridDayDto>, String> {
    month_grid(pool.inner(), year, month).await
}
```

Already in `lib.rs` (line 38): `commands::get_month_grid`.

### Feature 2 — dual-year Economia side-by-side

**What exists**: `src/screens/AnnualScreen.tsx` (369 lines) shows one year at a
time with a `MonthNav`-based year selector (lines 144–147):

```tsx
// AnnualScreen.tsx:144-148
export function AnnualScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);
  const q = useCommand(`annual_metrics:${year}`, () => getAnnualMetrics(year));
  const months = q.data?.months ?? [];
```

It already renders Entradas/Economia/Economizado% per month for a single year,
totalled at the bottom (lines 264–369). No dual-year column exists.

The `EconomizadoSparkline` component (lines 54–142) renders a bar chart; it is
for the single-year sparkline and is NOT reused in the dual-year view.

**Rust command** (already registered, no new command needed):

```rust
// forecast_cmds.rs:943-949
#[tauri::command]
pub async fn get_annual_metrics(
    pool: State<'_, SqlitePool>,
    year: i32,
) -> Result<AnnualMetricsDto, String> {
    annual_metrics(pool.inner(), year, chrono::Local::now().date_naive()).await
}
```

Already in `lib.rs` (line 37): `commands::get_annual_metrics`.

**Importer already handles multi-year**: `parse_economia_sheet` in
`src-tauri/src/google_sheets/import.rs` (line 1385) parses side-by-side year
blocks and writes both years to the database — so the data is there.

**TypeScript API** (`src/lib/api.ts`, lines 658–665):

```ts
// api.ts:658-665
export interface AnnualMetrics {
  year: number;
  months: MonthMetric[];
}

export function getAnnualMetrics(year: number): Promise<AnnualMetrics> {
  return invoke("get_annual_metrics", { year });
}
```

`MonthMetric` (api.ts:145-161) carries `income_cents`, `economia_cents`,
`savings_rate_bps` per month — all we need for the dual-year comparison.

**What does NOT exist**: a dual-year side-by-side view for Economia.
Confirmed by: `grep -rn "DualYear\|EconomiaCompar\|dual.*year\|two.*year" src/` → no results.

### Navigation wiring

`src/shell/AppShell.tsx` defines the `Screen` union type (lines 22–31) and the
`NAV_ANALYSIS` array (lines 57–62):

```tsx
// AppShell.tsx:22-31
export type Screen =
  | "dashboard"
  | "totais"
  | "anuais"
  | "horizonte"
  | "transactions"
  | "tags"
  | "copilot"
  | "methodology"
  | "settings";

// AppShell.tsx:57-62
const NAV_ANALYSIS: NavItem[] = [
  { key: "totais", label: "Totais", icon: Calculator },
  { key: "anuais", label: "Anual", icon: TrendingUp },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "tags", label: "Tags", icon: TagsIcon },
];
```

`src/App.tsx` maps `Screen` values to components (lines 88–99):

```tsx
// App.tsx:88-99
{
  screen === "totais" && <TotaisScreen />;
}
{
  screen === "anuais" && <AnnualScreen />;
}
{
  screen === "horizonte" && <HorizonteScreen />;
}
{
  screen === "tags" && <TagsScreen />;
}
```

`SCREEN_META` records the title and crumb for each screen (AppShell.tsx:33-43).

### CSS classes in use

Reuse these existing patterns (defined in `src/App.css`):

- `.fc-scroll` (line 2201): `overflow-x: auto` — horizontal scroll for wide
  tables on narrow viewports.
- `.fc-table th:not(:first-child), .fc-table td:not(:first-child)` (lines
  2205–2208): right-align all columns except the first.
- `.fc-today td` (line 2210): `background: var(--primary-quiet)` for the
  "today" row highlight.
- `.fc-today__tag` (line 2214): uppercase micro label "hoje".
- `.fc-foot td, .fc-foot th` (lines 2225–2237): footer rows with
  `var(--bg-subtle)` background and stronger border-top.
- `.dash-card` / `.dash-card__head` / `.dash-card__body` (lines 558–590): card
  chrome used by MonthLedgerCard and the Annual screen.
- `.txn-table` (line 593): collapsed, `var(--fs-sm)`, full-width.

### React Compiler + static style convention

React Compiler is ON (no manual `memo`). Static `CSSProperties` objects are
hoisted as `const` outside components. Example from `AnnualScreen.tsx`:

```tsx
// AnnualScreen.tsx:25-38
const th: React.CSSProperties = {
  textAlign: "right",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  …
};

const tdNum: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-3) var(--space-4)",
};
```

No inline object literals in JSX prop positions.

### Repo conventions (apply throughout this plan)

- Money = positive-magnitude integer cents. Format via `<Money>` or `formatBRL`.
- Method-neutral language: never name the external spreadsheet method, course,
  or app in code or comments. Generic descriptions only (this is a public repo).
- `useCommand(key, fetcher)` — `key` must embed all arguments so different
  months/years get separate cache slots (e.g. `"month_grid:2026-03"`). The
  `fetcher` must be referentially stable (module-level arrow or captured closure
  that does not capture render-scope variables). See `useCommand.ts:36-48` for
  the requirement.
- Error handling: degrade gracefully; show `<EmptyState variant="empty">` when
  no data and `<EmptyState variant="skeleton">` while loading.
- Tests in `vitest` + `@testing-library/react`. Use `mockCommands` /
  `mockInvoke` from `src/test/commands.ts` (already has `MONTH_GRID` fixture
  and the `AnnualMetrics` shape).

## Commands you will need

| Purpose          | Command              | Expected on success           |
| ---------------- | -------------------- | ----------------------------- |
| Typecheck        | `npm run typecheck`  | exit 0, no errors             |
| Lint             | `npm run lint`       | exit 0                        |
| Unit tests       | `npm run test:run`   | all pass                      |
| Rust checks      | `npm run rust:check` | exit 0 (check + clippy)       |
| React Doctor     | `npm run doctor`     | 0 findings (advisory only)    |
| Full gate        | `npm run check`      | exit 0                        |
| E2E visual smoke | `npm run e2e`        | all pass; inspect screenshots |

## Suggested executor toolkit

- Invoke the `neko-finance-design` skill when choosing layout proportions or
  spacing tokens for the new screens to stay aligned with the design system.
- Read `src/design-system/components/Money.tsx` and `EmptyState.tsx` before
  writing markup — their props and CSS classes are non-trivial.
- Read `src/screens/AnnualScreen.test.tsx` as the structural model for the new
  tests (uses `vi.mock` + `mockCommands` + `waitFor`).

## Scope

**In scope** (the only files you should create or modify):

- `src/screens/YearGridScreen.tsx` — new file: the full-year 12-month grid view.
- `src/screens/YearGridScreen.test.tsx` — new file: tests for the year-grid screen.
- `src/screens/EconomiaCompareScreen.tsx` — new file: the dual-year Economia
  side-by-side view.
- `src/screens/EconomiaCompareScreen.test.tsx` — new file: tests for the dual-year
  Economia screen.
- `src/shell/AppShell.tsx` — add two new `Screen` values and two `NAV_ANALYSIS`
  entries and two `SCREEN_META` entries.
- `src/App.tsx` — render the two new screens in the `screen ===` block.
- `plans/README.md` — update status row for plan 044 when done.

**Out of scope** (do NOT touch, even if they look related):

- `src/screens/AnnualScreen.tsx` — existing Visão anual (Economia sparkline +
  annual table) is NOT replaced; the dual-year Economia view is additive.
- `src/screens/dashboard/MonthLedgerCard.tsx` — keep the single-month card
  unchanged; the year-grid is a separate screen.
- `src-tauri/` — no new Rust commands needed; both views reuse `get_month_grid`
  and `get_annual_metrics` which are already exposed.
- `src/lib/api.ts` — no API changes; `getMonthGrid` and `getAnnualMetrics`
  already exist with the right signatures.
- `src/lib/saldoHeatmap.ts` — reuse as-is; do not add or change exports.
- Any migration file — no schema changes.
- Any other plan in `plans/`.

## Git workflow

- Branch: `advisor/044-parity-year-grid-economia-sidebyside`
- Commit per logical unit (Step 1–3 for Feature 1; Step 4–6 for Feature 2;
  Step 7 for nav; Step 8 for tests; Step 9 full gate) — do NOT squash.
- Message style from recent history: `feat: <short description> (#<n>)`.
  Example: `feat: year-grid + dual-year Economia parity views (plano 044)`.
- Do NOT push or open a PR unless the operator explicitly instructs it.

## Steps

---

### Step 1: Create `YearGridScreen.tsx` — scaffold with year selector and 12 parallel fetches

Create `src/screens/YearGridScreen.tsx`.

The screen needs all 12 months of `getMonthGrid` for the selected year loaded
in parallel. Because `useCommand` is a single-call hook (not array-capable),
call it 12 times at the top level — one per month — with stable, unique keys.
React's Rules of Hooks permit this as long as the number of calls is constant
(always 12):

```tsx
import { useState } from "react";
import { getMonthGrid, type MonthGridDay } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { MonthNav } from "../design-system/components/MonthNav";
import { EmptyState } from "../design-system/components/EmptyState";
import { Money } from "../design-system/components/Money";
import { fmtDayMonth, monthNamePtBR } from "../lib/format";
import { saldoBand, SALDO_BAND_FILL, SALDO_BAND_LABEL } from "../lib/saldoHeatmap";

const MONTHS_PT = [
  "Janeiro",
  "Fevereiro",
  "Março",
  "Abril",
  "Maio",
  "Junho",
  "Julho",
  "Agosto",
  "Setembro",
  "Outubro",
  "Novembro",
  "Dezembro",
];

// Static style objects hoisted outside the component (React Compiler convention).
const TH_STYLE: React.CSSProperties = {
  textAlign: "right",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  padding: "var(--space-2) var(--space-3)",
  whiteSpace: "nowrap",
};
const TD_NUM: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-2) var(--space-3)",
  whiteSpace: "nowrap",
};

export function YearGridScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);

  // 12 parallel fetches — one per month. Keys embed both year and month so
  // navigating to a different year triggers fresh fetches.
  const m01 = useCommand(`month_grid:${year}-01`, () => getMonthGrid(year, 1));
  const m02 = useCommand(`month_grid:${year}-02`, () => getMonthGrid(year, 2));
  const m03 = useCommand(`month_grid:${year}-03`, () => getMonthGrid(year, 3));
  const m04 = useCommand(`month_grid:${year}-04`, () => getMonthGrid(year, 4));
  const m05 = useCommand(`month_grid:${year}-05`, () => getMonthGrid(year, 5));
  const m06 = useCommand(`month_grid:${year}-06`, () => getMonthGrid(year, 6));
  const m07 = useCommand(`month_grid:${year}-07`, () => getMonthGrid(year, 7));
  const m08 = useCommand(`month_grid:${year}-08`, () => getMonthGrid(year, 8));
  const m09 = useCommand(`month_grid:${year}-09`, () => getMonthGrid(year, 9));
  const m10 = useCommand(`month_grid:${year}-10`, () => getMonthGrid(year, 10));
  const m11 = useCommand(`month_grid:${year}-11`, () => getMonthGrid(year, 11));
  const m12 = useCommand(`month_grid:${year}-12`, () => getMonthGrid(year, 12));

  const grids: {
    month: number;
    label: string;
    loading: boolean;
    data: MonthGridDay[];
  }[] = [
    { month: 1, label: MONTHS_PT[0], loading: m01.loading, data: m01.data ?? [] },
    { month: 2, label: MONTHS_PT[1], loading: m02.loading, data: m02.data ?? [] },
    { month: 3, label: MONTHS_PT[2], loading: m03.loading, data: m03.data ?? [] },
    { month: 4, label: MONTHS_PT[3], loading: m04.loading, data: m04.data ?? [] },
    { month: 5, label: MONTHS_PT[4], loading: m05.loading, data: m05.data ?? [] },
    { month: 6, label: MONTHS_PT[5], loading: m06.loading, data: m06.data ?? [] },
    { month: 7, label: MONTHS_PT[6], loading: m07.loading, data: m07.data ?? [] },
    { month: 8, label: MONTHS_PT[7], loading: m08.loading, data: m08.data ?? [] },
    { month: 9, label: MONTHS_PT[8], loading: m09.loading, data: m09.data ?? [] },
    { month: 10, label: MONTHS_PT[9], loading: m10.loading, data: m10.data ?? [] },
    { month: 11, label: MONTHS_PT[10], loading: m11.loading, data: m11.data ?? [] },
    { month: 12, label: MONTHS_PT[11], loading: m12.loading, data: m12.data ?? [] },
  ];

  return (
    <div style={{ maxWidth: 1100, margin: "0 auto", padding: "var(--space-2)" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-4)",
          marginBottom: "var(--space-6)",
          flexWrap: "wrap",
        }}
      >
        <div>
          <h1
            style={{
              fontSize: "var(--fs-h2)",
              fontWeight: "var(--fw-bold)",
              letterSpacing: "var(--ls-tight)",
              margin: 0,
            }}
          >
            Ano inteiro
          </h1>
          <p
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-1) 0 0",
            }}
          >
            Grade Data · Entrada · Saída · Diário · Saldo para cada mês de {year}.
          </p>
        </div>
        <MonthNav
          label={String(year)}
          onPrev={() => setYear((y) => y - 1)}
          onNext={() => setYear((y) => y + 1)}
          onToday={() => setYear(thisYear)}
          atToday={year === thisYear}
          prevLabel="Ano anterior"
          nextLabel="Próximo ano"
        />
      </header>

      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-6)" }}>
        {grids.map((g) => (
          <MonthSection
            key={g.month}
            label={g.label}
            loading={g.loading}
            grid={g.data}
          />
        ))}
      </div>
    </div>
  );
}
```

The `MonthSection` sub-component (defined in the same file, outside
`YearGridScreen`) renders the per-month card with the same table structure as
`MonthLedgerCard` — reuse the class names and heatmap logic:

```tsx
// Pure presentational — no state, no effects (React Compiler friendly).
function MonthSection({
  label,
  loading,
  grid,
}: {
  label: string;
  loading: boolean;
  grid: MonthGridDay[];
}) {
  const hasData = grid.some(
    (d) =>
      d.income_cents ||
      d.fixed_out_cents ||
      d.daily_out_cents ||
      d.balance_cents != null,
  );

  return (
    <section aria-label={label}>
      <h2
        style={{
          fontSize: "var(--fs-title)",
          fontWeight: "var(--fw-bold)",
          margin: "0 0 var(--space-3)",
          color: "var(--text-strong)",
        }}
      >
        {label}
      </h2>
      <div className="dash-card">
        <div className="dash-card__body" style={{ padding: 0 }}>
          {loading ? (
            <EmptyState variant="skeleton" skeletonRows={5} />
          ) : !hasData ? (
            <EmptyState
              variant="empty"
              title="Sem lançamentos"
              description="Nenhum dado importado para este mês."
            />
          ) : (
            <div className="fc-scroll">
              <table className="txn-table fc-table">
                <thead>
                  <tr>
                    <th scope="col" style={TH_STYLE}>
                      Data
                    </th>
                    <th scope="col" style={TH_STYLE}>
                      Entrada
                    </th>
                    <th scope="col" style={TH_STYLE}>
                      Saída
                    </th>
                    <th scope="col" style={TH_STYLE}>
                      Diário
                    </th>
                    <th scope="col" style={TH_STYLE}>
                      Saldo
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {grid.map((d) => (
                    <tr key={d.date}>
                      <td
                        style={{
                          padding: "var(--space-2) var(--space-3)",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {fmtDayMonth(d.date)}
                      </td>
                      <td style={TD_NUM}>
                        {d.income_cents ? (
                          <Money cents={d.income_cents} size="sm" sign="auto" />
                        ) : (
                          "—"
                        )}
                      </td>
                      <td style={TD_NUM}>
                        {d.fixed_out_cents ? (
                          <Money cents={d.fixed_out_cents} size="sm" />
                        ) : (
                          "—"
                        )}
                      </td>
                      <td style={TD_NUM}>
                        {d.daily_out_cents ? (
                          <Money cents={d.daily_out_cents} size="sm" />
                        ) : (
                          "—"
                        )}
                      </td>
                      {d.balance_cents == null ? (
                        <td style={{ ...TD_NUM, color: "var(--text-faint)" }}>—</td>
                      ) : (
                        <td
                          style={{
                            ...TD_NUM,
                            background: SALDO_BAND_FILL[saldoBand(d.balance_cents)],
                            color: "var(--text)",
                          }}
                          title={`Saldo ${SALDO_BAND_LABEL[saldoBand(d.balance_cents)]}`}
                        >
                          <Money cents={d.balance_cents} size="sm" sign="none" />
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
```

**IMPORTANT about `useCommand` and closure stability**: the fetcher passed to
each `useCommand` call must NOT inline a closure that closes over the `year`
state variable — if `year` changes between renders, the cached result for the
old year would be returned until the component remounts (the effect only re-runs
when `cmd` changes, not `fetcher`). The key `month_grid:${year}-01` already
embeds the year, so changing `year` produces a new key and triggers a fresh
fetch. This is correct — do NOT wrap the fetcher in `useCallback`.

**Verify**: `npm run typecheck` → exit 0, no TypeScript errors in the new file.

---

### Step 2: Create `EconomiaCompareScreen.tsx` — dual-year Economia side-by-side

Create `src/screens/EconomiaCompareScreen.tsx`.

The screen fetches `getAnnualMetrics` for two consecutive years (the "base year"
and "base year + 1") and renders them side-by-side, three columns each:
Entradas, Economia, Economizado%.

```tsx
import { useState } from "react";
import { getAnnualMetrics, type MonthMetric } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { EmptyState } from "../design-system/components/EmptyState";
import { SAVINGS_MIN_BPS } from "./totaisStatus";

const MONTHS_PT = [
  "Jan",
  "Fev",
  "Mar",
  "Abr",
  "Mai",
  "Jun",
  "Jul",
  "Ago",
  "Set",
  "Out",
  "Nov",
  "Dez",
];

// Static styles (React Compiler convention — hoist outside component).
const TH: React.CSSProperties = {
  textAlign: "right",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  padding: "var(--space-3) var(--space-4)",
};

const TH_YEAR: React.CSSProperties = {
  ...TH,
  textAlign: "center",
  color: "var(--text-strong)",
  fontSize: "var(--fs-sm)",
  borderBottom: "2px solid var(--border-strong)",
  letterSpacing: 0,
  textTransform: "none",
};

const TD_NUM: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-3) var(--space-4)",
  fontVariantNumeric: "tabular-nums",
};

const DIVIDER: React.CSSProperties = {
  borderLeft: "var(--bw-strong) solid var(--border-strong)",
};

export function EconomiaCompareScreen() {
  const thisYear = new Date().getFullYear();
  // "base year" is the earlier year shown on the left; right column = baseYear + 1.
  const [baseYear, setBaseYear] = useState(thisYear - 1);
  const yearA = baseYear;
  const yearB = baseYear + 1;

  const qA = useCommand(`annual_metrics:${yearA}`, () => getAnnualMetrics(yearA));
  const qB = useCommand(`annual_metrics:${yearB}`, () => getAnnualMetrics(yearB));

  const monthsA: MonthMetric[] = qA.data?.months ?? [];
  const monthsB: MonthMetric[] = qB.data?.months ?? [];

  // Year totals (weighted Economizado% = ΣEconomia / ΣEntradas).
  function yearTotals(months: MonthMetric[]) {
    const t = months.reduce(
      (a, m) => ({
        income: a.income + m.income_cents,
        economia: a.economia + m.economia_cents,
      }),
      { income: 0, economia: 0 },
    );
    return {
      ...t,
      savingsPct: t.income > 0 ? Math.round((t.economia / t.income) * 100) : 0,
    };
  }

  const totA = yearTotals(monthsA);
  const totB = yearTotals(monthsB);
  const loading = qA.loading || qB.loading;
  const hasAnyData =
    monthsA.some((m) => m.income_cents !== 0 || m.economia_cents !== 0) ||
    monthsB.some((m) => m.income_cents !== 0 || m.economia_cents !== 0);

  function savingsColor(pct: number): string {
    if (pct > 30) return "var(--primary)";
    if (pct >= SAVINGS_MIN_BPS / 100) return "var(--success-400)";
    return "var(--warning-400)";
  }

  return (
    <div style={{ maxWidth: 860, margin: "0 auto", padding: "var(--space-2)" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-4)",
          marginBottom: "var(--space-6)",
          flexWrap: "wrap",
        }}
      >
        <div>
          <h1
            style={{
              fontSize: "var(--fs-h2)",
              fontWeight: "var(--fw-bold)",
              letterSpacing: "var(--ls-tight)",
              margin: 0,
            }}
          >
            Economia: {yearA} vs {yearB}
          </h1>
          <p
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-1) 0 0",
            }}
          >
            Entradas, Economia e Economizado% mês a mês — dois anos lado a lado.
          </p>
        </div>
        <MonthNav
          label={`${yearA} · ${yearB}`}
          onPrev={() => setBaseYear((y) => y - 1)}
          onNext={() => setBaseYear((y) => y + 1)}
          onToday={() => setBaseYear(thisYear - 1)}
          atToday={baseYear === thisYear - 1}
          prevLabel="Par de anos anterior"
          nextLabel="Próximo par de anos"
        />
      </header>

      {loading ? (
        <EmptyState variant="skeleton" skeletonRows={7} />
      ) : !hasAnyData ? (
        <EmptyState
          variant="empty"
          title="Sem dados de Economia"
          description="Importe a aba Economia em Configurações › Google Sheets."
        />
      ) : (
        <div className="dash-card">
          <div className="dash-card__body">
            <div style={{ overflowX: "auto" }}>
              <table
                style={{
                  width: "100%",
                  borderCollapse: "collapse",
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                <thead>
                  {/* Year header row */}
                  <tr>
                    <th style={{ ...TH, textAlign: "left" }} rowSpan={2} scope="col">
                      Mês
                    </th>
                    <th
                      colSpan={3}
                      style={{
                        ...TH_YEAR,
                        borderRight: "var(--bw-strong) solid var(--border-strong)",
                      }}
                      scope="colgroup"
                    >
                      {yearA}
                    </th>
                    <th colSpan={3} style={TH_YEAR} scope="colgroup">
                      {yearB}
                    </th>
                  </tr>
                  {/* Column header row */}
                  <tr style={{ borderBottom: "var(--bw-hair) solid var(--border)" }}>
                    <th style={TH} scope="col">
                      Entradas
                    </th>
                    <th style={TH} scope="col">
                      Economia
                    </th>
                    <th
                      style={{
                        ...TH,
                        borderRight: "var(--bw-strong) solid var(--border-strong)",
                      }}
                      scope="col"
                    >
                      <InfoPopover term="economizado">Economizado</InfoPopover>
                    </th>
                    <th style={{ ...TH, ...DIVIDER }} scope="col">
                      Entradas
                    </th>
                    <th style={TH} scope="col">
                      Economia
                    </th>
                    <th style={TH} scope="col">
                      <InfoPopover term="economizado">Economizado</InfoPopover>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {Array.from({ length: 12 }, (_, i) => {
                    const mA = monthsA[i];
                    const mB = monthsB[i];
                    const emptyA =
                      !mA || (mA.income_cents === 0 && mA.economia_cents === 0);
                    const emptyB =
                      !mB || (mB.income_cents === 0 && mB.economia_cents === 0);
                    return (
                      <tr
                        key={i}
                        style={{ borderBottom: "var(--bw-hair) solid var(--border)" }}
                      >
                        <td
                          style={{
                            padding: "var(--space-3) var(--space-4)",
                            fontWeight: "var(--fw-semibold)",
                            color: "var(--text)",
                          }}
                        >
                          {MONTHS_PT[i]}
                        </td>
                        {/* Year A columns */}
                        <td style={{ ...TD_NUM, opacity: emptyA ? 0.4 : 1 }}>
                          {mA ? (
                            <Money cents={mA.income_cents} size="sm" sign="auto" />
                          ) : (
                            "—"
                          )}
                        </td>
                        <td style={{ ...TD_NUM, opacity: emptyA ? 0.4 : 1 }}>
                          {mA ? <Money cents={mA.economia_cents} size="sm" /> : "—"}
                        </td>
                        <td
                          style={{
                            ...TD_NUM,
                            borderRight: "var(--bw-strong) solid var(--border-strong)",
                            fontFamily: "var(--font-money)",
                            color: emptyA
                              ? "var(--text-faint)"
                              : savingsColor(mA ? mA.savings_rate_bps / 100 : 0),
                            opacity: emptyA ? 0.4 : 1,
                          }}
                        >
                          {emptyA ? "—" : `${(mA!.savings_rate_bps / 100).toFixed(0)}%`}
                        </td>
                        {/* Year B columns */}
                        <td
                          style={{ ...TD_NUM, ...DIVIDER, opacity: emptyB ? 0.4 : 1 }}
                        >
                          {mB ? (
                            <Money cents={mB.income_cents} size="sm" sign="auto" />
                          ) : (
                            "—"
                          )}
                        </td>
                        <td style={{ ...TD_NUM, opacity: emptyB ? 0.4 : 1 }}>
                          {mB ? <Money cents={mB.economia_cents} size="sm" /> : "—"}
                        </td>
                        <td
                          style={{
                            ...TD_NUM,
                            fontFamily: "var(--font-money)",
                            color: emptyB
                              ? "var(--text-faint)"
                              : savingsColor(mB ? mB.savings_rate_bps / 100 : 0),
                            opacity: emptyB ? 0.4 : 1,
                          }}
                        >
                          {emptyB ? "—" : `${(mB!.savings_rate_bps / 100).toFixed(0)}%`}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
                <tfoot>
                  <tr
                    style={{
                      borderTop: "var(--bw-strong) solid var(--border-strong)",
                      fontWeight: "var(--fw-bold)",
                    }}
                  >
                    <td
                      style={{
                        padding: "var(--space-3) var(--space-4)",
                        textTransform: "uppercase",
                        letterSpacing: "var(--ls-label)",
                        fontSize: "var(--fs-label)",
                        color: "var(--text)",
                      }}
                    >
                      Total
                    </td>
                    <td style={TD_NUM}>
                      <Money cents={totA.income} size="sm" sign="auto" />
                    </td>
                    <td style={TD_NUM}>
                      <Money cents={totA.economia} size="sm" />
                    </td>
                    <td
                      title="Economizado anual = ΣEconomia ÷ ΣEntradas (meta 20–30%)"
                      style={{
                        ...TD_NUM,
                        borderRight: "var(--bw-strong) solid var(--border-strong)",
                        fontFamily: "var(--font-money)",
                        color: savingsColor(totA.savingsPct),
                      }}
                    >
                      {totA.savingsPct}%
                    </td>
                    <td style={{ ...TD_NUM, ...DIVIDER }}>
                      <Money cents={totB.income} size="sm" sign="auto" />
                    </td>
                    <td style={TD_NUM}>
                      <Money cents={totB.economia} size="sm" />
                    </td>
                    <td
                      title="Economizado anual = ΣEconomia ÷ ΣEntradas (meta 20–30%)"
                      style={{
                        ...TD_NUM,
                        fontFamily: "var(--font-money)",
                        color: savingsColor(totB.savingsPct),
                      }}
                    >
                      {totB.savingsPct}%
                    </td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
```

Check that `InfoPopover` is exported from its module before importing it:

```bash
grep -n "export function InfoPopover\|export.*InfoPopover" src/design-system/components/InfoPopover.tsx
```

If the file/export does not exist, replace `<InfoPopover term="economizado">Economizado</InfoPopover>`
with a plain `<span>Economizado</span>` and add a `// TODO: InfoPopover` comment.

Also check that `SAVINGS_MIN_BPS` is exported from `totaisStatus.ts`:

```bash
grep -n "SAVINGS_MIN_BPS" src/screens/totaisStatus.ts
```

`AnnualScreen.tsx` line 8 imports it — so it is exported. Confirm:
`import { SAVINGS_MIN_BPS } from "./totaisStatus";` in `EconomiaCompareScreen.tsx`
(note: the file lives in `src/screens/`, so no path prefix needed).

**Verify**: `npm run typecheck` → exit 0, no errors in the new file.

---

### Step 3: Wire both new screens into `AppShell.tsx` and `App.tsx`

**3a. `src/shell/AppShell.tsx`**

Add two values to the `Screen` union (lines 22–31):

```tsx
export type Screen =
  | "dashboard"
  | "totais"
  | "anuais"
  | "ano-inteiro" // ← new: full-year 12-month grid
  | "economia-compare" // ← new: dual-year Economia side-by-side
  | "horizonte"
  | "transactions"
  | "tags"
  | "copilot"
  | "methodology"
  | "settings";
```

Add entries to `SCREEN_META` (after the existing `"anuais"` entry):

```tsx
"ano-inteiro":      { title: "Ano inteiro",       crumb: "Grade dia a dia — 12 meses" },
"economia-compare": { title: "Economia comparada", crumb: "Dois anos lado a lado" },
```

Add two items to `NAV_ANALYSIS` (after the existing `"anuais"` item):

```tsx
const NAV_ANALYSIS: NavItem[] = [
  { key: "totais", label: "Totais", icon: Calculator },
  { key: "anuais", label: "Anual", icon: TrendingUp },
  { key: "ano-inteiro", label: "Ano inteiro", icon: LayoutList }, // ← new
  { key: "economia-compare", label: "Economia comparada", icon: GitCompareArrows }, // ← new
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "tags", label: "Tags", icon: TagsIcon },
];
```

Import the two new icons from `lucide-react`. First check which icons are
available for the concepts:

```bash
node -e "const l = require('lucide-react'); console.log(Object.keys(l).filter(k => /List|Table|Grid|Compare|Columns/.test(k)))"
```

Reasonable choices: `LayoutList` for the year grid (a long scrollable list),
`GitCompareArrows` or `Columns2` for the comparison. Use whichever is present
in the installed lucide-react version. If neither is available, fall back to
`Table2` (grid) and `BarChart2` (compare). Do NOT add a new package.

**3b. `src/App.tsx`**

Import the two new screens at the top:

```tsx
import { YearGridScreen } from "./screens/YearGridScreen";
import { EconomiaCompareScreen } from "./screens/EconomiaCompareScreen";
```

Add two `screen ===` branches in the render block (after the `"anuais"` branch):

```tsx
{
  screen === "anuais" && <AnnualScreen />;
}
{
  screen === "ano-inteiro" && <YearGridScreen />;
}
{
  screen === "economia-compare" && <EconomiaCompareScreen />;
}
{
  screen === "horizonte" && <HorizonteScreen />;
}
```

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.
The two new nav items appear in the sidebar; clicking them renders the
placeholder content (from the previous steps).

---

### Step 4: Write tests — `YearGridScreen.test.tsx`

Create `src/screens/YearGridScreen.test.tsx`.

Model after `src/screens/AnnualScreen.test.tsx` (uses `vi.mock` + `mockCommands`

- `waitFor`) and `src/screens/dashboard/MonthLedgerCard.test.tsx`.

The challenge: the screen calls `getMonthGrid` 12 times with different months.
`mockCommands` routes by command name only, so all 12 calls get the same mock
response. This is fine for characterization tests.

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import { mockCommands, mockInvoke, MONTH_GRID } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("YearGridScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders all 12 month sections", async () => {
    // get_month_grid returns the same MONTH_GRID for all 12 calls.
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);

    // Wait for at least one month to load.
    await waitFor(() => expect(screen.getByText("Janeiro")).toBeInTheDocument());
    expect(screen.getByText("Fevereiro")).toBeInTheDocument();
    expect(screen.getByText("Dezembro")).toBeInTheDocument();
    // 12 section headings total.
    expect(screen.getAllByRole("region").length).toBe(12); // or getAllByRole("heading", { level: 2 })
  });

  it("shows termômetro coloring on non-null Saldo cells", async () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);
    await waitFor(() => expect(screen.getByText("Janeiro")).toBeInTheDocument());
    // At least one Saldo-colored cell must be present (MONTH_GRID has balance_cents set).
    // The cell title contains the band label from SALDO_BAND_LABEL.
    const titledCells = document.querySelectorAll("td[title]");
    expect(titledCells.length).toBeGreaterThan(0);
  });

  it("shows empty state per month when no data", async () => {
    mockCommands({ get_month_grid: [] });
    render(<YearGridScreen />);
    await waitFor(() =>
      expect(screen.getAllByText(/Sem lançamentos/).length).toBeGreaterThan(0),
    );
  });

  it("shows year heading with the current year", () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);
    expect(screen.getByText("Ano inteiro")).toBeInTheDocument();
  });
});
```

**Verify**: `npm run test:run -- --reporter=verbose` → all new tests in
`YearGridScreen.test.tsx` pass.

---

### Step 5: Write tests — `EconomiaCompareScreen.test.tsx`

Create `src/screens/EconomiaCompareScreen.test.tsx`.

Model after `src/screens/AnnualScreen.test.tsx`.

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { EconomiaCompareScreen } from "./EconomiaCompareScreen";
import type { AnnualMetrics, MonthMetric } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Helper: build a MonthMetric with just the savings-relevant fields populated.
function mk(month: number, income: number, economia: number, bps: number): MonthMetric {
  return {
    year: 2025,
    month,
    income_cents: income,
    performance_cents: income - economia,
    cost_of_living_cents: income - economia,
    fixed_out_cents: income - economia,
    daily_out_cents: 0,
    real_daily_avg_cents: 0,
    economia_cents: economia,
    savings_rate_bps: bps,
  };
}

const METRICS_2025: AnnualMetrics = {
  year: 2025,
  months: [
    mk(1, 500_000, 100_000, 2000),
    ...Array.from({ length: 11 }, (_, i) => mk(i + 2, 0, 0, 0)),
  ],
};

const METRICS_2026: AnnualMetrics = {
  year: 2026,
  months: [
    mk(1, 600_000, 150_000, 2500),
    ...Array.from({ length: 11 }, (_, i) => mk(i + 2, 0, 0, 0)),
  ],
};

describe("EconomiaCompareScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders both year columns in the header", async () => {
    mockCommands({ get_annual_metrics: METRICS_2025 });
    render(<EconomiaCompareScreen />);
    await waitFor(() =>
      expect(screen.getByText("Economia: 2025 vs 2026")).toBeInTheDocument(),
    );
    // Both year labels appear in the column headers.
    expect(screen.getAllByText("2025").length).toBeGreaterThan(0);
    expect(screen.getAllByText("2026").length).toBeGreaterThan(0);
  });

  it("renders 12 month rows", async () => {
    mockCommands({ get_annual_metrics: METRICS_2025 });
    render(<EconomiaCompareScreen />);
    await waitFor(() => expect(screen.getByText("Jan")).toBeInTheDocument());
    expect(screen.getByText("Dez")).toBeInTheDocument();
    expect(screen.getAllByText("Jan").length).toBeGreaterThanOrEqual(1);
  });

  it("total Economizado% row is weighted (ΣEconomia/ΣEntradas), not average of monthly rates", async () => {
    // Jan: 20% (100k/500k). All other months: 0%. Weighted annual = 100k/500k = 20%.
    // Simple average of non-zero months would also be 20% here — use a two-month case:
    const two: AnnualMetrics = {
      year: 2025,
      months: [
        mk(1, 100_000, 30_000, 3000), // 30%
        mk(2, 300_000, 30_000, 1000), // 10%
        ...Array.from({ length: 10 }, (_, i) => mk(i + 3, 0, 0, 0)),
      ],
    };
    mockCommands({ get_annual_metrics: two });
    render(<EconomiaCompareScreen />);
    // Weighted: 60k/400k = 15%.
    await waitFor(() => expect(screen.getAllByText("Total").length).toBeGreaterThan(0));
    expect(screen.getAllByText("15%").length).toBeGreaterThan(0); // appears in at least one year column
    expect(screen.queryAllByText("20%").length).toBe(0); // NOT the simple average
  });

  it("shows empty state when no data at all", async () => {
    const empty: AnnualMetrics = {
      year: 2025,
      months: Array.from({ length: 12 }, (_, i) => mk(i + 1, 0, 0, 0)),
    };
    mockCommands({ get_annual_metrics: empty });
    render(<EconomiaCompareScreen />);
    await waitFor(() =>
      expect(screen.getByText(/Sem dados de Economia/)).toBeInTheDocument(),
    );
  });
});
```

Note: `mockCommands` maps by command name (`get_annual_metrics`), so both
`qA` and `qB` inside the screen receive the same fixture. For unit tests of
the comparison logic, use a fixture that has two distinct months with data.

**Verify**: `npm run test:run -- --reporter=verbose` → all new tests in
`EconomiaCompareScreen.test.tsx` pass, including the weighted-Economizado% test.

---

### Step 6: Run React Doctor

```
npm run doctor
```

Expected: 0 findings. If the Doctor flags any inline style in the new
components (threshold is ≥ 8 inline props), move those objects to `const` style
objects outside the component. Do NOT add a `react-doctor-disable-next-line`
comment unless the flag is a confirmed false positive.

**Verify**: `npm run doctor` → 0 findings (or unchanged from baseline).

---

### Step 7: Full gate

Run the complete check suite:

```
npm run check
```

Expected: exit 0. All lint, typecheck, tests, rust:check, doctor, privacy:scan
must pass.

Then run the E2E visual smoke test:

```
npm run e2e
```

Inspect the generated screenshots or traces. Specifically check:

1. The "Análise" nav section now shows the two new items ("Ano inteiro" and
   "Economia comparada") in the sidebar.
2. The "Ano inteiro" screen renders 12 month sections (though they may be empty
   if the Playwright fixture has no imported data).
3. The "Economia comparada" screen renders the two-year table with column
   headers for the year pair.
4. The existing `AnnualScreen` ("Anual" nav item) is unchanged.

**Verify**: `npm run check` → exit 0. `npm run e2e` → all pass, no visual
regression in existing screens.

---

## Test plan

### New tests in `YearGridScreen.test.tsx`

| Test case                                           | What it verifies                                                                         |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `renders all 12 month sections`                     | 12 `<section aria-label>` elements appear; "Janeiro" and "Dezembro" headings present     |
| `shows termômetro coloring on non-null Saldo cells` | at least one `<td title="Saldo …">` is rendered when MONTH_GRID has balance_cents        |
| `shows empty state per month when no data`          | `EmptyState` ("Sem lançamentos") shown for each month when `get_month_grid` returns `[]` |
| `shows year heading with the current year`          | `"Ano inteiro"` heading is present                                                       |

Structural model: `src/screens/AnnualScreen.test.tsx` and
`src/screens/dashboard/MonthLedgerCard.test.tsx`.

### New tests in `EconomiaCompareScreen.test.tsx`

| Test case                                 | What it verifies                                                                                 |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `renders both year columns in the header` | The page title and both year column headers appear                                               |
| `renders 12 month rows`                   | "Jan" through "Dez" rows are present                                                             |
| `total Economizado% is weighted`          | Two-month fixture: Jan=30%, Feb=10% → total=15% (not 20%); regression on the spreadsheet formula |
| `shows empty state when no data at all`   | "Sem dados de Economia" shown when all months have zero income and zero economia                 |

Structural model: `src/screens/AnnualScreen.test.tsx`.

Run: `npm run test:run` → all pass, including ≥ 8 new test cases total across
both new test files.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run typecheck` exits 0.
- [ ] `npm run lint` exits 0.
- [ ] `npm run test:run` exits 0; `YearGridScreen.test.tsx` has ≥ 4 passing
      tests; `EconomiaCompareScreen.test.tsx` has ≥ 4 passing tests, including
      the weighted-Economizado%-annual test.
- [ ] `npm run rust:check` exits 0 (Rust sources are untouched, but verify no
      accidental breakage).
- [ ] `npm run doctor` exits 0 (no new React Doctor findings).
- [ ] `npm run e2e` passes; existing screen screenshots show no visual regression.
- [ ] `npm run check` exits 0 (full gate).
- [ ] `grep -rn "YearGridScreen\|EconomiaCompareScreen" src/App.tsx` returns two
      import lines and two `{screen ===` branches.
- [ ] `grep -n '"ano-inteiro"\|"economia-compare"' src/shell/AppShell.tsx` returns
      entries in both the `Screen` type and `NAV_ANALYSIS`.
- [ ] No files outside the in-scope list are modified
      (`git diff --name-only` lists only the six in-scope files + two new test files).
- [ ] `src/screens/AnnualScreen.tsx` is unmodified (the dual-year view is
      additive, not a replacement).
- [ ] `plans/README.md` status row for plan 044 updated to DONE (or IN PROGRESS
      if partially landed).

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the "Current state" locations does not match the excerpts —
  `MonthLedgerCard.tsx:41-57`, `AnnualScreen.tsx:144-148`, `AppShell.tsx:22-31`
  or `AppShell.tsx:57-62` differ from the plan (the codebase has drifted since
  this plan was written).
- `getMonthGrid` or `getAnnualMetrics` are absent from `src/lib/api.ts`, or
  their signatures differ from `api.ts:663-665` and `api.ts:679-681`.
- `SAVINGS_MIN_BPS` is not exported from `src/screens/totaisStatus.ts`.
- The `InfoPopover` component does not exist in `src/design-system/components/`
  (substitute with a plain `<span>` as described in Step 2, but report).
- A step's `npm run typecheck` or `npm run test:run` fails twice after a
  reasonable fix attempt.
- The fix appears to require touching an out-of-scope file (e.g. adding a new
  Tauri command to `src-tauri/` or modifying `src/lib/api.ts`).
- The 12 parallel `useCommand` calls in `YearGridScreen` cause a React Rules of
  Hooks violation (this should not happen — the count is always exactly 12 — but
  if it does, stop and propose an alternative architecture, such as a single
  batched query or a custom hook array).
- Any `npm run e2e` screenshot shows visual corruption in an existing screen.

## Maintenance notes

- **The `YearGridScreen` makes 12 parallel Tauri IPC calls on mount.** On a
  cold cache this is 12 concurrent SQLite reads, each light. If this causes
  perceptible jank on slow machines, a follow-up plan could add a single
  batched command `get_year_grid(year: i32) -> Vec<MonthGridData>` on the Rust
  side and replace the 12 `useCommand` calls with one. Keep that command pure
  and testable (see `month_grid` in `forecast_cmds.rs:877-931` as the model).
- **The `EconomiaCompareScreen` default base year is `thisYear - 1`** (so the
  right column shows the current year). If the user has only one year of data,
  one column will show all empty rows — this is intentional (matches the
  spreadsheet behavior when a year column is empty).
- **The dual-year Economia view is NOT a replacement for `AnnualScreen`.**
  The existing "Anual" nav item (with the savings sparkline and the full
  performance/cost table) is preserved. The new "Economia comparada" view is
  narrower — Entradas/Economia/% only — focused on the savings-rate comparison
  the spreadsheet's savings tab provided.
- **Navigation order in `NAV_ANALYSIS`**: the new items are inserted between
  "Anual" and "Horizonte". If a future plan restructures the nav (e.g. plan
  016-style), the ordering should be revisited.
- **Reviewer checklist for the PR**:
  - Confirm the `useCommand` keys in `YearGridScreen` embed both year and month
    (e.g. `"month_grid:2026-03"`), not just the month, so changing the year
    triggers fresh fetches.
  - Confirm the weighted-Economizado%-annual formula in `EconomiaCompareScreen`
    uses `ΣEconomia / ΣEntradas` (not an average of the monthly `savings_rate_bps`
    values), matching `AnnualScreen.tsx:161-162`.
  - Confirm static style objects are hoisted outside both new components (no
    `{{ … }}` inline literals in JSX prop positions).
  - Confirm no Rust files were modified (no new commands needed).
