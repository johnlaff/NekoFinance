import "./ano.css";
import { useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import {
  getAnnualMetrics,
  getForecast,
  getMonthGrid,
  isTauri,
  type MonthMetric,
  type MonthEnd,
  type MonthGridDay,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { todayISO } from "../lib/format";
import {
  fmtBRL,
  fmtCompact,
  fmtSigned,
  MES,
  MES_ABBR,
  saldoBand,
} from "../lib/nkFormat";

const TAB_OPTIONS = [
  { value: "ano", label: "Este ano" },
  { value: "cmp", label: "Comparar anos" },
];

// Stable fetchers (module-level, no inline arrows — required by useCommand contract).
function fetchForecast() {
  return getForecast();
}

// Stable fetcher cache keyed by year — avoids re-creating inline arrows that
// would break useCommand's dep-tracking contract.
const _fetcherCache = new Map<number, () => ReturnType<typeof getAnnualMetrics>>();
function stableFetcher(year: number): () => ReturnType<typeof getAnnualMetrics> {
  const cached = _fetcherCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _fetcherCache.set(year, fn);
  return fn;
}

const _historicalEndFetcherCache = new Map<string, () => Promise<MonthEnd[]>>();
function stableHistoricalEndFetcher(
  year: number,
  today: string,
): () => Promise<MonthEnd[]> {
  const key = `${year}:${today}`;
  const cached = _historicalEndFetcherCache.get(key);
  if (cached) return cached;
  const fn = async () => {
    const monthNums = pastMonthNumbersForYear(year, today);
    const grids = await Promise.all(
      monthNums.map(async (month) => ({
        month,
        days: await getMonthGrid(year, month),
      })),
    );
    return grids.flatMap(({ month, days }) => {
      const balance = lastNonNullBalance(days);
      return balance == null ? [] : [{ year, month, balance_cents: balance }];
    });
  };
  _historicalEndFetcherCache.set(key, fn);
  return fn;
}

// ------------------------------------------------------------------ helpers --

function yearOf(iso: string): number {
  return parseInt(iso.split("-")[0] ?? "0", 10);
}

function monthIndexOf(iso: string): number {
  return parseInt(iso.split("-")[1] ?? "1", 10) - 1;
}

function buildEndMap(ends: MonthEnd[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const e of ends) {
    map.set(`${e.year}-${e.month}`, e.balance_cents);
  }
  return map;
}

function pastMonthNumbersForYear(year: number, today: string): number[] {
  const forecastYear = yearOf(today);
  const currentMonth = monthIndexOf(today) + 1;
  if (year < forecastYear) return Array.from({ length: 12 }, (_, i) => i + 1);
  if (year > forecastYear) return [];
  return Array.from({ length: Math.max(0, currentMonth - 1) }, (_, i) => i + 1);
}

function lastNonNullBalance(days: MonthGridDay[]): number | null {
  for (let i = days.length - 1; i >= 0; i -= 1) {
    const balance = days[i]?.balance_cents;
    if (balance != null) return balance;
  }
  return null;
}

function realizedMonthCutoff(
  year: number,
  forecastYear: number,
  currentMonthIdx: number,
): number {
  if (year < forecastYear) return 11;
  if (year > forecastYear) return -1;
  return currentMonthIdx;
}

interface AnnualTotals {
  income: number;
  saidaTotal: number;
  diario: number;
  economia: number;
  performance: number;
}

function computeTotals(realized: MonthMetric[]): AnnualTotals {
  return realized.reduce<AnnualTotals>(
    (acc, m) => ({
      income: acc.income + m.income_cents,
      saidaTotal: acc.saidaTotal + m.cost_of_living_cents,
      diario: acc.diario + m.daily_out_cents,
      economia: acc.economia + m.economia_cents,
      performance: acc.performance + m.performance_cents,
    }),
    { income: 0, saidaTotal: 0, diario: 0, economia: 0, performance: 0 },
  );
}

// Pad months array to exactly 12 rows, filling gaps with zero-value rows.
function padTo12(months: MonthMetric[], year: number): MonthMetric[] {
  return Array.from({ length: 12 }, (_, i) => {
    const found = months.find((m) => m.month === i + 1);
    return (
      found ?? {
        year,
        month: i + 1,
        income_cents: 0,
        performance_cents: 0,
        cost_of_living_cents: 0,
        fixed_out_cents: 0,
        daily_out_cents: 0,
        cartao_cents: 0,
        real_daily_avg_cents: 0,
        economia_cents: 0,
        patrimonio_cents: 0,
        daily_projected_cents: 0,
        savings_rate_bps: 0,
      }
    );
  });
}

function getEcon(arr: MonthMetric[], mNum: number): number {
  return arr.find((m) => m.month === mNum)?.economia_cents ?? 0;
}

// ------------------------------------------------------------------ sub-components --

function YearNav({
  year,
  onPrev,
  onNext,
  onToday,
  atToday,
}: {
  year: number;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  atToday: boolean;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
      <button
        type="button"
        onClick={onPrev}
        aria-label="Ano anterior"
        className="ano-nav-btn"
      >
        <ChevronLeft size={14} strokeWidth={1.75} />
      </button>
      <button
        type="button"
        onClick={onToday}
        disabled={atToday}
        aria-label="Ir para o ano atual"
        className="ano-nav-year"
        style={{ cursor: atToday ? "default" : "pointer" }}
      >
        {year}
      </button>
      <button
        type="button"
        onClick={onNext}
        aria-label="Próximo ano"
        className="ano-nav-btn"
      >
        <ChevronRight size={14} strokeWidth={1.75} />
      </button>
    </div>
  );
}

function AnoChart({
  months,
  currentMonthIdx,
}: {
  months: MonthMetric[];
  currentMonthIdx: number;
}) {
  const maxPerf = Math.max(...months.map((m) => Math.abs(m.performance_cents)), 1);

  return (
    <>
      <div className="ano-chart">
        {months.map((m) => {
          const mIdx = m.month - 1;
          const h = (Math.abs(m.performance_cents) / maxPerf) * 100;
          const future = mIdx > currentMonthIdx;
          const isCurrent = mIdx === currentMonthIdx;
          return (
            <div className="ano-col" key={m.month}>
              <div
                className="ano-col__bar"
                style={{
                  height: `${h}%`,
                  background:
                    m.performance_cents >= 0 ? "var(--money-pos)" : "var(--money-neg)",
                  opacity: future ? 0.3 : isCurrent ? 1 : 0.7,
                }}
              />
              <span className="ano-col__m">{MES_ABBR[mIdx]}</span>
            </div>
          );
        })}
      </div>
      <div className="ano-chart__caption">
        Resultado por mês · meses à frente em tom mais claro (projeção)
      </div>
    </>
  );
}

function AnoTable({
  months,
  currentMonthIdx,
  endMap,
  year,
}: {
  months: MonthMetric[];
  currentMonthIdx: number;
  endMap: Map<string, number>;
  year: number;
}) {
  const rows = padTo12(months, year);
  const realized = rows.filter((m) => m.month - 1 <= currentMonthIdx);
  const totals = computeTotals(realized);
  const econPct = totals.income > 0 ? (totals.economia / totals.income) * 100 : 0;

  return (
    <table className="ano-tbl">
      <thead>
        <tr>
          <th>Mês</th>
          <th>Entradas</th>
          <th>Saída total</th>
          <th>Diário</th>
          <th>Economia</th>
          <th>Resultado</th>
          <th>Saldo fim</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((m) => {
          const mIdx = m.month - 1;
          const isCurrent = mIdx === currentMonthIdx;
          const isFuture = mIdx > currentMonthIdx;
          const rowClass = isCurrent ? "is-current" : isFuture ? "is-future" : "";
          const endBal = endMap.get(`${year}-${m.month}`);
          const band = saldoBand(endBal ?? null);
          // Saldo fim can come from forecast for current/future months or from
          // month-grid history for realized months in any displayed year.
          const showEndBal = endBal !== undefined;
          return (
            <tr key={m.month} className={rowClass}>
              <td>{MES[mIdx]}</td>
              <td
                style={{
                  color: mIdx <= currentMonthIdx ? "var(--money-pos)" : undefined,
                }}
              >
                {fmtBRL(m.income_cents)}
              </td>
              <td>{fmtBRL(m.cost_of_living_cents)}</td>
              <td>{fmtBRL(m.daily_out_cents)}</td>
              <td>{fmtBRL(m.economia_cents)}</td>
              <td
                style={{
                  color:
                    m.performance_cents >= 0 ? "var(--money-pos)" : "var(--money-neg)",
                }}
              >
                {fmtSigned(m.performance_cents)}
              </td>
              <td style={{ color: showEndBal ? band.text : "var(--text-faint)" }}>
                {showEndBal ? fmtBRL(endBal) : "—"}
              </td>
            </tr>
          );
        })}
      </tbody>
      <tfoot>
        <tr>
          <td>Realizado</td>
          <td>{fmtBRL(totals.income)}</td>
          <td>{fmtBRL(totals.saidaTotal)}</td>
          <td>{fmtBRL(totals.diario)}</td>
          <td>{fmtBRL(totals.economia)}</td>
          <td
            style={{
              color: totals.performance >= 0 ? "var(--money-pos)" : "var(--money-neg)",
            }}
          >
            {fmtSigned(totals.performance)}
          </td>
          <td
            title={`Economizado acumulado: ${econPct.toFixed(0)}%`}
            style={{ color: "var(--text-faint)" }}
          >
            {econPct.toFixed(0)}%
          </td>
        </tr>
      </tfoot>
    </table>
  );
}

function AnoCmpSection({
  yearA,
  yearB,
  monthsA,
  monthsB,
  currentMonthIdx,
  currentYear,
}: {
  yearA: number;
  yearB: number;
  monthsA: MonthMetric[];
  monthsB: MonthMetric[];
  currentMonthIdx: number;
  currentYear: number;
}) {
  const pairs = MES_ABBR.map((_, i) => ({
    a: getEcon(monthsA, i + 1),
    b: getEcon(monthsB, i + 1),
  }));

  const maxEcon = Math.max(...pairs.map((p) => Math.max(p.a, p.b)), 1);
  const yearBisCurrent = yearB === currentYear;

  return (
    <section className="card">
      <div className="ano-legend">
        <span>
          <i style={{ background: "var(--chart-3)" }} />
          {yearA}
        </span>
        <span>
          <i style={{ background: "var(--primary)" }} />
          {yearB}
        </span>
        <span style={{ marginLeft: "auto", color: "var(--text-faint)" }}>
          Economia guardada por mês
        </span>
      </div>
      <div className="ano-cmp">
        {MES_ABBR.map((mm, i) => {
          const econA = pairs[i]?.a ?? 0;
          const econB = pairs[i]?.b ?? 0;
          const isFutureB = yearBisCurrent && i > currentMonthIdx;
          return (
            <div className="ano-cmp__row" key={mm}>
              <span className="ano-cmp__m">{mm}</span>
              <span
                className="ano-cmp__bar"
                style={{
                  width: `${Math.max(12, (econA / maxEcon) * 100)}%`,
                  background: "var(--chart-3)",
                }}
              >
                {fmtCompact(econA)}
              </span>
              <span
                className="ano-cmp__bar"
                style={{
                  width: `${Math.max(12, (econB / maxEcon) * 100)}%`,
                  background: "var(--primary)",
                  opacity: isFutureB ? 0.4 : 1,
                }}
              >
                {fmtCompact(econB)}
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}

// ------------------------------------------------------------------ main screen --

export function AnnualScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);
  const [tab, setTab] = useState("ano");

  // Annual metrics for selected year
  const annualQ = useCommand(`annual_metrics:${year}`, stableFetcher(year));
  // Forecast: month-end balances + today date (current-month detection)
  const forecastQ = useCommand("get_forecast", fetchForecast);

  // Compare tab fetches year-1 vs year
  const yearA = year - 1;
  const yearB = year;
  const cmpQA = useCommand(`annual_metrics:${yearA}`, stableFetcher(yearA));
  const cmpQB = useCommand(`annual_metrics:${yearB}`, stableFetcher(yearB));

  const today = forecastQ.data?.today ?? todayISO();
  const forecastYear = today ? yearOf(today) : thisYear;
  const currentMonthIdx = today ? monthIndexOf(today) : new Date().getMonth();
  const displayedMonthCutoff = realizedMonthCutoff(year, forecastYear, currentMonthIdx);
  const historicalEndsQ = useCommand(
    `month_grid_end_balances:${year}:${today}`,
    stableHistoricalEndFetcher(year, today),
  );

  const months: MonthMetric[] = annualQ.data?.months ?? [];
  const historicalEndMap = buildEndMap(historicalEndsQ.data ?? []);
  const endMap = buildEndMap(forecastQ.data?.month_end ?? []);
  for (const [key, value] of historicalEndMap) endMap.set(key, value);

  // KPI totals over realized months only (matches prototype)
  const realizedMonths = months.filter((m) => m.month - 1 <= displayedMonthCutoff);
  const totEnt = realizedMonths.reduce((s, m) => s + m.income_cents, 0);
  const totSaida = realizedMonths.reduce((s, m) => s + m.cost_of_living_cents, 0);
  // Nunca re-derivar: Performance = Entradas − (custo de vida + economia + patrimônio), já
  // calculada pelo motor. Somar aqui de novo (totEnt − totSaida) ignora economia/patrimônio
  // e diverge do rodapé "Realizado" da tabela abaixo.
  const totPerf = realizedMonths.reduce((s, m) => s + m.performance_cents, 0);
  const totEcon = realizedMonths.reduce((s, m) => s + m.economia_cents, 0);
  const econPct = totEnt > 0 ? (totEcon / totEnt) * 100 : 0;

  const loading =
    annualQ.loading ||
    historicalEndsQ.loading ||
    (tab === "cmp" && (cmpQA.loading || cmpQB.loading));

  // Pad to 12 for the bar chart (need all 12 slots)
  const rows12 = padTo12(months, year);

  return (
    <div className="ano">
      <div className="ano-head">
        <YearNav
          year={year}
          onPrev={() => setYear((y) => y - 1)}
          onNext={() => setYear((y) => y + 1)}
          onToday={() => setYear(thisYear)}
          atToday={year === thisYear}
        />
        <SegmentedControl
          size="sm"
          ariaLabel="Visão"
          value={tab}
          onChange={setTab}
          options={TAB_OPTIONS}
        />
      </div>

      {loading ? (
        <div
          style={{
            height: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-faint)",
            fontSize: 13,
          }}
        >
          Carregando…
        </div>
      ) : tab === "ano" ? (
        <>
          <div className="ano-kpis">
            <div className="ano-kpi">
              <p className="ano-kpi__l">Entradas no ano</p>
              <div className="ano-kpi__v" style={{ color: "var(--money-pos)" }}>
                {fmtCompact(totEnt)}
              </div>
            </div>
            <div className="ano-kpi">
              <p className="ano-kpi__l">Saída total</p>
              <div className="ano-kpi__v" style={{ color: "var(--text-strong)" }}>
                {fmtCompact(totSaida)}
              </div>
            </div>
            <div className="ano-kpi">
              <p className="ano-kpi__l">Performance acum.</p>
              <div
                className="ano-kpi__v"
                style={{
                  color: totPerf >= 0 ? "var(--money-pos)" : "var(--money-neg)",
                }}
              >
                {fmtCompact(totPerf)}
              </div>
            </div>
            <div className="ano-kpi">
              <p className="ano-kpi__l">Economizado</p>
              <div
                className="ano-kpi__v"
                style={{
                  color: econPct >= 20 ? "var(--money-pos)" : "var(--warning-400)",
                }}
              >
                {econPct.toFixed(0)}%
              </div>
            </div>
          </div>

          <section className="card">
            <AnoChart months={rows12} currentMonthIdx={displayedMonthCutoff} />
          </section>

          <section className="card" style={{ overflowX: "auto" }}>
            <AnoTable
              months={months}
              currentMonthIdx={displayedMonthCutoff}
              endMap={endMap}
              year={year}
            />
          </section>

          {!isTauri && (
            <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
              Preview web — abra o app desktop para ver seus dados.
            </p>
          )}
        </>
      ) : (
        <AnoCmpSection
          yearA={yearA}
          yearB={yearB}
          monthsA={cmpQA.data?.months ?? []}
          monthsB={cmpQB.data?.months ?? []}
          currentMonthIdx={currentMonthIdx}
          currentYear={thisYear}
        />
      )}
    </div>
  );
}
