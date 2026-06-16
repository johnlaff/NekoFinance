import { Fragment, useState } from "react";
import { Plus, Search, Tag as TagIcon } from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { ProvBadge } from "../design-system/components/ProvBadge";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { MovBadge, type MovKind } from "../design-system/components/MovBadge";
import {
  getRecentTransactions,
  isTauri,
  listTags,
  setTransactionTags,
  type Tag,
  type TagRef,
  type TransactionRow,
} from "../lib/api";
import { fmtDate, monthNamePtBR } from "../lib/format";
import { Money } from "../design-system/components/Money";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { NewTransactionForm } from "./NewTransactionForm";
import { ConflictGate } from "../features/reconcile/ConflictGate";

/** Explicit seam: server-side pagination/FTS5 search replaces this in a later slice. */
const FETCH_LIMIT = 500;

/** Descrição "fallback" gerada no import quando a célula não tinha nota (ex.: "Saída 2026-06-01"). */
const GENERIC_DESC = /^(Entrada|Saída|Diário) \d{4}-\d{2}-\d{2}$/;

/** Rótulo do separador de mês no Livro-razão: "Junho de 2026". */
function monthSepLabel(ym: string): string {
  const name = monthNamePtBR(`${ym}-01`);
  return `${name.charAt(0).toUpperCase()}${name.slice(1)} de ${ym.slice(0, 4)}`;
}

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

/**
 * Tipo de movimento do método (os 5 pilares), derivado de type + is_fixed + payment_method:
 * income→entrada, transfer→economia, despesa fixa (coluna Saída)→saída, crédito variável→cartão,
 * o resto→diário. É a leitura por tipo que o usuário tem nas colunas separadas da planilha.
 */
export function movKind(t: TransactionRow): MovKind {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
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

/** Chip colorido de uma tag anexada (somente leitura) no Livro-razão. */
function TagChip({ tag }: { tag: TagRef }) {
  return (
    <span
      className="txn-chip"
      style={{
        borderColor: tag.color,
        color: "var(--text)",
      }}
    >
      <span
        aria-hidden="true"
        className="txn-tag-dot"
        style={{ background: tag.color }}
      />
      {tag.emoji ? `${tag.emoji} ` : ""}
      {tag.name}
    </span>
  );
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
  const [tagEditId, setTagEditId] = useState<string | null>(null);
  const {
    data: transactions = [],
    loading,
    error,
  } = useCommand(`get_recent_transactions:${reloadKey}`, () =>
    getRecentTransactions(FETCH_LIMIT),
  );
  const allTags: Tag[] = useCommand(`list_tags:${reloadKey}`, listTags).data ?? [];

  /** Anexa/remove uma tag do lançamento e recarrega (fecha o loop de diagnóstico do método). */
  async function toggleTag(t: TransactionRow, tagId: string) {
    const has = t.tags.some((x) => x.id === tagId);
    const next = has
      ? t.tags.filter((x) => x.id !== tagId).map((x) => x.id)
      : [...t.tags.map((x) => x.id), tagId];
    await setTransactionTags(t.id, next);
    invalidateCommands();
    setReloadKey((k) => k + 1);
  }

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
                  <th scope="col">Tipo</th>
                  <th scope="col">Descrição</th>
                  <th scope="col">Método</th>
                  <th scope="col">Valor</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((t, i) => {
                  const ym = t.date.slice(0, 7);
                  const showMonth = i === 0 || visible[i - 1]!.date.slice(0, 7) !== ym;
                  const generic = !!t.description && GENERIC_DESC.test(t.description);
                  return (
                    <Fragment key={t.id}>
                      {showMonth && (
                        <tr className="txn-month-sep">
                          <th scope="colgroup" colSpan={5}>
                            {monthSepLabel(ym)}
                          </th>
                        </tr>
                      )}
                      <tr className={t.is_projection ? "projection" : ""}>
                        <td>{fmtDate(t.date)}</td>
                        <td>
                          <MovBadge kind={movKind(t)} showLabel size={16} />
                        </td>
                        <td>
                          {t.description ? (
                            <span
                              style={
                                generic
                                  ? { color: "var(--text-faint)", fontStyle: "italic" }
                                  : undefined
                              }
                            >
                              {t.description}
                            </span>
                          ) : (
                            "—"
                          )}{" "}
                          <ProvBadge provenance={t.provenance} />
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
                          {t.tags.map((tag) => (
                            <TagChip key={tag.id} tag={tag} />
                          ))}
                          <button
                            type="button"
                            className="txn-tag-btn"
                            aria-label={`Editar tags de ${t.description || "lançamento"}`}
                            aria-expanded={tagEditId === t.id}
                            onClick={() =>
                              setTagEditId((id) => (id === t.id ? null : t.id))
                            }
                          >
                            <TagIcon size={13} strokeWidth={1.75} />
                          </button>
                        </td>
                        <td>
                          <span className="txn-method">{methodLabel(t)}</span>
                        </td>
                        <td style={{ textAlign: "right" }}>
                          {t.type === "income" ? (
                            <Money cents={Math.abs(t.amount)} size="sm" sign="auto" />
                          ) : (
                            <Money cents={-Math.abs(t.amount)} size="sm" sign="auto" />
                          )}
                        </td>
                      </tr>
                      {tagEditId === t.id && (
                        <tr className="txn-tag-editor">
                          <td colSpan={5}>
                            {allTags.length === 0 ? (
                              <span style={{ color: "var(--text-muted)" }}>
                                Crie tags na aba Tags para classificar este lançamento.
                              </span>
                            ) : (
                              <span className="txn-tag-picker">
                                {allTags.map((tag) => {
                                  const on = t.tags.some((x) => x.id === tag.id);
                                  return (
                                    <button
                                      type="button"
                                      key={tag.id}
                                      aria-pressed={on}
                                      className={`txn-tag-opt ${on ? "is-on" : ""}`}
                                      onClick={() => void toggleTag(t, tag.id)}
                                    >
                                      <span
                                        aria-hidden="true"
                                        className="txn-tag-dot"
                                        style={{ background: tag.color }}
                                      />
                                      {tag.emoji ? `${tag.emoji} ` : ""}
                                      {tag.name}
                                    </button>
                                  );
                                })}
                              </span>
                            )}
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
