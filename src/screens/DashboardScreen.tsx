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

  // Guardrail duplo (caixa × poupança). "Pode gastar" honesto = o mais apertado dos dois.
  const savingsBinds = forecast?.binding_guardrail === "savings";
  const targetPct = forecast ? Math.round(forecast.savings_target_bps / 100) : 25;
  const ym = forecast ? forecast.today.slice(0, 7) : "";
  // Tabela diária mostra só o mês corrente; o histórico completo é o Livro-razão (slice 8).
  const dailyThisMonth = (forecast?.daily ?? []).filter(
    (d) => d.date.slice(0, 7) === ym,
  );
  // Performance dos próximos meses (Caixa ≠ Performance): expõe meses magros — é onde o
  // "cartão sequestra o salário futuro" aparece.
  const monthsAhead = (forecast?.months ?? [])
    .filter((m) => `${m.year}-${String(m.month).padStart(2, "0")}` >= ym)
    .slice(0, 4);
  // Poupança do ANO (a meta 20–30% é média anual): realizada (honesta) vs projetada (otimista).
  const annual = forecast?.annual_savings ?? null;
  const realizedRatePct = annual ? (annual.realized_rate_bps / 100).toFixed(1) : "0.0";
  const projectedRatePct = annual
    ? (annual.projected_rate_bps / 100).toFixed(1)
    : "0.0";
  // Previsibilidade: meses futuros incompletos (futuro vazio = projeção otimista demais).
  const hasBaseline = (forecast?.baseline_outflow_cents ?? 0) > 0;
  const incompleteMonths = (forecast?.coverage ?? []).filter((c) => !c.is_complete);
  const firstIncomplete = incompleteMonths[0];
  // Meses incompletos (chave YYYY-MM) — usados para marcar a performance como não-confiável.
  const incompleteKeys = new Set(
    incompleteMonths.map((c) => `${c.year}-${String(c.month).padStart(2, "0")}`),
  );
  const hasCoverage = (forecast?.coverage.length ?? 0) > 0;
  const trustedLabel = forecast?.trusted_through_month
    ? monthNamePtBR(`${forecast.trusted_through_month}-01`)
    : null;
  // Coaching de adaptação — o "colchão": o dono não registra Economia formal (linha do método),
  // guarda o excedente como buffer em caixa. É adaptação VÁLIDA; o app reconhece antes de ensinar.
  // (Quando a aba Economia for importada — slice 7 — a poupança formal substitui o proxy net.)
  const colchaoCents = annual?.realized_savings_cents ?? 0;

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
              hoje{" "}
              {savingsBinds
                ? `sem furar sua meta de poupança do ano (${targetPct}%).`
                : "antes do menor saldo do futuro."}
            </div>
          )}
          {forecast &&
            savingsBinds &&
            forecast.savings_headroom_cents !== null &&
            forecast.savings_headroom_cents < 0 && (
              <div className="dash-hero__line dash-safe__ctx">
                Sua poupança do ano está em {realizedRatePct}% (meta {targetPct}%). Em
                caixa há{" "}
                <b className="dash-hero__money">
                  {fmtBRL(forecast.cash_headroom_cents)}
                </b>
                , mas isso é sua reserva — gastar no cartão hoje vira fatura e afunda os
                meses à frente.
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

      {forecast && (summary?.transaction_count ?? 0) > 0 && (
        <div
          role="region"
          aria-labelledby="dash-predict-title"
          className={`dash-card dash-predict ${firstIncomplete ? "is-incomplete" : ""}`}
        >
          <div className="dash-card__head">
            <span className="dash-card__title" id="dash-predict-title">
              <CalendarRange
                size={16}
                strokeWidth={1.75}
                className="dash-card__ic"
                aria-hidden="true"
              />
              Previsibilidade
            </span>
            {trustedLabel && (
              <span className="dash-predict__trusted">
                confiável até <b>{trustedLabel}</b>
              </span>
            )}
          </div>
          <div className="dash-card__body">
            {!hasBaseline ? (
              <p className="dash-predict__neutral">
                Ainda não há meses realizados suficientes para avaliar a confiabilidade
                da projeção. Importe mais histórico da planilha.
              </p>
            ) : !hasCoverage ? (
              <p className="dash-predict__neutral">
                Nenhum mês futuro lançado além de hoje — a projeção só enxerga o
                presente. Lance os próximos meses para prever o ano.
              </p>
            ) : !firstIncomplete ? (
              <p className="dash-predict__ok">
                Seus meses futuros estão completos — a projeção é confiável até o fim
                dos dados lançados.
              </p>
            ) : (
              <>
                <p className="dash-predict__warn">
                  De{" "}
                  <b>
                    {monthNamePtBR(
                      `${firstIncomplete.year}-${String(firstIncomplete.month).padStart(2, "0")}-01`,
                    )}
                  </b>{" "}
                  em diante a projeção está otimista demais. Somando os{" "}
                  {incompleteMonths.length} meses incompletos, faltam{" "}
                  <b className="dash-hero__money">
                    {fmtBRL(forecast.total_missing_cents)}
                  </b>{" "}
                  de gastos não lançados (fatura do cartão e gastos variáveis). Sem
                  isso, o saldo e a poupança projetados mentem.
                </p>
                <div className="dash-predict__rows">
                  {incompleteMonths.map((c) => {
                    const iso = `${c.year}-${String(c.month).padStart(2, "0")}-01`;
                    const pct = Math.max(
                      0,
                      Math.min(100, Math.floor(c.coverage_bps / 100)),
                    );
                    const label = monthNamePtBR(iso);
                    return (
                      <div
                        key={iso}
                        className="dash-predict__row"
                        aria-label={`${label}: ${pct}% do gasto típico lançado, falta ${fmtBRL(c.estimated_missing_cents)}`}
                      >
                        <span className="dash-predict__month">{label}</span>
                        <span className="dash-predict__bar" aria-hidden="true">
                          <span
                            className="dash-predict__fill"
                            style={{ width: `${pct}%` }}
                          />
                        </span>
                        <span className="dash-predict__pct">
                          {pct}% lançado · falta {fmtBRL(c.estimated_missing_cents)}
                        </span>
                      </div>
                    );
                  })}
                </div>
                <p className="dash-predict__hint">
                  Para prever o ano, lance em cada mês à frente: o <b>saldo de hoje</b>{" "}
                  (só conta-corrente), o <b>salário</b> (valor conservador, não o
                  esperado), as <b>contas fixas</b>, a <b>fatura do cartão</b> no
                  vencimento com os parcelados e o <b>diário estimado</b> em todos os
                  dias. O método é claro — futuro vazio engana.
                </p>
              </>
            )}
            {annual && (
              <p className="dash-predict__savings">
                Poupança do ano (estimada): <b>{realizedRatePct}%</b> realizado ·
                referência 20–30%
                {incompleteMonths.length > 0 && (
                  <span className="dash-predict__muted">
                    {" "}
                    (projetado {projectedRatePct}%, mas otimista — o futuro está
                    incompleto)
                  </span>
                )}
              </p>
            )}
          </div>
        </div>
      )}

      {forecast && annual && (summary?.transaction_count ?? 0) > 0 && (
        <div
          role="region"
          aria-labelledby="dash-colchao-title"
          className="dash-card dash-colchao"
        >
          <div className="dash-card__head">
            <span className="dash-card__title" id="dash-colchao-title">
              <Sparkles
                size={16}
                strokeWidth={1.75}
                className="dash-card__ic"
                aria-hidden="true"
              />
              Adaptação ao método — seu colchão
            </span>
            <span className="dash-colchao__phase">fase: calibrar</span>
          </div>
          <div className="dash-card__body">
            <div className="dash-colchao__nums">
              <div className="dash-colchao__num">
                <span className="dash-colchao__label">Economia registrada</span>
                <span className="dash-colchao__val dash-colchao__val--muted">R$ 0</span>
              </div>
              <div className="dash-colchao__num">
                <span className="dash-colchao__label">
                  Colchão este ano (realizado)
                </span>
                <span
                  className={`dash-colchao__val ${colchaoCents < 0 ? "negative" : "positive"}`}
                >
                  {fmtBRL(colchaoCents)} · {realizedRatePct}%
                </span>
              </div>
            </div>
            <p className="dash-colchao__text">
              {colchaoCents >= 0
                ? "Você não registra Economia formal — guarda o que sobra como colchão para cobrir os meses negativos sem sacar investimento. É uma adaptação válida do método."
                : "Este ano você usou parte do colchão para cobrir meses negativos — exatamente o que o buffer existe para fazer. Sem Economia formal ainda, mas o saldo não furou."}
            </p>
            <p className="dash-colchao__next">
              <b>Próximo nível, quando quiser:</b> registrar a Economia (meta 20–30% da
              renda) como uma saída mensal e separar a reserva. Isso transforma o
              colchão em hábito e protege de sacar investimento na hora errada.
            </p>
          </div>
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
          sublabel="Meta: 12 meses de gastos"
        />
      </div>

      {forecast && monthsAhead.length > 0 && (
        <div className="dash-card dash-perf">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <TrendingUp
                size={16}
                strokeWidth={1.75}
                className="dash-card__ic"
                aria-hidden="true"
              />
              Performance por mês — caixa não é poupança
            </span>
            <span className="dash-perf__hint">referência anual 20–30%</span>
          </div>
          <div className="dash-perf__row">
            {monthsAhead.map((m) => {
              const ym = `${m.year}-${String(m.month).padStart(2, "0")}`;
              const iso = `${ym}-01`;
              const ratePct = Math.floor(m.savings_rate_bps / 100);
              // Mês incompleto (faltam fatura/variáveis) → a performance/taxa é OTIMISTA e
              // não pode ser exibida como real (auditoria vs planilha oficial, P1). Meses isolados
              // variam por design (meta 20–30% é média ANUAL); não rotulamos como "abaixo".
              const incompleto = incompleteKeys.has(ym);
              const monthLabel = monthNamePtBR(iso);
              return (
                <div
                  key={iso}
                  className={`dash-perf__cell ${incompleto ? "is-incomplete" : ""}`}
                  aria-label={
                    incompleto
                      ? `${monthLabel}: incompleto — projeção otimista, falta lançar gastos`
                      : `${monthLabel}: performance ${fmtBRL(m.performance_cents)}, ${ratePct}% da renda`
                  }
                >
                  <span className="dash-perf__month">{monthLabel}</span>
                  {incompleto ? (
                    <>
                      <span className="dash-perf__val dash-perf__val--muted">
                        {fmtBRL(m.performance_cents)}
                      </span>
                      <span className="dash-perf__rate">incompleto ⚠</span>
                    </>
                  ) : (
                    <>
                      <span
                        className={`dash-perf__val ${m.performance_cents < 0 ? "negative" : "positive"}`}
                      >
                        {fmtBRL(m.performance_cents)}
                      </span>
                      <span className="dash-perf__rate">{ratePct}% da renda</span>
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

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
