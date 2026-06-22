import * as React from "react";

/**
 * Overall financial-health status with a radial ring. Always shows a word
 * alongside color so the status is accessible without color perception.
 *
 * @startingPoint section="Finance" subtitle="HealthBadge — status pill with progress ring" viewport="320x80"
 */
export interface HealthBadgeProps {
  /** Financial health level — drives color, ring fill, and default label. */
  level?: "strong" | "steady" | "watch" | "risk";
  /**
   * Overrides the default label for the level. Use method copy such as
   * "Sobrou dinheiro", "Dentro da renda", "Dentro do ideal".
   */
  label?: string;
  /** 0–100 ring fill. Defaults to a sensible value per level. */
  score?: number;
  /** Small line under the label, e.g. "3 meses de reserva". */
  sublabel?: string;
  size?: "md" | "lg";
  className?: string;
}

export function HealthBadge(props: HealthBadgeProps): JSX.Element;
