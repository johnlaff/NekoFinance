import type { CSSProperties } from "react";

/**
 * SegBar — a barra de composição do DS (fatias lado a lado, cada uma com sua
 * cor). Mostra COMO um total se divide, não progresso — para progresso/régua,
 * use `Meter`/`RangeRuler`. A LISTA ao lado é o dado; a barra é reforço visual —
 * decorativa por padrão, `role="img"` com texto equivalente completo quando
 * `label` é passado. Fatias zeradas não renderizam (composição sem fatia é
 * ausência legítima, e a lista vizinha segue dizendo o R$ 0,00).
 */

export interface Segment {
  /** Identidade estável da fatia (ex.: o nome do componente que ela representa). */
  name: string;
  /** Fração 0–1 do total (frações são normalizadas pela soma quando ≠ 1). */
  fraction: number;
  color: string;
}

const ROOT: CSSProperties = { display: "flex", gap: 4 };
const SLICE: CSSProperties = { borderRadius: "var(--radius-pill)" };

export function SegBar({
  segments,
  height = 9,
  label,
  className,
}: {
  segments: Segment[];
  height?: number;
  /** Texto equivalente completo; presente → role="img", ausente → decorativa. */
  label?: string;
  className?: string;
}) {
  const visible = segments.filter((s) => s.fraction > 0);
  const total = visible.reduce((sum, s) => sum + s.fraction, 0);
  return (
    <div
      className={className}
      style={{ ...ROOT, height }}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      {visible.map((s) => (
        <span
          key={s.name}
          style={{
            ...SLICE,
            flexGrow: Number(
              ((s.fraction / Math.max(total, Number.MIN_VALUE)) * 100).toFixed(3),
            ),
            flexShrink: 1,
            flexBasis: 0,
            background: s.color,
          }}
        />
      ))}
    </div>
  );
}
