import { CalendarRange } from "lucide-react";
import type { Forecast } from "../../lib/api";
import { fmtBRL, monthNamePtBR } from "../../lib/format";
import { Disclosure } from "../../design-system/components/Disclosure";
import { Money } from "../../design-system/components/Money";
import { InfoPopover } from "../../design-system/components/InfoPopover";

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
  // Economizado% do método = Economia registrada (transfers→reserva) ÷ Entradas — não o net
  // superávit/colchão (esse vive no ColchaoCard). Espelha a coluna % da aba Economia da planilha.
  const economizadoPct = Math.round(
    (annual.registered_economia_cents / Math.max(1, annual.realized_income_cents)) *
      100,
  );
  const trustedLabel = forecast.trusted_through_month
    ? monthNamePtBR(`${forecast.trusted_through_month}-01`)
    : null;

  // Mesmo guia de pré-lançamento nos dois estados que pedem ação (sem cobertura e incompleto).
  const preLaunchHelp = (
    <Disclosure title="Como pré-lançar o ano">
      <p>
        Em cada mês à frente, lance o <b>saldo de hoje</b> (só conta-corrente), o{" "}
        <b>salário</b> conservador, as <b>contas fixas</b>, a <b>fatura do cartão</b> no
        vencimento e o <b>diário estimado</b> em todos os dias. Futuro vazio engana a
        previsão.
      </p>
    </Disclosure>
  );

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
          <InfoPopover term="previsibilidade">Previsibilidade</InfoPopover>
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
          <>
            <p className="dash-predict__neutral">
              Nenhum mês futuro lançado além de hoje. A projeção só enxerga o presente.
              Lance os próximos meses para prever o ano.
            </p>
            {preLaunchHelp}
          </>
        ) : !firstIncomplete ? (
          <p className="dash-predict__ok">
            Seus meses futuros estão completos. A projeção é confiável até o fim dos
            dados lançados.
          </p>
        ) : (
          <>
            <p className="dash-predict__warn">
              A partir de{" "}
              <b>
                {monthNamePtBR(
                  `${firstIncomplete.year}-${String(firstIncomplete.month).padStart(2, "0")}-01`,
                )}
              </b>{" "}
              faltam <Money cents={forecast.total_missing_cents} size="sm" /> de gastos
              não lançados. A projeção está otimista até você pré-lançar.
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
                      {pct}% · falta {fmtBRL(c.estimated_missing_cents)}
                    </span>
                  </div>
                );
              })}
            </div>
            {preLaunchHelp}
          </>
        )}
        <p className="dash-predict__savings">
          Economizado no ano: <b>{economizadoPct}%</b> realizado, referência 20 a 30%
        </p>
      </div>
    </section>
  );
}
