import type { CSSProperties } from "react";
import { CalendarClock } from "lucide-react";
import { todayISO } from "../../lib/format";

// Static style objects (React Compiler requirement — never inline in JSX).
const BANNER: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  background: "var(--bg-subtle)",
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  lineHeight: 1.4,
};

const ICON: CSSProperties = {
  flexShrink: 0,
  color: "var(--primary)",
};

/**
 * Aviso discreto: mostra há quantos dias o usuário não registra um lançamento real.
 * Some quando o último lançamento é de hoje (já está em dia). Linguagem neutra e
 * factual — nunca gamificada (sem pontos, streaks ou recompensas).
 *
 * O cálculo usa datas de PAREDE locais (não UTC): compara a data ISO do último
 * lançamento com `todayISO()`, ambas em `YYYY-MM-DD`.
 */
export function LastLoggedBanner({
  lastRealTxDate,
}: {
  lastRealTxDate: string | null;
}) {
  const today = todayISO();

  if (!lastRealTxDate) {
    // `<output>` tem role implícito "status" (live region polite) — preferido a
    // `<div role="status">` (HTML nativo > role explícito).
    return (
      <output style={BANNER}>
        <CalendarClock size={15} strokeWidth={1.75} style={ICON} aria-hidden />
        <span>Nenhum lançamento ainda. Registre sua primeira saída.</span>
      </output>
    );
  }

  // Diferença em dias inteiros de calendário (data de parede, fuso local).
  const last = new Date(lastRealTxDate + "T00:00:00");
  const now = new Date(today + "T00:00:00");
  const diffDays = Math.round((now.getTime() - last.getTime()) / 86_400_000);

  if (diffDays <= 0) {
    // Já lançou hoje — nenhum aviso necessário.
    return null;
  }

  const label =
    diffDays === 1
      ? "Você lançou pela última vez ontem."
      : `Você lançou pela última vez há ${diffDays} dias.`;

  return (
    <output style={BANNER}>
      <CalendarClock size={15} strokeWidth={1.75} style={ICON} aria-hidden />
      <span>{label}</span>
    </output>
  );
}
