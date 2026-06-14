import { useState } from "react";
import { getAnnualMetrics } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import { InfoPopover } from "../design-system/components/InfoPopover";

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

const th: React.CSSProperties = {
  textAlign: "right",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  padding: "var(--space-3) var(--space-4)",
};

export function AnnualScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);
  const q = useCommand(`annual_metrics:${year}`, () => getAnnualMetrics(year));
  const months = q.data?.months ?? [];

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
            Visão anual
          </h1>
          <p
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-1) 0 0",
            }}
          >
            As 4 métricas-herói mês a mês, o ano inteiro de uma vez.
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

      {q.loading ? (
        <div style={{ color: "var(--text-muted)" }}>Carregando o ano…</div>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table
            style={{
              width: "100%",
              borderCollapse: "collapse",
              fontVariantNumeric: "tabular-nums",
            }}
          >
            <thead>
              <tr style={{ borderBottom: "var(--bw-hair) solid var(--border)" }}>
                <th style={{ ...th, textAlign: "left" }}>Mês</th>
                <th style={th}>Performance</th>
                <th style={th}>Custo de vida</th>
                <th style={th}>
                  <InfoPopover term="economizado">Economizado</InfoPopover>
                </th>
                <th style={th}>Diário médio</th>
              </tr>
            </thead>
            <tbody>
              {months.map((m) => {
                const empty = m.income_cents === 0 && m.cost_of_living_cents === 0;
                return (
                  <tr
                    key={m.month}
                    style={{
                      borderBottom: "var(--bw-hair) solid var(--border)",
                      opacity: empty ? 0.45 : 1,
                    }}
                  >
                    <td
                      style={{
                        padding: "var(--space-3) var(--space-4)",
                        fontWeight: "var(--fw-semibold)",
                        color: "var(--text)",
                      }}
                    >
                      {MONTHS_PT[m.month - 1]}
                    </td>
                    <td
                      style={{
                        textAlign: "right",
                        padding: "var(--space-3) var(--space-4)",
                      }}
                    >
                      <Money cents={m.performance_cents} size="sm" sign="auto" />
                    </td>
                    <td
                      style={{
                        textAlign: "right",
                        padding: "var(--space-3) var(--space-4)",
                      }}
                    >
                      <Money cents={m.cost_of_living_cents} size="sm" />
                    </td>
                    <td
                      style={{
                        textAlign: "right",
                        padding: "var(--space-3) var(--space-4)",
                        fontFamily: "var(--font-money)",
                        color: "var(--text)",
                      }}
                    >
                      {(m.savings_rate_bps / 100).toFixed(0)}%
                    </td>
                    <td
                      style={{
                        textAlign: "right",
                        padding: "var(--space-3) var(--space-4)",
                      }}
                    >
                      <Money cents={m.real_daily_avg_cents} size="sm" />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
