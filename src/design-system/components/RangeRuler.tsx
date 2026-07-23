import type { CSSProperties } from "react";

/**
 * RangeRuler — a régua de faixa-alvo do método (trilho + zona + marcas + pino).
 * O instrumento que posiciona um percentual contra uma zona ideal (ex.: economia
 * 20–30% numa escala fixa 0→40). O NÚMERO ao lado é o dado; a régua é reforço
 * visual — decorativa por padrão, `role="img"` com texto equivalente completo
 * quando `label` é passado. O pino satura nas bordas da escala; o valor
 * verdadeiro é responsabilidade do texto vizinho (a barra satura, o número não).
 * Em inline-style (convenção do DS); nada anima (o pino monta parado).
 */

export interface RulerMark {
  /** Posição na unidade da escala (ex.: 20 numa escala 0→40). */
  at: number;
  label: string;
}

export interface RulerPin {
  /** Posição do pino na unidade da escala; fora dela, estaciona na borda. */
  value: number;
  /** Rótulo curto sob medida (ex.: "0%"). */
  label: string;
  /** Cor de status do método (nunca cor de marca por acidente). */
  color: string;
}

const ROOT: CSSProperties = { position: "relative", height: 58 };
const BAND: CSSProperties = {
  position: "absolute",
  top: 26,
  left: 0,
  right: 0,
  height: 8,
  borderRadius: "var(--radius-pill)",
  background: "var(--surface-2)",
};
const ZONE: CSSProperties = {
  position: "absolute",
  top: 0,
  bottom: 0,
  borderRadius: "var(--radius-pill)",
  background: "var(--success-400)",
  opacity: 0.3,
};
const MARK: CSSProperties = {
  position: "absolute",
  top: 40,
  transform: "translateX(-50%)",
  fontSize: 10.5,
  color: "var(--text-faint)",
  fontVariantNumeric: "tabular-nums",
};
const PIN_LABEL: CSSProperties = {
  position: "absolute",
  top: 0,
  fontSize: 12,
  fontWeight: 600,
  fontVariantNumeric: "tabular-nums",
  lineHeight: 1,
};
const PIN_TICK: CSSProperties = {
  position: "absolute",
  top: 14,
  width: 2,
  height: 9,
  transform: "translateX(-50%)",
};

/** Alinhamento do rótulo do pino: as bordas empurram o texto para dentro. */
function pinLabelTransform(pct: number): string {
  if (pct <= 6) return "translateX(0)";
  if (pct >= 94) return "translateX(-100%)";
  return "translateX(-50%)";
}

export function RangeRuler({
  max,
  zone,
  marks,
  pin,
  label,
  className,
}: {
  /** Fim da escala, na unidade dos valores (a escala começa em 0). */
  max: number;
  /** Zona-alvo destacada no trilho. */
  zone: { from: number; to: number };
  marks: RulerMark[];
  /** `null` quando a régua não julga (ex.: sem registro). */
  pin: RulerPin | null;
  /** Texto equivalente completo; presente → role="img", ausente → decorativa. */
  label?: string;
  className?: string;
}) {
  const pct = (v: number) => (Math.max(0, Math.min(max, v)) / max) * 100;
  const zoneLeft = pct(zone.from);
  const zoneWidth = pct(zone.to) - zoneLeft;
  const pinPct = pin ? pct(pin.value) : 0;
  return (
    <div
      className={className}
      style={ROOT}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      <div style={BAND}>
        <span style={{ ...ZONE, left: `${zoneLeft}%`, width: `${zoneWidth}%` }} />
      </div>
      {pin ? (
        <>
          <span
            style={{
              ...PIN_LABEL,
              left: `${pinPct}%`,
              transform: pinLabelTransform(pinPct),
              color: pin.color,
            }}
          >
            {pin.label}
          </span>
          <span style={{ ...PIN_TICK, left: `${pinPct}%`, background: pin.color }} />
        </>
      ) : null}
      {marks.map((mark) => (
        <span key={mark.at} style={{ ...MARK, left: `${pct(mark.at)}%` }}>
          {mark.label}
        </span>
      ))}
    </div>
  );
}
