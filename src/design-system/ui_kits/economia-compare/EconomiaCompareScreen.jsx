/* Neko Finance — Economia comparada (ui_kit).
   Dois anos lado a lado: Entradas · Economia · Economizado% mês a mês.
   Espelha EconomiaCompareScreen.tsx — PT-BR · R$ mono tabular · zero dependências.
   Expõe window.EconomiaCompareScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Money, MonthNav, InfoPopover, EmptyState } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectEconomiaCompareCSS() {
  if (document.getElementById("economia-compare-css")) return;
  const s = document.createElement("style");
  s.id = "economia-compare-css";
  s.textContent = `
/* Layout da tela */
.ec { max-width: 860px; margin: 0 auto; padding: var(--space-2); }

/* Cabeçalho */
.ec-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  margin-bottom: var(--space-6);
  flex-wrap: wrap;
}
.ec-header__lead { min-width: 0; }
.ec-title {
  font-size: var(--fs-h2);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight);
  color: var(--text-strong);
  margin: 0;
  line-height: var(--lh-snug);
}
.ec-subtitle {
  color: var(--text-muted);
  font-size: var(--fs-sm);
  margin: var(--space-1) 0 0;
  line-height: var(--lh-normal);
}

/* Card */
.dash-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-1);
}
.dash-card__body { padding: var(--space-4) var(--space-6) var(--space-6); }

/* Tabela */
.ec-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
.ec-table {
  width: 100%;
  border-collapse: collapse;
  font-variant-numeric: tabular-nums;
}

/* Cabeçalhos */
.ec-th {
  text-align: right;
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-muted);
  padding: var(--space-3) var(--space-4);
  white-space: nowrap;
}
.ec-th--left { text-align: left; }
.ec-th--year {
  text-align: center;
  color: var(--text-strong);
  font-size: var(--fs-sm);
  font-weight: var(--fw-semibold);
  border-bottom: 2px solid var(--border-strong);
  letter-spacing: 0;
  text-transform: none;
  padding: var(--space-3) var(--space-4);
}
.ec-th--year-a {
  border-right: var(--bw-strong) solid var(--border-strong);
}
.ec-th--group-end {
  border-right: var(--bw-strong) solid var(--border-strong);
}
.ec-th--divider {
  border-left: var(--bw-strong) solid var(--border-strong);
}

/* Células numéricas */
.ec-td {
  text-align: right;
  padding: var(--space-3) var(--space-4);
  font-variant-numeric: tabular-nums;
}
.ec-td--month {
  padding: var(--space-3) var(--space-4);
  font-weight: var(--fw-semibold);
  color: var(--text);
  text-align: left;
  white-space: nowrap;
}
.ec-td--divider {
  border-left: var(--bw-strong) solid var(--border-strong);
}
.ec-td--group-end {
  border-right: var(--bw-strong) solid var(--border-strong);
}

/* Linhas */
.ec-row { border-bottom: var(--bw-hair) solid var(--border); }
.ec-row:last-child { border-bottom: none; }
.ec-row--head { border-bottom: var(--bw-hair) solid var(--border); }
.ec-row--foot {
  border-top: var(--bw-strong) solid var(--border-strong);
  font-weight: var(--fw-bold);
}

/* Total label */
.ec-td--total {
  padding: var(--space-3) var(--space-4);
  text-transform: uppercase;
  letter-spacing: var(--ls-label);
  font-size: var(--fs-label);
  color: var(--text);
  font-weight: var(--fw-bold);
  white-space: nowrap;
}

/* Taxa economizado% — colorida semanticamente */
.ec-rate { font-family: var(--font-money); font-variant-numeric: tabular-nums; }
.ec-rate--strong { color: var(--primary); }
.ec-rate--ok { color: var(--success-400); }
.ec-rate--warn { color: var(--warning-400); }
.ec-rate--faint { color: var(--text-faint); }

/* Legenda de referência */
.ec-legend {
  display: flex;
  gap: var(--space-5);
  flex-wrap: wrap;
  padding: var(--space-4) var(--space-5);
  border-top: var(--bw-hair) solid var(--border);
  font-size: var(--fs-micro);
  color: var(--text-faint);
  align-items: center;
}
.ec-legend__dot {
  display: inline-block;
  width: 8px; height: 8px;
  border-radius: var(--radius-circle);
  margin-right: var(--space-2);
  flex-shrink: 0;
  vertical-align: middle;
}
.ec-legend__item { display: flex; align-items: center; gap: 0; white-space: nowrap; }

@media (prefers-reduced-motion: reduce) {
  * { transition: none !important; animation: none !important; }
}
`;
  document.head.appendChild(s);
})();

/* ---- Dados de demo ---- */

// SAVINGS_MIN_PCT = SAVINGS_MIN_BPS / 100 = 2000 / 100 = 20%
const SAVINGS_MIN_PCT = 20;

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

// Dados representativos — dois anos consecutivos (2025 vs 2026).
// Valores em centavos.
const DEMO_2025 = [
  { income: 982000, economia: 245000, rate_bps: 2494 }, // Jan 24,9%
  { income: 982000, economia: 310000, rate_bps: 3157 }, // Fev 31,6%
  { income: 982000, economia: 198000, rate_bps: 2016 }, // Mar 20,2%
  { income: 982000, economia: 176000, rate_bps: 1792 }, // Abr 17,9%
  { income: 1074000, economia: 290000, rate_bps: 2700 }, // Mai 27,0%
  { income: 982000, economia: 203000, rate_bps: 2067 }, // Jun 20,7%
  { income: 982000, economia: 221000, rate_bps: 2251 }, // Jul 22,5%
  { income: 982000, economia: 180000, rate_bps: 1834 }, // Ago 18,3%
  { income: 982000, economia: 258000, rate_bps: 2628 }, // Set 26,3%
  { income: 982000, economia: 195000, rate_bps: 1986 }, // Out 19,9%
  { income: 982000, economia: 332000, rate_bps: 3381 }, // Nov 33,8%
  { income: 1264000, economia: 341000, rate_bps: 2698 }, // Dez 27,0%
];

const DEMO_2026 = [
  { income: 1050000, economia: 262000, rate_bps: 2495 }, // Jan 25,0%
  { income: 1050000, economia: 340000, rate_bps: 3238 }, // Fev 32,4%
  { income: 1050000, economia: 201000, rate_bps: 1914 }, // Mar 19,1%
  { income: 1050000, economia: 284000, rate_bps: 2705 }, // Abr 27,0%
  { income: 1050000, economia: 241000, rate_bps: 2295 }, // Mai 23,0%
  { income: 1050000, economia: 189000, rate_bps: 1800 }, // Jun 18,0% (mês em andamento)
  // Jul–Dez: meses futuros sem dados
  null,
  null,
  null,
  null,
  null,
  null,
];

function yearTotals(months) {
  const filled = months.filter(Boolean);
  const totalIncome = filled.reduce((a, m) => a + m.income, 0);
  const totalEco = filled.reduce((a, m) => a + m.economia, 0);
  const savingsPct = totalIncome > 0 ? Math.round((totalEco / totalIncome) * 100) : 0;
  return { income: totalIncome, economia: totalEco, savingsPct };
}

function savingsRateClass(pct) {
  if (pct > 30) return "ec-rate ec-rate--strong";
  if (pct >= SAVINGS_MIN_PCT) return "ec-rate ec-rate--ok";
  return "ec-rate ec-rate--warn";
}

/* ---- Sub-componente: 3 células de um mês ---- */
function MonthCells({ m, leadingDivider, trailingDivider }) {
  const empty = !m;
  const pct = m ? Math.round(m.rate_bps / 100) : 0;
  const dim = empty ? 0.4 : 1;

  const entrTdClass = ["ec-td", leadingDivider ? "ec-td--divider" : ""]
    .filter(Boolean)
    .join(" ");

  const pctTdClass = ["ec-td", trailingDivider ? "ec-td--group-end" : ""]
    .filter(Boolean)
    .join(" ");

  return (
    <>
      <td className={entrTdClass} style={{ opacity: dim }}>
        {m ? (
          <Money cents={m.income} size="sm" sign="auto" />
        ) : (
          <span style={{ color: "var(--text-faint)" }}>—</span>
        )}
      </td>
      <td className="ec-td" style={{ opacity: dim }}>
        {m ? (
          <Money cents={m.economia} size="sm" />
        ) : (
          <span style={{ color: "var(--text-faint)" }}>—</span>
        )}
      </td>
      <td className={pctTdClass} style={{ opacity: dim }}>
        {empty ? (
          <span className="ec-rate ec-rate--faint">—</span>
        ) : (
          <span className={savingsRateClass(pct)}>{pct}%</span>
        )}
      </td>
    </>
  );
}

/* ---- Sub-componente: 3 células de total ---- */
function TotalCells({ tot, leadingDivider, trailingDivider }) {
  const entrTdClass = ["ec-td", leadingDivider ? "ec-td--divider" : ""]
    .filter(Boolean)
    .join(" ");

  const pctTdClass = ["ec-td", trailingDivider ? "ec-td--group-end" : ""]
    .filter(Boolean)
    .join(" ");

  return (
    <>
      <td className={entrTdClass}>
        <Money cents={tot.income} size="sm" sign="auto" />
      </td>
      <td className="ec-td">
        <Money cents={tot.economia} size="sm" />
      </td>
      <td
        className={pctTdClass}
        title="Economizado anual = ΣEconomia ÷ ΣEntradas (meta 20–30%)"
      >
        <span className={savingsRateClass(tot.savingsPct)}>{tot.savingsPct}%</span>
      </td>
    </>
  );
}

/* ---- Tela principal ---- */
function EconomiaCompareScreen(props) {
  const [baseYear, setBaseYear] = React.useState(2025);
  const yearA = baseYear;
  const yearB = baseYear + 1;

  // Para a demo, só temos dados de 2025/2026.
  const monthsA = baseYear === 2025 ? DEMO_2025 : Array(12).fill(null);
  const monthsB = baseYear === 2025 ? DEMO_2026 : Array(12).fill(null);

  const totA = yearTotals(monthsA);
  const totB = yearTotals(monthsB);

  const hasAnyData = monthsA.some(Boolean) || monthsB.some(Boolean);

  return (
    <div className="ec">
      {/* Cabeçalho: título + navegador de par de anos */}
      <header className="ec-header">
        <div className="ec-header__lead">
          <h1 className="ec-title">
            Economia: {yearA} vs {yearB}
          </h1>
          <p className="ec-subtitle">
            Entradas, Economia e Economizado% mês a mês — dois anos lado a lado.
          </p>
        </div>
        <MonthNav
          label={`${yearA} · ${yearB}`}
          onPrev={() => setBaseYear((y) => y - 1)}
          onNext={() => setBaseYear((y) => y + 1)}
          onToday={() => setBaseYear(2025)}
          atToday={baseYear === 2025}
          prevLabel="Par de anos anterior"
          nextLabel="Próximo par de anos"
        />
      </header>

      {/* Corpo: tabela comparativa */}
      {!hasAnyData ? (
        <EmptyState
          variant="empty"
          title="Sem dados de Economia"
          description="Importe a aba Economia em Configurações › Google Sheets."
        />
      ) : (
        <div className="dash-card">
          <div className="dash-card__body" style={{ padding: 0 }}>
            <div className="ec-scroll">
              <table
                className="ec-table"
                role="table"
                aria-label={`Economia comparada ${yearA} vs ${yearB}`}
              >
                <thead>
                  {/* Linha 1: grupo por ano */}
                  <tr>
                    <th
                      className="ec-th ec-th--left"
                      rowSpan={2}
                      scope="col"
                      style={{
                        padding:
                          "var(--space-3) var(--space-4) var(--space-3) var(--space-6)",
                      }}
                    >
                      Mês
                    </th>
                    <th
                      colSpan={3}
                      className="ec-th--year ec-th--year-a"
                      scope="colgroup"
                    >
                      {yearA}
                    </th>
                    <th colSpan={3} className="ec-th--year" scope="colgroup">
                      {yearB}
                    </th>
                  </tr>
                  {/* Linha 2: colunas de dados */}
                  <tr className="ec-row--head">
                    <th className="ec-th" scope="col">
                      Entradas
                    </th>
                    <th className="ec-th" scope="col">
                      Economia
                    </th>
                    <th className="ec-th ec-th--group-end" scope="col">
                      <InfoPopover term="economizado">Economizado</InfoPopover>
                    </th>
                    <th className="ec-th ec-th--divider" scope="col">
                      Entradas
                    </th>
                    <th className="ec-th" scope="col">
                      Economia
                    </th>
                    <th className="ec-th" scope="col">
                      <InfoPopover term="economizado">Economizado</InfoPopover>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {MONTHS_PT.map((label, i) => (
                    <tr key={i} className="ec-row">
                      <td
                        className="ec-td--month"
                        style={{ paddingLeft: "var(--space-6)" }}
                      >
                        {label}
                      </td>
                      <MonthCells
                        m={monthsA[i]}
                        leadingDivider={false}
                        trailingDivider={true}
                      />
                      <MonthCells
                        m={monthsB[i]}
                        leadingDivider={true}
                        trailingDivider={false}
                      />
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr className="ec-row--foot">
                    <td
                      className="ec-td--total"
                      style={{ paddingLeft: "var(--space-6)" }}
                    >
                      Total
                    </td>
                    <TotalCells
                      tot={totA}
                      leadingDivider={false}
                      trailingDivider={true}
                    />
                    <TotalCells
                      tot={totB}
                      leadingDivider={true}
                      trailingDivider={false}
                    />
                  </tr>
                </tfoot>
              </table>
            </div>

            {/* Legenda de cores */}
            <div className="ec-legend" aria-label="Legenda de cores do Economizado%">
              <span className="ec-legend__item">
                <span
                  className="ec-legend__dot"
                  style={{ background: "var(--primary)" }}
                  aria-hidden="true"
                />
                {"> 30% — acima da meta"}
              </span>
              <span className="ec-legend__item">
                <span
                  className="ec-legend__dot"
                  style={{ background: "var(--success-400)" }}
                  aria-hidden="true"
                />
                {"20–30% — dentro do ideal"}
              </span>
              <span className="ec-legend__item">
                <span
                  className="ec-legend__dot"
                  style={{ background: "var(--warning-400)" }}
                  aria-hidden="true"
                />
                {"< 20% — abaixo da meta"}
              </span>
              <span style={{ marginLeft: "auto", color: "var(--text-faint)" }}>
                Economizado% = Economia ÷ Entradas (meta 20–30%)
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

window.EconomiaCompareScreen = EconomiaCompareScreen;
