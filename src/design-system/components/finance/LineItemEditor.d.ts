import * as React from "react";

/**
 * Controlled editor for the itemised parts of a transaction (plano 036).
 * Each part has a positive-magnitude amount in cents and a free-text description.
 * The parent owns the list; every add / remove / edit fires `onChange` with the full
 * new array. When ≥1 item exists the parent form's Valor field should become read-only
 * (auto-summed). A running total row appears once there are ≥2 items.
 *
 * @startingPoint section="Finance" subtitle="LineItemEditor — itemised transaction parts" viewport="480x260"
 */
export interface LineItemDraft {
  /** Part amount as a positive integer in centavos (e.g. 8500 = R$ 85,00). */
  amount_cents: number;
  /** Free-text description — never written as a formula. */
  description: string;
  /** Zero-based insertion order. Re-indexed on every remove. */
  position: number;
}

export interface LineItemEditorProps {
  /**
   * Ordered list of draft line-items. Omit (or pass `undefined`) to let the
   * component manage its own demo state — useful for standalone previews.
   */
  items?: LineItemDraft[];
  /**
   * Called with the complete updated list whenever the user adds, removes, or
   * edits any item. Required in controlled mode; omit for standalone/demo mode.
   */
  onChange?: (items: LineItemDraft[]) => void;
  /** Disables all inputs and buttons. */
  disabled?: boolean;
}

export function LineItemEditor(props: LineItemEditorProps): JSX.Element;
