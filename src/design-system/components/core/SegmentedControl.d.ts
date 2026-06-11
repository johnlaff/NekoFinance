import * as React from "react";

export interface SegmentedOption {
  value: string;
  label: string;
  /** Optional leading status dot color (e.g. an owner accent). */
  dot?: string;
}

export interface SegmentedControlProps {
  /** Options as strings or {value,label,dot}. */
  options: (string | SegmentedOption)[];
  /** Currently selected value. */
  value: string;
  onChange?: (value: string) => void;
  size?: "sm" | "md";
  className?: string;
}

/**
 * Compact single-select toggle for 2–4 mutually exclusive views
 * (e.g. All / Personal / Partner / Shared, or month ranges).
 */
export function SegmentedControl(props: SegmentedControlProps): JSX.Element;
