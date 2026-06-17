import type { TransactionRow } from "../lib/api";

export type TransactionScope = "all" | "credit" | "future";

/** Pure filter used by the screen; exported for direct testing. */
export function filterTransactions(
  rows: TransactionRow[],
  scope: TransactionScope,
  query: string,
): TransactionRow[] {
  const q = query.trim().toLocaleLowerCase("pt-BR");
  return rows.filter((t) => {
    if (scope === "credit" && t.payment_method !== "credit") return false;
    if (scope === "future" && !t.is_projection) return false;
    if (q && !t.description.toLocaleLowerCase("pt-BR").includes(q)) return false;
    return true;
  });
}
