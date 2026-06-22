/* Neko Finance — Ano inteiro (YearGridScreen).
   Grade dia a dia para todos os 12 meses do ano: Data · Entrada · Saída · Diário · Saldo.
   A coluna Saldo usa o heatmap de cinco faixas canônicas (termômetro da planilha de referência).
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.YearGridScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { MonthNav, EmptyState } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectAnoInteiroCSS() {
  if (document.getElementById("ano-inteiro-css")) return;
  const s = document.createElement("style");
  s.id = "ano-inteiro-css";
  s.textContent = `
/* Página */
.yr { max-width: 1100px; margin: 0 auto; padding: var(--space-2); }

/* Cabeçalho */
.yr-header {
  display: flex; align-items: center; justify-content: space-between;
  gap: var(--space-4); margin-bottom: var(--space-6); flex-wrap: wrap;
}
.yr-title {
  font-size: var(--fs-h2); font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight); margin: 0; color: var(--text-strong);
}
.yr-subtitle {
  color: var(--text-muted); font-size: var(--fs-sm);
  margin: var(--space-1) 0 0;
}

/* Sections */
.yr-sections { display: flex; flex-direction: column; gap: var(--space-6); }

/* Título de seção (mês) */
.yr-month-title {
  font-size: var(--fs-title); font-weight: var(--fw-bold);
  margin: 0 0 var(--space-3); color: var(--text-strong);
}

/* Card compartilhado */
.yr-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-1);
}
.yr-card__body { padding: 0; }

/* Scroll horizontal */
.yr-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }

/* Tabela */
.yr-table {
  width: 100%; border-collapse: collapse;
  font-size: var(--fs-sm); line-height: var(--lh-snug);
}
.yr-table thead th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--bw-hair) solid var(--border);
  font-size: var(--fs-label); font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label); text-transform: uppercase;
  color: var(--text-muted); text-align: right; white-space: nowrap;
}
.yr-table thead th:first-child { text-align: left; }
.yr-table tbody td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--bw-hair) solid var(--border);
  color: var(--text); text-align: right; white-space: nowrap;
  font-family: var(--font-money); font-variant-numeric: tabular-nums;
}
.yr-table tbody td:first-child {
  font-family: var(--font-sans); text-align: left;
  font-size: var(--fs-sm); color: var(--text-muted);
}
.yr-table tbody tr:last-child td { border-bottom: none; }
.yr-table tbody tr:hover td { background: var(--surface-hover); }
.yr-td-dash { color: var(--text-faint); }
.yr-td-saldo-empty { color: var(--text-faint); text-align: right; }

/* Legenda do heatmap */
.yr-legend {
  display: flex; align-items: center; gap: var(--space-5);
  padding: var(--space-3) var(--space-4);
  border-top: var(--bw-hair) solid var(--border);
  flex-wrap: wrap;
}
.yr-legend__label {
  font-size: var(--fs-micro); color: var(--text-faint);
  letter-spacing: var(--ls-label); text-transform: uppercase;
  margin-right: var(--space-1);
}
.yr-legend__item {
  display: flex; align-items: center; gap: var(--space-2);
  font-size: var(--fs-micro); color: var(--text-muted);
}
.yr-legend__swatch {
  width: 10px; height: 10px; border-radius: 2px; flex-shrink: 0;
}

@media (prefers-reduced-motion: reduce) {
  .yr-table tbody tr { transition: none; }
}
`;
  document.head.appendChild(s);
})();

/* ---- Helpers ---- */
function fmtBRL(cents) {
  const abs = Math.abs(cents);
  const n = (abs / 100).toLocaleString("pt-BR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return "R$ " + n;
}

function fmtDayMonth(dateStr) {
  // dateStr = "2026-01-05" → "05/01"
  const parts = dateStr.split("-");
  return `${parts[2]}/${parts[1]}`;
}

/** Classifica o saldo (centavos) nas mesmas 5 faixas canônicas da planilha. */
function saldoBand(cents) {
  if (cents < -50000) return "critical";
  if (cents < 0) return "negative";
  if (cents <= 100000) return "tight";
  if (cents <= 200000) return "ok";
  return "comfortable";
}

const BAND_FILL = {
  critical: "var(--saldo-band-critical-fill)",
  negative: "var(--saldo-band-negative-fill)",
  tight: "var(--saldo-band-tight-fill)",
  ok: "var(--saldo-band-ok-fill)",
  comfortable: "var(--saldo-band-comfortable-fill)",
};

const BAND_LABEL = {
  critical: "crítico",
  negative: "negativo",
  tight: "apertado",
  ok: "ok",
  comfortable: "folga",
};

/* ---- Dados de demonstração ---- */
/**
 * Gera linhas diárias representativas para um mês.
 * Apenas dias com algum lançamento são exibidos (como na grade real).
 */
function makeMonthData(year, month) {
  // Data de referência: hoje é 21/06/2026
  const today = new Date(2026, 5, 21); // mês 0-indexado
  const mDate = new Date(year, month - 1, 1);
  const isPast = mDate < new Date(2026, 5, 1);
  const isCurrent = month === 6 && year === 2026;
  const isFuture = mDate > new Date(2026, 5, 1);

  if (isFuture) return []; // meses futuros sem dados

  const DAYS_IN_MONTH = new Date(year, month, 0).getDate();

  // Padrões por mês — números fictícios mas realistas
  const monthPatterns = {
    1: {
      income: 850000,
      salary_day: 5,
      out_days: [8, 10, 15],
      daily_avg: 18000,
      start_bal: 312000,
    },
    2: {
      income: 850000,
      salary_day: 5,
      out_days: [7, 12, 14],
      daily_avg: 15000,
      start_bal: 218500,
    },
    3: {
      income: 850000,
      salary_day: 5,
      out_days: [8, 11, 15],
      daily_avg: 19000,
      start_bal: 174200,
    },
    4: {
      income: 850000,
      salary_day: 7,
      out_days: [9, 13, 16],
      daily_avg: 17000,
      start_bal: 156900,
    },
    5: {
      income: 850000,
      salary_day: 5,
      out_days: [8, 10, 14],
      daily_avg: 21000,
      start_bal: 241300,
    },
    6: {
      income: 850000,
      salary_day: 5,
      out_days: [10, 15],
      daily_avg: 14500,
      start_bal: 357800,
    },
  };

  const p = monthPatterns[month] || monthPatterns[1];
  const rows = [];
  let balance = p.start_bal;

  for (let d = 1; d <= DAYS_IN_MONTH; d++) {
    const dateStr = `${year}-${String(month).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
    const dayOfMonth = d;
    const currDate = new Date(year, month - 1, d);
    if (isCurrent && currDate > today) break; // mês corrente: só até hoje

    const income = dayOfMonth === p.salary_day ? p.income : 0;
    const fixed_out = p.out_days.includes(dayOfMonth)
      ? Math.round(80000 + Math.random() * 40000)
      : 0;
    // Dias úteis com diário (segunda a sexta)
    const dow = currDate.getDay();
    const isWorkday = dow >= 1 && dow <= 5;
    const daily_out =
      isWorkday && !p.out_days.includes(dayOfMonth)
        ? Math.round(p.daily_avg * (0.6 + Math.random() * 0.8))
        : 0;

    // Só emite linhas que têm algum dado
    if (!income && !fixed_out && !daily_out) continue;

    balance = balance + income - fixed_out - daily_out;

    rows.push({
      date: dateStr,
      income_cents: income,
      fixed_out_cents: fixed_out,
      daily_out_cents: daily_out,
      balance_cents: balance,
    });
  }
  return rows;
}

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
];

/* ---- Sub-componentes ---- */

/** Legenda do heatmap (exibida uma vez, dentro da seção de junho). */
function SaldoLegend() {
  const items = [
    { band: "comfortable", label: "folga (> R$ 2.000)" },
    { band: "ok", label: "ok (R$ 1.000–2.000)" },
    { band: "tight", label: "apertado (R$ 0–1.000)" },
    { band: "negative", label: "negativo" },
    { band: "critical", label: "crítico (< −R$ 500)" },
  ];
  return (
    <div className="yr-legend" aria-label="Legenda do heatmap de saldo">
      <span className="yr-legend__label">Saldo</span>
      {items.map((i) => (
        <span key={i.band} className="yr-legend__item">
          <span
            className="yr-legend__swatch"
            style={{ background: BAND_FILL[i.band] }}
            aria-hidden="true"
          />
          {i.label}
        </span>
      ))}
    </div>
  );
}

/** Tabela de um mês. */
function MonthTable({ grid }) {
  return (
    <div className="yr-scroll">
      <table className="yr-table">
        <thead>
          <tr>
            <th scope="col">Data</th>
            <th scope="col">Entrada</th>
            <th scope="col">Saída</th>
            <th scope="col">Diário</th>
            <th scope="col">Saldo</th>
          </tr>
        </thead>
        <tbody>
          {grid.map((d) => {
            const band = d.balance_cents != null ? saldoBand(d.balance_cents) : null;
            return (
              <tr key={d.date}>
                <td>{fmtDayMonth(d.date)}</td>
                <td>
                  {d.income_cents ? (
                    <span style={{ color: "var(--money-pos)" }}>
                      {fmtBRL(d.income_cents)}
                    </span>
                  ) : (
                    <span className="yr-td-dash">—</span>
                  )}
                </td>
                <td>
                  {d.fixed_out_cents ? (
                    fmtBRL(d.fixed_out_cents)
                  ) : (
                    <span className="yr-td-dash">—</span>
                  )}
                </td>
                <td>
                  {d.daily_out_cents ? (
                    fmtBRL(d.daily_out_cents)
                  ) : (
                    <span className="yr-td-dash">—</span>
                  )}
                </td>
                {d.balance_cents == null ? (
                  <td className="yr-td-saldo-empty">—</td>
                ) : (
                  <td
                    style={{
                      textAlign: "right",
                      background: BAND_FILL[band],
                      color: "var(--text)",
                    }}
                    title={`Saldo ${BAND_LABEL[band]}`}
                  >
                    {fmtBRL(d.balance_cents)}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Seção de um mês: título + card com tabela ou estado vazio. */
function MonthSection({ label, monthNum, grid, showLegend }) {
  const hasData = grid.length > 0;

  return (
    <section aria-label={label}>
      <h2 className="yr-month-title">{label}</h2>
      <div className="yr-card">
        <div className="yr-card__body">
          {!hasData ? (
            <EmptyState
              variant="empty"
              title="Sem lançamentos"
              description="Nenhum dado importado para este mês."
            />
          ) : (
            <MonthTable grid={grid} />
          )}
          {showLegend && hasData && <SaldoLegend />}
        </div>
      </div>
    </section>
  );
}

/* ---- Tela completa ---- */
function YearGridScreen(props) {
  const THIS_YEAR = 2026;
  const [year, setYear] = React.useState(THIS_YEAR);

  // 12 grids gerados a partir dos dados de demo
  const grids = MONTHS_PT.map((label, idx) => ({
    month: idx + 1,
    label,
    data: makeMonthData(year, idx + 1),
  }));

  // Exibe a legenda do heatmap no primeiro mês que tenha dados (para não repetir)
  let legendShown = false;

  return (
    <div className="yr">
      {/* Cabeçalho */}
      <header className="yr-header">
        <div>
          <h1 className="yr-title">Ano inteiro</h1>
          <p className="yr-subtitle">
            Grade Data · Entrada · Saída · Diário · Saldo para cada mês de {year}.
          </p>
        </div>
        <MonthNav
          label={String(year)}
          onPrev={() => setYear((y) => y - 1)}
          onNext={() => setYear((y) => y + 1)}
          onToday={() => setYear(THIS_YEAR)}
          atToday={year === THIS_YEAR}
          prevLabel="Ano anterior"
          nextLabel="Próximo ano"
        />
      </header>

      {/* Grade dos 12 meses */}
      <div className="yr-sections">
        {grids.map((g) => {
          const showLegend = !legendShown && g.data.length > 0;
          if (showLegend) legendShown = true;
          return (
            <MonthSection
              key={g.month}
              label={g.label}
              monthNum={g.month}
              grid={g.data}
              showLegend={showLegend}
            />
          );
        })}
      </div>
    </div>
  );
}

window.YearGridScreen = YearGridScreen;
