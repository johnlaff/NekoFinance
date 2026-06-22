import React from "react";

// MonthNav — temporal navigation control "< Mês/Ano >" + "Hoje" shortcut.
// Self-contained; inline-style pattern (no CSS injection needed).

// Inline SVG chevrons (Lucide-style, 18×18, strokeWidth 2, round caps/joins).
function ChevronLeft() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="m15 18-6-6 6-6" />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="m9 18 6-6-6-6" />
    </svg>
  );
}

function arrowBtnStyle(enabled) {
  return {
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
    flexShrink: 0,
  };
}

const TODAY_BTN_STYLE = {
  marginLeft: "var(--space-2)",
  padding: "var(--space-2) var(--space-4)",
  borderRadius: "var(--radius-pill)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--primary-quiet)",
  color: "var(--primary-quiet-text)",
  fontSize: "var(--fs-sm)",
  fontWeight: "var(--fw-semibold)",
  cursor: "pointer",
  lineHeight: 1.4,
};

const LABEL_STYLE = {
  minWidth: 150,
  textAlign: "center",
  fontSize: "var(--fs-title)",
  fontWeight: "var(--fw-bold)",
  letterSpacing: "var(--ls-tight)",
  color: "var(--text-strong)",
  fontFamily: "var(--font-sans)",
};

export function MonthNav({
  label = "Junho de 2026",
  onPrev = () => {},
  onNext = () => {},
  onToday = () => {},
  canPrev = true,
  canNext = true,
  atToday = false,
  prevLabel = "Mês anterior",
  nextLabel = "Próximo mês",
  className = "",
}) {
  return (
    <div
      className={className || undefined}
      style={{ display: "inline-flex", alignItems: "center", gap: "var(--space-3)" }}
    >
      <button
        type="button"
        aria-label={prevLabel}
        disabled={!canPrev}
        onClick={onPrev}
        style={arrowBtnStyle(canPrev)}
      >
        <ChevronLeft />
      </button>

      <span aria-live="polite" style={LABEL_STYLE}>
        {label}
      </span>

      <button
        type="button"
        aria-label={nextLabel}
        disabled={!canNext}
        onClick={onNext}
        style={arrowBtnStyle(canNext)}
      >
        <ChevronRight />
      </button>

      {!atToday && (
        <button type="button" onClick={onToday} style={TODAY_BTN_STYLE}>
          Hoje
        </button>
      )}
    </div>
  );
}
