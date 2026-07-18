import type { CSSProperties } from "react";
import { InfoPopover } from "./InfoPopover";

// Selo do terceiro tipo de número: um valor DERIVADO (não registrado/escolhido) só aparece com
// esta marca ao lado — nunca como veredito. A palavra é obrigatória (status nunca é só cor); o
// popover carrega a didática do ritual que tornaria o número um veredito.
// Estático e içado: recriar por render é desperdício e o React Compiler não memoiza literais.
const MARK_STYLE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  padding: "1px 7px",
  borderRadius: 999,
  border: "1px solid color-mix(in srgb, var(--state-estimate) 45%, transparent)",
  color: "var(--state-estimate)",
  fontSize: "var(--fs-caption, 11px)",
  fontWeight: 500,
  lineHeight: 1.6,
  whiteSpace: "nowrap",
};

export interface EstimateMarkProps {
  /** Didática do ritual: por que isto é estimativa e qual gesto a tornaria veredito. */
  term: { title?: string; body: string };
  className?: string;
}

export function EstimateMark({ term, className }: EstimateMarkProps) {
  return (
    <InfoPopover term={term} hideMarker {...(className ? { className } : {})}>
      <span style={MARK_STYLE}>Estimativa</span>
    </InfoPopover>
  );
}
