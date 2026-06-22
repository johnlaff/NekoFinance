import React from "react";

// PhaseBadge — método-adaptation phase (Mapear · Calibrar · Operar) with a
// 3-segment progress indicator. Self-contained; inline-style convention (like Badge/MovBadge).

const PHASES = [
  { key: "map", label: "Mapear" },
  { key: "calibrate", label: "Calibrar" },
  { key: "operate", label: "Operar" },
];

const SR = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0 0 0 0)",
  whiteSpace: "nowrap",
  border: 0,
};

const WRAP = {
  display: "inline-flex",
  alignItems: "center",
  gap: "7px",
  height: 22,
  padding: "0 10px",
  borderRadius: "var(--radius-pill)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border)",
  fontSize: "var(--fs-micro)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  whiteSpace: "nowrap",
};

export function PhaseBadge({ phase = "calibrate" }) {
  const idx = Math.max(
    0,
    PHASES.findIndex((p) => p.key === phase),
  );
  const current = PHASES[idx] || PHASES[0];
  return (
    <span style={WRAP}>
      <span style={SR}>{`Fase de adaptação: ${current.label} (${idx + 1} de 3)`}</span>
      <span aria-hidden="true" style={{ display: "inline-flex", gap: 2 }}>
        {PHASES.map((p, i) => (
          <span
            key={p.key}
            style={{
              width: 9,
              height: 4,
              borderRadius: 1,
              background: i <= idx ? "var(--primary)" : "var(--surface-2)",
            }}
          />
        ))}
      </span>
      <span aria-hidden="true">{current.label}</span>
    </span>
  );
}
