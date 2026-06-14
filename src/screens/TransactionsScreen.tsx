import { useState } from "react";
import { Plus, Search } from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { ProvBadge } from "../design-system/components/ProvBadge";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { getRecentTransactions, isTauri, type TransactionRow } from "../lib/api";
import { fmtDate } from "../lib/format";
import { Money } from "../design-system/components/Money";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { NewTransactionForm } from "./NewTransactionForm";
import { ConflictGate } from "../features/reconcile/ConflictGate";

/** Explicit seam: server-side pagination/FTS5 search replaces this in a later slice. */
const FETCH_LIMIT = 500;

export type TransactionScope = "all" | "credit" | "future";

const METHOD_LABELS: Record<string, string> = {
  debit: "Débito",
  credit: "Crédito",
  pix: "PIX",
  transfer: "Transferência",
  cash: "Dinheiro",
};

/** Rótulo amigável do método de pagamento (Débito, PIX…); entrada sem método vira "Entrada". */
export function methodLabel(t: TransactionRow): string {
  if (t.payment_method) return METHOD_LABELS[t.payment_method] ?? t.payment_method;
  return t.type === "income" ? "Entrada" : "—";
}

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
  const [showForm, setShowForm] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const {
    data: transactions = [],
    loading,
    error,
  } = useCommand(`get_recent_transactions:${reloadKey}`, () =>
    getRecentTransactions(FETCH_LIMIT),
  );

  function handleCreated() {
    invalidateCommands();
    setReloadKey((k) => k + 1);
    setShowForm(false);
  }

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
      <ConflictGate onResolved={handleCreated} />
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
        <Button
          size="sm"
          variant={showForm ? "ghost" : "primary"}
          iconLeft={<Plus size={15} strokeWidth={2} />}
          onClick={() => setShowForm((v) => !v)}
        >
          {showForm ? "Fechar" : "Novo lançamento"}
        </Button>
      </div>

      {showForm && (
        <div style={{ marginBottom: "var(--space-4)" }}>
          <NewTransactionForm onCreated={handleCreated} />
        </div>
      )}

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
                      {t.description || "—"} <ProvBadge provenance={t.provenance} />
                      {t.owners.length >= 2 && (
                        <span
                          style={{
                            display: "inline-flex",
                            gap: 4,
                            marginLeft: 6,
                            verticalAlign: "middle",
                          }}
                        >
                          {t.owners.map((name) => (
                            <OwnerChip key={name} name={name} />
                          ))}
                        </span>
                      )}
                    </td>
                    <td>
                      <span className="txn-method">{methodLabel(t)}</span>
                    </td>
                    <td style={{ textAlign: "right" }}>
                      {t.type === "income" ? (
                        <Money cents={Math.abs(t.amount)} size="sm" sign="auto" />
                      ) : (
                        <Money cents={-Math.abs(t.amount)} size="sm" sign="none" />
                      )}
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
