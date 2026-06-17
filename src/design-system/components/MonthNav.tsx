import { ChevronLeft, ChevronRight } from "lucide-react";

/**
 * MonthNav — navegação temporal "< Mês/Ano >" + botão "Hoje". A navegação pelo horizonte é o
 * coração do método (olhar para frente). Componente puro/presentational (sem estado): o pai
 * controla o mês ativo.
 */
interface MonthNavProps {
  /** Rótulo do mês ativo, ex.: "Fevereiro de 2026". */
  label: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  canPrev?: boolean;
  canNext?: boolean;
  /** Quando já está no mês corrente, esconde o botão "Hoje". */
  atToday?: boolean;
  /** aria-labels das setas — sobrescreva quando a navegação não for por mês (ex.: ano). */
  prevLabel?: string;
  nextLabel?: string;
  className?: string;
}

const arrowBtn = (enabled: boolean): React.CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  width: "var(--hit-min)",
  height: "var(--hit-min)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--surface)",
  color: enabled ? "var(--text)" : "var(--text-faint)",
  cursor: enabled ? "pointer" : "not-allowed",
  opacity: enabled ? 1 : 0.5,
  transition: "background-color var(--dur-fast) var(--ease-standard)",
});

// Botão "Hoje" (estático): hoistado para não recriar por render.
const TODAY_BTN_STYLE: React.CSSProperties = {
  marginLeft: "var(--space-2)",
  padding: "var(--space-2) var(--space-4)",
  borderRadius: "var(--radius-pill)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--primary-quiet)",
  color: "var(--primary-quiet-text)",
  fontSize: "var(--fs-sm)",
  fontWeight: "var(--fw-semibold)",
  cursor: "pointer",
};

export function MonthNav({
  label,
  onPrev,
  onNext,
  onToday,
  canPrev = true,
  canNext = true,
  atToday = true,
  prevLabel = "Mês anterior",
  nextLabel = "Próximo mês",
  className = "",
}: MonthNavProps) {
  return (
    <div
      className={className}
      style={{ display: "inline-flex", alignItems: "center", gap: "var(--space-3)" }}
    >
      <button
        type="button"
        aria-label={prevLabel}
        disabled={!canPrev}
        onClick={onPrev}
        style={arrowBtn(canPrev)}
      >
        <ChevronLeft size={18} strokeWidth={2} />
      </button>
      <span
        aria-live="polite"
        style={{
          minWidth: 150,
          textAlign: "center",
          fontSize: "var(--fs-title)",
          fontWeight: "var(--fw-bold)",
          letterSpacing: "var(--ls-tight)",
        }}
      >
        {label}
      </span>
      <button
        type="button"
        aria-label={nextLabel}
        disabled={!canNext}
        onClick={onNext}
        style={arrowBtn(canNext)}
      >
        <ChevronRight size={18} strokeWidth={2} />
      </button>
      {!atToday && (
        <button type="button" onClick={onToday} style={TODAY_BTN_STYLE}>
          Hoje
        </button>
      )}
    </div>
  );
}
