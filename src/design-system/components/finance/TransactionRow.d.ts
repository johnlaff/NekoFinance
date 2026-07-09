import * as React from "react";

/**
 * TransactionRow — dense transaction line for ledger tables and the import-review screen.
 *
 * Four-column grid: date · description+meta · method pill · amount.
 * When `lump` items are provided a chevron button expands the fatura sub-rows.
 * `future` renders a diagonal brass-tinted stripe pattern behind the row.
 * `passthrough` dims the amount and adds a "repasse" badge.
 *
 * @startingPoint section="Finance" subtitle="TransactionRow — linha de lançamento expandível" viewport="700x160"
 */
export interface LumpItem {
  /** Short description of the line item. */
  what: string;
  /** Amount in integer cents (positive = entrada, negative = saída). */
  amount: number;
  /** Optional owner node rendered inside the lump sub-row. */
  owner?: React.ReactNode;
  /** Whether this sub-item is a passthrough (repasse). */
  passthrough?: boolean;
}

export interface TransactionRowProps {
  /** Short date string, e.g. "21/06" or "05 Jun". Rendered in tabular mono. */
  date: string;
  /** Transaction description / merchant name. */
  desc: string;
  /** Amount in integer cents. Positive = entrada (green); negative = saída (neutral). */
  amount: number;
  /** Payment method label, e.g. "Débito", "Crédito". Rendered as a pill. Omit to hide. */
  method?: string;
  /**
   * Data provenance — determines the colored dot label.
   * - `importado`: veio da planilha (Da planilha)
   * - `manual`: lançado no app (Do app)
   * - `projetado`: previsão ainda não realizada (Previsto)
   * - `conciliado`: cruzado com o banco (Conferido)
   */
  provenance?: "importado" | "manual" | "projetado" | "conciliado";
  /** Owner node (e.g. OwnerChip) rendered in the metadata sub-row. */
  owner?: React.ReactNode;
  /** Free-text note from the spreadsheet cell. Rendered in italic. */
  note?: string;
  /** Passthrough (repasse) — dims amount and shows "repasse" badge. */
  passthrough?: boolean;
  /** Future / projected row — adds a diagonal brass-tinted stripe background. */
  future?: boolean;
  /** Fatura lump items — enables the expand/collapse chevron for sub-rows. */
  lump?: LumpItem[];
  /** Whether the lump panel starts open. */
  defaultOpen?: boolean;
  /** Selected state — jade left rail + tinted background. */
  selected?: boolean;
  /** Click handler; makes the row interactive (role="button", keyboard support). */
  onClick?: () => void;
  /** Extra CSS class applied to the outer wrapper. */
  className?: string;
}

export function TransactionRow(props: TransactionRowProps): JSX.Element;
