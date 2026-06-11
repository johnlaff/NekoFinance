import * as React from "react";

/**
 * Human-approval gate for an AI-proposed Google Sheets write. Shows exactly
 * which cells change (before → after), why, and requires explicit action.
 * This is a first-class privacy/trust surface — never auto-apply.
 * @startingPoint section="Finance" subtitle="ApprovalDiffCard — human-approved sheet write" viewport="520x320"
 */
export interface ApprovalDiffCardProps {
  title?: string;
  /** Target sheet name, e.g. "Expenses 2025". */
  sheet: string;
  /** Cell range / row, e.g. "D1204:F1204". */
  range?: string;
  /** Cell-level changes to render as a before → after diff. */
  changes: DiffChange[];
  /** Mia's rationale / citation for the change. */
  note?: React.ReactNode;
  /** Approval state — drives the header pill. */
  status?: "pending" | "approved" | "rejected";
  /** Action buttons row (e.g. Approve / Edit / Reject). */
  actions?: React.ReactNode;
  className?: string;
}

export interface DiffChange {
  /** Field / column label, e.g. "Category", "Owner", "Amount". */
  field: string;
  /** Current value (struck through). Omit/empty for an addition. */
  before?: string;
  /** Proposed value (jade). */
  after: string;
}

export function ApprovalDiffCard(props: ApprovalDiffCardProps): JSX.Element;
