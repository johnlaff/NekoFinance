import { useState } from "react";
import { CalendarCheck } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import { createTransaction, type DashboardSummary } from "../../lib/api";
import { fmtBRL, parseBRLToCents } from "../../lib/format";
import { invalidateCommands } from "../../lib/useCommand";

function todayISO(): string {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Check-in diário — o ritual do método: a cada dia o dono registra o gasto variável (Diário) e vê
 * o quanto já gastou contra o teto do dia. Registro rápido (um campo + botão) que cria um Diário
 * realizado de hoje e atualiza o dashboard. O form completo (tipo/tags/Repetir) fica nas Transações.
 */
export function DailyCheckinCard({
  summary,
  onLogged,
}: {
  summary: DashboardSummary;
  onLogged: () => void;
}) {
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const spent = summary.daily_spend_today;
  const ceiling = summary.daily_budget;
  const remaining = ceiling - spent;
  const overspent = ceiling > 0 && remaining < 0;
  const pct = ceiling > 0 ? Math.min(100, Math.round((spent / ceiling) * 100)) : 0;

  const cents = parseBRLToCents(amount);
  const canSubmit = cents != null && cents > 0 && !busy;

  async function logSpend() {
    if (cents == null || cents <= 0) {
      setError("Informe um valor válido.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await createTransaction({
        txnType: "expense",
        amountCents: cents,
        description: null,
        date: todayISO(),
        paymentMethod: "debit",
        isFixed: false, // Diário = variável, débito/dinheiro
        tagIds: [],
        recurrence: null,
      });
      invalidateCommands();
      setAmount("");
      setBusy(false);
      onLogged();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <section aria-labelledby="dash-checkin-title" className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title" id="dash-checkin-title">
          <CalendarCheck
            size={16}
            strokeWidth={1.75}
            className="dash-card__ic"
            aria-hidden="true"
          />
          Check-in de hoje
        </span>
        <span
          style={{
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-semibold)",
            color: overspent ? "var(--danger-400)" : "var(--text-muted)",
          }}
        >
          {ceiling > 0
            ? overspent
              ? `${fmtBRL(-remaining)} acima do teto`
              : `${fmtBRL(remaining)} disponível`
            : "sem teto definido"}
        </span>
      </div>
      <div className="dash-card__body">
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "baseline",
            marginBottom: "var(--space-2)",
          }}
        >
          <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
            Diário de hoje
          </span>
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontWeight: "var(--fw-bold)",
              color: "var(--text)",
            }}
          >
            {fmtBRL(spent)}
            {ceiling > 0 && (
              <span style={{ color: "var(--text-faint)", fontWeight: "var(--fw-regular)" }}>
                {" "}
                / {fmtBRL(ceiling)}
              </span>
            )}
          </span>
        </div>

        {ceiling > 0 && (
          <div
            aria-hidden="true"
            style={{
              height: 6,
              borderRadius: "var(--radius-pill)",
              background: "var(--bg-subtle)",
              overflow: "hidden",
              marginBottom: "var(--space-4)",
            }}
          >
            <div
              style={{
                width: `${pct}%`,
                height: "100%",
                background: overspent ? "var(--danger-400)" : "var(--type-diario)",
                transition: "width var(--t-hover)",
              }}
            />
          </div>
        )}

        <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
          <input
            aria-label="Gasto de hoje"
            inputMode="decimal"
            placeholder="Quanto gastou hoje? (R$)"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && canSubmit) void logSpend();
            }}
            style={{
              flex: 1,
              height: "var(--hit-min)",
              padding: "0 var(--space-3)",
              background: "var(--bg-subtle)",
              border: "var(--bw-hair) solid var(--border)",
              borderRadius: "var(--radius-xs)",
              color: "var(--text)",
              fontFamily: "var(--font-money)",
              fontSize: "var(--fs-body)",
            }}
          />
          <Button variant="primary" disabled={!canSubmit} onClick={() => void logSpend()}>
            {busy ? "…" : "Registrar"}
          </Button>
        </div>
        {error && (
          <p
            style={{
              color: "var(--danger-400)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-2) 0 0",
            }}
          >
            {error}
          </p>
        )}
      </div>
    </section>
  );
}
