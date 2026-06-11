import { useState } from "react";
import { Search } from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { getRecentTransactions, isTauri, type TransactionRow } from "../lib/api";
import { fmtBRL, fmtDate } from "../lib/format";
import { useCommand } from "../lib/useCommand";

/** Explicit seam: server-side pagination/FTS5 search replaces this in a later slice. */
const FETCH_LIMIT = 500;

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

export function TransactionsScreen({
  query,
  onQueryChange,
}: {
  query: string;
  onQueryChange: (query: string) => void;
}) {
  const [scope, setScope] = useState<TransactionScope>("all");
  const {
    data: transactions = [],
    loading,
    error,
  } = useCommand("get_recent_transactions", () => getRecentTransactions(FETCH_LIMIT));

  // React Compiler memoizes; no manual useMemo needed.
  const visible = filterTransactions(transactions, scope, query);

  if (!isTauri) {
    return (
      <div className="dash">
        <div className="dash-hero">
          <div className="dash-hero__txt">
            <div className="dash-hero__line">
              <b>Preview web.</b> Abra o app desktop para ver suas transações.
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="dash">
        <EmptyState variant="skeleton" skeletonRows={8} />
      </div>
    );
  }

  if (error) {
    return (
      <div className="dash">
        <EmptyState
          variant="error"
          title="Não foi possível carregar as transações"
          description={error}
          action={
            <Button variant="primary" onClick={() => window.location.reload()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  return (
    <div className="dash">
      <div className="txs-tools">
        <SegmentedControl
          size="sm"
          value={scope}
          onChange={(v) => setScope(v as TransactionScope)}
          options={[
            { value: "all", label: "Todas" },
            { value: "credit", label: "Crédito" },
            { value: "future", label: "Futuro" },
          ]}
        />
        <label className="ak-search txs-tools__search">
          <Search size={15} strokeWidth={1.75} />
          <input
            aria-label="Filtrar por descrição"
            placeholder="Filtrar por descrição…"
            type="search"
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
          />
        </label>
        <span className="txs-tools__sp" />
        <Badge tone="secondary">
          {visible.length} {visible.length === 1 ? "exibida" : "exibidas"}
        </Badge>
      </div>

      <div className="dash-card">
        <div className="dash-card__body" style={{ padding: 0 }}>
          {visible.length === 0 ? (
            <EmptyState
              variant="empty"
              title="Nenhuma transação encontrada"
              description={
                transactions.length === 0
                  ? "Importe sua planilha em Configurações para começar."
                  : "Nenhum resultado para o filtro atual."
              }
            />
          ) : (
            <table className="txn-table">
              <thead>
                <tr>
                  <th scope="col">Data</th>
                  <th scope="col">Descrição</th>
                  <th scope="col">Método</th>
                  <th scope="col">Valor</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((t) => (
                  <tr className={t.is_projection ? "projection" : ""} key={t.id}>
                    <td>{fmtDate(t.date)}</td>
                    <td>
                      {t.description || "—"}{" "}
                      {t.is_projection && <Badge tone="secondary">previsto</Badge>}
                    </td>
                    <td>{t.payment_method || t.type}</td>
                    <td
                      className={`money ${t.type === "income" ? "positive" : "negative"}`}
                    >
                      {fmtBRL(Math.abs(t.amount))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
