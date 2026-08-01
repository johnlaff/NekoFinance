import "./mes.css";
import { useEffect, useState } from "react";
import { ChartColumn, Clock3, PiggyBank, TrendingUp, Wallet } from "lucide-react";
import {
  getAnnualMetrics,
  getDashboardSummary,
  getForecast,
  ownerTotalsForMonth,
  isTauri,
  type MonthMetric,
  type OwnerTotal,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { MonthNav } from "../design-system/components/MonthNav";
import { EmptyState } from "../design-system/components/EmptyState";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { Money, SignedMoney } from "../design-system/components/Money";
import { HealthBadge } from "../design-system/components/HealthBadge";
import { Badge } from "../design-system/components/Badge";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { NoRecordDash } from "../design-system/components/NoRecordDash";
import { RangeRuler } from "../design-system/components/RangeRuler";
import { SegBar } from "../design-system/components/SegBar";
import { fmtBRL, MES, MES_ABBR } from "../lib/nkFormat";
import { SR_ONLY } from "../design-system/srOnly";
import { setCrumb } from "../shell/crumbStore";
import {
  currentMonthMetric,
  performanceStatus,
  economizadoStatus,
  custoVidaStatus,
  pctDisplay,
  serieLeitura,
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

// ---------------------------------------------------------------------------
// Cards do bento
// ---------------------------------------------------------------------------

function EconomiaCard({
  m,
  ytdBps,
  noRecord,
}: {
  m: MonthMetric;
  ytdBps: number;
  noRecord: boolean;
}) {
  const pct = pctDisplay(m.savings_rate_bps);
  const status = economizadoStatus(m.savings_rate_bps);
  const pinColor =
    status.level === "strong"
      ? "var(--success-400)"
      : status.level === "steady"
        ? "var(--primary)"
        : "var(--warning-400)";
  return (
    <section className="mes__card mes__card--econ" aria-labelledby="mes-econ-t">
      <header className="mes__cardhead">
        <PiggyBank size={16} strokeWidth={1.75} className="ic" />
        <h3 id="mes-econ-t">Economia guardada</h3>
        <InfoPopover term="economizado" hideMarker>
          <span className="mes__how">
            Como funciona?
            <span style={SR_ONLY}> — Economia guardada</span>
          </span>
        </InfoPopover>
      </header>
      {noRecord ? (
        <>
          <div className="mes__hero-pct">
            <NoRecordDash
              term={{
                title: "Economia sem registro",
                body: "A planilha ainda não tem lançamentos de Economia. Registre o primeiro aporte para a régua ler o mês.",
              }}
            />
          </div>
          <RangeRuler
            className="mes__ruler"
            max={40}
            zone={{ from: 20, to: 30 }}
            marks={RULER_MARKS}
            pin={null}
            label="Régua de economia de 0% a 40% com zona-alvo de 20% a 30%; sem registro de economia"
          />
          <p className="mes__cardfoot mes__rulnote">
            Sem registro de economia na planilha — a régua espera o primeiro aporte.
          </p>
        </>
      ) : (
        <>
          <div className="mes__hero-pct">
            {pct}%
            <small>
              <Money cents={m.economia_cents} size="inherit" /> de{" "}
              <Money cents={m.income_cents} size="inherit" /> que entraram
            </small>
          </div>
          <RangeRuler
            className="mes__ruler"
            max={40}
            zone={{ from: 20, to: 30 }}
            marks={RULER_MARKS}
            pin={{ value: m.savings_rate_bps / 100, label: `${pct}%`, color: pinColor }}
            label={`Régua de economia de 0% a 40% com zona-alvo de 20% a 30%; ${MES[m.month - 1]} em ${pct}%`}
          />
          <footer className="mes__cardfoot">
            <HealthBadge level={status.level} label={status.label} />
            <p className="mes__rulnote">
              No ano: {pctDisplay(ytdBps)}% — a régua julga a média anual, não o mês.
            </p>
          </footer>
        </>
      )}
    </section>
  );
}

const RULER_MARKS = [
  { at: 20, label: "20%" },
  { at: 30, label: "30%" },
  { at: 40, label: "40%" },
];

function CustoCard({ m, cardMode }: { m: MonthMetric; cardMode: boolean }) {
  const status = custoVidaStatus(m.cost_of_living_cents, m.income_cents);
  const total = Math.max(m.cost_of_living_cents, 1);
  const parts = [
    {
      name: "Saídas fixas",
      val: m.fixed_out_cents,
      color: "var(--type-saida)",
      context: null as string | null,
    },
    {
      name: "Diário",
      val: m.daily_out_cents,
      color: "var(--type-diario)",
      context:
        cardMode && m.daily_out_cents === 0
          ? "Não lançado — o variável vive no cartão"
          : null,
    },
    {
      name: "Cartão",
      val: m.cartao_cents,
      color: "var(--type-cartao)",
      context: null as string | null,
    },
  ];
  const segLabel = `Composição do custo de vida: ${parts
    .map((p) => `${p.name} ${fmtBRL(p.val)}`)
    .join(", ")}`;
  return (
    <section className="mes__card mes__card--custo" aria-labelledby="mes-custo-t">
      <header className="mes__cardhead">
        <Wallet size={16} strokeWidth={1.75} className="ic" />
        <h3 id="mes-custo-t">Custo de vida</h3>
        <InfoPopover term="custo_de_vida" hideMarker>
          <span className="mes__how">
            Como funciona?
            <span style={SR_ONLY}> — Custo de vida</span>
          </span>
        </InfoPopover>
      </header>
      <div className="mes__kpi-val">
        <Money cents={m.cost_of_living_cents} size="inherit" />
      </div>
      <SegBar
        className="mes__segbar"
        segments={parts.map((p) => ({
          name: p.name,
          fraction: p.val / total,
          color: p.color,
        }))}
        label={segLabel}
      />
      <ul className="mes__comp">
        {parts.map((p) => (
          <li key={p.name}>
            <span
              aria-hidden="true"
              className="mes__dot"
              style={{ background: p.color }}
            />
            <span className="mes__comp-name">
              {p.name}
              {p.context ? <small>{p.context}</small> : null}
            </span>
            <span className="mes__comp-val">
              <Money cents={p.val} size="inherit" />
            </span>
          </li>
        ))}
      </ul>
      <footer className="mes__cardfoot">
        <HealthBadge level={status.level} label={status.label} />
      </footer>
    </section>
  );
}

function PerformanceCard({ m }: { m: MonthMetric }) {
  const status = performanceStatus(m.performance_cents);
  return (
    <section className="mes__card mes__card--kpi" aria-labelledby="mes-perf-t">
      <header className="mes__cardhead">
        <TrendingUp size={16} strokeWidth={1.75} className="ic" />
        <h3 id="mes-perf-t">Performance</h3>
        <InfoPopover term="performance" hideMarker>
          <span className="mes__how">
            Como funciona?
            <span style={SR_ONLY}> — Performance</span>
          </span>
        </InfoPopover>
      </header>
      <div
        className="mes__kpi-val"
        style={{
          color: m.performance_cents >= 0 ? "var(--money-pos)" : "var(--money-neg)",
        }}
      >
        <SignedMoney cents={m.performance_cents} size="inherit" />
      </div>
      {/* A conta exibida precisa fechar com a Performance do motor: Economia, Patrimônio e a
          previsão de diário restante são termos explícitos (Performance = Entradas − Custo de
          vida − Economia − Patrimônio − Previsão de diário). */}
      <p className="mes__equation">
        Entradas <Money cents={m.income_cents} size="inherit" /> − Custo de vida{" "}
        <Money cents={m.cost_of_living_cents} size="inherit" />
        {m.economia_cents > 0 ? (
          <>
            {" "}
            − Economia <Money cents={m.economia_cents} size="inherit" />
          </>
        ) : null}
        {m.patrimonio_cents > 0 ? (
          <>
            {" "}
            − Patrimônio <Money cents={m.patrimonio_cents} size="inherit" />
          </>
        ) : null}
        {m.daily_projected_cents > 0 ? (
          <>
            {" "}
            − Previsão de diário{" "}
            <Money cents={m.daily_projected_cents} size="inherit" />
          </>
        ) : null}
      </p>
      <footer className="mes__cardfoot">
        <HealthBadge level={status.level} label={status.label} />
      </footer>
    </section>
  );
}

function DiarioMedioCard({ m, isCurrent }: { m: MonthMetric; isCurrent: boolean }) {
  return (
    <section className="mes__card mes__card--kpi" aria-labelledby="mes-diario-t">
      <header className="mes__cardhead">
        <Clock3 size={16} strokeWidth={1.75} className="ic" />
        <h3 id="mes-diario-t">Diário médio</h3>
        <InfoPopover term="diario_medio" hideMarker>
          <span className="mes__how">
            Como funciona?
            <span style={SR_ONLY}> — Diário médio</span>
          </span>
        </InfoPopover>
      </header>
      <div className="mes__kpi-val">
        <Money cents={m.real_daily_avg_cents} size="inherit" />
      </div>
      <p className="mes__equation">
        {isCurrent
          ? "Média realizada por dia até hoje"
          : "Média realizada por dia no mês"}
      </p>
      {m.real_daily_avg_cents === 0 ? (
        <footer className="mes__cardfoot">
          <Badge tone="secondary">Zerado</Badge>
        </footer>
      ) : null}
    </section>
  );
}

function ComparadoCard({
  trend,
  activeYear,
  activeMonth,
  onPick,
}: {
  trend: MonthMetric[];
  activeYear: number;
  activeMonth: number;
  onPick: (ym: string) => void;
}) {
  // Escala honesta: teto = max(40%, maior valor da janela) — a grade nunca
  // mente a altura relativa das barras; zero é chão.
  const scaleMax = Math.max(4000, ...trend.map((t) => t.savings_rate_bps));
  return (
    <section className="mes__card mes__card--comp" aria-labelledby="mes-comp-t">
      <header className="mes__cardhead">
        <ChartColumn size={16} strokeWidth={1.75} className="ic" />
        <h3 id="mes-comp-t">Comparado aos meses anteriores</h3>
      </header>
      <div className="mes__comp-body">
        {/* Cada barra é atalho para o mês dela (mesma navegação do MonthNav). */}
        <div className="mes__bars">
          {trend.map((t) => {
            const isSel = t.year === activeYear && t.month === activeMonth;
            const pct = pctDisplay(t.savings_rate_bps);
            const h = (t.savings_rate_bps / scaleMax) * 100;
            return (
              <button
                type="button"
                className={`mes__bar${isSel ? " mes__bar--now" : ""}`}
                key={`${t.year}-${t.month}`}
                aria-label={`${MES[t.month - 1]}: ${pct}% — ver o mês`}
                aria-current={isSel ? "true" : undefined}
                onClick={() => onPick(ymOf(t))}
              >
                <i aria-hidden="true" style={{ height: `${h.toFixed(2)}%` }} />
                <em>{MES_ABBR[t.month - 1]}</em>
                <b>{pct}%</b>
              </button>
            );
          })}
        </div>
        <p className="mes__comp-note">{serieLeitura(trend)}</p>
      </div>
    </section>
  );
}

function OwnerTotalsCard({ ownerTotals }: { ownerTotals: OwnerTotal[] }) {
  return (
    <section className="mes__card mes__card--owners" aria-label="Por titular">
      <header className="mes__cardhead">
        <h3>Por titular</h3>
      </header>
      <div className="mes__owners">
        {ownerTotals.map((o) => (
          <span key={o.owner_person_id} className="mes__owner">
            <OwnerChip name={o.owner_name} avatar />
            <Money cents={o.total_cents} size="md" />
          </span>
        ))}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Tela — composição fina
// ---------------------------------------------------------------------------

export function TotaisScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
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

  // O crumb da appbar mostra o mês visto; `setCrumb` é função de módulo
  // (identidade fixa), então o efeito só re-dispara quando o rótulo muda.
  const crumbMonth = Number(activeYmForOwners.slice(5, 7)) || 0;
  const crumbLabel = crumbMonth
    ? `${MES[crumbMonth - 1]} de ${activeYmForOwners.slice(0, 4)}`
    : null;
  useEffect(() => {
    setCrumb("mes", crumbLabel);
    return () => setCrumb("mes", null);
  }, [crumbLabel]);

  if (forecastQ.loading) {
    return <EmptyState variant="skeleton" skeletonRows={6} />;
  }
  // Falha de carga e ausência de dado são estados diferentes: o primeiro anuncia por
  // `role="alert"` e não manda importar planilha; o segundo é lacuna do método, e é a Mia
  // quem diz o que fazer com ela.
  if (forecastQ.error) {
    return (
      <EmptyState
        variant="error"
        title="Não foi possível carregar os totais"
        description="A leitura dos cálculos do mês falhou. Tente de novo em instantes."
      />
    );
  }

  if (!forecast) {
    return (
      <EmptyState
        icon={<MiaAvatar width={22} height={22} />}
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
        icon={<MiaAvatar width={22} height={22} />}
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
  const cardMode = isCurrent && summaryQ.data?.spending_mode === "card";
  const noRecord = forecast.annual_savings.economia_state === "no_record";

  // Série: últimos 6 meses até o mês visto, ordem cronológica.
  const trendStart = Math.max(0, idx - 5);
  const trend = idx >= 0 ? months.slice(trendStart, idx + 1) : [m];

  return (
    <div className="mes">
      <header className="mes__head">
        <div className="mes__head-text">
          <h2>{monthName} em números</h2>
          <p className="mes__context">
            A conta que o método faz todo mês: o que entrou, o que a vida custou, o que
            sobrou guardado.
          </p>
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
      </header>

      <div className="mes__bento">
        <EconomiaCard
          m={m}
          ytdBps={forecast.annual_savings.economia_ruler_rate_bps}
          noRecord={noRecord}
        />
        <CustoCard m={m} cardMode={cardMode} />
        <PerformanceCard m={m} />
        <DiarioMedioCard m={m} isCurrent={isCurrent} />
        {!noRecord && (
          <ComparadoCard
            trend={trend}
            activeYear={m.year}
            activeMonth={m.month}
            onPick={(ym) => setSelectedYm(ym === todayYm ? null : ym)}
          />
        )}
        {ownerTotals.length >= 2 && <OwnerTotalsCard ownerTotals={ownerTotals} />}
      </div>

      {!isTauri && (
        <p className="mes__webhint">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
