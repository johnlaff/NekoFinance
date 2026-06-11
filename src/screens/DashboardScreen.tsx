import { useEffect, useState } from "react";
import { Minus, Receipt, Sparkles, TrendingDown, TrendingUp } from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { MetricTile } from "../design-system/components/MetricTile";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import {
  getDashboardSummary,
  getRecentTransactions,
  isTauri,
  type DashboardSummary,
  type TransactionRow,
} from "../lib/api";
import { fmtBRL, fmtDate } from "../lib/format";

export function DashboardScreen({ onAskMia }: { onAskMia: () => void }) {
  const [scope, setScope] = useState("overview");
  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const [transactions, setTransactions] = useState<TransactionRow[]>([]);
  const [loading, setLoading] = useState(isTauri);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    async function load() {
      try {
        const [s, t] = await Promise.all([
          getDashboardSummary(),
          getRecentTransactions(20),
        ]);
        setSummary(s);
        setTransactions(t);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    }
    void load();
  }, []);

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

  const dailyPercent = summary
    ? summary.daily_budget > 0
      ? Math.round((summary.daily_spend_today / summary.daily_budget) * 100)
      : 0
    : 0;

  const filteredTransactions = transactions.filter((t) => {
    if (scope === "credito") return t.payment_method === "credit";
    if (scope === "projecoes") return t.is_projection;
    return true;
  });

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
        </div>
        <Button
          variant="secondary"
          iconLeft={<Sparkles size={16} strokeWidth={1.75} />}
          onClick={onAskMia}
        >
          Perguntar à Mia
        </Button>
      </div>

      <div className="dash-grid4">
        <MetricTile
          label="Saldo projetado"
          value={summary ? fmtBRL(summary.balance) : "—"}
          icon={<TrendingUp size={15} strokeWidth={1.75} />}
          sublabel="Fim do mês"
        />
        <MetricTile
          label="Diário hoje"
          value={summary ? fmtBRL(summary.daily_spend_today) : "—"}
          delta={`${dailyPercent}%`}
          deltaDir={dailyPercent > 100 ? "down" : "up"}
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
              <Receipt size={16} strokeWidth={1.75} className="dash-card__ic" />
              {scope === "overview"
                ? "Transações recentes"
                : scope === "credito"
                  ? "Apenas crédito"
                  : "Projeções futuras"}
            </span>
            <SegmentedControl
              size="sm"
              value={scope}
              onChange={setScope}
              options={[
                { value: "overview", label: "Todas" },
                { value: "credito", label: "Crédito" },
                { value: "projecoes", label: "Futuro" },
              ]}
            />
          </div>
          <div className="dash-card__body" style={{ padding: 0 }}>
            {filteredTransactions.length === 0 ? (
              <EmptyState
                variant="empty"
                title="Nenhuma transação"
                description="Conecte o Google Sheets e importe sua planilha."
              />
            ) : (
              <table className="txn-table">
                <thead>
                  <tr>
                    <th scope="col">Data</th>
                    <th scope="col">Descrição</th>
                    <th scope="col">Valor</th>
                    <th scope="col">Método</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredTransactions.map((t) => (
                    <tr className={t.is_projection ? "projection" : ""} key={t.id}>
                      <td>{fmtDate(t.date)}</td>
                      <td>{t.description || "—"}</td>
                      <td
                        className={`money ${t.type === "income" ? "positive" : "negative"}`}
                      >
                        {fmtBRL(Math.abs(t.amount))}
                      </td>
                      <td>{t.payment_method || t.type}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
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
    </div>
  );
}
