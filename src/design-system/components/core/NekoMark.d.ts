import * as React from "react";

/**
 * App logo mark — the Neko cat-face SVG glyph rendered at any size.
 * Fills with `currentColor`; pass `color` or a CSS `className` to tint.
 * @startingPoint section="Core" subtitle="NekoMark — app logo mark SVG" viewport="160x160"
 */
export interface NekoMarkProps {
  /** Rendered width in pixels. Defaults to 48. */
  width?: number;
  /** Rendered height in pixels. Defaults to 48. */
  height?: number;
  /**
   * Fill color for the glyph. Accepts any CSS color or `var(--token)`.
   * Defaults to `var(--primary)` (jade).
   */
  color?: string;
  /** Additional CSS class names to apply to the SVG element. */
  className?: string;
  /** Inline styles merged onto the SVG element. */
  style?: React.CSSProperties;
  /**
   * Accessible label for the SVG when used as a meaningful image.
   * Defaults to "Neko". Pass `aria-hidden={true}` to silence the label
   * when the mark is purely decorative (e.g. inside a labelled button).
   */
  "aria-label"?: string;
  /** Set to `true` to hide the mark from assistive technology. */
  "aria-hidden"?: boolean | "true" | "false";
}

/**
 * App logo mark — the Neko cat-face SVG glyph rendered at any size.
 */
export function NekoMark(props: NekoMarkProps): JSX.Element;
