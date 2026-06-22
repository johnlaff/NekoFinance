import "./horizonte.css";
import {
  AlertTriangle,
  CalendarRange,
  CheckCircle2,
  Calculator,
  CalendarClock,
} from "lucide-react";
import { getForecast, getUpcomingBills, isTauri } from "../lib/api";
import { fmtDate } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { fmtBRL, MES, saldoBand } from "../lib/nkFormat";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";
import { ProvBadge } from "../design-system/components/ProvBadge";

/** Janela do calendário de contas a vencer (dias a partir de hoje). */
const BILLS_WINDOW_DAYS = 60;
const fetchUpcomingBills = () => getUpcomingBills(BILLS_WINDOW_DAYS);

/** Formata ISO YYYY-MM-DD como "DD/mês" abreviado para o banner de alerta. */
function fmtDM(iso: string): string {
  const parts = iso.split("-");
  if (parts.length < 3) return iso;
  const d = parseInt(parts[2] ?? "0", 10);
  const m = parseInt(parts[1] ?? "0", 10) - 1;
  return `${d} de ${MES[m]?.toLowerCase() ?? ""}`;
}

export function HorizonteScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const billsQ = useCommand("get_upcoming_bills:60", fetchUpcomingBills);

  const forecast = forecastQ.data ?? null;

  // All daily points from today onward (the full-year trajectory).
  const daily = forecast?.daily ?? [];

  // Month-end balance cards (current month onwards from forecast.month_end).
  const monthEnds = forecast?.month_end ?? [];

  // Compute status banner values.
  const vals = daily.map((d) => d.balance_cents);
  const minVal = vals.length > 0 ? Math.min(...vals) : 0;
  const minIdx = vals.length > 0 ? vals.indexOf(minVal) : -1;
  const minDay = minIdx >= 0 ? daily[minIdx] : null;
  const endVal = daily.length > 0 ? (daily[daily.length - 1]?.balance_cents ?? 0) : 0;
  const hasDeficit = minVal < 0;

  const today = forecast?.today ?? "";
  const endBand = saldoBand(endVal);

  const bills = billsQ.data ?? [];

  return (
    <div className="hz">
      <div className="hz-title">Horizonte de saldos</div>

      {/* Status banner */}
      {daily.length > 0 && minDay != null ? (
        <div className={"hz-alert" + (hasDeficit ? "" : " hz-alert--ok")}>
          <span
            className="hz-alert__ic"
            style={{ color: hasDeficit ? "var(--warning-400)" : "var(--success-400)" }}
          >
            {hasDeficit ? (
              <AlertTriangle size={22} strokeWidth={1.75} />
            ) : (
              <CheckCircle2 size={22} strokeWidth={1.75} />
            )}
          </span>
          <div>
            <div className="hz-alert__t">
              {hasDeficit
                ? `Seu saldo chega a ${fmtBRL(minVal)} em ${fmtDM(minDay.date)}.`
                : `Saldo positivo o ano inteiro. O menor ponto foi ${fmtBRL(minVal)} em ${fmtDM(minDay.date)}.`}
            </div>
            <div className="hz-alert__s">
              {hasDeficit
                ? "Antecipe uma entrada ou adie uma saída antes dessa data."
                : "Com os lançamentos atuais, você não fica no vermelho."}
            </div>
          </div>
        </div>
      ) : null}

      {/* Area chart — Trajetória até dezembro */}
      {daily.length > 0 ? (
        <section className="card">
          <div className="card__head">
            <span className="card__title">
              <CalendarRange size={16} strokeWidth={1.75} className="ic" />
              Trajetória até dezembro
            </span>
            <span
              style={{
                fontFamily: "var(--font-money)",
                fontSize: 12.5,
                color: endBand.text,
              }}
            >
              fim do ano {fmtBRL(endVal)}
            </span>
          </div>
          <div className="card__body">
            <BalanceTrajectory daily={daily} today={today} variant="full" />
          </div>
        </section>
      ) : null}

      {/* Month-end balance cards */}
      {monthEnds.length > 0 ? (
        <section className="card">
          <div className="card__head">
            <span className="card__title">
              <Calculator size={16} strokeWidth={1.75} className="ic" />
              Saldo no fim de cada mês
            </span>
          </div>
          <div className="card__body">
            <div className="hz-months">
              {monthEnds.map((me) => {
                const band = saldoBand(me.balance_cents);
                const isNeg = band.key === "negative" || band.key === "critical";
                return (
                  <div
                    className="hz-mcard"
                    key={`${me.year}-${me.month}`}
                    style={isNeg ? { background: "var(--danger-tint)" } : undefined}
                  >
                    <div className="hz-mcard__m">{MES[me.month - 1]}</div>
                    <div className="hz-mcard__v" style={{ color: band.text }}>
                      {fmtBRL(me.balance_cents)}
                    </div>
                    <div className="hz-mcard__b" style={{ color: band.text }}>
                      <span
                        className="hz-mcard__chip"
                        style={{ background: band.text }}
                      />
                      {band.label}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </section>
      ) : null}

      {/* Vencimentos próximos (upcoming bills) */}
      <section aria-labelledby="hz-bills-title">
        <h2 id="hz-bills-title" className="hz-bills-title">
          Vencimentos próximos
        </h2>
        {bills.length === 0 ? (
          <div className="hz-empty">Nenhum vencimento nos próximos 60 dias</div>
        ) : (
          <ul className="hz-bills-list">
            {bills.map((b) => (
              <li key={b.id} className="hz-bill-row">
                <span className="hz-bill-date">
                  <CalendarClock size={12} strokeWidth={1.75} aria-hidden="true" />
                  {fmtDate(b.due_date)}
                </span>
                <span className="hz-bill-desc">{b.description || "—"}</span>
                {b.is_projection && <ProvBadge provenance="projetado" />}
                <span className="hz-bill-amt">−{fmtBRL(Math.abs(b.amount))}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
