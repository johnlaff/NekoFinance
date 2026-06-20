import { useState } from "react";
import { AlertTriangle, Sparkles } from "lucide-react";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { Money } from "../design-system/components/Money";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { getDashboardSummary, getForecast, isTauri } from "../lib/api";
import { formatBRL, fmtDayMonth, monthNamePtBR } from "../lib/format";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { PrevisibilidadeCard } from "./dashboard/PrevisibilidadeCard";
import { ColchaoCard } from "./dashboard/ColchaoCard";
import { colchaoPhase } from "./dashboard/colchaoPhase";
import { PerformanceCard } from "./dashboard/PerformanceCard";
import { DailyCheckinCard } from "./dashboard/DailyCheckinCard";
import { MonthLedgerCard } from "./dashboard/MonthLedgerCard";

export function DashboardScreen({
  onAskMia,
  onQuickAddAmountRef,
}: {
  onAskMia: () => void;
  /** Ref do campo de valor do check-in rápido — repassado ao AppShell p/ o atalho "N". */
  onQuickAddAmountRef?: (ref: HTMLInputElement | null) => void;
}) {
  // Chaves de cache COMPARTILHADAS (sem sufixo) → o dashboard reaproveita o `get_forecast` /
  // `get_dashboard_summary` já buscados por outras telas, em vez de um slot privado que forçava
  // re-fetch a cada visita. `invalidateCommands()` (em handleLogged/handleReload) limpa o cache
  // inteiro, então o próximo render rebusca fresco. `ledgerKey` força só o re-fetch do grid mensal.
  const [ledgerKey, setLedgerKey] = useState(0);
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const summary = summaryQ.data ?? null;
  const forecast = forecastQ.data ?? null;
  const loading = summaryQ.loading || forecastQ.loading;
  const error = summaryQ.error ?? forecastQ.error;

  if (!isTauri) {
    return (
      <div className="dash">
        <div className="dash-hero">
          <div className="dash-hero__txt">
            <div className="dash-hero__line">
              <b>Preview web.</b> Abra o app desktop para ver seus dados.
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="dash">
        <EmptyState variant="skeleton" skeletonRows={6} />
      </div>
    );
  }

  if (error) {
    return (
      <div className="dash">
        <EmptyState
          variant="error"
          title="Não foi possível carregar os dados"
          description={error}
          action={
            <Button variant="primary" onClick={handleReload}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  const deficit =
    forecast?.deepest_deficit && forecast.deepest_deficit.balance_cents < 0
      ? forecast.deepest_deficit
      : null;

  // Guardrail duplo (caixa × poupança). "Pode gastar" honesto = o mais apertado dos dois.
  const savingsBinds = forecast?.binding_guardrail === "savings";
  const targetPct = forecast ? Math.round(forecast.savings_target_bps / 100) : 25;
  const hasData = (summary?.transaction_count ?? 0) > 0;
  // Diário médio do mês corrente (Σ diário realizado ÷ dias decorridos) para o ritmo no check-in.
  const ym = forecast?.today.slice(0, 7);
  const monthDailyAvgCents =
    forecast?.months.find((m) => `${m.year}-${String(m.month).padStart(2, "0")}` === ym)
      ?.real_daily_avg_cents ?? 0;

  function handleLogged() {
    invalidateCommands();
    setLedgerKey((k) => k + 1);
  }

  function handleReload() {
    invalidateCommands();
    setLedgerKey((k) => k + 1);
  }

  return (
    <div className="dash">
      <section className="dash-hero" aria-label="Quanto posso gastar hoje">
        <div className="dash-hero__lead">
          <p className="dash-hero__label">
            <InfoPopover term="pode_gastar">Pode gastar até</InfoPopover>
          </p>
          <p className="dash-hero__kpi">
            {forecast ? formatBRL(forecast.safe_to_spend_today_cents) : "—"}
            <span className="dash-hero__kpi-suffix">hoje</span>
          </p>
          <p className="dash-hero__reason">
            {!forecast
              ? "Importe sua planilha para ver a previsão."
              : savingsBinds
                ? `O menor de dois limites: respeita sua meta de guardar ${targetPct}% no ano.`
                : "O menor de dois limites: o que o caixa aguenta sem nenhum dia no vermelho."}
          </p>
          <div className="dash-hero__row">
            {summary && summary.transaction_count > 0 && (
              <dl className="dash-hero__stats">
                <div>
                  <dt>Reserva</dt>
                  <dd>{summary.reserve_months.toFixed(1)} meses</dd>
                </div>
                <div>
                  <dt>Lançamentos</dt>
                  <dd>{summary.transaction_count}</dd>
                </div>
              </dl>
            )}
            <Button
              variant="secondary"
              size="sm"
              iconLeft={<Sparkles size={15} strokeWidth={1.75} />}
              onClick={onAskMia}
            >
              Conhecer a Mia
            </Button>
          </div>
        </div>

        {forecast && forecast.daily.length > 1 && (
          <aside className="dash-hero__forecast" aria-label="Saldo projetado do mês">
            <div className="dash-hero__forecast-head">
              <span>Saldo no fim de {monthNamePtBR(forecast.today)}</span>
              <Money
                cents={forecast.month_end[0]?.balance_cents ?? 0}
                size="md"
                sign="auto"
              />
            </div>
            <BalanceTrajectory
              daily={forecast.daily}
              today={forecast.today}
              variant="compact"
            />
            <p className="dash-hero__forecast-foot">
              {deficit ? (
                <span className="negative">
                  Pode faltar em {fmtDayMonth(deficit.date)}:{" "}
                  <Money cents={deficit.balance_cents} size="sm" sign="negative" />
                </span>
              ) : (
                "Como seu saldo deve evoluir até o fim do mês."
              )}
            </p>
          </aside>
        )}
      </section>

      {deficit && (
        <output className="dash-deficit">
          <AlertTriangle size={15} strokeWidth={1.75} />
          <span>
            Buraco previsto de{" "}
            <Money cents={deficit.balance_cents} size="sm" sign="negative" /> em{" "}
            {fmtDayMonth(deficit.date)}. Precisa de entrada nova ou corte até lá.
          </span>
        </output>
      )}

      {summary && hasData && (
        <DailyCheckinCard
          summary={summary}
          monthAvgCents={monthDailyAvgCents}
          onLogged={handleLogged}
          onAmountRef={onQuickAddAmountRef}
        />
      )}

      {forecast && hasData && <PrevisibilidadeCard forecast={forecast} />}

      {forecast && hasData && (
        <ColchaoCard forecast={forecast} phase={colchaoPhase(summary, forecast)} />
      )}

      {forecast && <PerformanceCard forecast={forecast} />}

      {forecast && <MonthLedgerCard today={forecast.today} reloadKey={ledgerKey} />}

      <PocketsCard />
    </div>
  );
}
