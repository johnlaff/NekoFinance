import "./calendario.css";
import { useState } from "react";
import {
  getForecast,
  getAnnualMetrics,
  getMonthGrid,
  isTauri,
  type ForecastDay,
  type MonthMetric,
  type MonthGridDay,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { MonthNav } from "../design-system/components/MonthNav";
import { fmtBRL, fmtCompact, MES, saldoBand } from "../lib/nkFormat";
import { todayISO } from "../lib/format";

const DOW = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];

const SEG_OPTIONS = [
  { value: "mes", label: "Mês" },
  { value: "ano", label: "Ano inteiro" },
];

/** Builds the day-of-month matrix for a given year/month (0-based month index).
 *  Returns an array of day numbers (1-N) or null for empty leading/trailing cells.
 *  The first column is Sunday (index 0). */
function monthMatrix(year: number, month: number): (number | null)[] {
  const dim = new Date(year, month + 1, 0).getDate();
  const firstWeekday = new Date(year, month, 1).getDay(); // 0=Sun
  const cells: (number | null)[] = [];
  for (let i = 0; i < firstWeekday; i++) cells.push(null);
  for (let d = 1; d <= dim; d++) cells.push(d);
  while (cells.length % 7 !== 0) cells.push(null);
  return cells;
}

/** Build an ISO date string from year, 0-based month, and day number. */
function isoDate(year: number, month: number, day: number): string {
  const mm = String(month + 1).padStart(2, "0");
  const dd = String(day).padStart(2, "0");
  return `${year}-${mm}-${dd}`;
}

/** Index ForecastDay[] into a Map keyed by ISO date for O(1) lookup. */
function indexByDate(days: ForecastDay[]): Map<string, ForecastDay> {
  const m = new Map<string, ForecastDay>();
  for (const d of days) m.set(d.date, d);
  return m;
}

function indexGridByDate(days: MonthGridDay[]): Map<string, MonthGridDay> {
  const m = new Map<string, MonthGridDay>();
  for (const d of days) m.set(d.date, d);
  return m;
}

const _annualFetcherCache = new Map<
  number,
  () => ReturnType<typeof getAnnualMetrics>
>();
function annualFetcher(year: number): () => ReturnType<typeof getAnnualMetrics> {
  const cached = _annualFetcherCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _annualFetcherCache.set(year, fn);
  return fn;
}

const _monthGridFetcherCache = new Map<string, () => ReturnType<typeof getMonthGrid>>();
function monthGridFetcher(
  year: number,
  month: number,
): () => ReturnType<typeof getMonthGrid> {
  const key = `${year}-${month}`;
  const cached = _monthGridFetcherCache.get(key);
  if (cached) return cached;
  const fn = () => getMonthGrid(year, month);
  _monthGridFetcherCache.set(key, fn);
  return fn;
}

/** Fetcher estável que agrega o month-grid dos 12 meses do ano (visão "Ano inteiro"):
 *  sem ele, as células dos meses fora do mês exibido ficariam sem saldo realizado. */
const _yearGridFetcherCache = new Map<number, () => Promise<MonthGridDay[]>>();
function yearGridFetcher(year: number): () => Promise<MonthGridDay[]> {
  const cached = _yearGridFetcherCache.get(year);
  if (cached) return cached;
  const fn = async () => {
    const grids = await Promise.all(
      Array.from({ length: 12 }, (_, i) => getMonthGrid(year, i + 1)),
    );
    return grids.flat();
  };
  _yearGridFetcherCache.set(year, fn);
  return fn;
}

const emptyGridFetcher = () => Promise.resolve<MonthGridDay[]>([]);

const LEGEND = (
  <div className="cal-legend">
    <span>
      <i style={{ background: "var(--saldo-band-comfortable-fill)" }} />
      Folga
    </span>
    <span>
      <i style={{ background: "var(--saldo-band-ok-fill)" }} />
      OK
    </span>
    <span>
      <i style={{ background: "var(--saldo-band-tight-fill)" }} />
      Apertado
    </span>
    <span>
      <i style={{ background: "var(--saldo-band-negative-fill)" }} />
      Negativo
    </span>
    <span>
      <i style={{ background: "var(--saldo-band-critical-fill)" }} />
      Crítico
    </span>
  </div>
);

export function YearGridScreen() {
  const TODAY = todayISO();
  const thisYear = parseInt(TODAY.slice(0, 4), 10);
  const thisMonth = parseInt(TODAY.slice(5, 7), 10) - 1; // 0-based

  const [tab, setTab] = useState<string>("mes");
  const [off, setOff] = useState(0);

  // Clamp month offset to valid range (0–11).
  const rawMonth = thisMonth + off;
  const clampedMonth = Math.max(0, Math.min(11, rawMonth));

  // Tauri data: forecast gives today-forward projected balances; month-grid restores past sheet balances.
  const forecastQ = useCommand("get_forecast", getForecast);
  const forecast = forecastQ.data;
  const dailyAll: ForecastDay[] = forecast?.daily ?? [];
  const forecastBalanceMap = indexByDate(dailyAll);
  const monthNum = clampedMonth + 1;
  const monthGridQ = useCommand(
    `get_month_grid:${thisYear}:${monthNum}`,
    monthGridFetcher(thisYear, monthNum),
  );
  // Visão "Ano inteiro": agrega o grid dos 12 meses (a key muda com a aba, então o
  // hook roda sempre — sem hooks condicionais — e só busca o ano quando necessário).
  const yearGridQ = useCommand(
    tab === "ano" ? `get_month_grid:year:${thisYear}` : "get_month_grid:year:off",
    tab === "ano" ? yearGridFetcher(thisYear) : emptyGridFetcher,
  );
  const realizedBalanceMap = indexGridByDate([
    ...(yearGridQ.data ?? []),
    ...(monthGridQ.data ?? []),
  ]);

  // Annual metrics for the year-view month summaries.
  const annualQ = useCommand(`get_annual_metrics:${thisYear}`, annualFetcher(thisYear));
  const annualMetrics: MonthMetric[] = annualQ.data?.months ?? [];
  // Index by 0-based month for quick lookup (API months are 1-based).
  const monthMetricMap = new Map<number, MonthMetric>();
  for (const mm of annualMetrics) monthMetricMap.set(mm.month - 1, mm);

  const balanceForDate = (iso: string): number | null => {
    if (iso < TODAY) return realizedBalanceMap.get(iso)?.balance_cents ?? null;
    return forecastBalanceMap.get(iso)?.balance_cents ?? null;
  };

  // ---- Year view ----
  if (tab === "ano") {
    return (
      <div className="cal">
        <div className="cal-head">
          <div className="cal-title">Ano inteiro · {thisYear}</div>
          <SegmentedControl
            size="sm"
            ariaLabel="Visão"
            value={tab}
            onChange={setTab}
            options={SEG_OPTIONS}
          />
        </div>
        <section className="card">
          <div style={{ padding: "16px 18px 0" }}>{LEGEND}</div>
          <div className="cal-year">
            {MES.map((name, m) => {
              const cells = monthMatrix(thisYear, m);
              const metric = monthMetricMap.get(m);
              const perf = metric?.performance_cents ?? null;
              return (
                <div key={m}>
                  <div className="cal-mini__h">
                    <span>{name}</span>
                    {perf != null ? (
                      <span
                        className="cal-mini__perf"
                        style={{
                          color: perf >= 0 ? "var(--money-pos)" : "var(--money-neg)",
                        }}
                      >
                        {fmtCompact(perf)}
                      </span>
                    ) : null}
                  </div>
                  <div className="cal-mini__grid">
                    {cells.map((d, i) => {
                      if (d == null) return <span key={i} />;
                      const iso = isoDate(thisYear, m, d);
                      const balance = balanceForDate(iso);
                      const band = saldoBand(balance);
                      const future = iso > TODAY;
                      return (
                        <span
                          key={i}
                          className={
                            "cal-mini__cell" +
                            (iso === TODAY ? " cal-mini__cell--today" : "")
                          }
                          title={`${String(d).padStart(2, "0")}/${String(m + 1).padStart(2, "0")} · ${balance != null ? fmtBRL(balance) : "—"}`}
                          style={{
                            background:
                              band.fill === "transparent"
                                ? "var(--bg-subtle)"
                                : band.fill,
                            opacity: future ? 0.45 : 1,
                          }}
                        />
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </div>
        </section>
        {!isTauri && (
          <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
            Preview web — abra o app desktop para ver seus dados.
          </p>
        )}
      </div>
    );
  }

  // ---- Month view ----
  const m = clampedMonth;
  const cells = monthMatrix(thisYear, m);
  const atToday = m === thisMonth;

  return (
    <div className="cal">
      <div className="cal-head">
        <div className="cal-title">Calendário</div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <MonthNav
            label={`${MES[m]} de ${thisYear}`}
            atToday={atToday}
            onPrev={() => setOff((o) => o - 1)}
            onNext={() => setOff((o) => o + 1)}
            onToday={() => setOff(0)}
            prevLabel="Anterior"
            nextLabel="Próximo"
          />
          <SegmentedControl
            size="sm"
            ariaLabel="Visão"
            value={tab}
            onChange={setTab}
            options={SEG_OPTIONS}
          />
        </div>
      </div>
      <section className="card">
        <div style={{ padding: "16px 18px 0" }}>{LEGEND}</div>
        <div className="cal-dow">
          {DOW.map((d) => (
            <span key={d}>{d}</span>
          ))}
        </div>
        <div className="cal-grid">
          {cells.map((d, i) => {
            if (d == null)
              return <div key={`empty-${i}`} className="cal-cell cal-cell--empty" />;
            const iso = isoDate(thisYear, m, d);
            const balance = balanceForDate(iso);
            const band = saldoBand(balance);
            const future = iso > TODAY;
            return (
              <div
                key={iso}
                className={
                  "cal-cell" +
                  (iso === TODAY ? " cal-cell--today" : "") +
                  (future ? " cal-cell--future" : "")
                }
                style={{
                  background:
                    band.fill === "transparent" ? "var(--surface)" : band.fill,
                }}
                title={
                  balance != null
                    ? `Saldo ${fmtBRL(balance)}`
                    : iso > TODAY
                      ? "Projeção indisponível"
                      : "Sem dados"
                }
              >
                <span className="cal-cell__d">{d}</span>
                {balance != null ? (
                  <span className="cal-cell__s" style={{ color: band.text }}>
                    {fmtCompact(balance)}
                  </span>
                ) : (
                  <span className="cal-cell__s" style={{ color: "var(--text-faint)" }}>
                    —
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </section>
      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
