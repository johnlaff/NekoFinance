import { Sparkles } from "lucide-react";
import type { Forecast } from "../../lib/api";
import { Money } from "../../design-system/components/Money";
import { Disclosure } from "../../design-system/components/Disclosure";
import { PhaseBadge, type Phase } from "../../design-system/components/PhaseBadge";

/**
 * Coaching de adaptação — o "colchão". Muitos guardam o excedente como buffer em caixa (net
 * superávit) em vez de registrar Economia formal (transfer→reserva). É adaptação VÁLIDA: o app
 * reconhece ANTES de ensinar (padrão SOTA de coaching), tom calmo, sem punir. Mostra os DOIS
 * números lado a lado: Economia registrada (método) e colchão/net (adaptação), sem confundi-los.
 */
export function ColchaoCard({ forecast, phase }: { forecast: Forecast; phase: Phase }) {
  const annual = forecast.annual_savings;
  const colchaoCents = annual.realized_savings_cents;
  const registeredEconomia = annual.registered_economia_cents;
  const realizedRatePct = (annual.realized_rate_bps / 100).toFixed(1);

  return (
    <section aria-labelledby="dash-colchao-title" className="dash-card dash-colchao">
      <div className="dash-card__head">
        <span className="dash-card__title" id="dash-colchao-title">
          <Sparkles
            size={16}
            strokeWidth={1.75}
            className="dash-card__ic"
            aria-hidden="true"
          />
          Seu colchão
        </span>
        <span title="Fases do método — Mapear: menos de 30 lançamentos. Calibrar: ajustando o diário. Operar: ≥ 20% economizado no ano e ≥ 3 meses de reserva.">
          <PhaseBadge phase={phase} />
        </span>
      </div>
      <div className="dash-card__body">
        <div className="dash-colchao__nums">
          <div className="dash-colchao__num">
            <span className="dash-colchao__label">Economia registrada</span>
            <span
              className={`dash-colchao__val${registeredEconomia > 0 ? "" : " dash-colchao__val--muted"}`}
            >
              <Money cents={registeredEconomia} size="md" sign="auto" />
            </span>
          </div>
          <div className="dash-colchao__num">
            <span className="dash-colchao__label">
              Colchão este ano (sobra até hoje)
            </span>
            <span className="dash-colchao__val">
              <Money cents={colchaoCents} size="md" sign="auto" /> · {realizedRatePct}%
            </span>
          </div>
        </div>
        <p className="dash-colchao__text">
          {colchaoCents >= 0
            ? "Você guarda o que sobra como colchão para cobrir os meses negativos sem sacar investimento. Adaptação válida do método."
            : "Este ano você usou parte do colchão para cobrir meses negativos, exatamente para o que o colchão existe. O saldo não furou."}
        </p>
        <Disclosure title="Próximo nível, quando quiser">
          <p>
            Registrar a Economia (meta 20 a 30% da renda) como uma saída mensal e
            separar a reserva. Isso vira hábito e protege de sacar investimento na hora
            errada.
          </p>
        </Disclosure>
      </div>
    </section>
  );
}
