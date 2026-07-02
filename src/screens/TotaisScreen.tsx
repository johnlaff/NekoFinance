import "./mes.css";
import { useState } from "react";
import { TrendingUp, Wallet, PiggyBank, LayoutList, GitCompare } from "lucide-react";
import {
  getAnnualMetrics,
  getForecast,
  ownerTotalsForMonth,
  isTauri,
  type MonthMetric,
  type OwnerTotal,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { MonthNav } from "../design-system/components/MonthNav";
import { EmptyState } from "../design-system/components/EmptyState";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { Money } from "../design-system/components/Money";
import { fmtBRL, fmtSigned, fmtCompact, MES, MES_ABBR } from "../lib/nkFormat";
import {
  currentMonthMetric,
  performanceStatus,
  economizadoStatus,
  custoVidaStatus,
} from "./totaisStatus";

/** "YYYY-MM" from a MonthMetric. */
function ymOf(m: { year: number; month: number }): string {
  return `${m.year}-${String(m.month).padStart(2, "0")}`;
}

const _annualFetcherCache = new Map<
  number,
  () => ReturnType<typeof getAnnualMetrics>
>();
function annualFetcher(year: number): () => ReturnType<typeof getAnnualMetrics> {
  const cached = _annualFetcherCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _annualFetcherCache.set(year, fn);
  return fn;
}

function mergePastAnnualWithForecastMonths(
  annualMonths: MonthMetric[],
  forecastMonths: MonthMetric[],
  today: string,
): MonthMetric[] {
  const todayYm = today.slice(0, 7);
  const byMonth = new Map<string, MonthMetric>();
  for (const month of annualMonths) {
    if (ymOf(month) < todayYm) byMonth.set(ymOf(month), month);
  }
  for (const month of forecastMonths) {
    byMonth.set(ymOf(month), month);
  }
  return Array.from(byMonth.values()).toSorted(
    (a, b) => a.year - b.year || a.month - b.month,
  );
}

/** StatusChip: a small dot + label badge matching the existing design vocabulary. */
function StatusChip({ level, label }: { level: string; label: string }) {
  const colors: Record<string, { bg: string; fg: string; dot: string }> = {
    strong: {
      bg: "var(--success-tint)",
      fg: "var(--success-400)",
      dot: "var(--success-400)",
    },
    steady: {
      bg: "var(--primary-quiet)",
      fg: "var(--primary-quiet-text)",
      dot: "var(--primary)",
    },
    watch: {
      bg: "var(--warning-tint)",
      fg: "var(--warning-400)",
      dot: "var(--warning-400)",
    },
    risk: {
      bg: "var(--danger-tint)",
      fg: "var(--danger-400)",
      dot: "var(--danger-400)",
    },
  };
  const t = colors[level] ?? colors["watch"]!;
  return (
    <span className="status-chip" style={{ background: t.bg, color: t.fg }}>
      <span
        aria-hidden="true"
        className="status-chip__dot"
        style={{ background: t.dot }}
      />
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Sub-components (split from the giant TotaisScreen for no-giant-component)
// ---------------------------------------------------------------------------

interface HeroTilesProps {
  isCurrent: boolean;
  performance: number;
  entradas: number;
  saidaTotal: number;
  custoVida: number;
  economia: number;
  patrimonio: number;
  economizadoPct: number;
  ytdPctLabel: string;
  perfStatus: { level: string; label: string };
  custoStatus: { level: string; label: string };
  econStatus: { level: string; label: string };
}

function HeroTiles({
  isCurrent,
  performance,
  entradas,
  saidaTotal,
  custoVida,
  economia,
  patrimonio,
  economizadoPct,
  ytdPctLabel,
  perfStatus,
  custoStatus,
  econStatus,
}: HeroTilesProps) {
  return (
    <div className="mes-result">
      {/* Resultado */}
      <div className="mes-tile mes-tile--hero">
        <p className="mes-tile__lab">
          <TrendingUp size={14} strokeWidth={1.75} />
          <span>Performance</span>
          {isCurrent ? (
            <span style={{ fontWeight: 400, textTransform: "none" }}> (parcial)</span>
          ) : null}
        </p>
        <div
          className="mes-tile__val"
          style={{
            color: performance >= 0 ? "var(--money-pos)" : "var(--money-neg)",
          }}
        >
          {fmtSigned(performance)}
        </div>
        {/* A conta exibida precisa fechar com a Performance do motor: Economia e Patrimônio
            são termos explícitos (Performance = Entradas − Saída total − Economia − Patrimônio). */}
        <p className="mes-tile__sub">
          Entradas {fmtBRL(entradas)} − Saída total {fmtBRL(saidaTotal)}
          {economia > 0 ? <> − Economia {fmtBRL(economia)}</> : null}
          {patrimonio > 0 ? <> − Patrimônio {fmtBRL(patrimonio)}</> : null}
        </p>
        <div style={{ marginTop: 10 }}>
          <StatusChip level={perfStatus.level} label={perfStatus.label} />
        </div>
      </div>

      {/* Custo de vida */}
      <div className="mes-tile">
        <p className="mes-tile__lab">
          <Wallet size={14} strokeWidth={1.75} />
          Custo de vida
        </p>
        <div className="mes-tile__val" style={{ color: "var(--text-strong)" }}>
          {fmtBRL(custoVida)}
        </div>
        <p className="mes-tile__sub">= Saída Total (saídas incl. cartão + diário)</p>
        <div style={{ marginTop: 10 }}>
          <StatusChip level={custoStatus.level} label={custoStatus.label} />
        </div>
      </div>

      {/* Economizado */}
      <div className="mes-tile">
        <p className="mes-tile__lab">
          <PiggyBank size={14} strokeWidth={1.75} />
          Economizado
        </p>
        <div
          className="mes-tile__val"
          style={{
            color: economizadoPct >= 20 ? "var(--money-pos)" : "var(--warning-400)",
          }}
        >
          {economizadoPct.toFixed(0)}%
        </div>
        <p className="mes-tile__sub">
          {fmtBRL(economia)} guardados · meta de 20% a 30%
        </p>
        <p className="mes-tile__sub mes-tile__sub--ytd">{ytdPctLabel}</p>
        <div style={{ marginTop: 10 }}>
          <StatusChip level={econStatus.level} label={econStatus.label} />
        </div>
      </div>
    </div>
  );
}

interface OutPartsCardProps {
  saidaTotal: number;
  economia: number;
  patrimonio: number;
  outParts: { name: string; val: number; color: string }[];
  outTotal: number;
}

function OutPartsCard({
  saidaTotal,
  economia,
  patrimonio,
  outParts,
  outTotal,
}: OutPartsCardProps) {
  return (
    <section className="card" aria-label="Para onde foi o dinheiro">
      <div className="card__head">
        <span className="card__title">
          <LayoutList size={16} strokeWidth={1.75} className="ic" />
          Para onde foi o dinheiro
        </span>
        <span className="card__head-money">
          {fmtBRL(saidaTotal + economia + patrimonio)}
        </span>
      </div>
      <div className="card__body">
        <div className="mes-bar">
          {outParts.map((p) =>
            p.val > 0 ? (
              <span
                key={p.name}
                className="mes-bar__seg"
                style={{
                  background: p.color,
                  width: ((p.val / outTotal) * 100).toFixed(2) + "%",
                }}
              />
            ) : null,
          )}
        </div>
        <div className="mes-leg">
          {outParts.map((p) => (
            <div className="mes-leg__row" key={p.name}>
              <span className="mes-leg__dot" style={{ background: p.color }} />
              <span className="mes-leg__name">{p.name}</span>
              <span className="mes-leg__amt">{fmtBRL(p.val)}</span>
              <span className="mes-leg__pct">
                {Math.round((p.val / outTotal) * 100)}%
              </span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

interface FlowCardProps {
  entradas: number;
  saidaTotal: number;
  economia: number;
  patrimonio: number;
  performance: number;
}

function FlowCard({
  entradas,
  saidaTotal,
  economia,
  patrimonio,
  performance,
}: FlowCardProps) {
  return (
    <section className="card" aria-label="Entrou e Saiu">
      <div className="card__head">
        <span className="card__title">
          <GitCompare size={16} strokeWidth={1.75} className="ic" />
          Entrou × Saiu
        </span>
      </div>
      <div className="card__body">
        <div className="mes-flow">
          <div className="mes-flow__row">
            <span className="mes-flow__lab">Entradas</span>
            <span className="mes-flow__track">
              <span
                className="mes-flow__fill"
                style={{ width: "100%", background: "var(--money-pos)" }}
              />
            </span>
            <span className="mes-flow__amt" style={{ color: "var(--money-pos)" }}>
              {fmtBRL(entradas)}
            </span>
          </div>
          <div className="mes-flow__row">
            <span className="mes-flow__lab">Saída total</span>
            <span className="mes-flow__track">
              <span
                className="mes-flow__fill"
                style={{
                  width:
                    Math.min(100, (saidaTotal / Math.max(entradas, 1)) * 100).toFixed(
                      2,
                    ) + "%",
                  background: "var(--type-saida)",
                }}
              />
            </span>
            <span className="mes-flow__amt" style={{ color: "var(--money-neg)" }}>
              {fmtBRL(saidaTotal)}
            </span>
          </div>
          {/* Economia é um termo do resultado (Performance = Entradas − Saída total −
              Economia); sem ela a aritmética exibida não fecharia com o "Sobrou no mês". */}
          {economia > 0 ? (
            <div className="mes-flow__row">
              <span className="mes-flow__lab">Economia</span>
              <span className="mes-flow__track">
                <span
                  className="mes-flow__fill"
                  style={{
                    width:
                      Math.min(100, (economia / Math.max(entradas, 1)) * 100).toFixed(
                        2,
                      ) + "%",
                    background: "var(--type-economia)",
                  }}
                />
              </span>
              <span className="mes-flow__amt" style={{ color: "var(--type-economia)" }}>
                {fmtBRL(economia)}
              </span>
            </div>
          ) : null}
          {/* Patrimônio também é termo do resultado (previdência/ilíquido sai do saldo). */}
          {patrimonio > 0 ? (
            <div className="mes-flow__row">
              <span className="mes-flow__lab">Patrimônio</span>
              <span className="mes-flow__track">
                <span
                  className="mes-flow__fill"
                  style={{
                    width:
                      Math.min(100, (patrimonio / Math.max(entradas, 1)) * 100).toFixed(
                        2,
                      ) + "%",
                    background: "var(--text-muted)",
                  }}
                />
              </span>
              <span className="mes-flow__amt" style={{ color: "var(--text-muted)" }}>
                {fmtBRL(patrimonio)}
              </span>
            </div>
          ) : null}
        </div>
        <div className="mes-flow__summary">
          <span className="mes-flow__summary-lab">Sobrou no mês</span>
          <span
            className="mes-flow__summary-val"
            style={{
              color: performance >= 0 ? "var(--money-pos)" : "var(--money-neg)",
            }}
          >
            {fmtSigned(performance)}
          </span>
        </div>
      </div>
    </section>
  );
}

interface TrendCardProps {
  trend: { year: number; month: number; performance_cents: number }[];
  maxAbs: number;
  activeYear: number;
  activeMonth: number;
}

function TrendCard({ trend, maxAbs, activeYear, activeMonth }: TrendCardProps) {
  return (
    <section className="card" aria-label="Resultado nos últimos meses">
      <div className="card__head">
        <span className="card__title">
          <TrendingUp size={16} strokeWidth={1.75} className="ic" />
          Resultado nos últimos meses
        </span>
        <span className="mes-trend__ref-label">referência anual de 20% a 30%</span>
      </div>
      <div className="card__body">
        <div className="mes-trend">
          {trend.map((t, i) => {
            const h = (Math.abs(t.performance_cents) / maxAbs) * 100;
            const pos = t.performance_cents >= 0;
            const isSel = t.year === activeYear && t.month === activeMonth;
            const abbr = MES_ABBR[t.month - 1] ?? "";
            return (
              <div className="mes-trend__col" key={`${t.year}-${t.month}-${i}`}>
                <span className="mes-trend__val-label">
                  {fmtCompact(t.performance_cents)}
                </span>
                <div
                  className="mes-trend__bar"
                  style={{
                    height: h.toFixed(2) + "%",
                    background: pos ? "var(--money-pos)" : "var(--money-neg)",
                    opacity: isSel ? 1 : 0.45,
                  }}
                />
                <span className="mes-trend__m">{abbr}</span>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

interface OwnerTotalsCardProps {
  ownerTotals: OwnerTotal[];
}

function OwnerTotalsCard({ ownerTotals }: OwnerTotalsCardProps) {
  return (
    <section className="card" aria-label="Por titular">
      <div className="card__head">
        <span className="card__title">Por titular</span>
      </div>
      <div className="card__body">
        <div style={{ display: "flex", gap: 32, flexWrap: "wrap" }}>
          {ownerTotals.map((o) => (
            <span
              key={o.owner_person_id}
              style={{ display: "flex", flexDirection: "column", gap: 8 }}
            >
              <OwnerChip name={o.owner_name} avatar />
              <Money cents={o.total_cents} size="md" />
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Main screen — thin composition
// ---------------------------------------------------------------------------

export function TotaisScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const [selectedYm, setSelectedYm] = useState<string | null>(null);
  const forecast = forecastQ.data ?? null;
  const forecastYear = Number(forecast?.today.slice(0, 4)) || new Date().getFullYear();
  const annualQ = useCommand(
    `annual_metrics:${forecastYear}:totais`,
    annualFetcher(forecastYear),
  );

  // Derive owner query key before any conditional return to keep hook order stable.
  const activeYmForOwners = selectedYm ?? forecast?.today.slice(0, 7) ?? "";
  const ownerYear = Number(activeYmForOwners.slice(0, 4)) || 0;
  const ownerMonth = Number(activeYmForOwners.slice(5, 7)) || 0;
  const ownerTotalsQ = useCommand(
    `owner_totals_for_month:${ownerYear}:${ownerMonth}`,
    () => ownerTotalsForMonth(ownerYear, ownerMonth),
  );
  const ownerTotals: OwnerTotal[] = ownerTotalsQ.data ?? [];

  if (forecastQ.loading) {
    return <EmptyState variant="skeleton" skeletonRows={6} />;
  }
  if (forecastQ.error || !forecast) {
    return (
      <EmptyState
        title="Sem dados para os totais"
        description="Importe a planilha ou lance um movimento para ver os cálculos do mês."
      />
    );
  }

  const months = mergePastAnnualWithForecastMonths(
    annualQ.data?.months ?? [],
    forecast.months,
    forecast.today,
  );
  const todayYm = forecast.today.slice(0, 7);
  const activeYm = selectedYm ?? todayYm;
  const idx = months.findIndex((x) => ymOf(x) === activeYm);
  const m =
    idx >= 0 ? months[idx]! : currentMonthMetric(forecast.months, forecast.today);

  if (!m) {
    return (
      <EmptyState
        title="Mês sem movimentos"
        description="Ainda não há lançamentos no mês corrente para calcular os totais."
      />
    );
  }

  const canPrev = idx > 0;
  const canNext = idx >= 0 && idx < months.length - 1;
  const goPrev = () => {
    if (canPrev) setSelectedYm(ymOf(months[idx - 1]!));
  };
  const goNext = () => {
    if (canNext) setSelectedYm(ymOf(months[idx + 1]!));
  };
  const goToday = () => setSelectedYm(null);

  const isCurrent = activeYm === todayYm;
  const monthName = MES[m.month - 1] ?? "";

  // Derived metrics
  const performance = m.performance_cents;
  const entradas = m.income_cents;
  const saidaTotal = m.cost_of_living_cents; // custo de vida = saída total
  const custoVida = m.cost_of_living_cents;
  const economia = m.economia_cents;
  const patrimonio = m.patrimonio_cents;
  const economizadoPct = m.savings_rate_bps / 100;
  const fixedOut = m.fixed_out_cents;
  const dailyOut = m.daily_out_cents;
  const cartao = m.cartao_cents;

  // "Para onde foi o dinheiro" bar segments.
  const outParts = [
    { name: "Saídas fixas", val: fixedOut, color: "var(--type-saida)" },
    { name: "Cartão", val: cartao, color: "var(--type-cartao)" },
    { name: "Diário", val: dailyOut, color: "var(--type-diario)" },
    { name: "Economia", val: economia, color: "var(--type-economia)" },
    { name: "Patrimônio", val: patrimonio, color: "var(--text-muted)" },
  ];
  const outTotal = Math.max(saidaTotal + economia + patrimonio, 1);

  // Trend: last 6 months in chronological order (most recent = current idx).
  const trendStart = Math.max(0, idx - 5);
  const trend = months.slice(trendStart, idx + 1);
  const maxAbs = Math.max(...trend.map((t) => Math.abs(t.performance_cents)), 1);

  // Annual savings for the annual economizado% reference.
  const a = forecast.annual_savings;
  const ytdPctRaw = Math.round(
    (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
  );
  const ytdPct = Math.min(ytdPctRaw, 100);
  const ytdPctLabel =
    ytdPctRaw > 100
      ? "no ano: >100% acumulado · meta 20–30% (média anual)"
      : `no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`;

  // Status badges (keeps existing test assertions for label text).
  const perfStatus = performanceStatus(performance);
  const econStatus = economizadoStatus(m.savings_rate_bps);
  const custoStatus = custoVidaStatus(custoVida, entradas);

  return (
    <div className="mes">
      {/* Header */}
      <div className="mes-head">
        <div className="mes-title">
          {monthName} {m.year}
        </div>
        <MonthNav
          label={`${monthName} de ${m.year}`}
          onPrev={goPrev}
          onNext={goNext}
          onToday={goToday}
          canPrev={canPrev}
          canNext={canNext}
          atToday={isCurrent}
          prevLabel="Mês anterior"
          nextLabel="Próximo mês"
        />
      </div>

      {/* Hero tiles: Resultado, Custo de vida, Economizado */}
      <HeroTiles
        isCurrent={isCurrent}
        performance={performance}
        entradas={entradas}
        saidaTotal={saidaTotal}
        custoVida={custoVida}
        economia={economia}
        patrimonio={patrimonio}
        economizadoPct={economizadoPct}
        ytdPctLabel={ytdPctLabel}
        perfStatus={perfStatus}
        custoStatus={custoStatus}
        econStatus={econStatus}
      />

      {/* Two-column cards */}
      <div className="mes-grid2">
        <OutPartsCard
          saidaTotal={saidaTotal}
          economia={economia}
          patrimonio={patrimonio}
          outParts={outParts}
          outTotal={outTotal}
        />
        <FlowCard
          entradas={entradas}
          saidaTotal={saidaTotal}
          economia={economia}
          patrimonio={patrimonio}
          performance={performance}
        />
      </div>

      {/* Resultado nos últimos meses (trend) */}
      <TrendCard
        trend={trend}
        maxAbs={maxAbs}
        activeYear={m.year}
        activeMonth={m.month}
      />

      {/* Diário médio tile (text anchor kept for tests) */}
      <section className="card" aria-label="Diário médio">
        <div className="card__head">
          <span className="card__title">Diário médio</span>
        </div>
        <div className="card__body">
          <div
            className="mes-tile__val"
            style={{ fontSize: 24, color: "var(--text-strong)" }}
          >
            <Money cents={m.real_daily_avg_cents} size="lg" />
          </div>
          <p className="mes-tile__sub">média realizada por dia até hoje</p>
        </div>
      </section>

      {/* Por titular (shown only when 2+ owners) */}
      {ownerTotals.length >= 2 && <OwnerTotalsCard ownerTotals={ownerTotals} />}

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
