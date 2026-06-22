import * as React from "react";

/**
 * Method-adaptation phase badge — shows where the user is on the Mapear →
 * Calibrar → Operar journey with a 3-segment progress indicator. The method is
 * a journey; the phase shows progress without demanding maturity not yet earned.
 * @startingPoint section="Finance" subtitle="PhaseBadge — Mapear · Calibrar · Operar" viewport="220x60"
 */
export interface PhaseBadgeProps {
  /** Current adaptation phase. Fills 1, 2 or 3 of the progress segments. */
  phase?: "map" | "calibrate" | "operate";
}

/**
 * Method-adaptation phase badge (Mapear · Calibrar · Operar) with a 3-segment
 * progress indicator.
 */
export function PhaseBadge(props: PhaseBadgeProps): JSX.Element;
