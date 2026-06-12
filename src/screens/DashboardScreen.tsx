import {
  AlertTriangle,
  CalendarRange,
  Minus,
  Sparkles,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { MetricTile } from "../design-system/components/MetricTile";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { getDashboardSummary, getForecast, isTauri } from "../lib/api";
import { fmtBRL, fmtDayMonth, monthNamePtBR } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { useCountUp } from "../lib/useCountUp";

export function DashboardScreen({ onAskMia }: { onAskMia: () => void }) {
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const summary = summaryQ.data ?? null;
  const forecast = forecastQ.data ?? null;
  const loading = summaryQ.loading || forecastQ.loading;
  const error = summaryQ.error ?? forecastQ.error;
  const animatedBalance = useCountUp(summary?.balance ?? 0, "saldo-projetado");

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

  const trendIcon =
    summary?.reserve_trend === "up"
      ? "▲"
      : summary?.reserve_trend === "down"
        ? "▼"
        : "—";

  const deficit =
    forecast?.deepest_deficit && forecast.deepest_deficit.balance_cents < 0
      ? forecast.deepest_deficit
      : null;

  return (
    <div className="dash">
      <div className="dash-hero">
        <div className="dash-hero__txt">
          <div className="dash-hero__line">
            {summary && summary.transaction_count > 0 ? (
              <>
                <b>{summary.transaction_count} transações</b> no banco local. Reserva:{" "}
                <span className="dash-hero__money">
                  {summary.reserve_months.toFixed(1)} meses
                </span>
                .
              </>
            ) : (
              "Nenhuma transação ainda. Conecte o Google Sheets e importe sua planilha."
            )}
          </div>
          {forecast && (
            <div className="dash-hero__line dash-safe">
              Pode gastar até{" "}
              <b className="dash-hero__money">
                {fmtBRL(forecast.safe_to_spend_today_cents)}
              </b>{" "}
              hoje sem furar o mês.
            </div>
          )}
        </div>
        <Button
          variant="secondary"
          iconLeft={<Sparkles size={16} strokeWidth={1.75} />}
          onClick={onAskMia}
        >
          Perguntar à Mia
        </Button>
      </div>

      {deficit && (
        <div className="dash-deficit" role="status">
          <AlertTriangle size={15} strokeWidth={1.75} />
          <span>
            Buraco previsto:{" "}
            <b className="dash-hero__money">{fmtBRL(deficit.balance_cents)}</b> em{" "}
            {fmtDayMonth(deficit.date)} — é preciso entrada nova ou corte até lá.
          </span>
        </div>
      )}

      <div className="dash-grid4">
        <MetricTile
          label="Saldo projetado"
          value={summary ? fmtBRL(animatedBalance) : "—"}
          icon={<TrendingUp size={15} strokeWidth={1.75} />}
          sublabel={forecast ? `Fim de ${monthNamePtBR(forecast.today)}` : "Fim do mês"}
        />
        <MetricTile
          label="Diário hoje"
          value={summary ? fmtBRL(summary.daily_spend_today) : "—"}
          sublabel={summary ? `de ${fmtBRL(summary.daily_budget)}` : ""}
        />
        <MetricTile
          label="Crédito no mês"
          value={summary ? fmtBRL(summary.credit_spend_month) : "—"}
          icon={<TrendingDown size={15} strokeWidth={1.75} />}
          sublabel="Régua 2 — fatura acumulada"
        />
        <MetricTile
          label="Reserva"
          value={summary ? `${summary.reserve_months.toFixed(1)}m ${trendIcon}` : "—"}
          icon={<Minus size={15} strokeWidth={1.75} />}
          deltaDir={
            summary?.reserve_trend === "up"
              ? "up"
              : summary?.reserve_trend === "down"
                ? "down"
                : "neutral"
          }
          sublabel="Meta: 6 meses de gastos"
        />
      </div>

      <div className="dash-2col">
        <div className="dash-card">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <CalendarRange size={16} strokeWidth={1.75} className="dash-card__ic" />
              Previsão diária — {forecast ? monthNamePtBR(forecast.today) : "mês atual"}
            </span>
          </div>
          <div className="dash-card__body" style={{ padding: 0 }}>
            {!forecast || (summary?.transaction_count ?? 0) === 0 ? (
              <EmptyState
                variant="empty"
                title="Nenhuma transação"
                description="Conecte o Google Sheets e importe sua planilha."
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
                    {forecast.daily.map((d) => {
                      const isToday = d.date === forecast.today;
                      return (
                        <tr key={d.date} className={isToday ? "fc-today" : ""}>
                          <td>
                            {fmtDayMonth(d.date)}
                            {isToday && <span className="fc-today__tag">hoje</span>}
                          </td>
                          <td className={d.income_cents ? "money positive" : "money"}>
                            {d.income_cents ? fmtBRL(d.income_cents) : "—"}
                          </td>
                          <td className="money">
                            {d.fixed_out_cents ? fmtBRL(d.fixed_out_cents) : "—"}
                          </td>
                          <td className="money">
                            {d.daily_out_cents ? fmtBRL(d.daily_out_cents) : "—"}
                          </td>
                          <td
                            className={d.balance_cents < 0 ? "money negative" : "money"}
                          >
                            {fmtBRL(d.balance_cents)}
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

        <aside className="assistant-panel">
          <div className="assistant-header">
            <MiaAvatar width={40} height={40} />
            <div>
              <p className="assistant-label">Copiloto</p>
              <h2 className="assistant-name">Mia</h2>
            </div>
          </div>
          <p>
            {summary && summary.transaction_count > 0
              ? "Seus dados estão carregados. Mia vai poder diagnosticar padrões de gasto, reserva e crédito."
              : "Importe dados da planilha primeiro. Depois Mia analisa seus gastos."}
          </p>
          <div className="assistant-note">
            {summary
              ? `Reserva: ${summary.reserve_months.toFixed(1)} meses — ${summary.reserve_trend === "down" ? "caindo" : summary.reserve_trend === "up" ? "subindo" : "estável"}`
              : "Sem dados de reserva ainda."}
          </div>
          <Badge tone="secondary">Chat em desenvolvimento</Badge>
        </aside>
      </div>

      <PocketsCard />
    </div>
  );
}
