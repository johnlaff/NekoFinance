import type { CSSProperties, ReactNode } from "react";
import { InfoPopover } from "./InfoPopover";

// Estado "Sem registro": a lacuna nunca vira número — travessão + rótulo + didática com CTA.
// O travessão usa aria-hidden (o rótulo é a informação); o popover explica o que falta e o CTA
// (do caller) leva ao gesto que cria o registro.
const WRAP_STYLE: CSSProperties = {
  display: "inline-flex",
  alignItems: "baseline",
  gap: 6,
  color: "var(--state-no-record)",
};

const DASH_STYLE: CSSProperties = {
  fontFamily: "var(--font-money)",
  fontVariantNumeric: "tabular-nums",
};

const LABEL_STYLE: CSSProperties = {
  fontSize: "var(--fs-micro)",
  fontWeight: 500,
  // A didática mora no popover; o pontilhado é o convite para abri-la.
  textDecoration: "underline dotted",
  textUnderlineOffset: 3,
};

export interface NoRecordDashProps {
  /** Didática da lacuna: o que não está registrado e qual gesto cria o registro. */
  term: { title?: string; body: string };
  /** Rótulo do estado (default "Sem registro"). */
  label?: string;
  /** Ação pequena que leva ao gesto (ex.: link para estipular o teto). */
  cta?: ReactNode;
  className?: string;
}

export function NoRecordDash({
  term,
  label = "Sem registro",
  cta,
  className,
}: NoRecordDashProps) {
  return (
    <span style={WRAP_STYLE} className={className}>
      <span style={DASH_STYLE} aria-hidden="true">
        —
      </span>
      <InfoPopover term={term} hideMarker>
        <span style={LABEL_STYLE}>{label}</span>
      </InfoPopover>
      {cta}
    </span>
  );
}
