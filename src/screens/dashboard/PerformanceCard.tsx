import { TrendingUp } from "lucide-react";
import type { Forecast } from "../../lib/api";
import { fmtBRL, monthNamePtBR } from "../../lib/format";

/**
 * Performance por mês (Caixa ≠ Performance): expõe os meses magros — onde o "cartão sequestra o
 * salário futuro". Meses incompletos (faltam fatura/variáveis) são marcados como otimistas, sem
 * taxa enganosa. A meta 20–30% é MÉDIA ANUAL, então não rotulamos um mês isolado como "abaixo".
 */
export function PerformanceCard({ forecast }: { forecast: Forecast }) {
  const ym = forecast.today.slice(0, 7);
  const monthsAhead = forecast.months
    .filter((m) => `${m.year}-${String(m.month).padStart(2, "0")}` >= ym)
    .slice(0, 4);
  if (monthsAhead.length === 0) return null;

  const incompleteKeys = new Set(
    forecast.coverage
      .filter((c) => !c.is_complete)
      .map((c) => `${c.year}-${String(c.month).padStart(2, "0")}`),
  );

  return (
    <section aria-labelledby="dash-perf-title" className="dash-card dash-perf">
      <div className="dash-card__head">
        <span className="dash-card__title" id="dash-perf-title">
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
          const key = `${m.year}-${String(m.month).padStart(2, "0")}`;
          const iso = `${key}-01`;
          const ratePct = Math.floor(m.savings_rate_bps / 100);
          const incompleto = incompleteKeys.has(key);
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
    </section>
  );
}
