import * as React from "react";

/**
 * Human-approval gate for an AI-proposed Google Sheets write-back. Shows exactly
 * which cells change (antes → depois), the rationale, and requires explicit user action.
 * This is a core trust surface — no write ever fires without confirmation.
 * @startingPoint section="Finance" subtitle="ApprovalDiffCard — aprovação de escrita na planilha" viewport="520x340"
 */
export interface ApprovalDiffCardProps {
  /** Card heading, e.g. "Mudança proposta". */
  title?: string;
  /** Target sheet name, e.g. "Gastos 2025". */
  sheet: string;
  /** Cell range / row, e.g. "D1204:F1204". */
  range?: string;
  /** Cell-level changes rendered as a before → after diff. */
  changes: DiffChange[];
  /** Mia's rationale / citation for the change. */
  note?: React.ReactNode;
  /** Approval state — drives the header pill. */
  status?: "pending" | "approved" | "rejected";
  /** Action buttons row (e.g. Aprovar / Editar / Recusar). */
  actions?: React.ReactNode;
  className?: string;
}

export interface DiffChange {
  /** Field / column label, e.g. "Categoria", "Dono", "Valor". */
  field: string;
  /** Current value (struck through). Omit or pass empty string for an addition. */
  before?: string;
  /** Proposed value (jade highlight). */
  after: string;
}

export function ApprovalDiffCard(props: ApprovalDiffCardProps): JSX.Element;
