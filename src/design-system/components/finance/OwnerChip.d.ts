import * as React from "react";

export interface OwnerChipProps {
  /** Person or group name; initials are derived for the avatar. */
  name: string;
  /** Ownership class — sets the avatar accent. shared shows a split avatar. */
  type?: "personal" | "partner" | "shared";
  /** Optional role qualifier shown after a divider: "Payer", "Beneficiary", "Responsible". */
  role?: string;
  /** Drop the pill background/border (for use inside dense rows). */
  bare?: boolean;
  className?: string;
}

/**
 * Identifies who owns / is responsible for a transaction or budget line.
 * Encodes the personal / partner / shared distinction that is core to Neko.
 */
export function OwnerChip(props: OwnerChipProps): JSX.Element;
