import * as React from "react";

export interface CitationLine {
  label: string;
  value: string;
}

export interface CitationProps {
  /** inline = a small numbered source chip; tool = a deterministic calc result block. */
  variant?: "inline" | "tool";
  /** Reference number for inline chips. */
  index?: number;
  /** Source label, e.g. "Sheet ‘Expenses 2025’ · row 1,204". */
  source?: React.ReactNode;
  /** (tool) The function/expression evaluated, e.g. "sum(Dining, May 2025)". */
  fn?: React.ReactNode;
  /** (tool) Itemized contributing rows. */
  lines?: CitationLine[];
  /** (tool) Final total row, highlighted in jade. */
  total?: CitationLine | null;
  className?: string;
}

/**
 * Provenance for Mia's numbers. inline: a numbered chip placed after a figure.
 * tool: a deterministic calculation card (itemized lines → total) that makes
 * an AI answer auditable back to sheet rows.
 */
export function Citation(props: CitationProps): JSX.Element;
