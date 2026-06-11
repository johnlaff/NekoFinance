import * as React from "react";

export interface TransactionRowProps {
  /** Short date, e.g. "08 Jun". Rendered in tabular mono. */
  date: string;
  /** Merchant / description (truncates). */
  merchant: string;
  /** Category label. */
  category?: string;
  /** Category dot color (a chart-series var). */
  categoryColor?: string;
  /** An <OwnerChip> (use bare) for the responsible owner. */
  owner?: React.ReactNode;
  /** Pre-formatted amount, e.g. "642.18". */
  amount: string;
  /** Positive (income) → green with +; otherwise neutral. */
  positive?: boolean;
  /** Reconciliation status. */
  status?: "reconciled" | "imported" | "needs-owner";
  /** Auto-categorization confidence — shows a 3-bar meter instead of a dot. */
  confidence?: "high" | "medium" | "low";
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

/**
 * A single dense transaction line for tables and import review. Carries
 * owner, category, amount, status and (during import) AI confidence.
 */
export function TransactionRow(props: TransactionRowProps): JSX.Element;
