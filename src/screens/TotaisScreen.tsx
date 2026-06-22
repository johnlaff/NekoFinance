import "./mes.css";
import { useState } from "react";
import { TrendingUp, Wallet, PiggyBank, LayoutList, GitCompare } from "lucide-react";
import { getForecast, ownerTotalsForMonth, isTauri, type OwnerTotal } from "../lib/api";
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
  SAVINGS_MIN_BPS,
} from "./totaisStatus";

// Re-export the canonical constant so external importers (tests, AnnualScreen) can pull it from
// a single source without creating a circular dep. The real definition lives in totaisStatus.ts.
export { SAVINGS_MIN_BPS };

/** "YYYY-MM" from a MonthMetric. */
function ymOf(m: { year: number; month: number }): string {
  return `${m.year}-${String(m.month).padStart(2, "0")}`;
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
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 11px 4px 9px",
        borderRadius: "var(--radius-pill)",
        fontSize: 12,
        fontWeight: 600,
        background: t.bg,
        color: t.fg,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          background: t.dot,
          flex: "none",
        }}
      />
      {label}
    </span>
  );
}

export function TotaisScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const [selectedYm, setSelectedYm] = useState<string | null>(null);
  const forecast = forecastQ.data ?? null;

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

  const months = forecast.months.toSorted(
    (a, b) => a.year - b.year || a.month - b.month,
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
  const economizadoPct = m.savings_rate_bps / 100;
  const fixedOut = m.fixed_out_cents;
  const dailyOut = m.daily_out_cents;

  // "Para onde foi o dinheiro" bar segments.
  // cartão is included in fixed_out_cents per MonthMetric docs. No separate field exists.
  const outParts = [
    { name: "Saídas fixas", val: fixedOut, color: "var(--type-saida)" },
    { name: "Cartão", val: 0, color: "var(--type-cartao)" },
    { name: "Diário", val: dailyOut, color: "var(--type-diario)" },
    { name: "Economia", val: economia, color: "var(--type-economia)" },
  ];
  const outTotal = Math.max(saidaTotal + economia, 1);

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
          <p className="mes-tile__sub">
            Entradas {fmtBRL(entradas)} − Saída total {fmtBRL(saidaTotal)}
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
          <p className="mes-tile__sub" style={{ marginTop: 4, fontSize: 11.5 }}>
            {ytdPctLabel}
          </p>
          <div style={{ marginTop: 10 }}>
            <StatusChip level={econStatus.level} label={econStatus.label} />
          </div>
        </div>
      </div>

      {/* Two-column cards */}
      <div className="mes-grid2">
        {/* Para onde foi o dinheiro */}
        <section className="card" aria-label="Para onde foi o dinheiro">
          <div className="card__head">
            <span className="card__title">
              <LayoutList size={16} strokeWidth={1.75} className="ic" />
              Para onde foi o dinheiro
            </span>
            <span
              style={{
                fontFamily: "var(--font-money)",
                fontSize: 12.5,
                color: "var(--text-faint)",
              }}
            >
              {fmtBRL(saidaTotal + economia)}
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

        {/* Entrou × Saiu */}
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
                        Math.min(
                          100,
                          (saidaTotal / Math.max(entradas, 1)) * 100,
                        ).toFixed(2) + "%",
                      background: "var(--type-saida)",
                    }}
                  />
                </span>
                <span className="mes-flow__amt" style={{ color: "var(--money-neg)" }}>
                  {fmtBRL(saidaTotal)}
                </span>
              </div>
            </div>
            <div
              style={{
                marginTop: 18,
                paddingTop: 14,
                borderTop: "1px solid var(--border)",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "baseline",
              }}
            >
              <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
                Sobrou no mês
              </span>
              <span
                style={{
                  fontFamily: "var(--font-money)",
                  fontVariantNumeric: "tabular-nums",
                  fontSize: 18,
                  fontWeight: 700,
                  color: performance >= 0 ? "var(--money-pos)" : "var(--money-neg)",
                }}
              >
                {fmtSigned(performance)}
              </span>
            </div>
          </div>
        </section>
      </div>

      {/* Resultado nos últimos meses (trend) */}
      <section className="card" aria-label="Resultado nos últimos meses">
        <div className="card__head">
          <span className="card__title">
            <TrendingUp size={16} strokeWidth={1.75} className="ic" />
            Resultado nos últimos meses
          </span>
          <span style={{ fontSize: 11.5, color: "var(--text-faint)" }}>
            referência anual de 20% a 30%
          </span>
        </div>
        <div className="card__body">
          <div className="mes-trend">
            {trend.map((t, i) => {
              const h = (Math.abs(t.performance_cents) / maxAbs) * 100;
              const pos = t.performance_cents >= 0;
              const isSel = t.year === m.year && t.month === m.month;
              const abbr = MES_ABBR[t.month - 1] ?? "";
              return (
                <div className="mes-trend__col" key={`${t.year}-${t.month}-${i}`}>
                  <span
                    style={{
                      fontFamily: "var(--font-money)",
                      fontSize: 10.5,
                      color: "var(--text-faint)",
                    }}
                  >
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
      {ownerTotals.length >= 2 && (
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
      )}

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
