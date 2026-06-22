import * as React from "react";

/**
 * Collapsible disclosure panel — always-visible trigger with an animated
 * body that expands on demand. Uses the grid-template-rows collapse trick
 * for jank-free animation. Supports bare (inline in a card) and card
 * (autonomous, self-contained surface) modes, plus optional accent strips.
 *
 * @startingPoint section="Core" subtitle="Disclosure — collapsible panel" viewport="420x260"
 */
export interface DisclosureProps {
  /**
   * Always-visible heading text or node.
   * @default "Detalhes da transação"
   */
  title?: React.ReactNode;
  /**
   * Optional one-line summary shown below the title when collapsed.
   * Useful for a "R$ 1.240,00 · 3 lançamentos" preview line.
   */
  summary?: React.ReactNode;
  /**
   * Leading icon (20×20). Pass an inline SVG or a DS icon element.
   */
  icon?: React.ReactNode;
  /**
   * Semantic accent applied as a coloured left border (card mode) or
   * tinted title text (bare mode). "ok" = jade success, "warn" = brass
   * warning, "brass" = secondary brand warmth.
   */
  accent?: "ok" | "warn" | "brass";
  /**
   * Optional badge node placed inline after the title text — e.g. a count
   * chip or status badge.
   */
  badge?: React.ReactNode;
  /**
   * Whether the panel starts open.
   * @default false
   */
  defaultOpen?: boolean;
  /**
   * Bare mode omits card chrome (background, border, shadow). Default true
   * because most uses are nested inside an existing card surface —
   * card-within-card is forbidden.
   * @default true
   */
  bare?: boolean;
  /** Content revealed when the panel is open. */
  children?: React.ReactNode;
  className?: string;
}

/**
 * Collapsible disclosure panel — always-visible trigger with an animated
 * body that expands on demand.
 */
export function Disclosure(props: DisclosureProps): JSX.Element;
