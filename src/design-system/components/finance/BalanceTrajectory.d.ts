import * as React from "react";

/**
 * Area chart of the projected balance trajectory — the forecast sparkline.
 * Draws once on mount with a stroke-draw animation (respects prefers-reduced-motion).
 * Interactive: hover reveals a crosshair + floating tooltip (dia · saldo).
 * Deficit is never color-only: a dashed zero-line band and a "R$ 0" label accompany it.
 * Accessible: aria-label summarises today's balance, the minimum point, and the horizon end.
 * @startingPoint section="Finance" subtitle="BalanceTrajectory — projected balance area chart" viewport="720x300"
 */
export interface BalanceTrajectoryProps {
  /**
   * Ordered array of forecast days, each with a date (YYYY-MM-DD) and balance in cents.
   * Defaults to a synthetic 30-day demo series when omitted.
   */
  daily?: Array<{ date: string; balance_cents: number }>;
  /**
   * ISO date string (YYYY-MM-DD) for today, used to place the "hoje" marker.
   * Defaults to the current date when omitted.
   */
  today?: string;
  /**
   * `"full"` — tall chart (260 px svg height) used in the Horizonte screen.
   * `"compact"` — short chart (120 px) embedded in the hero forecast tile.
   * @default "full"
   */
  variant?: "full" | "compact";
}

/**
 * Area chart of the projected balance trajectory — the forecast sparkline.
 * Draws once on mount with a stroke-draw animation (respects prefers-reduced-motion).
 * Interactive: hover reveals a crosshair + floating tooltip (dia · saldo).
 * Deficit is never color-only: a dashed zero-line band and a "R$ 0" label accompany it.
 */
export function BalanceTrajectory(props: BalanceTrajectoryProps): JSX.Element;
