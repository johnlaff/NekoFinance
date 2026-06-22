import * as React from "react";

/**
 * Labeled text / number / money input matching the app's NewTransactionForm field style.
 * Background: --bg-subtle. Border: --border-input (WCAG 1.4.11 compliant).
 * Radius: --radius-xs. Height: --hit-min. Label: uppercase, --ls-label.
 *
 * @startingPoint section="Core" subtitle="Input — labeled text, number, or money field" viewport="360x120"
 */
export interface InputProps extends Omit<
  React.InputHTMLAttributes<HTMLInputElement>,
  "prefix"
> {
  /** Field label rendered above the control (uppercase, muted). */
  label?: string;
  /** Adds a danger asterisk next to the label. */
  required?: boolean;
  /** Static text or node before the value (e.g. "R$"). */
  prefix?: React.ReactNode;
  /** Static text or node after the value (e.g. "USD"). */
  suffix?: React.ReactNode;
  /** 16×16 leading icon node (inline SVG recommended). */
  icon?: React.ReactNode;
  /** Render value with tabular mono font, right-aligned — for monetary amounts. */
  money?: boolean;
  /** Makes the underlying input read-only and dims the background (used when value is auto-calculated). */
  readOnly?: boolean;
  /** Error message; paints the danger border and replaces hint. */
  error?: string;
  /** Helper text below the field; suppressed when error is set. */
  hint?: string;
  disabled?: boolean;
}

export function Input(props: InputProps): JSX.Element;
