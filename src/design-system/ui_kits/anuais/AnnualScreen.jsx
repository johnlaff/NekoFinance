/* Neko Finance — Visão anual (anuais).
   Entradas, economia e métricas de todos os meses do ano em uma tabela.
   Inclui sparkline de Economizado% com faixa-meta 20–30% sombreada.
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.AnnualScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Money, MonthNav, InfoPopover } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectAnuaisCSS() {
  if (document.getElementById("anuais-css")) return;
  const s = document.createElement("style");
  s.id = "anuais-css";
  s.textContent = `
/* Layout */
.an { max-width: 860px; margin: 0 auto; padding: var(--space-2); }

/* Cabeçalho */
.an-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  margin-bottom: var(--space-6);
  flex-wrap: wrap;
}
.an-title {
  font-size: var(--fs-h2);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight);
  margin: 0;
  color: var(--text-strong);
}
.an-subtitle {
  color: var(--text-muted);
  font-size: var(--fs-sm);
  margin: var(--space-1) 0 0;
  line-height: var(--lh-normal);
}

/* Card */
.an-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-1);
}
.an-card__body {
  padding: var(--space-4) var(--space-6) var(--space-6);
}

/* Sparkline */
.an-spark {
  margin-bottom: var(--space-6);
}
.an-spark__bars {
  display: flex;
  gap: 4px;
  align-items: flex-end;
  height: 56px;
  position: relative;
}
.an-spark__band {
  position: absolute;
  left: 0;
  right: 0;
  background: var(--success-tint);
  border-radius: 2px;
  pointer-events: none;
}
.an-spark__bar {
  flex: 1;
  border-radius: 2px 2px 0 0;
  position: relative;
  z-index: 1;
  transition: opacity 0.15s ease;
}
@media (prefers-reduced-motion: reduce) {
  .an-spark__bar { transition: none; }
}
.an-spark__bar:hover { opacity: 0.8; }
.an-spark__legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  align-items: center;
  margin-top: var(--space-2);
  font-size: var(--fs-micro);
  color: var(--text-faint);
}
.an-spark__dot {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.an-spark__swatch {
  width: 9px;
  height: 9px;
  border-radius: 2px;
  flex-shrink: 0;
}

/* Aviso sem Economia */
.an-no-economia {
  margin: 0 0 var(--space-6);
  font-size: var(--fs-sm);
  color: var(--text-muted);
  line-height: var(--lh-normal);
}

/* Tabela */
.an-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
.an-table {
  width: 100%;
  border-collapse: collapse;
  font-variant-numeric: tabular-nums;
}
.an-table th {
  text-align: right;
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-muted);
  padding: var(--space-3) var(--space-4);
  white-space: nowrap;
}
.an-table th.col-mes { text-align: left; }
.an-table th:first-child { text-align: left; }
.an-thead-row {
  border-bottom: var(--bw-hair) solid var(--border);
}
.an-table td {
  text-align: right;
  padding: var(--space-3) var(--space-4);
  font-size: var(--fs-sm);
  white-space: nowrap;
}
.an-table td.col-mes {
  text-align: left;
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.an-tbody-row {
  border-bottom: var(--bw-hair) solid var(--border);
}
.an-tbody-row.is-empty { opacity: 0.45; }
.an-pct {
  font-family: var(--font-money);
  font-variant-numeric: tabular-nums;
}
.an-tfoot-row {
  border-top: var(--bw-strong) solid var(--border-strong);
  font-weight: var(--fw-bold);
}
.an-tfoot-row td, .an-tfoot-row th {
  padding: var(--space-3) var(--space-4);
  font-size: var(--fs-sm);
  white-space: nowrap;
}
.an-tfoot-row th {
  text-align: left;
  text-transform: uppercase;
  letter-spacing: var(--ls-label);
  font-size: var(--fs-label);
  color: var(--text);
}
.an-tfoot-row td {
  text-align: right;
}
`;
  document.head.appendChild(s);
})();

/* ---- Dados de demonstração ---- */
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

/* Ano de demonstração: 2025. Meses Jan–Jun com dados; Jul–Dez vazios (ano passado). */
const DEMO_MONTHS = [
  {
    month: 1,
    income_cents: 850000,
    economia_cents: 192000,
    savings_rate_bps: 2259,
    performance_cents: 210000,
    cost_of_living_cents: 640000,
    real_daily_avg_cents: 19200,
  },
  {
    month: 2,
    income_cents: 850000,
    economia_cents: 212000,
    savings_rate_bps: 2494,
    performance_cents: 230000,
    cost_of_living_cents: 620000,
    real_daily_avg_cents: 20700,
  },
  {
    month: 3,
    income_cents: 850000,
    economia_cents: 152000,
    savings_rate_bps: 1788,
    performance_cents: 169000,
    cost_of_living_cents: 681000,
    real_daily_avg_cents: 18900,
  },
  {
    month: 4,
    income_cents: 850000,
    economia_cents: 245000,
    savings_rate_bps: 2882,
    performance_cents: 258000,
    cost_of_living_cents: 592000,
    real_daily_avg_cents: 17400,
  },
  {
    month: 5,
    income_cents: 1020000,
    economia_cents: 310000,
    savings_rate_bps: 3039,
    performance_cents: 328000,
    cost_of_living_cents: 692000,
    real_daily_avg_cents: 21800,
  },
  {
    month: 6,
    income_cents: 850000,
    economia_cents: 171000,
    savings_rate_bps: 2012,
    performance_cents: 185000,
    cost_of_living_cents: 665000,
    real_daily_avg_cents: 22100,
  },
  {
    month: 7,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
  {
    month: 8,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
  {
    month: 9,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
  {
    month: 10,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
  {
    month: 11,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
  {
    month: 12,
    income_cents: 0,
    economia_cents: 0,
    savings_rate_bps: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    real_daily_avg_cents: 0,
  },
];

/* ---- Sub-componentes ---- */

/** Item da legenda do sparkline: amostra de cor + rótulo. */
function LegendDot({ color, label }) {
  return (
    <span className="an-spark__dot">
      <span
        aria-hidden="true"
        className="an-spark__swatch"
        style={{ background: color }}
      />
      {label}
    </span>
  );
}

/** Mini-barras de Economizado% por mês, com faixa-meta 20–30% sombreada. */
function EconomizadoSparkline({ months }) {
  const data = months.map((m) => ({
    pct: m.savings_rate_bps / 100,
    empty: m.income_cents === 0 && m.cost_of_living_cents === 0,
    label: MONTHS_PT[m.month - 1],
  }));
  const maxPct = Math.max(40, ...data.map((d) => d.pct));
  const H = 56;
  const bandTop = ((maxPct - 30) / maxPct) * H;
  const bandHeight = (10 / maxPct) * H; /* faixa 20–30% */

  return (
    <div className="an-spark">
      <div
        role="img"
        aria-label="Tendência de Economizado% por mês. Faixa-meta de 20 a 30% sombreada."
        className="an-spark__bars"
      >
        <span
          aria-hidden="true"
          className="an-spark__band"
          style={{ top: bandTop, height: bandHeight }}
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
              className="an-spark__bar"
              title={`${d.label}: ${d.empty ? "—" : `${d.pct.toFixed(0)}%`}`}
              style={{ height: h, background: color }}
            />
          );
        })}
      </div>
      <div className="an-spark__legend">
        <span>Economizado% por mês:</span>
        <LegendDot color="var(--success-tint)" label="meta 20–30%" />
        <LegendDot color="var(--success-400)" label="dentro" />
        <LegendDot color="var(--warning-400)" label="abaixo" />
        <LegendDot color="var(--primary)" label="acima" />
      </div>
    </div>
  );
}

/* ---- Tela principal ---- */
function AnnualScreen(props) {
  const [year, setYear] = React.useState(2025);
  const thisYear = 2026;

  const months = DEMO_MONTHS;

  /* Totais do ano — espelha a lógica de production */
  const totals = months.reduce(
    (a, m) => ({
      performance: a.performance + m.performance_cents,
      cost: a.cost + m.cost_of_living_cents,
      income: a.income + m.income_cents,
      economia: a.economia + m.economia_cents,
    }),
    { performance: 0, cost: 0, income: 0, economia: 0 },
  );

  /* Economizado% anual = ΣEconomia / ΣEntradas — NÃO média das taxas mensais */
  const annualSavingsPct =
    totals.income > 0 ? Math.round((totals.economia / totals.income) * 100) : 0;

  const hasYearData = months.some(
    (m) => m.income_cents !== 0 || m.cost_of_living_cents !== 0,
  );
  const hasEconomia = months.some((m) => m.economia_cents !== 0);

  /* 3 estados de cor: acima de 30% jade, dentro 20–30% verde, abaixo âmbar */
  const savingsColor =
    annualSavingsPct > 30
      ? "var(--primary)"
      : annualSavingsPct >= 20
        ? "var(--success-400)"
        : "var(--warning-400)";

  return (
    <div className="an">
      {/* Cabeçalho: título + navegação por ano */}
      <header className="an-header">
        <div>
          <h1 className="an-title">Visão anual</h1>
          <p className="an-subtitle">
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

      <div className="an-card">
        <div className="an-card__body">
          {/* Sparkline de Economizado% — só quando há dados no ano */}
          {hasYearData &&
            (hasEconomia ? (
              <EconomizadoSparkline months={months} />
            ) : (
              <p className="an-no-economia">
                Sem Economia registrada em {year} — importe a aba{" "}
                <strong>Economia</strong> em Configurações › Google Sheets para ver a
                tendência de quanto você guardou (meta 20–30%).
              </p>
            ))}

          {/* Tabela anual */}
          <div className="an-scroll">
            <table className="an-table">
              <thead>
                <tr className="an-thead-row">
                  <th scope="col" className="col-mes">
                    Mês
                  </th>
                  <th scope="col">Entradas</th>
                  <th scope="col">Economia</th>
                  <th scope="col">
                    <InfoPopover term="economizado">Economizado</InfoPopover>
                  </th>
                  <th scope="col">Performance</th>
                  <th scope="col">Custo de vida</th>
                  <th scope="col">Diário médio</th>
                </tr>
              </thead>
              <tbody>
                {months.map((m) => {
                  const empty = m.income_cents === 0 && m.cost_of_living_cents === 0;
                  const pct = m.savings_rate_bps / 100;
                  return (
                    <tr
                      key={m.month}
                      className={`an-tbody-row${empty ? " is-empty" : ""}`}
                    >
                      <td className="col-mes">{MONTHS_PT[m.month - 1]}</td>
                      <td>
                        <Money cents={m.income_cents} size="sm" sign="auto" />
                      </td>
                      <td>
                        <Money cents={m.economia_cents} size="sm" />
                      </td>
                      <td
                        className="an-pct"
                        style={{ color: empty ? "var(--text-faint)" : "var(--text)" }}
                      >
                        {empty ? "—" : `${pct.toFixed(0)}%`}
                      </td>
                      <td>
                        <Money cents={m.performance_cents} size="sm" sign="auto" />
                      </td>
                      <td>
                        <Money cents={m.cost_of_living_cents} size="sm" />
                      </td>
                      <td>
                        {empty ? (
                          <span style={{ color: "var(--text-faint)" }}>—</span>
                        ) : (
                          <Money cents={m.real_daily_avg_cents} size="sm" />
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>

              {/* Rodapé de totais — só quando há dados no ano */}
              {hasYearData && (
                <tfoot>
                  <tr className="an-tfoot-row">
                    <th scope="row">Total</th>
                    <td>
                      <Money cents={totals.income} size="sm" sign="auto" />
                    </td>
                    <td>
                      <Money cents={totals.economia} size="sm" />
                    </td>
                    <td
                      className="an-pct"
                      title="Economizado no ano = total economizado ÷ total de entradas (meta 20–30%)"
                      style={{ color: savingsColor }}
                    >
                      {annualSavingsPct}%
                    </td>
                    <td>
                      <Money cents={totals.performance} size="sm" sign="auto" />
                    </td>
                    <td>
                      <Money cents={totals.cost} size="sm" />
                    </td>
                    <td
                      style={{ color: "var(--text-faint)" }}
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
    </div>
  );
}

window.AnnualScreen = AnnualScreen;
