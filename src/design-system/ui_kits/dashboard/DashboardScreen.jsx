/* Neko Finance — Dashboard screen (reconciled).
   "Quanto posso gastar hoje" — hero KPI + BalanceTrajectory + DailyCheckin + cards de análise.
   Todas as seções espelham os componentes reais: ColchaoCard, DailyCheckinCard, MonthLedgerCard,
   PerformanceCard, PrevisibilidadeCard, WriteBackPending, LastLoggedBanner.
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.DashboardScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const {
  Badge,
  Button,
  Disclosure,
  Money,
  BalanceTrajectory,
  PhaseBadge,
  MovBadge,
  MonthNav,
} = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectDashCSS() {
  if (document.getElementById("dashboard-css")) return;
  const s = document.createElement("style");
  s.id = "dashboard-css";
  s.textContent = `
/* Layout */
.dash { display:flex; flex-direction:column; gap:var(--space-7); max-width:1100px; }

/* Cards compartilhados */
.dash-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-1);
}
.dash-card__head {
  display: flex; align-items: center; justify-content: space-between; gap: var(--space-5);
  padding: var(--space-5) var(--space-6) var(--space-4);
}
.dash-card__title {
  display: flex; align-items: center; gap: var(--space-3);
  font-size: var(--fs-sm); font-weight: var(--fw-semibold); color: var(--text-strong);
}
.dash-card__ic { color: var(--text-faint); }
.dash-card__body { padding: var(--space-4) var(--space-6) var(--space-6); }

/* Hero */
.dash-hero {
  display: grid;
  grid-template-columns: 1fr 340px;
  gap: var(--space-7);
  padding: var(--space-7) var(--space-8);
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-2);
}
@media (max-width: 900px) { .dash-hero { grid-template-columns: 1fr; } }

.dash-hero__lead { display:flex; flex-direction:column; gap: var(--space-4); min-width:0; }
.dash-hero__label {
  font-size: var(--fs-sm); font-weight: var(--fw-medium);
  color: var(--text-muted); letter-spacing: var(--ls-label); text-transform: uppercase;
}
.dash-hero__kpi {
  font-family: var(--font-money); font-variant-numeric: tabular-nums;
  font-size: var(--fs-display-hero); font-weight: var(--fw-bold);
  color: var(--text-strong); letter-spacing: var(--ls-tight);
  line-height: var(--lh-tight);
  display: flex; align-items: baseline; gap: var(--space-3);
}
.dash-hero__kpi-suffix {
  font-family: var(--font-sans); font-size: var(--fs-body);
  font-weight: var(--fw-regular); color: var(--text-muted);
}
.dash-hero__reason {
  font-size: var(--fs-sm); color: var(--text-muted); max-width: 480px;
  line-height: var(--lh-normal);
}
.dash-hero__row { display:flex; align-items:center; gap: var(--space-6); flex-wrap: wrap; }
.dash-hero__stats { display:flex; gap: var(--space-7); margin:0; padding:0; }
.dash-hero__stats > div { display:flex; flex-direction:column; gap: var(--space-1); }
.dash-hero__stats dt {
  font-size: var(--fs-micro); font-weight: var(--fw-medium); color: var(--text-faint);
  letter-spacing: var(--ls-label); text-transform: uppercase;
}
.dash-hero__stats dd {
  font-size: var(--fs-sm); font-weight: var(--fw-semibold); color: var(--text);
  font-family: var(--font-money); font-variant-numeric: tabular-nums;
  margin: 0;
}

/* Forecast aside */
.dash-hero__forecast {
  display: flex; flex-direction: column; gap: var(--space-3);
  min-width: 0;
}
.dash-hero__forecast-head {
  display: flex; align-items: baseline; justify-content: space-between;
  gap: var(--space-3);
  font-size: var(--fs-sm); color: var(--text-muted);
}
.dash-hero__forecast-foot {
  font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
  line-height: var(--lh-normal);
}
.dash-hero__forecast-foot .negative { color: var(--money-neg); }

/* Déficit banner */
.dash-deficit {
  display: flex; align-items: center; gap: var(--space-3);
  padding: var(--space-3) var(--space-5);
  background: var(--danger-tint);
  border: var(--bw-hair) solid var(--danger-500);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm); color: var(--money-neg);
}

/* Aviso de último lançamento */
.dash-banner {
  display: flex; align-items: center; gap: var(--space-3);
  padding: var(--space-3) var(--space-5);
  background: var(--bg-subtle);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm); color: var(--text-muted);
}
.dash-banner__ic { color: var(--primary); flex-shrink: 0; }

/* WriteBack pending */
.dash-wb {
  display: grid; gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  background: var(--bg-subtle);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-sm);
}
.dash-wb__head {
  display: flex; align-items: center; gap: var(--space-3);
  color: var(--warning-400); font-size: var(--fs-sm);
}
.dash-wb__actions { display:flex; gap: var(--space-3); flex-wrap: wrap; }

/* Check-in diário */
.dash-checkin__body { padding: var(--space-4) var(--space-6) var(--space-5); display:flex; flex-direction:column; gap:var(--space-4); }
.dash-checkin__top {
  display: flex; align-items: baseline; justify-content: space-between;
  font-size: var(--fs-sm);
}
.dash-checkin__spent { font-family: var(--font-money); font-variant-numeric: tabular-nums; }
.dash-checkin__bar-track {
  height: 6px; border-radius: var(--radius-pill); background: var(--bg-subtle); overflow: hidden;
}
.dash-checkin__bar-fill {
  height: 100%; border-radius: var(--radius-pill);
  background: var(--type-diario);
  transform-origin: left;
  transition: transform var(--dur-slow) var(--ease-entrance);
}
.dash-checkin__bar-fill--over { background: var(--danger-500); }
@media (prefers-reduced-motion: reduce) {
  .dash-checkin__bar-fill { transition: none; }
}
.dash-checkin__kinds {
  display: flex; gap: var(--space-2); flex-wrap: wrap;
}
.dash-checkin__kind-btn {
  display: inline-flex; align-items: center; gap: var(--space-2);
  height: 32px; padding: 0 var(--space-3);
  border-radius: var(--radius-sm); cursor: pointer;
  border: var(--bw-hair) solid var(--border);
  background: transparent; color: var(--text);
  font-family: var(--font-sans); font-size: var(--fs-sm);
  transition: var(--t-hover);
}
.dash-checkin__kind-btn--active {
  background: var(--surface-selected); border-color: var(--primary);
}
.dash-checkin__inputs {
  display: flex; gap: var(--space-3); align-items: center;
}
.dash-checkin__input {
  flex: 1; height: 36px; padding: 0 var(--space-3);
  background: var(--bg-subtle);
  border: var(--bw-hair) solid var(--border-input);
  border-radius: var(--radius-xs);
  color: var(--text); font-family: var(--font-money); font-size: var(--fs-body);
}
.dash-checkin__desc {
  font-family: var(--font-sans);
}
.dash-checkin__hint {
  font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
}
.dash-checkin__avg {
  font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
}

/* Previsibilidade */
.dash-predict__head-trusted {
  font-size: var(--fs-micro); color: var(--text-faint);
}
.dash-predict__ok { font-size: var(--fs-sm); color: var(--success-500); margin: 0; }
.dash-predict__warn { font-size: var(--fs-sm); color: var(--money-neg); margin: 0; }
.dash-predict__neutral { font-size: var(--fs-sm); color: var(--text-muted); margin: 0; }
.dash-predict__rows { display:flex; flex-direction:column; gap: var(--space-4); margin-top: var(--space-4); }
.dash-predict__row { display:flex; align-items:center; gap: var(--space-4); font-size: var(--fs-sm); }
.dash-predict__month { width: 64px; color: var(--text-muted); flex-shrink:0; }
.dash-predict__bar {
  flex: 1; height: 5px; border-radius: var(--radius-pill);
  background: var(--bg-subtle); overflow:hidden;
}
.dash-predict__fill { height:100%; background: var(--chart-1); border-radius: var(--radius-pill); }
.dash-predict__pct { font-size: var(--fs-micro); color: var(--text-faint); white-space: nowrap; }
.dash-predict__savings {
  margin-top: var(--space-5); padding-top: var(--space-4);
  border-top: var(--bw-hair) solid var(--border);
  font-size: var(--fs-sm); color: var(--text-muted);
}

/* Colchão */
.dash-colchao__nums { display:flex; gap: var(--space-7); flex-wrap: wrap; margin-bottom: var(--space-5); }
.dash-colchao__num { display:flex; flex-direction:column; gap: var(--space-1); }
.dash-colchao__label { font-size: var(--fs-micro); color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase; }
.dash-colchao__val { font-family: var(--font-money); font-variant-numeric: tabular-nums; font-size: var(--fs-money-md); color: var(--text); }
.dash-colchao__val--muted { color: var(--text-faint); }
.dash-colchao__text { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--space-4); line-height: var(--lh-normal); }

/* Performance por mês */
.dash-perf__hint { font-size: var(--fs-micro); color: var(--text-faint); }
.dash-perf__row {
  display: flex; gap: var(--space-5);
  padding: var(--space-4) var(--space-6) var(--space-6);
  flex-wrap: wrap;
}
.dash-perf__cell {
  flex: 1; min-width: 120px;
  display: flex; flex-direction:column; gap: var(--space-1);
  padding: var(--space-4) var(--space-5);
  background: var(--bg-subtle);
  border-radius: var(--radius-sm);
  border: var(--bw-hair) solid var(--border);
}
.dash-perf__cell.is-incomplete { opacity: 0.7; }
.dash-perf__month { font-size: var(--fs-micro); color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase; }
.dash-perf__val { font-family: var(--font-money); font-variant-numeric: tabular-nums; font-size: var(--fs-money-md); color: var(--text); }
.dash-perf__val--muted { color: var(--text-muted); }
.dash-perf__rate { font-size: var(--fs-micro); color: var(--text-faint); }

/* Dia a dia (grade do mês) */
.dash-ledger-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
.dash-ledger-table {
  width: 100%; border-collapse: collapse;
  font-size: var(--fs-sm); line-height: var(--lh-snug);
}
.dash-ledger-table thead th {
  padding: var(--space-3) var(--space-5);
  border-bottom: var(--bw-hair) solid var(--border);
  font-size: var(--fs-micro); font-weight: var(--fw-semibold);
  color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase;
  text-align: right; white-space: nowrap;
}
.dash-ledger-table thead th:first-child { text-align: left; }
.dash-ledger-table tbody td {
  padding: var(--space-3) var(--space-5);
  border-bottom: var(--bw-hair) solid var(--border);
  color: var(--text); font-family: var(--font-money); font-variant-numeric: tabular-nums;
  text-align: right; white-space: nowrap;
}
.dash-ledger-table tbody td:first-child { font-family: var(--font-sans); text-align: left; }
.dash-ledger-table tbody tr.is-today td { background: var(--surface-selected); }
.dash-ledger-table tfoot td, .dash-ledger-table tfoot th {
  padding: var(--space-3) var(--space-5);
  font-size: var(--fs-sm); font-weight: var(--fw-semibold);
  border-top: var(--bw-hair) solid var(--border-strong);
  font-family: var(--font-money); font-variant-numeric: tabular-nums;
  text-align: right; white-space: nowrap; color: var(--text);
}
.dash-ledger-table tfoot th { font-family: var(--font-sans); text-align: left; }
.dash-today-tag {
  margin-left: var(--space-2);
  padding: 1px 5px;
  background: var(--surface-selected);
  border-radius: var(--radius-xs);
  font-size: var(--fs-micro); color: var(--primary);
  vertical-align: middle;
}
.money-pos { color: var(--money-pos); }
.money-neg { color: var(--money-neg); }
`;
  document.head.appendChild(s);
})();

/* ---- helpers ---- */
function fmtBRL(cents) {
  const abs = Math.abs(cents);
  const n = (abs / 100).toLocaleString("pt-BR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return "R$ " + n;
}
function moneyColor(cents) {
  if (cents > 0) return "var(--money-pos)";
  if (cents < 0) return "var(--money-neg)";
  return "var(--text-muted)";
}

/* ---- Sub-componentes estáticos de demo ---- */

/** Herói: "Pode gastar até" + BalanceTrajectory. */
function HeroSection() {
  // Dados fictícios representativos para a tela de demo.
  const safeToSpend = 32700; // R$ 327,00
  const monthEndBalance = 215400; // R$ 2.154,00
  const today = "2026-06-21";
  const savingsBinds = false;
  const reserveMonths = 7.4;
  const txnCount = 226;

  // Trajetória diária fictícia — 30 dias de junho com tendência realista.
  const daily = Array.from({ length: 30 }, (_, i) => {
    const d = String(i + 1).padStart(2, "0");
    const past = i < 21;
    const balance = past
      ? Math.round(3200 * 100 - i * 52 * 100 + (i % 7 === 0 ? 650000 : 0))
      : Math.round(2500 * 100 - (i - 20) * 48 * 100);
    return {
      date: `2026-06-${d}`,
      balance_cents: balance,
      projected: !past,
    };
  });

  return (
    <section className="dash-hero" aria-label="Quanto posso gastar hoje">
      <div className="dash-hero__lead">
        <p className="dash-hero__label">Pode gastar até</p>
        <p className="dash-hero__kpi">
          {fmtBRL(safeToSpend)}
          <span className="dash-hero__kpi-suffix">hoje</span>
        </p>
        <p className="dash-hero__reason">
          {savingsBinds
            ? "O menor de dois limites: respeita sua meta de guardar 25% no ano."
            : "O menor de dois limites: o que o caixa aguenta sem nenhum dia no vermelho."}
        </p>
        <div className="dash-hero__row">
          <dl className="dash-hero__stats">
            <div>
              <dt>Reserva</dt>
              <dd>{reserveMonths.toFixed(1)} meses</dd>
            </div>
            <div>
              <dt>Lançamentos</dt>
              <dd>{txnCount}</dd>
            </div>
          </dl>
          <Button
            variant="secondary"
            size="sm"
            iconLeft={<Icon name="sparkles" size={15} />}
          >
            Conhecer a Mia
          </Button>
        </div>
      </div>

      <aside className="dash-hero__forecast" aria-label="Saldo projetado do mês">
        <div className="dash-hero__forecast-head">
          <span>Saldo no fim de junho</span>
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontVariantNumeric: "tabular-nums",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-semibold)",
              color: moneyColor(monthEndBalance),
            }}
          >
            {fmtBRL(monthEndBalance)}
          </span>
        </div>
        <BalanceTrajectory daily={daily} today={today} variant="compact" />
        <p className="dash-hero__forecast-foot">
          Como seu saldo deve evoluir até o fim do mês.
        </p>
      </aside>
    </section>
  );
}

/** Aviso: último lançamento foi há 2 dias (demo). */
function LastLoggedBanner() {
  return (
    <div className="dash-banner" role="status">
      <Icon
        name="calendar"
        size={15}
        style={{ color: "var(--primary)", flexShrink: 0 }}
      />
      <span>Você lançou pela última vez há 2 dias.</span>
    </div>
  );
}

/** Check-in diário — versão estática do DailyCheckinCard. */
function DailyCheckinCard() {
  const [kind, setKind] = React.useState("diario");
  const ceiling = 32700;
  const spent = 14500;
  const remaining = ceiling - spent;
  const pct = Math.min(100, Math.round((spent / ceiling) * 100));
  const overspent = remaining < 0;

  const KINDS = ["entrada", "saida", "diario", "cartao", "economia"];

  return (
    <section aria-labelledby="dash-checkin-title" className="dash-card">
      <div className="dash-card__head">
        <span style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <span className="dash-card__title" id="dash-checkin-title">
            <Icon
              name="calendar"
              size={16}
              className="dash-card__ic"
              aria-hidden="true"
            />
            Diário de hoje
          </span>
          <span style={{ fontSize: "var(--fs-micro)", color: "var(--text-faint)" }}>
            Diário, cartão ou saída — registre o que aconteceu hoje
          </span>
        </span>
        <span
          style={{
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-semibold)",
            color: overspent ? "var(--danger-500)" : "var(--text-muted)",
          }}
        >
          {overspent
            ? `${fmtBRL(-remaining)} acima do teto`
            : `${fmtBRL(remaining)} disponível`}
        </span>
      </div>
      <div className="dash-checkin__body">
        <div className="dash-checkin__top">
          <span style={{ color: "var(--text-muted)" }}>Diário registrado hoje</span>
          <span className="dash-checkin__spent">
            <span
              style={{
                fontFamily: "var(--font-money)",
                fontVariantNumeric: "tabular-nums",
                fontWeight: "var(--fw-bold)",
              }}
            >
              {fmtBRL(spent)}
            </span>
            <span
              style={{ color: "var(--text-faint)", fontWeight: "var(--fw-regular)" }}
            >
              {" / "}
              {fmtBRL(ceiling)}
            </span>
          </span>
        </div>

        {/* Barra de progresso */}
        <progress
          value={pct}
          max={100}
          aria-label={`${pct}% do teto diário usado`}
          style={{
            position: "absolute",
            width: 1,
            height: 1,
            overflow: "hidden",
            clip: "rect(0,0,0,0)",
          }}
        />
        <div aria-hidden="true" className="dash-checkin__bar-track">
          <div
            className={`dash-checkin__bar-fill${overspent ? " dash-checkin__bar-fill--over" : ""}`}
            style={{ width: "100%", transform: `scaleX(${pct / 100})` }}
          />
        </div>

        <p className="dash-checkin__avg">Média do mês: R$ 145,00/dia</p>

        {/* Seletor de tipo */}
        <div
          className="dash-checkin__kinds"
          role="radiogroup"
          aria-label="Tipo de movimento"
        >
          {KINDS.map((k) => (
            <button
              key={k}
              type="button"
              role="radio"
              aria-checked={kind === k}
              disabled={k === "economia"}
              onClick={() => setKind(k)}
              className={`dash-checkin__kind-btn${kind === k ? " dash-checkin__kind-btn--active" : ""}`}
              style={k === "economia" ? { opacity: 0.5, cursor: "not-allowed" } : {}}
            >
              <MovBadge kind={k} showLabel size={14} />
            </button>
          ))}
        </div>

        <input
          aria-label="Descrição (opcional)"
          placeholder="Descrição (opcional) — ex.: mercado, aluguel…"
          className="dash-checkin__input dash-checkin__desc"
          defaultValue=""
        />

        <div className="dash-checkin__inputs">
          <input
            aria-label="Valor do lançamento (R$)"
            inputMode="decimal"
            placeholder="Valor de hoje (R$)"
            className="dash-checkin__input"
          />
          <Button variant="primary">Registrar</Button>
        </div>

        {kind === "saida" && (
          <p className="dash-checkin__hint">
            Saída = despesa fixa do mês — contas, fatura no vencimento.
          </p>
        )}
        {kind === "cartao" && (
          <p className="dash-checkin__hint">
            Cartão = compra no crédito (entra na fatura).
          </p>
        )}
        {kind === "entrada" && (
          <p className="dash-checkin__hint">Entrada = renda recebida no mês.</p>
        )}
      </div>
    </section>
  );
}

/** PrevisibilidadeCard — versão estática. */
function PrevisibilidadeCard() {
  const incompleteMonths = [
    { label: "julho", pct: 38, falta: 182400 },
    { label: "agosto", pct: 12, falta: 274100 },
  ];

  return (
    <section aria-labelledby="dash-predict-title" className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title" id="dash-predict-title">
          <Icon
            name="calendarRange"
            size={16}
            className="dash-card__ic"
            aria-hidden="true"
          />
          Previsibilidade
        </span>
        <span className="dash-predict__head-trusted">
          confiável até <strong>junho</strong>
        </span>
      </div>
      <div className="dash-card__body">
        <p className="dash-predict__warn">
          A partir de <strong>julho</strong> faltam{" "}
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontVariantNumeric: "tabular-nums",
            }}
          >
            R$ 4.564,00
          </span>{" "}
          de gastos não lançados. A projeção está otimista até você pré-lançar.
        </p>
        <div className="dash-predict__rows">
          {incompleteMonths.map((m) => (
            <div
              key={m.label}
              className="dash-predict__row"
              aria-label={`${m.label}: ${m.pct}% do gasto típico lançado, falta ${fmtBRL(m.falta)}`}
            >
              <span className="dash-predict__month">{m.label}</span>
              <span className="dash-predict__bar" aria-hidden="true">
                <span className="dash-predict__fill" style={{ width: `${m.pct}%` }} />
              </span>
              <span className="dash-predict__pct">
                {m.pct}% · falta {fmtBRL(m.falta)}
              </span>
            </div>
          ))}
        </div>
        <Disclosure title="Como pré-lançar o ano">
          <p
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--text-muted)",
              margin: 0,
              lineHeight: "var(--lh-normal)",
            }}
          >
            Em cada mês à frente, lance o <strong>saldo de hoje</strong> (só
            conta-corrente), o <strong>salário</strong> conservador, as{" "}
            <strong>contas fixas</strong>, a <strong>fatura do cartão</strong> no
            vencimento e o <strong>diário estimado</strong> em todos os dias. Futuro
            vazio engana a previsão.
          </p>
        </Disclosure>
        <p className="dash-predict__savings">
          Economizado no ano: <strong>8%</strong> realizado, referência 20 a 30%
        </p>
      </div>
    </section>
  );
}

/** ColchaoCard — versão estática. */
function ColchaoCard() {
  const colchaoCents = 183200;
  const registeredEconomia = 0;
  const realizedRatePct = "8.4";

  return (
    <section aria-labelledby="dash-colchao-title" className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title" id="dash-colchao-title">
          <Icon
            name="sparkles"
            size={16}
            className="dash-card__ic"
            aria-hidden="true"
          />
          Seu colchão
        </span>
        <PhaseBadge phase="calibrate" />
      </div>
      <div className="dash-card__body">
        <div className="dash-colchao__nums">
          <div className="dash-colchao__num">
            <span className="dash-colchao__label">Economia registrada</span>
            <span
              className={`dash-colchao__val${registeredEconomia <= 0 ? " dash-colchao__val--muted" : ""}`}
              style={{
                color:
                  registeredEconomia > 0 ? "var(--money-pos)" : "var(--text-faint)",
              }}
            >
              {fmtBRL(registeredEconomia)}
            </span>
          </div>
          <div className="dash-colchao__num">
            <span className="dash-colchao__label">
              Colchão este ano (sobra até hoje)
            </span>
            <span className="dash-colchao__val" style={{ color: "var(--money-pos)" }}>
              {fmtBRL(colchaoCents)} · {realizedRatePct}%
            </span>
          </div>
        </div>
        <p className="dash-colchao__text">
          Você guarda o que sobra como colchão para cobrir os meses negativos sem sacar
          investimento. Adaptação válida do método.
        </p>
        <Disclosure title="Próximo nível, quando quiser">
          <p
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--text-muted)",
              margin: 0,
              lineHeight: "var(--lh-normal)",
            }}
          >
            Registrar a Economia (meta 20 a 30% da renda) como uma saída mensal e
            separar a reserva. Isso vira hábito e protege de sacar investimento na hora
            errada.
          </p>
        </Disclosure>
      </div>
    </section>
  );
}

/** PerformanceCard — versão estática. */
function PerformanceCard() {
  const months = [
    { label: "junho", performance: 53200, rate: 8, incomplete: false },
    { label: "julho", performance: 71400, rate: 0, incomplete: true },
    { label: "agosto", performance: 60100, rate: 0, incomplete: true },
    { label: "setembro", performance: 58900, rate: 0, incomplete: true },
  ];

  return (
    <section aria-labelledby="dash-perf-title" className="dash-card">
      <div className="dash-card__head">
        <span
          className="dash-card__title"
          id="dash-perf-title"
          title="Caixa não é poupança: um mês pode ter saldo positivo e ainda assim performance baixa."
        >
          <Icon
            name="trendingUp"
            size={16}
            className="dash-card__ic"
            aria-hidden="true"
          />
          Performance por mês
        </span>
        <span className="dash-perf__hint">referência anual 20–30%</span>
      </div>
      <div className="dash-perf__row">
        {months.map((m) => (
          <div
            key={m.label}
            className={`dash-perf__cell${m.incomplete ? " is-incomplete" : ""}`}
            aria-label={
              m.incomplete
                ? `${m.label}: incompleto, projeção otimista`
                : `${m.label}: performance ${fmtBRL(m.performance)}, economizado ${m.rate}%`
            }
          >
            <span className="dash-perf__month">{m.label}</span>
            {m.incomplete ? (
              <>
                <span className={`dash-perf__val dash-perf__val--muted`}>
                  {fmtBRL(m.performance)}
                </span>
                <span
                  className="dash-perf__rate"
                  style={{ color: "var(--warning-500)" }}
                >
                  <Icon
                    name="alertTriangle"
                    size={11}
                    style={{ verticalAlign: "-1px", marginRight: 3 }}
                  />
                  incompleto
                </span>
              </>
            ) : (
              <>
                <span className="dash-perf__val" style={{ color: "var(--money-pos)" }}>
                  {fmtBRL(m.performance)}
                </span>
                <span className="dash-perf__rate">economizado {m.rate}%</span>
              </>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

/** MonthLedgerCard — versão estática "Dia a dia". */
function MonthLedgerCard() {
  const [ym, setYm] = React.useState("2026-06");
  const today = "2026-06-21";
  const year = 2026;
  const monthLabel = "Junho";

  // Amostra representativa: alguns dias com dados
  const rows = [
    {
      date: "2026-06-18",
      label: "18/06",
      entrada: 0,
      saida: 0,
      diario: 4200,
      saldo: 324800,
    },
    {
      date: "2026-06-19",
      label: "19/06",
      entrada: 0,
      saida: 85000,
      diario: 0,
      saldo: 239800,
    },
    {
      date: "2026-06-20",
      label: "20/06",
      entrada: 0,
      saida: 0,
      diario: 6700,
      saldo: 233100,
    },
    {
      date: "2026-06-21",
      label: "21/06",
      entrada: 0,
      saida: 0,
      diario: 14500,
      saldo: 218600,
    },
    {
      date: "2026-06-22",
      label: "22/06",
      entrada: 0,
      saida: 0,
      diario: null,
      saldo: null,
    },
    {
      date: "2026-06-23",
      label: "23/06",
      entrada: 0,
      saida: 0,
      diario: null,
      saldo: null,
    },
  ];

  // Saldo heatmap simplificado
  function saldoStyle(cents) {
    if (cents == null) return {};
    if (cents < 0)
      return { background: "rgba(224, 98, 91, 0.32)", color: "var(--text)" };
    if (cents < 50000)
      return { background: "rgba(224, 163, 62, 0.16)", color: "var(--text)" };
    if (cents < 200000)
      return { background: "rgba(52, 185, 129, 0.15)", color: "var(--text)" };
    return { background: "rgba(52, 185, 129, 0.30)", color: "var(--text)" };
  }

  const foot = {
    entrada: rows.reduce((s, r) => s + (r.entrada || 0), 0),
    saida: rows.reduce((s, r) => s + (r.saida || 0), 0),
    diario: rows.reduce((s, r) => s + (r.diario || 0), 0),
  };
  foot.saidaTotal = foot.saida + foot.diario;
  foot.performance = foot.entrada - foot.saidaTotal;

  return (
    <div className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title">
          <Icon name="calendarRange" size={16} className="dash-card__ic" />
          Dia a dia
        </span>
        <MonthNav
          label={`${monthLabel} de ${year}`}
          onPrev={() => {}}
          onNext={() => {}}
          onToday={() => {}}
          atToday={true}
          prevLabel="Mês anterior"
          nextLabel="Próximo mês"
        />
      </div>
      <div className="dash-card__body" style={{ padding: 0 }}>
        <div className="dash-ledger-scroll">
          <table className="dash-ledger-table">
            <thead>
              <tr>
                <th scope="col" style={{ textAlign: "left" }}>
                  Data
                </th>
                <th scope="col">Entrada</th>
                <th scope="col" title="Saídas fixas e a fatura do cartão no vencimento">
                  Saída
                </th>
                <th scope="col">Diário</th>
                <th scope="col">Saldo</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.date} className={r.date === today ? "is-today" : ""}>
                  <td style={{ fontFamily: "var(--font-sans)" }}>
                    {r.label}
                    {r.date === today && <span className="dash-today-tag">hoje</span>}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    {r.entrada ? (
                      <span className="money-pos">{fmtBRL(r.entrada)}</span>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    {r.saida ? fmtBRL(r.saida) : "—"}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    {r.diario ? fmtBRL(r.diario) : "—"}
                  </td>
                  <td style={{ textAlign: "right", ...saldoStyle(r.saldo) }}>
                    {r.saldo != null ? fmtBRL(r.saldo) : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <th scope="row">Total</th>
                <td style={{ textAlign: "right" }}>
                  {foot.entrada > 0 ? (
                    <span className="money-pos">{fmtBRL(foot.entrada)}</span>
                  ) : (
                    "—"
                  )}
                </td>
                <td style={{ textAlign: "right" }}>{fmtBRL(foot.saida)}</td>
                <td style={{ textAlign: "right" }}>{fmtBRL(foot.diario)}</td>
                <td style={{ textAlign: "right", color: "var(--text-faint)" }}>—</td>
              </tr>
              <tr>
                <th scope="row">Saída Total</th>
                <td
                  colSpan={3}
                  style={{
                    textAlign: "right",
                    color: "var(--text-faint)",
                    fontSize: "var(--fs-micro)",
                  }}
                >
                  saídas + diário
                </td>
                <td style={{ textAlign: "right" }}>{fmtBRL(foot.saidaTotal)}</td>
              </tr>
              <tr>
                <th
                  scope="row"
                  title="Resultado contábil do mês: entradas menos saída total."
                >
                  Resultado do mês
                </th>
                <td
                  colSpan={3}
                  style={{
                    textAlign: "right",
                    color: "var(--text-faint)",
                    fontSize: "var(--fs-micro)",
                  }}
                >
                  entradas − saída total
                </td>
                <td style={{ textAlign: "right", color: moneyColor(foot.performance) }}>
                  {foot.performance >= 0 ? "" : "−"}
                  {fmtBRL(Math.abs(foot.performance))}
                </td>
              </tr>
            </tfoot>
          </table>
        </div>
      </div>
    </div>
  );
}

/** WriteBack pending — versão estática (1 célula local pendente de envio). */
function WriteBackPending() {
  return (
    <div className="dash-wb">
      <div className="dash-wb__head">
        <Icon name="download" size={15} style={{ flexShrink: 0 }} aria-hidden="true" />
        <span aria-live="polite">1 célula local → planilha pendente</span>
      </div>
      <div className="dash-wb__actions">
        <Button variant="primary" size="sm">
          Sincronizar
        </Button>
        <Button variant="ghost" size="sm">
          Revisar e enviar
        </Button>
      </div>
    </div>
  );
}

/* ---- Tela completa ---- */
function DashboardScreen(props) {
  return (
    <div className="dash">
      <HeroSection />

      {/* Aviso: escassez de caixa prevista (ausente neste demo — dados positivos) */}
      {/* <DeficitBanner /> */}

      {/* WriteBack pendente: 1 célula local → planilha */}
      <WriteBackPending />

      {/* Último lançamento foi há 2 dias */}
      <LastLoggedBanner />

      {/* Check-in diário */}
      <DailyCheckinCard />

      {/* Previsibilidade: meses futuros incompletos */}
      <PrevisibilidadeCard />

      {/* Colchão: coaching do método */}
      <ColchaoCard />

      {/* Performance por mês */}
      <PerformanceCard />

      {/* Dia a dia: grade do mês */}
      <MonthLedgerCard />
    </div>
  );
}

window.DashboardScreen = DashboardScreen;
