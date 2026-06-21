import { useState } from "react";
import { getMonthGrid, type MonthGridDay } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { MonthNav } from "../design-system/components/MonthNav";
import { EmptyState } from "../design-system/components/EmptyState";
import { Money } from "../design-system/components/Money";
import { fmtDayMonth } from "../lib/format";
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
] as const;

// Estilos estáticos hoistados fora do componente (convenção do React Compiler).
const PAGE_STYLE: React.CSSProperties = {
  maxWidth: 1100,
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
const SECTIONS_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-6)",
};
const H2_STYLE: React.CSSProperties = {
  fontSize: "var(--fs-title)",
  fontWeight: "var(--fw-bold)",
  margin: "0 0 var(--space-3)",
  color: "var(--text-strong)",
};
const BODY_STYLE: React.CSSProperties = { padding: 0 };
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
const TD_DATE: React.CSSProperties = {
  padding: "var(--space-2) var(--space-3)",
  whiteSpace: "nowrap",
};
const TD_NUM: React.CSSProperties = {
  textAlign: "right",
  padding: "var(--space-2) var(--space-3)",
  whiteSpace: "nowrap",
};
const TD_SALDO_EMPTY: React.CSSProperties = {
  ...TD_NUM,
  color: "var(--text-faint)",
};

interface MonthGrid {
  month: number;
  label: string;
  loading: boolean;
  data: MonthGridDay[];
}

export function YearGridScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);

  // 12 fetches paralelos — um por mês. As chaves embutem ano e mês, então mudar
  // o ano gera chaves novas e dispara buscas frescas. As setas nunca mudam de
  // número entre renders (sempre exatamente 12), respeitando as Rules of Hooks.
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

  const grids: MonthGrid[] = [
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
    <div style={PAGE_STYLE}>
      <header style={HEADER_STYLE}>
        <div>
          <h1 style={H1_STYLE}>Ano inteiro</h1>
          <p style={SUBTITLE_STYLE}>
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

      <div style={SECTIONS_STYLE}>
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

// Puramente apresentacional — sem estado, sem efeitos (React Compiler friendly).
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
      <h2 style={H2_STYLE}>{label}</h2>
      <div className="dash-card">
        <div className="dash-card__body" style={BODY_STYLE}>
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
                      <td style={TD_DATE}>{fmtDayMonth(d.date)}</td>
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
                        <td style={TD_SALDO_EMPTY}>—</td>
                      ) : (
                        <td
                          className="money"
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
