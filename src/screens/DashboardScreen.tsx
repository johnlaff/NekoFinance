import { useState, type CSSProperties } from "react";
import { AlertTriangle, Sparkles, UploadCloud } from "lucide-react";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { ConflictGate } from "../features/reconcile/ConflictGate";
import { WriteBackPreview } from "../features/sheets/WriteBackPreview";
import { useWriteBackPending } from "../hooks/useWriteBackPending";
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
import { LastLoggedBanner } from "./dashboard/LastLoggedBanner";
import { MonthLedgerCard } from "./dashboard/MonthLedgerCard";

// Estilos estáticos hoisted (regra do React Compiler — nunca inline no JSX).
const WB_PENDING_BTN: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  width: "100%",
  textAlign: "left",
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--bg-subtle)",
  color: "var(--brass-400)",
  fontSize: "var(--fs-sm)",
  cursor: "pointer",
};

const WB_PENDING_BTN_DISABLED: CSSProperties = {
  ...WB_PENDING_BTN,
  cursor: "default",
  color: "var(--text-muted)",
};

const WB_ICON: CSSProperties = { flexShrink: 0 };

const WB_HINT: CSSProperties = { color: "var(--text-muted)", marginLeft: "auto" };

/**
 * Indicador de write-back pendente (plano 031): mostra quantas células locais ainda divergem da
 * planilha e abre o MESMO fluxo de aprovação humana do painel de Configurações (plano 028) — sem
 * reimplementar o diff/apply. Os conflitos de importação aparecem à parte (via `ConflictGate`),
 * pois bloqueiam o envio; aqui só tratamos das células prontas para enviar.
 *
 * Quando a flag-mestre está desligada, o banner ainda aparece (o usuário sabe que há algo a enviar)
 * mas vira um aviso não-clicável — o envio mora em Configurações, onde a flag e o OAuth vivem.
 */
function WriteBackStatusBanner({
  pendingCount,
  enabled,
  expanded,
  onToggle,
}: {
  pendingCount: number;
  enabled: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  const label = `${pendingCount} célula(s) local → planilha pendente(s)`;
  if (!enabled) {
    // `<output>` tem role implícito "status" (live region polite) — preferido a role explícito.
    return (
      <output style={WB_PENDING_BTN_DISABLED}>
        <UploadCloud size={15} strokeWidth={1.75} style={WB_ICON} aria-hidden />
        <span>{label}</span>
        <span style={WB_HINT}>Envio desativado nas Configurações.</span>
      </output>
    );
  }
  return (
    <button
      type="button"
      style={WB_PENDING_BTN}
      onClick={onToggle}
      aria-expanded={expanded}
    >
      <UploadCloud size={15} strokeWidth={1.75} style={WB_ICON} aria-hidden />
      <span aria-live="polite">{label}</span>
      <span style={WB_HINT}>{expanded ? "Fechar" : "Revisar e enviar"}</span>
    </button>
  );
}

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
  // Indicador de write-back pendente (plano 031): conta as células local → planilha por enviar e
  // os conflitos de importação que bloqueiam o envio. `showWriteBack` é a divulgação local do
  // painel de aprovação (reaproveitado de Configurações) abaixo do banner.
  const [showWriteBack, setShowWriteBack] = useState(false);
  const writeBack = useWriteBackPending();
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
    // Um lançamento local pode ter criado uma nova divergência → re-mede o write-back pendente.
    writeBack.refresh();
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

      {/* Conflitos de importação (plano 013): bloqueiam o write-back. Aparecem aqui também (não só
          em Lançamentos) para serem resolvidos sem sair do dashboard. O componente se auto-busca e
          some quando não há conflitos. Reaproveitado como está — sem reimplementar o gate humano. */}
      <ConflictGate onResolved={writeBack.refresh} />

      {/* Indicador de write-back pendente + divulgação do painel de aprovação (plano 028) abaixo.
          Some quando não há nada a enviar / fora de um sheet mapeado / durante o carregamento. */}
      {!writeBack.loading && writeBack.pendingCount > 0 && (
        <WriteBackStatusBanner
          pendingCount={writeBack.pendingCount}
          enabled={writeBack.enabled}
          expanded={showWriteBack}
          onToggle={() => setShowWriteBack((v) => !v)}
        />
      )}

      {showWriteBack && writeBack.spreadsheetId && writeBack.sheetName && (
        <WriteBackPreview
          spreadsheetId={writeBack.spreadsheetId}
          sheetName={writeBack.sheetName}
          clientId={writeBack.clientId}
        />
      )}

      {summary && hasData && (
        <LastLoggedBanner lastRealTxDate={summary.last_real_tx_date} />
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
