import { getForecast } from "../lib/api";
import type { ForecastDay } from "../lib/api";
import { monthNamePtBR } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { Money, formatBRL } from "../design-system/components/Money";
import { EmptyState } from "../design-system/components/EmptyState";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";
import {
  saldoBand,
  SALDO_BAND_FILL as BAND_FILL,
  SALDO_BAND_LABEL as BAND_LABEL,
  SALDO_BAND_LEGEND as BAND_LEGEND,
  type SaldoBand,
} from "../lib/saldoHeatmap";

// Reexporta para os testes e telas que importam o termômetro a partir desta tela (origem histórica).
export { saldoBand, type SaldoBand };

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
  const byYm = new Map<string, MonthCol>();
  for (const d of daily) {
    const ym = d.date.slice(0, 7);
    let col = byYm.get(ym);
    if (!col) {
      const label = monthNamePtBR(`${ym}-01`);
      col = { ym, label: label.charAt(0).toUpperCase() + label.slice(1), days: [] };
      byYm.set(ym, col);
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
    return <EmptyState variant="skeleton" skeletonRows={6} />;
  }
  if (forecastQ.error || !forecast || forecast.daily.length === 0) {
    return (
      <EmptyState
        title="Sem horizonte para projetar"
        description="O Horizonte mostra o saldo dia a dia no mesmo termômetro da planilha (verde = folga, vermelho = aperto). Para ver o futuro, lance as entradas e saídas dos próximos meses."
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
          Saldo projetado dia a dia, no mesmo termômetro da planilha: quanto mais verde,
          mais folga; quanto mais vermelho, mais aperto.
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
                  role="img"
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
                      // --text (não --text-muted): garante >=4.5:1 sobre TODAS as faixas-fundo
                      // do heatmap (as faixas verdes/vermelhas fortes derrubavam o muted < AA).
                      fontSize: "var(--fs-sm)",
                      color: "var(--text)",
                      width: 22,
                    }}
                  >
                    {d.day}
                  </span>
                  {/* `sign="none"` + --text: o saldo herda alto contraste (>=4.5:1 em TODAS as
                      faixas). O sinal +/− já é carregado pela COR da faixa-fundo; colorir o número
                      por cima caía abaixo de AA nas faixas fortes (vermelho/verde 0.3+). */}
                  <span aria-hidden="true" style={{ color: "var(--text)" }}>
                    <Money cents={d.balance} size="sm" sign="none" />
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
