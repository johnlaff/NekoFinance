import { getForecast } from "../lib/api";
import type { ForecastDay } from "../lib/api";
import { monthNamePtBR } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { Money, formatBRL } from "../design-system/components/Money";
import { EmptyState } from "../design-system/components/EmptyState";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";

export type SaldoBand = "critical" | "negative" | "tight" | "ok" | "comfortable";

/** Faixa de saldo (heatmap). Thresholds em centavos, fiéis ao método. */
export function saldoBand(cents: number): SaldoBand {
  if (cents < -50000) return "critical";
  if (cents < 0) return "negative";
  if (cents < 100000) return "tight";
  if (cents < 200000) return "ok";
  return "comfortable";
}

const BAND_FILL: Record<SaldoBand, string> = {
  critical: "var(--saldo-band-critical-fill)",
  negative: "var(--saldo-band-negative-fill)",
  tight: "var(--saldo-band-tight-fill)",
  ok: "var(--saldo-band-ok-fill)",
  comfortable: "var(--saldo-band-comfortable-fill)",
};

const BAND_LEGEND: { band: SaldoBand; label: string }[] = [
  { band: "critical", label: "crítico" },
  { band: "negative", label: "negativo" },
  { band: "tight", label: "apertado" },
  { band: "ok", label: "ok" },
  { band: "comfortable", label: "folga" },
];

const BAND_LABEL: Record<SaldoBand, string> = Object.fromEntries(
  BAND_LEGEND.map((l) => [l.band, l.label]),
) as Record<SaldoBand, string>;

interface DayCell {
  day: number;
  balance: number;
  isToday: boolean;
}
interface MonthCol {
  ym: string;
  label: string;
  days: DayCell[];
}

/** Agrupa a série diária do forecast por ano-mês (uma coluna por mês). */
export function groupByMonth(daily: ForecastDay[], today: string): MonthCol[] {
  const cols: MonthCol[] = [];
  for (const d of daily) {
    const ym = d.date.slice(0, 7);
    let col = cols.find((c) => c.ym === ym);
    if (!col) {
      const label = monthNamePtBR(`${ym}-01`);
      col = { ym, label: label.charAt(0).toUpperCase() + label.slice(1), days: [] };
      cols.push(col);
    }
    col.days.push({
      day: Number(d.date.slice(8, 10)),
      balance: d.balance_cents,
      isToday: d.date === today,
    });
  }
  return cols;
}

export function HorizonteScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const forecast = forecastQ.data ?? null;

  if (forecastQ.loading) {
    return (
      <div style={{ padding: "var(--space-8)", color: "var(--text-muted)" }}>
        Carregando o horizonte de saldos…
      </div>
    );
  }
  if (forecastQ.error || !forecast || forecast.daily.length === 0) {
    return (
      <EmptyState
        title="Sem horizonte para projetar"
        description="Lance entradas e saídas futuras para ver o saldo projetado mês a mês."
      />
    );
  }

  const cols = groupByMonth(forecast.daily, forecast.today);

  return (
    <div style={{ padding: "var(--space-2)" }}>
      <header style={{ marginBottom: "var(--space-6)" }}>
        <h1
          style={{
            fontSize: "var(--fs-h2)",
            fontWeight: "var(--fw-bold)",
            letterSpacing: "var(--ls-tight)",
            margin: 0,
          }}
        >
          Horizonte de saldos
        </h1>
        <p
          style={{
            color: "var(--text-muted)",
            fontSize: "var(--fs-sm)",
            margin: "var(--space-1) 0 0",
          }}
        >
          Saldo projetado dia a dia. Verde é folga, vermelho é aperto.
        </p>
      </header>

      {/* Trajetória do saldo — a leitura principal, preenche a largura */}
      <section
        style={{
          background: "var(--surface)",
          border: "var(--bw-hair) solid var(--border)",
          borderRadius: "var(--radius-lg)",
          boxShadow: "var(--shadow-1)",
          padding: "var(--space-5) var(--space-5) var(--space-3)",
          marginBottom: "var(--space-6)",
        }}
      >
        <BalanceTrajectory daily={forecast.daily} today={forecast.today} />
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "var(--space-4)",
            marginTop: "var(--space-3)",
            paddingTop: "var(--space-3)",
            borderTop: "var(--bw-hair) solid var(--border)",
          }}
        >
          {BAND_LEGEND.map((l) => (
            <span
              key={l.band}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 7,
                fontSize: "var(--fs-sm)",
                color: "var(--text-muted)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: "var(--radius-xs)",
                  background: BAND_FILL[l.band],
                }}
              />
              {l.label}
            </span>
          ))}
        </div>
      </section>

      <h2
        style={{
          fontSize: "var(--fs-label)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--ls-label)",
          textTransform: "uppercase",
          color: "var(--text-faint)",
          margin: "0 0 var(--space-3)",
        }}
      >
        Detalhe diário
      </h2>

      <div
        role="group"
        aria-label="Saldo projetado por dia, agrupado por mês"
        style={{
          display: "flex",
          gap: "var(--space-4)",
          overflowX: "auto",
          paddingBottom: "var(--space-4)",
        }}
      >
        {cols.map((col) => (
          <div
            key={col.ym}
            role="group"
            aria-label={col.label}
            style={{ minWidth: 140, flexShrink: 0 }}
          >
            <div
              aria-hidden="true"
              style={{
                fontSize: "var(--fs-label)",
                fontWeight: "var(--fw-bold)",
                letterSpacing: "var(--ls-label)",
                textTransform: "uppercase",
                color: "var(--text-muted)",
                padding: "var(--space-2) var(--space-3)",
                position: "sticky",
                top: 0,
              }}
            >
              {col.label}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              {col.days.map((d) => (
                <div
                  key={d.day}
                  aria-current={d.isToday ? "date" : undefined}
                  aria-label={`Dia ${d.day}: saldo ${formatBRL(d.balance)} (${BAND_LABEL[saldoBand(d.balance)]})`}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: "var(--space-3)",
                    padding: "var(--space-2) var(--space-3)",
                    borderRadius: "var(--radius-sm)",
                    background: BAND_FILL[saldoBand(d.balance)],
                    outline: d.isToday ? "2px solid var(--border-focus)" : "none",
                    fontVariantNumeric: "tabular-nums",
                  }}
                >
                  <span
                    aria-hidden="true"
                    style={{
                      fontSize: "var(--fs-sm)",
                      color: "var(--text-muted)",
                      width: 22,
                    }}
                  >
                    {d.day}
                  </span>
                  <span aria-hidden="true">
                    <Money cents={d.balance} size="sm" sign="auto" />
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
