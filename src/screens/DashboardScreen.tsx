import { useState } from "react";
import {
  AlertTriangle,
  CalendarRange,
  Minus,
  Sparkles,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { MetricTile } from "../design-system/components/MetricTile";
import { Money } from "../design-system/components/Money";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { getDashboardSummary, getForecast, isTauri } from "../lib/api";
import { saldoBand, SALDO_BAND_FILL, SALDO_BAND_LABEL } from "../lib/saldoHeatmap";
import { fmtBRL, fmtDayMonth, monthNamePtBR } from "../lib/format";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { PrevisibilidadeCard } from "./dashboard/PrevisibilidadeCard";
import { ColchaoCard } from "./dashboard/ColchaoCard";
import { PerformanceCard } from "./dashboard/PerformanceCard";
import { DailyCheckinCard } from "./dashboard/DailyCheckinCard";

export function DashboardScreen({ onAskMia }: { onAskMia: () => void }) {
  const [reloadKey, setReloadKey] = useState(0);
  const summaryQ = useCommand(
    `get_dashboard_summary:${reloadKey}`,
    getDashboardSummary,
  );
  const forecastQ = useCommand(`get_forecast:${reloadKey}`, getForecast);
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
            <Button variant="primary" onClick={() => window.location.reload()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  const reserveTrendIcon =
    summary?.reserve_trend === "up" ? (
      <TrendingUp size={15} strokeWidth={1.75} />
    ) : summary?.reserve_trend === "down" ? (
      <TrendingDown size={15} strokeWidth={1.75} />
    ) : (
      <Minus size={15} strokeWidth={1.75} />
    );

  const deficit =
    forecast?.deepest_deficit && forecast.deepest_deficit.balance_cents < 0
      ? forecast.deepest_deficit
      : null;

  // Guardrail duplo (caixa × poupança). "Pode gastar" honesto = o mais apertado dos dois.
  const savingsBinds = forecast?.binding_guardrail === "savings";
  const targetPct = forecast ? Math.round(forecast.savings_target_bps / 100) : 25;
  const ym = forecast ? forecast.today.slice(0, 7) : "";
  // Tabela diária mostra só o mês corrente; o histórico completo é o Livro-razão (slice 8).
  const dailyThisMonth = (forecast?.daily ?? []).filter(
    (d) => d.date.slice(0, 7) === ym,
  );
  const hasData = (summary?.transaction_count ?? 0) > 0;

  function handleLogged() {
    invalidateCommands();
    setReloadKey((k) => k + 1);
  }

  return (
    <div className="dash">
      <section className="dash-hero" aria-label="Quanto posso gastar hoje">
        <div className="dash-hero__lead">
          <p className="dash-hero__label">
            <InfoPopover term="pode_gastar">Pode gastar até</InfoPopover>
          </p>
          <p className="dash-hero__kpi">
            {forecast ? fmtBRL(forecast.safe_to_spend_today_cents) : "—"}
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

      <div className="dash-grid4">
        <MetricTile
          label="Saldo projetado"
          value={summary ? fmtBRL(summary.balance) : "—"}
          icon={<TrendingUp size={15} strokeWidth={1.75} />}
          sublabel={forecast ? `Fim de ${monthNamePtBR(forecast.today)}` : "Fim do mês"}
        />
        <MetricTile
          label="Diário de hoje"
          value={summary ? fmtBRL(summary.daily_spend_today) : "—"}
          sublabel={summary ? `de ${fmtBRL(summary.daily_budget)}` : ""}
        />
        <MetricTile
          label="Crédito no mês"
          value={summary?.has_credit ? fmtBRL(summary.credit_spend_month) : "—"}
          icon={<TrendingDown size={15} strokeWidth={1.75} />}
          sublabel={
            summary && !summary.has_credit
              ? "Sem cartão rastreado"
              : "No crédito, vira fatura no vencimento"
          }
        />
        <MetricTile
          label="Reserva"
          value={summary ? `${summary.reserve_months.toFixed(1)}m` : "—"}
          icon={reserveTrendIcon}
          sublabel="Mín. 6m · paz 12m+"
        />
      </div>

      {deficit && (
        <div className="dash-deficit" role="status">
          <AlertTriangle size={15} strokeWidth={1.75} />
          <span>
            Buraco previsto de{" "}
            <Money cents={deficit.balance_cents} size="sm" sign="negative" /> em{" "}
            {fmtDayMonth(deficit.date)}. Precisa de entrada nova ou corte até lá.
          </span>
        </div>
      )}

      {summary && hasData && (
        <DailyCheckinCard summary={summary} onLogged={handleLogged} />
      )}

      {forecast && hasData && <PrevisibilidadeCard forecast={forecast} />}

      {forecast && hasData && <ColchaoCard forecast={forecast} />}

      {forecast && <PerformanceCard forecast={forecast} />}

      <div className="dash-card">
        <div className="dash-card__head">
          <span className="dash-card__title">
            <CalendarRange size={16} strokeWidth={1.75} className="dash-card__ic" />
            Previsão diária de {forecast ? monthNamePtBR(forecast.today) : "mês atual"}
          </span>
        </div>
        <div className="dash-card__body" style={{ padding: 0 }}>
          {!forecast || (summary?.transaction_count ?? 0) === 0 ? (
            <EmptyState
              variant="empty"
              title="Nenhuma transação"
              description="Conecte o Google Sheets e importe sua planilha."
            />
          ) : dailyThisMonth.length === 0 ? (
            <EmptyState
              variant="empty"
              title="Sem projeção para este mês"
              description="Não há dias projetados no mês corrente."
            />
          ) : (
            <div className="fc-scroll">
              <table className="txn-table fc-table">
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
                  {dailyThisMonth.map((d) => {
                    const isToday = d.date === forecast.today;
                    return (
                      <tr key={d.date} className={isToday ? "fc-today" : ""}>
                        <td>
                          {fmtDayMonth(d.date)}
                          {isToday && <span className="fc-today__tag">hoje</span>}
                        </td>
                        <td className="money">
                          {d.income_cents ? (
                            <Money cents={d.income_cents} size="sm" sign="auto" />
                          ) : (
                            "—"
                          )}
                        </td>
                        <td className="money">
                          {d.fixed_out_cents ? (
                            <Money cents={d.fixed_out_cents} size="sm" />
                          ) : (
                            "—"
                          )}
                        </td>
                        <td className="money">
                          {d.daily_out_cents ? (
                            <Money cents={d.daily_out_cents} size="sm" />
                          ) : (
                            "—"
                          )}
                        </td>
                        {/* Saldo com o termômetro da planilha: a célula ganha o tom da faixa
                            (verde=folga … vermelho=aperto). O número herda --text p/ AA sobre
                            qualquer faixa; o sinal já vem da cor de fundo. */}
                        <td
                          className="money"
                          style={{
                            background: SALDO_BAND_FILL[saldoBand(d.balance_cents)],
                            color: "var(--text)",
                          }}
                          title={`Saldo ${SALDO_BAND_LABEL[saldoBand(d.balance_cents)]}`}
                        >
                          <Money cents={d.balance_cents} size="sm" sign="none" />
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      <PocketsCard />
    </div>
  );
}
