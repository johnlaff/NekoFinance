import * as React from "react";

/**
 * Mia's brand avatar — a cat-ear silhouette rendered as an inline SVG with jade
 * fill on a dark rounded background. Used as an identity mark next to Mia's
 * name in the copilot panel and chat threads.
 * @startingPoint section="Copilot" subtitle="MiaAvatar — Mia brand mark / avatar" viewport="120x120"
 */
export interface MiaAvatarProps {
  /** Width of the rendered SVG in pixels. @default 40 */
  width?: number;
  /** Height of the rendered SVG in pixels. @default 40 */
  height?: number;
  /** Additional CSS class names. */
  className?: string;
  /** Inline styles applied to the root SVG element. */
  style?: React.CSSProperties;
}

/**
 * Mia's brand avatar — a cat-ear silhouette rendered as an inline SVG with jade
 * fill on a dark rounded background.
 */
export function MiaAvatar(props: MiaAvatarProps): JSX.Element;
