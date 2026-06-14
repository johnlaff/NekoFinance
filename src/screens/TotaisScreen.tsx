import { getForecast } from "../lib/api";
import type { MonthMetric } from "../lib/api";
import { monthNamePtBR } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { HealthBadge, type HealthLevel } from "../design-system/components/HealthBadge";
import { EmptyState } from "../design-system/components/EmptyState";

const SAVINGS_TARGET_BPS = 2000; // meta 20% (piso da faixa 20–30%)

/** Encontra a métrica do mês corrente a partir do `today` do forecast. */
export function currentMonthMetric(
  months: MonthMetric[],
  today: string,
): MonthMetric | null {
  const [y, m] = today.split("-").map(Number);
  return months.find((x) => x.year === y && x.month === m) ?? null;
}

interface Status {
  level: HealthLevel;
  label: string;
}

export function performanceStatus(cents: number): Status {
  return cents >= 0
    ? { level: "strong", label: "Sobrou dinheiro" }
    : { level: "risk", label: "Faltou dinheiro" };
}

export function economizadoStatus(bps: number): Status {
  return bps >= SAVINGS_TARGET_BPS
    ? { level: "strong", label: "Dentro do ideal" }
    : { level: "watch", label: "Abaixo do ideal" };
}

export function custoVidaStatus(cost: number, income: number): Status {
  return cost <= income
    ? { level: "steady", label: "Dentro da renda" }
    : { level: "watch", label: "Acima da renda" };
}

function MetricRow({
  label,
  value,
  status,
  sublabel,
}: {
  label: string;
  value: React.ReactNode;
  status?: Status;
  sublabel?: string;
}) {
  return (
    <article
      style={{
        background: "var(--surface)",
        border: "var(--bw-hair) solid var(--border)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--elev-card)",
        padding: "var(--space-6)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-3)",
      }}
    >
      <span
        style={{
          fontSize: "var(--fs-label)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--ls-label)",
          textTransform: "uppercase",
          color: "var(--text-muted)",
        }}
      >
        {label}
      </span>
      <span style={{ display: "flex", alignItems: "baseline", gap: "var(--space-3)" }}>
        {value}
      </span>
      {status ? (
        <HealthBadge level={status.level} label={status.label} />
      ) : sublabel ? (
        <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-faint)" }}>
          {sublabel}
        </span>
      ) : null}
    </article>
  );
}

export function TotaisScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const forecast = forecastQ.data ?? null;

  if (forecastQ.loading) {
    return (
      <div style={{ padding: "var(--space-8)", color: "var(--text-muted)" }}>
        Carregando os totais do mês…
      </div>
    );
  }
  if (forecastQ.error || !forecast) {
    return (
      <EmptyState
        title="Sem dados para os totais"
        description="Importe a planilha ou lance um movimento para ver os cálculos do mês."
      />
    );
  }

  const m = currentMonthMetric(forecast.months, forecast.today);
  if (!m) {
    return (
      <EmptyState
        title="Mês sem movimentos"
        description="Ainda não há lançamentos no mês corrente para calcular os totais."
      />
    );
  }

  const pct = (m.savings_rate_bps / 100).toFixed(0);
  const iso = `${m.year}-${String(m.month).padStart(2, "0")}-01`;
  const monthLabel = monthNamePtBR(iso);
  const monthCap = monthLabel.charAt(0).toUpperCase() + monthLabel.slice(1);

  return (
    <div style={{ maxWidth: "var(--content-max)", margin: "0 auto", padding: "var(--space-2)" }}>
      <header style={{ marginBottom: "var(--space-6)" }}>
        <h1
          style={{
            fontSize: "var(--fs-h2)",
            fontWeight: "var(--fw-bold)",
            letterSpacing: "var(--ls-tight)",
            margin: 0,
          }}
        >
          {monthCap} de {m.year}
        </h1>
        <p style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)", margin: "var(--space-1) 0 0" }}>
          Cálculos do mês — performance, custo de vida, economizado e diário médio.
        </p>
      </header>

      <section
        aria-label="Cálculos do mês"
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: "var(--space-5)",
        }}
      >
        <MetricRow
          label="Performance"
          value={<Money cents={m.performance_cents} size="lg" sign="auto" />}
          status={performanceStatus(m.performance_cents)}
        />
        <MetricRow
          label="Economizado"
          value={
            <span
              style={{
                fontFamily: "var(--font-money)",
                fontSize: "var(--fs-money-lg)",
                fontWeight: "var(--fw-bold)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {pct}%
            </span>
          }
          status={economizadoStatus(m.savings_rate_bps)}
        />
        <MetricRow
          label="Custo de vida"
          value={<Money cents={m.cost_of_living_cents} size="lg" />}
          status={custoVidaStatus(m.cost_of_living_cents, m.income_cents)}
        />
        <MetricRow
          label="Diário médio"
          value={<Money cents={m.real_daily_avg_cents} size="lg" />}
          sublabel="média realizada por dia até hoje"
        />
      </section>

      <section aria-label="Movimentações do mês" style={{ marginTop: "var(--space-8)" }}>
        <h2
          style={{
            fontSize: "var(--fs-label)",
            fontWeight: "var(--fw-semibold)",
            letterSpacing: "var(--ls-label)",
            textTransform: "uppercase",
            color: "var(--text-muted)",
            margin: "0 0 var(--space-4)",
          }}
        >
          Movimentações do mês
        </h2>
        <div style={{ display: "flex", gap: "var(--space-8)", flexWrap: "wrap" }}>
          <span style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
            <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>Entradas</span>
            <Money cents={m.income_cents} size="md" sign="auto" />
          </span>
          <span style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
            <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>Economia</span>
            <Money cents={m.economia_cents} size="md" />
          </span>
        </div>
      </section>
    </div>
  );
}
