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

const tdNum: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-3) var(--space-4)",
};

export function AnnualScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);
  const q = useCommand(`annual_metrics:${year}`, () => getAnnualMetrics(year));
  const months = q.data?.months ?? [];

  // Linha TOTAL do ano (espelha o TOTAL da aba Economia da planilha). O Economizado% ANUAL é
  // ΣEconomia/ΣEntradas — NÃO a média das taxas mensais — e é o número que a meta 20–30% cobra.
  const totals = months.reduce(
    (a, m) => ({
      performance: a.performance + m.performance_cents,
      cost: a.cost + m.cost_of_living_cents,
      income: a.income + m.income_cents,
      economia: a.economia + m.economia_cents,
    }),
    { performance: 0, cost: 0, income: 0, economia: 0 },
  );
  const annualSavingsPct =
    totals.income > 0 ? Math.round((totals.economia / totals.income) * 100) : 0;
  const hasYearData = months.some(
    (m) => m.income_cents !== 0 || m.cost_of_living_cents !== 0,
  );
  // Verde quando dentro da faixa ideal anual do método (≥20%); âmbar abaixo.
  const savingsColor =
    annualSavingsPct >= 20 ? "var(--success-400)" : "var(--warning-400)";

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
            Entradas, economia e as métricas do mês, o ano inteiro de uma vez.
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
                <th style={th}>Entradas</th>
                <th style={th}>Economia</th>
                <th style={th}>
                  <InfoPopover term="economizado">Economizado</InfoPopover>
                </th>
                <th style={th}>Performance</th>
                <th style={th}>Custo de vida</th>
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
                    <td style={tdNum}>
                      <Money cents={m.income_cents} size="sm" sign="auto" />
                    </td>
                    <td style={tdNum}>
                      <Money cents={m.economia_cents} size="sm" />
                    </td>
                    <td
                      style={{
                        ...tdNum,
                        fontFamily: "var(--font-money)",
                        color: "var(--text)",
                      }}
                    >
                      {(m.savings_rate_bps / 100).toFixed(0)}%
                    </td>
                    <td style={tdNum}>
                      <Money cents={m.performance_cents} size="sm" sign="auto" />
                    </td>
                    <td style={tdNum}>
                      <Money cents={m.cost_of_living_cents} size="sm" />
                    </td>
                    <td style={tdNum}>
                      <Money cents={m.real_daily_avg_cents} size="sm" />
                    </td>
                  </tr>
                );
              })}
            </tbody>
            {hasYearData && (
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
                  <td style={tdNum}>
                    <Money cents={totals.income} size="sm" sign="auto" />
                  </td>
                  <td style={tdNum}>
                    <Money cents={totals.economia} size="sm" />
                  </td>
                  <td
                    title="Economizado no ano = total economizado ÷ total de entradas (meta 20–30%)"
                    style={{
                      ...tdNum,
                      fontFamily: "var(--font-money)",
                      color: savingsColor,
                    }}
                  >
                    {annualSavingsPct}%
                  </td>
                  <td style={tdNum}>
                    <Money cents={totals.performance} size="sm" sign="auto" />
                  </td>
                  <td style={tdNum}>
                    <Money cents={totals.cost} size="sm" />
                  </td>
                  <td style={{ ...tdNum, color: "var(--text-faint)" }}>—</td>
                </tr>
              </tfoot>
            )}
          </table>
        </div>
      )}
    </div>
  );
}
