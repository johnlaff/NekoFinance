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
] as const;

// Estilos estáticos (convenção do React Compiler — hoistar fora do componente).
const PAGE_STYLE: React.CSSProperties = {
  maxWidth: 860,
  margin: "0 auto",
  padding: "var(--space-2)",
};
const HEADER_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "var(--space-4)",
  marginBottom: "var(--space-6)",
  flexWrap: "wrap",
};
const H1_STYLE: React.CSSProperties = {
  fontSize: "var(--fs-h2)",
  fontWeight: "var(--fw-bold)",
  letterSpacing: "var(--ls-tight)",
  margin: 0,
};
const SUBTITLE_STYLE: React.CSSProperties = {
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  margin: "var(--space-1) 0 0",
};
const SCROLL_STYLE: React.CSSProperties = { overflowX: "auto" };
const TABLE_STYLE: React.CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  fontVariantNumeric: "tabular-nums",
};

const TH: React.CSSProperties = {
  textAlign: "right",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  padding: "var(--space-3) var(--space-4)",
};
const TH_LEFT: React.CSSProperties = { ...TH, textAlign: "left" };
const TH_YEAR: React.CSSProperties = {
  ...TH,
  textAlign: "center",
  color: "var(--text-strong)",
  fontSize: "var(--fs-sm)",
  borderBottom: "2px solid var(--border-strong)",
  letterSpacing: 0,
  textTransform: "none",
};
const TH_YEAR_A: React.CSSProperties = {
  ...TH_YEAR,
  borderRight: "var(--bw-strong) solid var(--border-strong)",
};

const DIVIDER: React.CSSProperties = {
  borderLeft: "var(--bw-strong) solid var(--border-strong)",
};
const TH_DIVIDER: React.CSSProperties = { ...TH, ...DIVIDER };
const TH_GROUP_END: React.CSSProperties = {
  ...TH,
  borderRight: "var(--bw-strong) solid var(--border-strong)",
};

const TD_NUM: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-3) var(--space-4)",
  fontVariantNumeric: "tabular-nums",
};
const TD_NUM_DIVIDER: React.CSSProperties = { ...TD_NUM, ...DIVIDER };
const TD_NUM_GROUP_END: React.CSSProperties = {
  ...TD_NUM,
  borderRight: "var(--bw-strong) solid var(--border-strong)",
};
const TD_MONTH: React.CSSProperties = {
  padding: "var(--space-3) var(--space-4)",
  fontWeight: "var(--fw-semibold)",
  color: "var(--text)",
};
const ROW_STYLE: React.CSSProperties = {
  borderBottom: "var(--bw-hair) solid var(--border)",
};
const HEAD_ROW_STYLE: React.CSSProperties = {
  borderBottom: "var(--bw-hair) solid var(--border)",
};
const FOOT_ROW_STYLE: React.CSSProperties = {
  borderTop: "var(--bw-strong) solid var(--border-strong)",
  fontWeight: "var(--fw-bold)",
};
const TD_TOTAL_LABEL: React.CSSProperties = {
  padding: "var(--space-3) var(--space-4)",
  textTransform: "uppercase",
  letterSpacing: "var(--ls-label)",
  fontSize: "var(--fs-label)",
  color: "var(--text)",
};

const SAVINGS_MIN_PCT = SAVINGS_MIN_BPS / 100;

function savingsColor(pct: number): string {
  if (pct > 30) return "var(--primary)";
  if (pct >= SAVINGS_MIN_PCT) return "var(--success-400)";
  return "var(--warning-400)";
}

interface YearTotals {
  income: number;
  economia: number;
  savingsPct: number;
}

// Totais do ano (Economizado% ponderado = ΣEconomia / ΣEntradas — espelha o
// TOTAL da aba Economia da planilha; NÃO a média das taxas mensais).
function yearTotals(months: MonthMetric[]): YearTotals {
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

function monthEmpty(m: MonthMetric | undefined): boolean {
  return !m || (m.income_cents === 0 && m.economia_cents === 0);
}

/** Três colunas de um mês para um ano (Entradas · Economia · Economizado%). */
function YearMonthCells({
  m,
  leadingDivider,
  trailingDivider,
}: {
  m: MonthMetric | undefined;
  leadingDivider: boolean;
  trailingDivider: boolean;
}) {
  const empty = monthEmpty(m);
  const pct = m ? m.savings_rate_bps / 100 : 0;
  const dim = empty ? 0.4 : 1;
  return (
    <>
      <td
        style={
          leadingDivider
            ? { ...TD_NUM_DIVIDER, opacity: dim }
            : { ...TD_NUM, opacity: dim }
        }
      >
        {m ? <Money cents={m.income_cents} size="sm" sign="auto" /> : "—"}
      </td>
      <td style={{ ...TD_NUM, opacity: dim }}>
        {m ? <Money cents={m.economia_cents} size="sm" /> : "—"}
      </td>
      <td
        style={{
          ...(trailingDivider ? TD_NUM_GROUP_END : TD_NUM),
          fontFamily: "var(--font-money)",
          color: empty ? "var(--text-faint)" : savingsColor(pct),
          opacity: dim,
        }}
      >
        {empty ? "—" : `${pct.toFixed(0)}%`}
      </td>
    </>
  );
}

/** Linha de total de um ano (3 colunas). */
function YearTotalCells({
  tot,
  leadingDivider,
  trailingDivider,
}: {
  tot: YearTotals;
  leadingDivider: boolean;
  trailingDivider: boolean;
}) {
  return (
    <>
      <td style={leadingDivider ? TD_NUM_DIVIDER : TD_NUM}>
        <Money cents={tot.income} size="sm" sign="auto" />
      </td>
      <td style={TD_NUM}>
        <Money cents={tot.economia} size="sm" />
      </td>
      <td
        title="Economizado anual = ΣEconomia ÷ ΣEntradas (meta 20–30%)"
        style={{
          ...(trailingDivider ? TD_NUM_GROUP_END : TD_NUM),
          fontFamily: "var(--font-money)",
          color: savingsColor(tot.savingsPct),
        }}
      >
        {tot.savingsPct}%
      </td>
    </>
  );
}

export function EconomiaCompareScreen() {
  const thisYear = new Date().getFullYear();
  // "base year" é o ano mais antigo (coluna esquerda); coluna direita = base + 1.
  const [baseYear, setBaseYear] = useState(thisYear - 1);
  const yearA = baseYear;
  const yearB = baseYear + 1;

  const qA = useCommand(`annual_metrics:${yearA}`, () => getAnnualMetrics(yearA));
  const qB = useCommand(`annual_metrics:${yearB}`, () => getAnnualMetrics(yearB));

  const monthsA: MonthMetric[] = qA.data?.months ?? [];
  const monthsB: MonthMetric[] = qB.data?.months ?? [];

  const totA = yearTotals(monthsA);
  const totB = yearTotals(monthsB);
  const loading = qA.loading || qB.loading;
  const hasAnyData =
    monthsA.some((m) => m.income_cents !== 0 || m.economia_cents !== 0) ||
    monthsB.some((m) => m.income_cents !== 0 || m.economia_cents !== 0);

  return (
    <div style={PAGE_STYLE}>
      <header style={HEADER_STYLE}>
        <div>
          <h1 style={H1_STYLE}>
            Economia: {yearA} vs {yearB}
          </h1>
          <p style={SUBTITLE_STYLE}>
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
            <div style={SCROLL_STYLE}>
              <table style={TABLE_STYLE}>
                <thead>
                  {/* Linha de cabeçalho dos anos */}
                  <tr>
                    <th style={TH_LEFT} rowSpan={2} scope="col">
                      Mês
                    </th>
                    <th colSpan={3} style={TH_YEAR_A} scope="colgroup">
                      {yearA}
                    </th>
                    <th colSpan={3} style={TH_YEAR} scope="colgroup">
                      {yearB}
                    </th>
                  </tr>
                  {/* Linha de cabeçalho das colunas */}
                  <tr style={HEAD_ROW_STYLE}>
                    <th style={TH} scope="col">
                      Entradas
                    </th>
                    <th style={TH} scope="col">
                      Economia
                    </th>
                    <th style={TH_GROUP_END} scope="col">
                      <InfoPopover term="economizado">Economizado</InfoPopover>
                    </th>
                    <th style={TH_DIVIDER} scope="col">
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
                  {Array.from({ length: 12 }, (_, i) => (
                    <tr key={i} style={ROW_STYLE}>
                      <td style={TD_MONTH}>{MONTHS_PT[i]}</td>
                      <YearMonthCells
                        m={monthsA[i]}
                        leadingDivider={false}
                        trailingDivider
                      />
                      <YearMonthCells
                        m={monthsB[i]}
                        leadingDivider
                        trailingDivider={false}
                      />
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr style={FOOT_ROW_STYLE}>
                    <td style={TD_TOTAL_LABEL}>Total</td>
                    <YearTotalCells tot={totA} leadingDivider={false} trailingDivider />
                    <YearTotalCells tot={totB} leadingDivider trailingDivider={false} />
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
