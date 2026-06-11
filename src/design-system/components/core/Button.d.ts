import * as React from "react";

/**
 * Primary action control for Neko Finance. Calm press feedback, no bounce.
 * @startingPoint section="Core" subtitle="Button — primary / secondary / ghost / danger" viewport="700x150"
 */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual style. primary = jade fill, secondary = bordered surface, ghost = quiet, danger = destructive. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  /** Control height. sm 30 / md 36 / lg 44. */
  size?: "sm" | "md" | "lg";
  /** Stretch to fill container width. */
  fullWidth?: boolean;
  /** 16×16 icon node rendered before the label. */
  iconLeft?: React.ReactNode;
  /** 16×16 icon node rendered after the label. */
  iconRight?: React.ReactNode;
  disabled?: boolean;
  children?: React.ReactNode;
}

/**
 * Primary action control for Neko Finance. Calm press feedback, no bounce.
 */
export function Button(props: ButtonProps): JSX.Element;
