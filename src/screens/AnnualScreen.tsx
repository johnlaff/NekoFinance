import { useState } from "react";
import { getAnnualMetrics } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { EmptyState } from "../design-system/components/EmptyState";

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

/** Item da legenda do sparkline: amostra de cor + rótulo. */
function LegendDot({ color, label }: { color: string; label: string }) {
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
      <span
        aria-hidden="true"
        style={{ width: 9, height: 9, borderRadius: 2, background: color }}
      />
      {label}
    </span>
  );
}

/** Mini-barras de Economizado% por mês, com a faixa-meta 20–30% sombreada (tendência do ano). */
function EconomizadoSparkline({
  months,
}: {
  months: {
    month: number;
    savings_rate_bps: number;
    income_cents: number;
    cost_of_living_cents: number;
  }[];
}) {
  const data = months.map((m) => ({
    pct: m.savings_rate_bps / 100,
    empty: m.income_cents === 0 && m.cost_of_living_cents === 0,
  }));
  const maxPct = Math.max(40, ...data.map((d) => d.pct));
  const H = 56;
  const bandTop = ((maxPct - 30) / maxPct) * H;
  const bandHeight = (10 / maxPct) * H; // faixa 20–30%
  return (
    <div style={{ margin: "0 0 var(--space-6)" }}>
      <div
        role="img"
        aria-label="Tendência de Economizado% por mês, com a faixa-meta de 20 a 30% sombreada"
        style={{
          display: "flex",
          gap: 4,
          alignItems: "flex-end",
          height: H,
          position: "relative",
        }}
      >
        <span
          aria-hidden="true"
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            top: bandTop,
            height: bandHeight,
            background: "var(--success-tint)",
            borderRadius: 2,
          }}
        />
        {data.map((d, i) => {
          const h = d.empty ? 2 : Math.max(2, (d.pct / maxPct) * H);
          const color = d.empty
            ? "var(--border)"
            : d.pct > 30
              ? "var(--primary)"
              : d.pct >= 20
                ? "var(--success-400)"
                : "var(--warning-400)";
          return (
            <span
              key={i}
              title={`${MONTHS_PT[i]}: ${d.empty ? "—" : `${d.pct.toFixed(0)}%`}`}
              style={{
                flex: 1,
                height: h,
                background: color,
                borderRadius: "2px 2px 0 0",
                position: "relative",
                zIndex: 1,
              }}
            />
          );
        })}
      </div>
      {/* Legenda visível: o gráfico mostra Economizado% por mês contra a meta do método (20–30%). */}
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "var(--space-3)",
          alignItems: "center",
          marginTop: "var(--space-2)",
          fontSize: "var(--fs-micro)",
          color: "var(--text-faint)",
        }}
      >
        <span>Economizado% por mês:</span>
        <LegendDot color="var(--success-tint)" label="meta 20–30%" />
        <LegendDot color="var(--success-400)" label="dentro" />
        <LegendDot color="var(--warning-400)" label="abaixo" />
        <LegendDot color="var(--primary)" label="acima" />
      </div>
    </div>
  );
}

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
  // Sem nenhuma Economia no ano, o sparkline fica achatado no zero (confuso). Nesse caso mostramos
  // uma dica em vez do gráfico — a Economia entra pela aba dedicada (Configurações › Google Sheets).
  const hasEconomia = months.some((m) => m.economia_cents !== 0);
  // 3 estados (mesma lógica do economizadoStatus em Totais): >30% guardando além do ideal
  // (jade/steady), 20–30% dentro do ideal (verde), <20% aquém (âmbar).
  const savingsColor =
    annualSavingsPct > 30
      ? "var(--primary)"
      : annualSavingsPct >= 20
        ? "var(--success-400)"
        : "var(--warning-400)";

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
        <EmptyState variant="skeleton" skeletonRows={7} />
      ) : (
        <div className="dash-card">
          <div className="dash-card__body">
            {hasYearData &&
              (hasEconomia ? (
                <EconomizadoSparkline months={months} />
              ) : (
                <p
                  style={{
                    margin: "0 0 var(--space-6)",
                    fontSize: "var(--fs-sm)",
                    color: "var(--text-muted)",
                  }}
                >
                  Sem Economia registrada em {year} — importe a aba{" "}
                  <strong>Economia</strong> em Configurações › Google Sheets para ver a
                  tendência de quanto você guardou (meta 20–30%).
                </p>
              ))}
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
                            color: empty ? "var(--text-faint)" : "var(--text)",
                          }}
                        >
                          {empty ? "—" : `${(m.savings_rate_bps / 100).toFixed(0)}%`}
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
                      <td
                        style={{ ...tdNum, color: "var(--text-faint)" }}
                        title="Diário médio não tem total anual — médias não se somam"
                      >
                        —
                      </td>
                    </tr>
                  </tfoot>
                )}
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
