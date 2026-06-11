import * as React from "react";

export interface HealthBadgeProps {
  /** Financial health level — drives color, ring fill, and label. */
  level?: "strong" | "steady" | "watch" | "risk";
  /** 0–100 ring fill. Defaults to a sensible value per level. */
  score?: number;
  /** Small line under the label, e.g. "3 months runway". */
  sublabel?: string;
  size?: "md" | "lg";
  className?: string;
}

/**
 * Overall financial-health status with a radial ring. Always shows a word
 * alongside color so the status is accessible without color perception.
 */
export function HealthBadge(props: HealthBadgeProps): JSX.Element;
