import * as React from "react";

export interface InputProps extends Omit<
  React.InputHTMLAttributes<HTMLInputElement>,
  "prefix"
> {
  /** Field label rendered above the control. */
  label?: string;
  required?: boolean;
  /** Static text before the value (e.g. "$"). */
  prefix?: React.ReactNode;
  /** Static text after the value (e.g. "USD"). */
  suffix?: React.ReactNode;
  /** 16×16 leading icon. */
  icon?: React.ReactNode;
  /** Render value with tabular mono, right-aligned — for amounts. */
  money?: boolean;
  /** Error message; also paints the danger border. */
  error?: string;
  /** Helper text below the field (suppressed when error is set). */
  hint?: string;
  disabled?: boolean;
}

/**
 * Text / number / money input with label, affixes, hint and error states.
 */
export function Input(props: InputProps): JSX.Element;
