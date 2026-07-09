/* Neko Finance — Totais screen (new).
   "Cálculos do mês" — performance, economizado, custo de vida, diário médio,
   movimentações e totais por titular.
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.TotaisScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Money, MonthNav, InfoPopover, OwnerChip, Disclosure } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectTotaisCSS() {
  if (document.getElementById("totais-css")) return;
  const s = document.createElement("style");
  s.id = "totais-css";
  s.textContent = `
/* Layout raiz */
.tot {
  max-width: var(--content-max);
  margin: 0 auto;
  padding: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
}

/* Cabeçalho */
.tot-header {
  margin-bottom: var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}
.tot-header__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  flex-wrap: wrap;
}
.tot-header__h1 {
  font-size: var(--fs-h2);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight);
  margin: 0;
  color: var(--text-strong);
}
.tot-header__desc {
  color: var(--text-muted);
  font-size: var(--fs-sm);
  margin: 0;
  line-height: var(--lh-normal);
}

/* Grelha de métricas */
.tot-metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: var(--space-5);
  margin-bottom: var(--space-8);
}

/* Card de métrica individual */
.tot-card {
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--elev-card);
  padding: var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.tot-card__label {
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-muted);
}
.tot-card__value {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
}
.tot-card__pct {
  font-family: var(--font-money);
  font-size: var(--fs-money-lg);
  font-weight: var(--fw-bold);
  font-variant-numeric: tabular-nums;
  color: var(--text-strong);
}
.tot-card__sublabel {
  font-size: var(--fs-sm);
  color: var(--text-faint);
  line-height: var(--lh-normal);
}

/* Chip de status (ponto + rótulo) */
.tot-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  align-self: flex-start;
  padding: 4px 11px 4px 9px;
  border-radius: var(--radius-pill);
  font-size: var(--fs-sm);
  font-weight: var(--fw-semibold);
}
.tot-chip__dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
}

/* Cabeçalho de seção */
.tot-section-head {
  font-size: var(--fs-label);
  font-weight: var(--fw-semibold);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  color: var(--text-muted);
  margin: 0 0 var(--space-4);
}

/* Seção: Movimentações */
.tot-movs {
  margin-bottom: var(--space-8);
}
.tot-movs__row {
  display: flex;
  gap: var(--space-8);
  flex-wrap: wrap;
}
.tot-mov {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}
.tot-mov__label {
  font-size: var(--fs-sm);
  color: var(--text-muted);
}
.tot-mov__hint {
  font-size: var(--fs-micro);
  color: var(--text-faint);
}

/* Separador visual entre Saídas e Saída Total */
.tot-mov--accent .tot-mov__label {
  color: var(--text);
  font-weight: var(--fw-semibold);
}

/* Barra de economizado (YTD) */
.tot-ytd {
  margin-top: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.tot-ytd__track {
  height: 5px;
  border-radius: var(--radius-pill);
  background: var(--bg-subtle);
  overflow: hidden;
}
.tot-ytd__fill {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--chart-1);
  transition: width var(--dur-slow) var(--ease-entrance);
}
@media (prefers-reduced-motion: reduce) {
  .tot-ytd__fill { transition: none; }
}
.tot-ytd__label {
  font-size: var(--fs-micro);
  color: var(--text-faint);
  line-height: var(--lh-normal);
}

/* Seção: Por titular */
.tot-owners {
  margin-bottom: var(--space-8);
}
.tot-owners__row {
  display: flex;
  gap: var(--space-8);
  flex-wrap: wrap;
}
.tot-owner {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

/* Disclosure nota metodológica */
.tot-note {
  margin-top: var(--space-8);
}
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
  return (cents < 0 ? "−" : "") + "R$ " + n;
}

/* STATUS_TONE mapeia HealthLevel → tokens de cor */
const STATUS_TONE = {
  strong: {
    dot: "var(--success-400)",
    fg: "var(--success-400)",
    bg: "var(--success-tint)",
  },
  steady: {
    dot: "var(--primary)",
    fg: "var(--primary-quiet-text)",
    bg: "var(--primary-quiet)",
  },
  watch: {
    dot: "var(--warning-400)",
    fg: "var(--warning-400)",
    bg: "var(--warning-tint)",
  },
  risk: {
    dot: "var(--danger-400)",
    fg: "var(--danger-400)",
    bg: "var(--danger-tint)",
  },
};

/* ---- StatusChip ---- */
function StatusChip({ level, label }) {
  const t = STATUS_TONE[level] || STATUS_TONE.steady;
  return (
    <span className="tot-chip" style={{ background: t.bg, color: t.fg }}>
      <span
        aria-hidden="true"
        className="tot-chip__dot"
        style={{ background: t.dot }}
      />
      {label}
    </span>
  );
}

/* ---- MetricCard ---- */
function MetricCard({ label, term, children, status, sublabel, ytdPct, ytdLabel }) {
  return (
    <article className="tot-card">
      <span className="tot-card__label">
        {term ? <InfoPopover term={term}>{label}</InfoPopover> : label}
      </span>
      <div className="tot-card__value">{children}</div>
      {status && <StatusChip level={status.level} label={status.label} />}
      {sublabel && <span className="tot-card__sublabel">{sublabel}</span>}
      {ytdPct != null && (
        <div className="tot-ytd">
          <div
            className="tot-ytd__track"
            role="progressbar"
            aria-valuenow={ytdPct}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`Economizado acumulado no ano: ${ytdPct}%`}
          >
            <div className="tot-ytd__fill" style={{ width: `${ytdPct}%` }} />
          </div>
          {ytdLabel && <p className="tot-ytd__label">{ytdLabel}</p>}
        </div>
      )}
    </article>
  );
}

/* ---- MovTotal: item de movimentação ---- */
function MovTotal({ label, cents, hint, sign = "none", accent }) {
  return (
    <span className={`tot-mov${accent ? " tot-mov--accent" : ""}`}>
      <span className="tot-mov__label">{label}</span>
      <Money cents={cents} size="md" sign={sign} />
      {hint && <span className="tot-mov__hint">{hint}</span>}
    </span>
  );
}

/* ---- Dados de demonstração ---- */
// Junho de 2026 — números realistas e representativos do método.
const DEMO = {
  year: 2026,
  month: 6,
  monthLabel: "Junho",

  // Performance: Entradas − Saída Total
  performance_cents: 53200, // R$ 532,00 — sobra positiva

  // Economizado%: taxa de poupança do mês
  savings_rate_bps: 2240, // 22,40% — dentro do ideal (20–30%)

  // Custo de vida = Saídas + Diário
  cost_of_living_cents: 693800, // R$ 6.938,00

  // Entradas do mês
  income_cents: 747000, // R$ 7.470,00

  // Diário médio realizado
  real_daily_avg_cents: 14700, // R$ 147,00/dia

  // Movimentações individuais
  fixed_out_cents: 385000, // R$ 3.850,00 (saídas fixas + cartão)
  daily_out_cents: 308800, // R$ 3.088,00 (gasto variável diário)
  economia_cents: 0, // R$ 0,00 (neste mês não houve registro Economia)

  // YTD Economizado (anual)
  ytd_pct_raw: 18, // 18% acumulado no ano (abaixo de 20% — "Abaixo do ideal")
  ytd_pct: 18,

  // Por titular
  owners: [
    { id: "ana", name: "Ana", who: "personal", total_cents: 432600 },
    { id: "parceira", name: "Ana", who: "partner", total_cents: 261200 },
  ],
};

/* ---- Lógica de status (espelha totaisStatus.ts) ---- */
function performanceStatus(cents) {
  return cents >= 0
    ? { level: "strong", label: "Sobrou dinheiro" }
    : { level: "risk", label: "Faltou dinheiro" };
}

function economizadoStatus(bps) {
  if (bps > 3000) return { level: "steady", label: "Acima do ideal" };
  if (bps >= 2000) return { level: "strong", label: "Dentro do ideal" };
  return { level: "watch", label: "Abaixo do ideal" };
}

function custoVidaStatus(cost, income) {
  return cost <= income
    ? { level: "steady", label: "Dentro da renda" }
    : { level: "watch", label: "Acima da renda" };
}

/* ---- Tela completa ---- */
function TotaisScreen(props) {
  const m = DEMO;
  const pct = Math.round(m.savings_rate_bps / 100);
  const ytdPct = Math.min(m.ytd_pct_raw, 100);
  const ytdLabel =
    m.ytd_pct_raw > 100
      ? "no ano: >100% acumulado · meta 20–30% (média anual)"
      : `no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`;

  return (
    <div className="tot">
      {/* Cabeçalho + MonthNav */}
      <header className="tot-header">
        <div className="tot-header__top">
          <h1 className="tot-header__h1">Totais</h1>
          <MonthNav
            label={`${m.monthLabel} de ${m.year}`}
            onPrev={() => {}}
            onNext={() => {}}
            onToday={() => {}}
            canPrev={true}
            canNext={false}
            atToday={true}
          />
        </div>
        <p className="tot-header__desc">
          Cálculos do mês: performance, custo de vida, economizado e diário médio.
        </p>
      </header>

      {/* Grelha de métricas */}
      <section aria-label="Cálculos do mês" className="tot-metrics">
        {/* Performance */}
        <MetricCard
          label="Performance"
          term="performance"
          status={performanceStatus(m.performance_cents)}
        >
          <Money cents={m.performance_cents} size="lg" sign="auto" />
        </MetricCard>

        {/* Economizado */}
        <MetricCard
          label="Economizado"
          term="economizado"
          status={economizadoStatus(m.savings_rate_bps)}
          ytdPct={ytdPct}
          ytdLabel={ytdLabel}
        >
          <span className="tot-card__pct">{pct}%</span>
        </MetricCard>

        {/* Custo de vida */}
        <MetricCard
          label="Custo de vida"
          term="custo_de_vida"
          status={custoVidaStatus(m.cost_of_living_cents, m.income_cents)}
          sublabel="= Saída Total (saídas incl. cartão + diário)"
        >
          <Money cents={m.cost_of_living_cents} size="lg" />
        </MetricCard>

        {/* Diário médio */}
        <MetricCard
          label="Diário médio"
          term="diario_medio"
          sublabel="média realizada por dia até hoje"
        >
          <Money cents={m.real_daily_avg_cents} size="lg" />
        </MetricCard>
      </section>

      {/* Movimentações do mês */}
      <section aria-label="Movimentações do mês" className="tot-movs">
        <h2 className="tot-section-head">Movimentações do mês</h2>
        <div className="tot-movs__row">
          <MovTotal label="Entradas" cents={m.income_cents} sign="auto" />
          <MovTotal
            label="Saídas"
            cents={m.fixed_out_cents}
            hint="fixas (cartão entra aqui)"
          />
          <MovTotal label="Diário" cents={m.daily_out_cents} hint="gasto variável" />
          <MovTotal label="Economia" cents={m.economia_cents} hint="guardado no mês" />
          <MovTotal
            label="Saída Total"
            cents={m.cost_of_living_cents}
            hint="saídas (incl. cartão) + diário = custo de vida"
            accent
          />
        </div>
      </section>

      {/* Por titular */}
      {m.owners.length >= 2 && (
        <section aria-label="Por titular" className="tot-owners">
          <h2 className="tot-section-head">Por titular</h2>
          <div className="tot-owners__row">
            {m.owners.map((o) => (
              <span key={o.id} className="tot-owner">
                <OwnerChip name={o.name} who={o.who} avatar />
                <Money cents={o.total_cents} size="md" />
              </span>
            ))}
          </div>
        </section>
      )}

      {/* Nota metodológica */}
      <div className="tot-note">
        <Disclosure title="Como o Neko calcula estes totais">
          <p
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--text-muted)",
              margin: 0,
              lineHeight: "var(--lh-normal)",
            }}
          >
            <strong>Performance</strong> = Entradas − Saída Total. Positivo significa
            que o mês ficou dentro da renda. <strong>Economizado%</strong> = o que foi
            registrado como Economia ÷ Entradas (meta 20–30% em média anual).{" "}
            <strong>Custo de vida</strong> = Saídas fixas + Diário — inclui cartão de
            crédito no vencimento. O <strong>Diário médio</strong> é a média realizada
            por dia até hoje, não uma meta. Estes cálculos espelham diretamente as
            colunas da planilha do método.
          </p>
        </Disclosure>
      </div>
    </div>
  );
}

window.TotaisScreen = TotaisScreen;
