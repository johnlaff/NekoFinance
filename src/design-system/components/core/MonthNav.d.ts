import * as React from "react";

/**
 * Temporal navigation control — previous/next arrows with a centred month label and an optional "Hoje" shortcut pill.
 * @startingPoint section="Core" subtitle="MonthNav — month/year navigator" viewport="420x60"
 */
export interface MonthNavProps {
  /** Formatted label for the active period, e.g. "Junho de 2026". */
  label?: string;
  /** Called when the left arrow is activated. */
  onPrev?: () => void;
  /** Called when the right arrow is activated. */
  onNext?: () => void;
  /** Called when the "Hoje" pill is activated. */
  onToday?: () => void;
  /** When false, the left arrow is dimmed and disabled. Defaults to true. */
  canPrev?: boolean;
  /** When false, the right arrow is dimmed and disabled. Defaults to true. */
  canNext?: boolean;
  /** When true, the "Hoje" pill is hidden (already on the current month). Defaults to false. */
  atToday?: boolean;
  /** Accessible label for the left arrow. Override when navigating by year instead of month. */
  prevLabel?: string;
  /** Accessible label for the right arrow. Override when navigating by year instead of month. */
  nextLabel?: string;
  /** Extra class added to the root element. */
  className?: string;
}

/**
 * Temporal navigation control — previous/next arrows with a centred month label and an optional "Hoje" shortcut pill.
 */
export function MonthNav(props: MonthNavProps): JSX.Element;
