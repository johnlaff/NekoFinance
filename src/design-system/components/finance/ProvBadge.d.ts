import * as React from "react";

/**
 * Transaction-provenance badge — shows how a transaction arrived (Da planilha,
 * Do app, or Previsto) with a colored dot paired with a word and an educational
 * tooltip. Color is never the sole signal: the label always accompanies the dot.
 * @startingPoint section="Finance" subtitle="ProvBadge — proveniência do lançamento" viewport="260x80"
 */
export interface ProvBadgeProps {
  /**
   * How the transaction arrived.
   * - `"importado"` — read from the spreadsheet as-is (dot: text-faint).
   * - `"manual"` — entered directly in the app and written back to the sheet (dot: info-400).
   * - `"projetado"` — future or automatic projection, not yet confirmed (dot: secondary / brass).
   */
  provenance?: "importado" | "manual" | "projetado";
}

/**
 * Transaction-provenance badge — a colored dot + label pill with an inline
 * educational tooltip explaining where the transaction came from.
 */
export function ProvBadge(props: ProvBadgeProps): JSX.Element;
