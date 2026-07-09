/* Neko Finance — Horizonte de saldos (ui_kit).
   Projeção mês a mês do saldo — termômetro visual de folga/aperto.
   Seções: gráfico BalanceTrajectory + legenda de faixas, detalhe diário por mês
   (colunas do calendário com heatmap de saldo), vencimentos próximos.
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.HorizonteScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Money, BalanceTrajectory, ProvBadge } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectHorizonteCSS() {
  if (document.getElementById("horizonte-css")) return;
  const s = document.createElement("style");
  s.id = "horizonte-css";
  s.textContent = `
/* Layout geral */
.hor { display: flex; flex-direction: column; gap: var(--space-6); padding: var(--space-2); max-width: 1200px; }

/* Cabeçalho */
.hor-head__title {
  font-size: var(--fs-h2);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight);
  margin: 0 0 var(--space-2);
  color: var(--text-strong);
}
.hor-head__desc {
  font-size: var(--fs-sm);
  color: var(--text-muted);
  margin: 0;
  line-height: var(--lh-normal);
  max-width: 560px;
}

/* Card do gráfico */
.hor-chart-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-1);
  padding: var(--space-6) var(--space-6) var(--space-4);
}
.hor-chart-card__legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--bw-hair) solid var(--border);
}
.hor-legend-item {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: var(--fs-sm);
  color: var(--text-muted);
}
.hor-legend-swatch {
  width: 12px;
  height: 12px;
  border-radius: var(--radius-xs);
  flex-shrink: 0;
}

/* Seção de etiqueta */
.hor-section-label {
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-faint);
  margin: 0 0 var(--space-3);
}

/* Colunas mensais de detalhe diário */
.hor-cols {
  display: flex;
  gap: var(--space-4);
  overflow-x: auto;
  padding-bottom: var(--space-4);
  -webkit-overflow-scrolling: touch;
}
.hor-col { min-width: 140px; flex-shrink: 0; }
.hor-col__month {
  font-size: var(--fs-label);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-muted);
  padding: var(--space-2) var(--space-3);
  position: sticky;
  top: 0;
  background: var(--bg);
}
.hor-col__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.hor-day {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-variant-numeric: tabular-nums;
  transition: outline 80ms ease;
}
.hor-day--today { outline: 2px solid var(--border-focus); }
.hor-day__num {
  font-size: var(--fs-sm);
  color: var(--text);
  width: 22px;
  flex-shrink: 0;
}
.hor-day__bal {
  color: var(--text);
  font-size: var(--fs-sm);
}

/* Vencimentos */
.hor-bills-title {
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-faint);
  margin: var(--space-6) 0 var(--space-3);
}
.hor-bills-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  list-style: none;
  margin: 0;
  padding: 0;
}
.hor-bill {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-sm);
}
.hor-bill__chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  background: var(--surface-2);
  border: var(--bw-hair) solid var(--border);
  font-size: var(--fs-micro);
  font-weight: var(--fw-medium);
  color: var(--text-muted);
  white-space: nowrap;
  flex-shrink: 0;
}
.hor-bill__desc {
  flex: 1;
  font-size: var(--fs-sm);
  color: var(--text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Empty state inline (vencimentos vazios) */
.hor-empty {
  padding: var(--space-7) var(--space-6);
  background: var(--bg-subtle);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-sm);
  text-align: center;
  color: var(--text-faint);
  font-size: var(--fs-sm);
}

@media (prefers-reduced-motion: reduce) {
  .hor-day { transition: none; }
}
`;
  document.head.appendChild(s);
})();

/* ---- Dados de demo representativos ---- */

/** Calcula a faixa de saldo (limiares absolutos da planilha, em centavos). */
function saldoBand(cents) {
  if (cents < -50000) return "critical"; // < −R$ 500
  if (cents < 0) return "negative"; // < R$ 0
  if (cents <= 100000) return "tight"; // ≤ R$ 1.000
  if (cents <= 200000) return "ok"; // ≤ R$ 2.000
  return "comfortable"; // > R$ 2.000
}

const BAND_FILL = {
  critical: "var(--saldo-band-critical-fill)",
  negative: "var(--saldo-band-negative-fill)",
  tight: "var(--saldo-band-tight-fill)",
  ok: "var(--saldo-band-ok-fill)",
  comfortable: "var(--saldo-band-comfortable-fill)",
};

const BAND_LEGEND = [
  { band: "comfortable", label: "folga (> R$ 2.000)" },
  { band: "ok", label: "ok (R$ 1.000–2.000)" },
  { band: "tight", label: "apertado (R$ 0–1.000)" },
  { band: "negative", label: "negativo (−R$ 500 a R$ 0)" },
  { band: "critical", label: "crítico (< −R$ 500)" },
];

/** Meses fictícios PT-BR abreviados. */
const MONTH_NAMES = [
  "",
  "jan",
  "fev",
  "mar",
  "abr",
  "mai",
  "jun",
  "jul",
  "ago",
  "set",
  "out",
  "nov",
  "dez",
];

function monthLabel(ym) {
  const [, m] = ym.split("-");
  const n = MONTH_NAMES[Number(m)];
  return n.charAt(0).toUpperCase() + n.slice(1);
}

/** Série diária de 3 meses (jun–ago 2026) com trajetória realista.
    Saldo começa confortável, aperta em julho, recupera em agosto. */
function buildDemoDaily() {
  const days = [];
  const months = [
    { ym: "2026-06", start: 315000, perDay: -5800 }, // R$ 3.150 → declínio suave
    { ym: "2026-07", start: 145000, perDay: -6200 }, // R$ 1.450 → faixa apertada/negativa
    { ym: "2026-08", start: 280000, perDay: -4500 }, // R$ 2.800 → salário nova entrada
  ];
  for (const { ym, start, perDay } of months) {
    const [year, month] = ym.split("-").map(Number);
    const daysInMonth = new Date(year, month, 0).getDate();
    for (let d = 1; d <= daysInMonth; d++) {
      const date = `${ym}-${String(d).padStart(2, "0")}`;
      // Injetar salário no dia 5 de jul e ago
      const payday = d === 5 && month > 6;
      const balance = start + (d - 1) * perDay + (payday ? 620000 : 0);
      days.push({ date, balance_cents: Math.round(balance) });
    }
  }
  return days;
}

const DEMO_DAILY = buildDemoDaily();
const DEMO_TODAY = "2026-06-21";

/** Agrupa a série diária em colunas por mês. */
function groupByMonth(daily, today) {
  const colsMap = new Map();
  const colsOrder = [];
  for (const d of daily) {
    const ym = d.date.slice(0, 7);
    if (!colsMap.has(ym)) {
      const col = { ym, label: monthLabel(ym), days: [] };
      colsMap.set(ym, col);
      colsOrder.push(col);
    }
    colsMap.get(ym).days.push({
      day: Number(d.date.slice(8, 10)),
      balance: d.balance_cents,
      isToday: d.date === today,
    });
  }
  return colsOrder;
}

/** Formata data "DD/MM" para o chip de vencimento. */
function fmtDate(iso) {
  const [, m, d] = iso.split("-");
  return `${d}/${m}`;
}

/* ---- Sub-componentes ---- */

function ChartSection() {
  return (
    <section className="hor-chart-card" aria-label="Trajetória do saldo projetado">
      <BalanceTrajectory daily={DEMO_DAILY} today={DEMO_TODAY} variant="full" />
      <div className="hor-chart-card__legend" aria-label="Legenda das faixas de saldo">
        {BAND_LEGEND.map((l) => (
          <span key={l.band} className="hor-legend-item">
            <span
              aria-hidden="true"
              className="hor-legend-swatch"
              style={{ background: BAND_FILL[l.band] }}
            />
            {l.label}
          </span>
        ))}
      </div>
    </section>
  );
}

function DailyDetail() {
  const cols = groupByMonth(DEMO_DAILY, DEMO_TODAY);
  return (
    <section aria-label="Saldo projetado por dia, agrupado por mês">
      <h2 className="hor-section-label">Detalhe diário</h2>
      <div className="hor-cols">
        {cols.map((col) => (
          <div key={col.ym} className="hor-col">
            <div aria-hidden="true" className="hor-col__month">
              {col.label}
            </div>
            <ul aria-label={col.label} className="hor-col__list">
              {col.days.map((d) => {
                const band = saldoBand(d.balance);
                return (
                  <li
                    key={d.day}
                    aria-current={d.isToday ? "date" : undefined}
                    aria-label={`Dia ${d.day}: faixa ${BAND_LEGEND.find((l) => l.band === band)?.label ?? band}`}
                    className={`hor-day${d.isToday ? " hor-day--today" : ""}`}
                    style={{ background: BAND_FILL[band] }}
                  >
                    <span aria-hidden="true" className="hor-day__num">
                      {d.day}
                    </span>
                    <span aria-hidden="true" className="hor-day__bal">
                      <Money cents={d.balance} size="sm" sign="none" />
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
    </section>
  );
}

/** Vencimentos próximos — demo com 4 contas a pagar nos próximos 60 dias. */
function UpcomingBills() {
  const bills = [
    {
      id: 1,
      due_date: "2026-06-25",
      description: "Aluguel",
      amount: 180000,
      is_projection: false,
    },
    {
      id: 2,
      due_date: "2026-06-28",
      description: "Fatura do cartão",
      amount: 243700,
      is_projection: false,
    },
    {
      id: 3,
      due_date: "2026-07-05",
      description: "Plano de saúde",
      amount: 58900,
      is_projection: true,
    },
    {
      id: 4,
      due_date: "2026-07-10",
      description: "Internet + streaming",
      amount: 18490,
      is_projection: true,
    },
  ];

  return (
    <section aria-labelledby="hor-bills-title">
      <h2 id="hor-bills-title" className="hor-bills-title">
        Vencimentos próximos
      </h2>
      <ul className="hor-bills-list">
        {bills.map((b) => (
          <li key={b.id} className="hor-bill">
            <span className="hor-bill__chip">
              <Icon name="calendar" size={12} stroke={1.75} aria-hidden="true" />
              {fmtDate(b.due_date)}
            </span>
            <span className="hor-bill__desc">{b.description}</span>
            {b.is_projection && <ProvBadge provenance="projetado" />}
            <Money cents={-Math.abs(b.amount)} size="sm" sign="auto" />
          </li>
        ))}
      </ul>
    </section>
  );
}

/* ---- Tela completa ---- */
function HorizonteScreen(props) {
  return (
    <div className="hor">
      {/* Cabeçalho */}
      <header>
        <h1 className="hor-head__title">Horizonte de saldos</h1>
        <p className="hor-head__desc">
          Saldo projetado dia a dia, no mesmo termômetro da planilha: quanto mais verde,
          mais folga; quanto mais vermelho, mais aperto.
        </p>
      </header>

      {/* Gráfico de trajetória + legenda */}
      <ChartSection />

      {/* Detalhe diário — colunas de calendário com heatmap */}
      <DailyDetail />

      {/* Vencimentos próximos (próximos 60 dias) */}
      <UpcomingBills />
    </div>
  );
}

window.HorizonteScreen = HorizonteScreen;
