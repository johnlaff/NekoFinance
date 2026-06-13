import { Sparkles } from "lucide-react";
import type { Forecast } from "../../lib/api";
import { fmtBRL } from "../../lib/format";

/**
 * Coaching de adaptação — o "colchão". O dono não registra Economia formal (linha do método);
 * guarda o excedente como buffer em caixa para cobrir os meses negativos. É adaptação VÁLIDA: o
 * app reconhece ANTES de ensinar (padrão SOTA de coaching), tom calmo, sem punir. Quando a aba
 * Economia for importada (slice 7), a poupança formal substitui este proxy de net realizado.
 */
export function ColchaoCard({ forecast }: { forecast: Forecast }) {
  const annual = forecast.annual_savings;
  const colchaoCents = annual.realized_savings_cents;
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
            <span className="dash-colchao__label">Colchão este ano (realizado)</span>
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
          renda) como uma saída mensal e separar a reserva. Isso transforma o colchão em
          hábito e protege de sacar investimento na hora errada.
        </p>
      </div>
    </section>
  );
}
