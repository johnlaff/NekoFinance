import * as React from "react";

/**
 * Small status / category label. Always pairs a tone with a word —
 * color is never the only signal, keeping status accessible.
 *
 * @startingPoint section="Core" subtitle="Badge — small status / category label" viewport="360x80"
 */
export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  /**
   * Semantic tone — drives tint background + text color.
   * @default "primary"
   */
  tone?: "success" | "warning" | "danger" | "info" | "primary" | "secondary";
  /** Show a leading status dot in the tone color (currentColor). */
  dot?: boolean;
  /** Square (4 px radius) instead of pill — for counts / codes. */
  square?: boolean;
  children?: React.ReactNode;
}

export function Badge(props: BadgeProps): JSX.Element;
