import * as React from "react";

export interface SwitchProps {
  /** Controlled checked state. */
  checked: boolean;
  onChange?: (checked: boolean) => void;
  /** Optional trailing label. */
  label?: string;
  disabled?: boolean;
  className?: string;
}

/**
 * On/off toggle for settings (e.g. "Connect Google Sheets", "Auto-categorize").
 * Use for immediate state changes, not for form submission choices.
 */
export function Switch(props: SwitchProps): JSX.Element;
