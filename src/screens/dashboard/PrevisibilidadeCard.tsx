import { CalendarRange } from "lucide-react";
import type { Forecast } from "../../lib/api";
import { fmtBRL, monthNamePtBR } from "../../lib/format";

/**
 * Previsibilidade: detecta meses futuros incompletos (futuro vazio = projeção otimista demais,
 * o "chá revelação" do método) e guia o pré-lançamento. Coverage ratio por mês — diferencial.
 */
export function PrevisibilidadeCard({ forecast }: { forecast: Forecast }) {
  const hasBaseline = forecast.baseline_outflow_cents > 0;
  const incompleteMonths = forecast.coverage.filter((c) => !c.is_complete);
  const firstIncomplete = incompleteMonths[0];
  const hasCoverage = forecast.coverage.length > 0;
  const annual = forecast.annual_savings;
  const realizedRatePct = (annual.realized_rate_bps / 100).toFixed(1);
  const projectedRatePct = (annual.projected_rate_bps / 100).toFixed(1);
  const trustedLabel = forecast.trusted_through_month
    ? monthNamePtBR(`${forecast.trusted_through_month}-01`)
    : null;

  return (
    <section
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
            Ainda não há meses realizados suficientes para avaliar a confiabilidade da
            projeção. Importe mais histórico da planilha.
          </p>
        ) : !hasCoverage ? (
          <p className="dash-predict__neutral">
            Nenhum mês futuro lançado além de hoje — a projeção só enxerga o presente.
            Lance os próximos meses para prever o ano.
          </p>
        ) : !firstIncomplete ? (
          <p className="dash-predict__ok">
            Seus meses futuros estão completos — a projeção é confiável até o fim dos
            dados lançados.
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
              <b className="dash-hero__money">{fmtBRL(forecast.total_missing_cents)}</b>{" "}
              de gastos não lançados (fatura do cartão e gastos variáveis). Sem isso, o
              saldo e a poupança projetados mentem.
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
              Para prever o ano, lance em cada mês à frente: o <b>saldo de hoje</b> (só
              conta-corrente), o <b>salário</b> (valor conservador, não o esperado), as{" "}
              <b>contas fixas</b>, a <b>fatura do cartão</b> no vencimento com os
              parcelados e o <b>diário estimado</b> em todos os dias. O método é claro —
              futuro vazio engana.
            </p>
          </>
        )}
        <p className="dash-predict__savings">
          Poupança do ano (estimada): <b>{realizedRatePct}%</b> realizado · referência
          20–30%
          {incompleteMonths.length > 0 && (
            <span className="dash-predict__muted">
              {" "}
              (projetado {projectedRatePct}%, mas otimista — o futuro está incompleto)
            </span>
          )}
        </p>
      </div>
    </section>
  );
}
