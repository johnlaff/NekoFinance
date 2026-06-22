import * as React from "react";

/**
 * Identifies the owner of a transaction or account — personal, partner, or shared.
 * Renders a colored dot + label (default) or an avatar ring + initials (avatar mode).
 * @startingPoint section="Finance" subtitle="OwnerChip — transaction owner chip" viewport="320x120"
 */
export interface OwnerChipProps {
  /** Ownership category — sets the accent color and default label. */
  who?: "personal" | "partner" | "shared";
  /** Override the default label ("Eu", "Parceiro(a)", "Compartilhado"). */
  name?: string;
  /** Secondary qualifier, e.g. "paga". Avoids the ARIA-reserved `role` attribute name. */
  note?: string;
  /** Strip the pill background and border (for use inside dense rows). */
  bare?: boolean;
  /** Show a circular avatar with initials instead of the colored dot. */
  avatar?: boolean;
  className?: string;
}

/**
 * Identifies the owner of a transaction or account — personal, partner, or shared.
 * Renders a colored dot + label (default) or an avatar ring + initials (avatar mode).
 */
export function OwnerChip(props: OwnerChipProps): JSX.Element;
