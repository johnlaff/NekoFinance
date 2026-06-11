import * as React from "react";

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  /** Semantic tone — drives tint background + text color. */
  tone?: "neutral" | "success" | "warning" | "danger" | "info" | "primary";
  /** Show a leading status dot in the tone color. */
  dot?: boolean;
  /** Square (xs radius) instead of pill — for counts / codes. */
  square?: boolean;
  children?: React.ReactNode;
}

/**
 * Small status / category label. Always pairs color with a word — never
 * color alone — so status stays accessible.
 */
export function Badge(props: BadgeProps): JSX.Element;
