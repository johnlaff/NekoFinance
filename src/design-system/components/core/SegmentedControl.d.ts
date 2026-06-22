import * as React from "react";

/**
 * Compact single-select toggle for 2–4 mutually exclusive views
 * (e.g. Dia / Semana / Mês, or filter scopes).
 * Renders as a radiogroup with roving tabindex and full arrow-key navigation.
 *
 * @startingPoint section="Core" subtitle="SegmentedControl — single-select segment toggle" viewport="360x60"
 */
export interface SegmentedOption {
  value: string;
  label: string;
}

export interface SegmentedControlProps {
  /** Options array. */
  options: SegmentedOption[];
  /** Currently selected value. */
  value: string;
  /** Called with the new value when the user selects a segment. */
  onChange?: (value: string) => void;
  /** "sm" reduces height to 28 px; "md" (default) is 32 px. */
  size?: "sm" | "md";
  /** Extra className applied to the wrapper. */
  className?: string;
  /** Disables all segments when true. */
  disabled?: boolean;
  /** Accessible label for the radiogroup element. Recommended when context is not self-evident. */
  ariaLabel?: string;
}

export function SegmentedControl(props: SegmentedControlProps): JSX.Element;
