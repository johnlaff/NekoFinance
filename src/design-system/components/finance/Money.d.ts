import * as React from "react";

/**
 * Inline monetary amount in BRL using tabular mono type, a real minus sign (−), and
 * optional sign-based colour (positive jade / negative red / neutral).
 * @startingPoint section="Finance" subtitle="Money — BRL amount in tabular mono" viewport="260x120"
 */
export interface MoneyProps {
  /** Value in integer cents, e.g. 1234567 = R$ 12.345,67. Negative values render "−R$ …". */
  cents: number;
  /** Type scale: sm (13 px) · md (15 px, default) · lg (22 px) · display (34 px). */
  size?: "sm" | "md" | "lg" | "display";
  /**
   * Colour behaviour:
   * - `"none"` — inherits parent colour (default).
   * - `"auto"` — jade for positive, red for negative, muted for zero.
   * - `"negative"` — forces red unconditionally.
   */
  sign?: "none" | "auto" | "negative";
  /** When true, omits the centavos fraction. */
  hideCents?: boolean;
  /** Overrides the auto-generated accessible label (e.g. "negativo R$ 1.234,56"). */
  ariaLabel?: string;
  className?: string;
}

/**
 * Inline monetary amount in BRL using tabular mono type, a real minus sign (−), and
 * optional sign-based colour (positive jade / negative red / neutral).
 */
export function Money(props: MoneyProps): JSX.Element;
