import * as React from "react";

/**
 * On/off toggle track-and-knob primitive used for immediate state changes in settings panels.
 * Matches the `gs-toggle` visual pattern from GoogleSheetsPanel (36×20 px track, jade when on,
 * WCAG-AA neutral when off). Not for binary form choices — use SegmentedControl for Ligado/Desligado
 * pairs that benefit from explicit labels on both sides.
 *
 * @startingPoint section="Core" subtitle="Switch — on/off toggle" viewport="280x60"
 */
export interface SwitchProps {
  /** Controlled checked state. Defaults to false so the component renders standalone. */
  checked?: boolean;
  /** Called with the next boolean value when the user toggles. */
  onChange?: (checked: boolean) => void;
  /** Optional trailing label rendered to the right of the track. */
  label?: string;
  /** Dims and blocks interaction. */
  disabled?: boolean;
  /** Extra class names appended to the root label. */
  className?: string;
}

export function Switch(props: SwitchProps): JSX.Element;
