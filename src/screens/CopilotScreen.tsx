import { Badge } from "../design-system/components/Badge";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import {
  getDashboardSummary,
  getForecast,
  type DashboardSummary,
  type Forecast,
} from "../lib/api";
import { formatBRL, monthNamePtBR } from "../lib/format";
import { useCommand } from "../lib/useCommand";

/**
 * Fatos determinísticos que a Mia já pode afirmar HOJE — derivados do motor (sem IA), como prova de
 * valor enquanto o chat não existe. São frases do método: reserva em meses, performance do mês,
 * pode-gastar e economizado no ano. Tudo vem de comandos já existentes (get_dashboard_summary /
 * get_forecast); nada é gerado por linguagem.
 */
function miaKnownFacts(
  summary: DashboardSummary | undefined,
  forecast: Forecast | undefined,
): string[] {
  const facts: string[] = [];
  if (summary && summary.transaction_count > 0) {
    facts.push(
      `Sua reserva cobre ${summary.reserve_months.toFixed(1)} meses de custo de vida (a meta mínima é 6).`,
    );
  }
  if (forecast) {
    // Economizado% do método = Economia registrada ÷ Entradas (não o net superávit/colchão).
    // Vem antes da performance/pode-gastar para não ser descartado pelo corte de 3 fatos.
    const a = forecast.annual_savings;
    const ytd = Math.round(
      (a.registered_economia_cents / Math.max(1, a.realized_income_cents)) * 100,
    );
    facts.push(`No ano, você economizou ${ytd}% (referência 20–30%).`);
    const ym = forecast.today.slice(0, 7);
    const cur = forecast.months.find(
      (m) => `${m.year}-${String(m.month).padStart(2, "0")}` === ym,
    );
    if (cur) {
      // Mês corrente: a performance inclui a previsão do diário que ainda falta — qualificamos.
      facts.push(
        `A performance projetada de ${monthNamePtBR(forecast.today)} está em ${formatBRL(cur.performance_cents)} (inclui o diário que ainda falta no mês).`,
      );
    }
    facts.push(
      `Você pode gastar até ${formatBRL(forecast.safe_to_spend_today_cents)} hoje sem furar suas metas.`,
    );
  }
  return facts.slice(0, 3);
}

export function CopilotScreen() {
  const summary = useCommand("get_dashboard_summary", getDashboardSummary).data;
  const forecast = useCommand("get_forecast", getForecast).data;
  const facts = miaKnownFacts(summary, forecast);

  return (
    <div className="dash">
      <div className="assistant-panel cop-panel">
        <div className="assistant-header">
          <MiaAvatar width={48} height={48} />
          <div>
            <p className="assistant-label">Copiloto</p>
            <h2 className="assistant-name">Mia</h2>
          </div>
          <span className="cop-panel__badge">
            <Badge tone="warning">Em desenvolvimento</Badge>
          </span>
        </div>
        <p>
          O chat da Mia ainda não está disponível nesta versão. Tudo o que você vê no
          app hoje é calculado pelo motor determinístico — nada é gerado por IA.
        </p>
      </div>

      {facts.length > 0 && (
        <section
          aria-labelledby="mia-knows-title"
          style={{
            background: "var(--surface)",
            border: "var(--bw-hair) solid var(--border)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-1)",
            padding: "var(--space-6)",
            marginBottom: "var(--space-6)",
          }}
        >
          <h2
            id="mia-knows-title"
            style={{
              fontSize: "var(--fs-label)",
              fontWeight: "var(--fw-semibold)",
              letterSpacing: "var(--ls-label)",
              textTransform: "uppercase",
              color: "var(--text-muted)",
              margin: "0 0 var(--space-4)",
            }}
          >
            O que a Mia já sabe · números do método, sem IA
          </h2>
          <ul
            style={{
              listStyle: "none",
              margin: 0,
              padding: 0,
              display: "flex",
              flexDirection: "column",
              gap: "var(--space-3)",
            }}
          >
            {facts.map((f) => (
              <li
                key={f}
                style={{
                  display: "flex",
                  gap: "var(--space-3)",
                  alignItems: "baseline",
                  color: "var(--text)",
                  fontSize: "var(--fs-body)",
                }}
              >
                <span aria-hidden="true" style={{ color: "var(--primary)" }}>
                  ↳
                </span>
                {f}
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="roadmap-panel">
        <div>
          <h2>O que a Mia vai fazer</h2>
        </div>
        <div>
          <ol>
            <li>
              Diagnóstico em linguagem natural: padrões de gasto, evolução da reserva e
              o peso real do crédito — sempre em modo leitura.
            </li>
            <li>
              Respostas a decisões: “posso comprar?”, “à vista ou parcelado?” — usando o
              saldo projetado, nunca cálculo improvisado.
            </li>
            <li>
              Escrita na planilha somente com a sua aprovação explícita, mostrando um
              diff antes → depois de cada alteração.
            </li>
          </ol>
        </div>
      </div>
    </div>
  );
}
