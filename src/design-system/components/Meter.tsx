import type { CSSProperties } from "react";

/**
 * Meter — a barra-régua canônica do DS (trilho pill + preenchimento por largura).
 * O NÚMERO/frase ao lado é o dado; a barra é reforço visual — por isso ela é
 * `aria-hidden` por padrão e só carrega nome acessível quando `label` é passado
 * (caso em que vira `role="img"` com o texto equivalente completo).
 * A largura satura em 100%; valor acima disso é responsabilidade do texto vizinho.
 */
export function Meter({
  fraction,
  color = "var(--primary)",
  height = 6,
  trackColor = "var(--surface-2)",
  label,
  className,
}: {
  /** Preenchimento 0–1 (valores fora do intervalo são clampados). */
  fraction: number;
  /** Cor do preenchimento (token; nunca cor de marca para status do método). */
  color?: string;
  height?: number;
  trackColor?: string;
  /** Texto equivalente completo; presente → role="img", ausente → decorativa. */
  label?: string;
  className?: string;
}) {
  const pct = Math.max(0, Math.min(1, fraction)) * 100;
  const trackStyle: CSSProperties = {
    height,
    borderRadius: "var(--radius-pill)",
    background: trackColor,
    overflow: "hidden",
  };
  return (
    <div
      className={className}
      style={trackStyle}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      <span
        style={{
          display: "block",
          height: "100%",
          width: `${pct}%`,
          borderRadius: "var(--radius-pill)",
          background: color,
        }}
      />
    </div>
  );
}
