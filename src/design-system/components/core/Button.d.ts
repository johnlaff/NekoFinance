import * as React from "react";

/**
 * Primary action control for Neko Finance. Calm press feedback, no bounce.
 * @startingPoint section="Core" subtitle="Button — primary / secondary / ghost / danger" viewport="700x150"
 */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual style. primary = jade fill, secondary = brass quiet tint, ghost = bordered quiet, danger = danger tint with danger text. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  /** Control height. sm 28px / md 36px / lg 44px. */
  size?: "sm" | "md" | "lg";
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
