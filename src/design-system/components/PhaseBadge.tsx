/**
 * PhaseBadge — a fase de adaptação ao método (Mapear · Calibrar · Operar) com indicador de progresso
 * em 3 segmentos (padrão do design system). O método é uma jornada; a fase mostra onde o usuário
 * está sem cobrar maturidade que ele ainda não tem.
 */
import type { CSSProperties } from "react";
import { SR_ONLY } from "../srOnly";

export type Phase = "map" | "calibrate" | "operate";

const PHASES: { key: Phase; label: string }[] = [
  { key: "map", label: "Mapear" },
  { key: "calibrate", label: "Calibrar" },
  { key: "operate", label: "Operar" },
];

const PHASE_BADGE_STYLE: CSSProperties = {
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

export function PhaseBadge({ phase }: { phase: Phase }) {
  const idx = PHASES.findIndex((p) => p.key === phase);
  const current = PHASES[idx] ?? PHASES[0]!;
  return (
    // Rótulo textual sr-only (em vez de role="img"); os segmentos e o nome visível ficam aria-hidden.
    <span style={PHASE_BADGE_STYLE}>
      <span
        style={SR_ONLY}
      >{`Fase de adaptação: ${current.label} (${idx + 1} de 3)`}</span>
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
