import { Fragment, useReducer } from "react";
import {
  ChevronDown,
  ChevronRight,
  MoreHorizontal,
  Plus,
  Tag as TagIcon,
} from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { ProvBadge } from "../design-system/components/ProvBadge";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { MovBadge, type MovKind } from "../design-system/components/MovBadge";
import {
  deleteSeriesAll,
  deleteSeriesFrom,
  deleteTransaction,
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
import { safeErrorMessage } from "../lib/errors";
import { NewTransactionForm, type TransactionEditValues } from "./NewTransactionForm";
import { ConflictGate } from "../features/reconcile/ConflictGate";
import { filterTransactions, type TransactionScope } from "./transactionsFilter";

/** Explicit seam: server-side pagination/FTS5 search replaces this in a later slice. */
const FETCH_LIMIT = 500;

/** Descrição "fallback" gerada no import quando a célula não tinha nota (ex.: "Saída 2026-06-01"). */
const GENERIC_DESC = /^(Entrada|Saída|Diário) \d{4}-\d{2}-\d{2}$/;

/** Rótulo do separador de mês no Livro-razão: "Junho de 2026". */
function monthSepLabel(ym: string): string {
  const name = monthNamePtBR(`${ym}-01`);
  return `${name.charAt(0).toUpperCase()}${name.slice(1)} de ${ym.slice(0, 4)}`;
}

const METHOD_LABELS: Record<string, string> = {
  debit: "Débito",
  credit: "Crédito",
  pix: "PIX",
  cash: "Dinheiro",
};

// Estilos estáticos hoistados (não recriam por render — convenção do React Compiler/Doctor).
const ACTION_CELL_STYLE: React.CSSProperties = {
  width: 32,
  textAlign: "right",
  paddingRight: 8,
};

const ACTION_PANEL_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-3)",
  flexWrap: "wrap",
  alignItems: "center",
};

const EDIT_FORM_WRAP_STYLE: React.CSSProperties = {
  marginBottom: "var(--space-4)",
};

// Plan 035: breakdown itemizado (lista de partes da nota de célula), só leitura.
const LINE_ITEMS_LIST_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-1)",
  margin: 0,
  paddingLeft: "var(--space-6)",
  listStyle: "none",
};

const LINE_ITEM_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-3)",
  alignItems: "baseline",
  fontSize: "var(--fs-sm)",
  color: "var(--text-muted)",
};

/** Rótulo amigável do método de pagamento (Débito, PIX…); entrada sem método vira "Entrada". */
function methodLabel(t: TransactionRow): string {
  if (t.payment_method) return METHOD_LABELS[t.payment_method] ?? t.payment_method;
  return t.type === "income" ? "Entrada" : "—";
}

/**
 * Tipo de movimento do método (os 5 pilares), derivado de type + is_fixed + payment_method:
 * income→entrada, transfer→economia, despesa fixa (coluna Saída)→saída, crédito variável→cartão,
 * o resto→diário. É a leitura por tipo que o usuário tem nas colunas separadas da planilha.
 */
function movKind(t: TransactionRow): MovKind {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
}

/**
 * Recupera o id de recorrência a partir do id de um lançamento ("uuid:index" → "uuid").
 * Sem dois-pontos = lançamento único (null). Evita ida ao backend só para saber se é série.
 */
function recurrenceIdOf(id: string): string | null {
  return id.includes(":") ? id.slice(0, id.lastIndexOf(":")) : null;
}

/** Converte uma linha do Livro-razão nos valores que o form precisa para entrar em modo edição. */
function toEditValues(t: TransactionRow): TransactionEditValues {
  return {
    id: t.id,
    type: t.type,
    amount: Math.abs(t.amount),
    description: t.description ?? "",
    date: t.date,
    payment_method: t.payment_method || null,
    is_fixed: t.is_fixed,
    recurrence_id: recurrenceIdOf(t.id),
  };
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

interface TransactionsUiState {
  scope: TransactionScope;
  showForm: boolean;
  reloadKey: number;
  tagEditId: string | null;
  tagSaving: string | null;
  tagError: string | null;
  actionRowId: string | null; // qual linha tem o painel de ações aberto
  actionError: string | null; // último erro de uma ação apagar/editar
  editingTxn: TransactionRow | null; // lançamento em edição (form inline)
  expandItemsId: string | null; // qual linha tem o breakdown itemizado aberto (plano 035)
}

type TransactionsUiAction =
  | { type: "setScope"; scope: TransactionScope }
  | { type: "toggleForm" }
  | { type: "reload" }
  | { type: "created" }
  | { type: "toggleTagEditor"; id: string }
  | { type: "tagSaveStart"; saveKey: string }
  | { type: "tagSaveSuccess" }
  | { type: "tagSaveError"; error: string }
  | { type: "toggleActionRow"; id: string }
  | { type: "actionError"; error: string }
  | { type: "actionClear" }
  | { type: "editTxn"; txn: TransactionRow }
  | { type: "editDone" }
  | { type: "toggleItems"; id: string };

const INITIAL_UI_STATE: TransactionsUiState = {
  scope: "all",
  showForm: false,
  reloadKey: 0,
  tagEditId: null,
  tagSaving: null,
  tagError: null,
  actionRowId: null,
  actionError: null,
  editingTxn: null,
  expandItemsId: null,
};

function transactionsUiReducer(
  state: TransactionsUiState,
  action: TransactionsUiAction,
): TransactionsUiState {
  switch (action.type) {
    case "setScope":
      return { ...state, scope: action.scope };
    case "toggleForm":
      return { ...state, showForm: !state.showForm };
    case "reload":
      return { ...state, reloadKey: state.reloadKey + 1 };
    case "created":
      return { ...state, reloadKey: state.reloadKey + 1, showForm: false };
    case "toggleTagEditor":
      return {
        ...state,
        tagEditId: state.tagEditId === action.id ? null : action.id,
        tagError: null,
      };
    case "tagSaveStart":
      return { ...state, tagSaving: action.saveKey, tagError: null };
    case "tagSaveSuccess":
      return {
        ...state,
        reloadKey: state.reloadKey + 1,
        tagSaving: null,
        tagError: null,
      };
    case "tagSaveError":
      return { ...state, tagSaving: null, tagError: action.error };
    case "toggleActionRow":
      return {
        ...state,
        actionRowId: state.actionRowId === action.id ? null : action.id,
        actionError: null,
      };
    case "actionError":
      return { ...state, actionError: action.error };
    case "actionClear":
      return { ...state, actionRowId: null, actionError: null };
    case "editTxn":
      return { ...state, editingTxn: action.txn, actionRowId: null };
    case "editDone":
      return {
        ...state,
        editingTxn: null,
        actionRowId: null,
        reloadKey: state.reloadKey + 1,
      };
    case "toggleItems":
      return {
        ...state,
        expandItemsId: state.expandItemsId === action.id ? null : action.id,
      };
  }
}

/** Linha principal de um lançamento no Livro-razão (data, tipo, descrição, método, valor, ações). */
function LedgerDataRow({
  t,
  tagEditOpen,
  actionOpen,
  itemsOpen,
  onToggleTagEditor,
  onToggleAction,
  onToggleItems,
}: {
  t: TransactionRow;
  tagEditOpen: boolean;
  actionOpen: boolean;
  itemsOpen: boolean;
  onToggleTagEditor: () => void;
  onToggleAction: () => void;
  onToggleItems: () => void;
}) {
  const generic = !!t.description && GENERIC_DESC.test(t.description);
  const hasItems = t.line_items.length > 0;
  return (
    <tr className={t.is_projection ? "projection" : ""}>
      <td>{fmtDate(t.date)}</td>
      <td>
        <MovBadge kind={movKind(t)} showLabel size={16} />
      </td>
      <td>
        {hasItems && (
          <button
            type="button"
            className="txn-tag-btn"
            aria-label={`${itemsOpen ? "Fechar" : "Ver"} itens de ${t.description || "lançamento"}`}
            aria-expanded={itemsOpen}
            onClick={onToggleItems}
          >
            {itemsOpen ? (
              <ChevronDown size={13} strokeWidth={1.75} />
            ) : (
              <ChevronRight size={13} strokeWidth={1.75} />
            )}
          </button>
        )}{" "}
        {t.description ? (
          <span
            title={
              generic
                ? "Sem nota na célula — reimporte via Google Sheets para trazer a descrição real"
                : undefined
            }
            style={
              generic ? { color: "var(--text-faint)", fontStyle: "italic" } : undefined
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
          aria-expanded={tagEditOpen}
          onClick={onToggleTagEditor}
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
      <td style={ACTION_CELL_STYLE}>
        <button
          type="button"
          className="txn-tag-btn"
          aria-label={`Ações para ${t.description || "lançamento"}`}
          aria-expanded={actionOpen}
          onClick={onToggleAction}
        >
          <MoreHorizontal size={13} strokeWidth={1.75} />
        </button>
      </td>
    </tr>
  );
}

/** Linha-painel de ações (Editar / Apagar) de um lançamento, aberta sob a sua linha. */
function ActionPanelRow({
  t,
  actionError,
  onEdit,
  onDeleteOne,
  onDeleteSeries,
}: {
  t: TransactionRow;
  actionError: string | null;
  onEdit: () => void;
  onDeleteOne: () => void;
  onDeleteSeries: () => void;
}) {
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        {actionError ? (
          <p className="txs-inline-error" role="alert">
            {actionError}
          </p>
        ) : null}
        <div style={ACTION_PANEL_STYLE}>
          <Button size="sm" variant="ghost" onClick={onEdit}>
            Editar
          </Button>
          {recurrenceIdOf(t.id) ? (
            <Button size="sm" variant="ghost" onClick={onDeleteSeries}>
              Apagar da série
            </Button>
          ) : (
            <Button size="sm" variant="ghost" onClick={onDeleteOne}>
              Apagar
            </Button>
          )}
        </div>
      </td>
    </tr>
  );
}

/** Linha-painel do editor de tags de um lançamento, aberta sob a sua linha. */
function TagEditorRow({
  t,
  allTags,
  tagSaving,
  tagError,
  onToggleTag,
}: {
  t: TransactionRow;
  allTags: Tag[];
  tagSaving: string | null;
  tagError: string | null;
  onToggleTag: (tagId: string) => void;
}) {
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        {allTags.length === 0 ? (
          <span style={{ color: "var(--text-muted)" }}>
            Crie tags na aba Tags para classificar este lançamento.
          </span>
        ) : (
          <>
            {tagError ? (
              <p className="txs-inline-error" role="alert">
                {tagError}
              </p>
            ) : null}
            <span className="txn-tag-picker">
              {allTags.map((tag) => {
                const on = t.tags.some((x) => x.id === tag.id);
                const saving = tagSaving === `${t.id}:${tag.id}`;
                return (
                  <button
                    type="button"
                    key={tag.id}
                    aria-pressed={on}
                    className={`txn-tag-opt ${on ? "is-on" : ""}`}
                    disabled={tagSaving !== null}
                    onClick={() => onToggleTag(tag.id)}
                  >
                    <span
                      aria-hidden="true"
                      className="txn-tag-dot"
                      style={{ background: tag.color }}
                    />
                    {tag.emoji ? `${tag.emoji} ` : ""}
                    {saving ? "Salvando…" : tag.name}
                  </button>
                );
              })}
            </span>
          </>
        )}
      </td>
    </tr>
  );
}

/**
 * Sub-linha de breakdown itemizado (plano 035): mostra cada parte da nota de célula como
 * `R$ <valor> — <descrição>`. SÓ LEITURA — edição/write-back ficam para o plano 036. O total
 * do lançamento pai é a SOMA destas partes e não é alterado aqui. Vale para passado E projetado.
 */
function LineItemsRow({ t }: { t: TransactionRow }) {
  if (t.line_items.length === 0) return null;
  // Espelha a direção do pai: despesa exibe as partes como saída (negativas).
  const sign = t.type === "income" ? 1 : -1;
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        <ul
          style={LINE_ITEMS_LIST_STYLE}
          aria-label={`Itens de ${t.description || "lançamento"}`}
        >
          {t.line_items.map((li) => (
            <li key={li.id} style={LINE_ITEM_STYLE}>
              <Money cents={sign * Math.abs(li.amount_cents)} size="sm" sign="auto" />
              <span>{li.description}</span>
            </li>
          ))}
        </ul>
      </td>
    </tr>
  );
}

/**
 * Tabela do Livro-razão: cabeçalho, separadores de mês e cada linha com seus painéis
 * (breakdown itemizado, ações, editor de tags). Extraída da tela para manter o componente
 * pai enxuto e legível (uma seção por componente).
 */
function LedgerTable({
  visible,
  ui,
  allTags,
  onToggleTagEditor,
  onToggleAction,
  onToggleItems,
  onEdit,
  onDeleteOne,
  onDeleteSeries,
  onToggleTag,
}: {
  visible: TransactionRow[];
  ui: TransactionsUiState;
  allTags: Tag[];
  onToggleTagEditor: (id: string) => void;
  onToggleAction: (id: string) => void;
  onToggleItems: (id: string) => void;
  onEdit: (t: TransactionRow) => void;
  onDeleteOne: (t: TransactionRow) => void;
  onDeleteSeries: (t: TransactionRow) => void;
  onToggleTag: (t: TransactionRow, tagId: string) => void;
}) {
  return (
    <table className="txn-table">
      <thead>
        <tr>
          <th scope="col">Data</th>
          <th scope="col">Tipo</th>
          <th scope="col">Descrição</th>
          <th scope="col">Método</th>
          <th scope="col">Valor</th>
          <th scope="col" aria-label="Ações" />
        </tr>
      </thead>
      <tbody>
        {visible.map((t, i) => {
          const ym = t.date.slice(0, 7);
          const showMonth = i === 0 || visible[i - 1]!.date.slice(0, 7) !== ym;
          return (
            <Fragment key={t.id}>
              {showMonth && (
                <tr className="txn-month-sep">
                  <th scope="colgroup" colSpan={6}>
                    {monthSepLabel(ym)}
                  </th>
                </tr>
              )}
              <LedgerDataRow
                t={t}
                tagEditOpen={ui.tagEditId === t.id}
                actionOpen={ui.actionRowId === t.id}
                itemsOpen={ui.expandItemsId === t.id}
                onToggleTagEditor={() => onToggleTagEditor(t.id)}
                onToggleAction={() => onToggleAction(t.id)}
                onToggleItems={() => onToggleItems(t.id)}
              />
              {ui.expandItemsId === t.id && <LineItemsRow t={t} />}
              {ui.actionRowId === t.id && (
                <ActionPanelRow
                  t={t}
                  actionError={ui.actionError}
                  onEdit={() => onEdit(t)}
                  onDeleteOne={() => onDeleteOne(t)}
                  onDeleteSeries={() => onDeleteSeries(t)}
                />
              )}
              {ui.tagEditId === t.id && (
                <TagEditorRow
                  t={t}
                  allTags={allTags}
                  tagSaving={ui.tagSaving}
                  tagError={ui.tagError}
                  onToggleTag={(tagId) => onToggleTag(t, tagId)}
                />
              )}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

export function TransactionsScreen({
  query,
  onGoToSettings,
}: {
  query: string;
  onGoToSettings: () => void;
}) {
  const [ui, dispatchUi] = useReducer(transactionsUiReducer, INITIAL_UI_STATE);
  const {
    data: transactions = [],
    loading,
    error,
  } = useCommand(`get_recent_transactions:${ui.reloadKey}`, () =>
    getRecentTransactions(FETCH_LIMIT),
  );
  const allTags: Tag[] = useCommand(`list_tags:${ui.reloadKey}`, listTags).data ?? [];

  /** Anexa/remove uma tag do lançamento e recarrega (fecha o loop de diagnóstico do método). */
  async function toggleTag(t: TransactionRow, tagId: string) {
    if (ui.tagSaving) return;
    const has = t.tags.some((x) => x.id === tagId);
    // Uma passada (reduce) em vez de .filter().map(): remove a tag clicada se já existe,
    // senão acrescenta ao final.
    const next = has
      ? t.tags.reduce<string[]>((acc, x) => {
          if (x.id !== tagId) acc.push(x.id);
          return acc;
        }, [])
      : [...t.tags.map((x) => x.id), tagId];
    dispatchUi({ type: "tagSaveStart", saveKey: `${t.id}:${tagId}` });
    try {
      await setTransactionTags(t.id, next);
      invalidateCommands();
      dispatchUi({ type: "tagSaveSuccess" });
    } catch (e) {
      dispatchUi({
        type: "tagSaveError",
        error: safeErrorMessage(
          e,
          "Não foi possível atualizar as tags. Tente novamente.",
        ),
      });
    }
  }

  function handleCreated() {
    invalidateCommands();
    dispatchUi({ type: "created" });
  }

  function handleReload() {
    invalidateCommands();
    dispatchUi({ type: "reload" });
  }

  /** Fecha o form de edição inline e recarrega a lista. */
  function handleSaved() {
    invalidateCommands();
    dispatchUi({ type: "editDone" });
  }

  /** Apaga um lançamento único (não recorrente), com confirmação. */
  async function handleDeleteOne(t: TransactionRow) {
    if (
      !window.confirm(
        `Apagar "${t.description || "este lançamento"}"? Esta ação não pode ser desfeita.`,
      )
    )
      return;
    try {
      await deleteTransaction(t.id);
      invalidateCommands();
      dispatchUi({ type: "reload" });
      dispatchUi({ type: "actionClear" });
    } catch (e) {
      dispatchUi({
        type: "actionError",
        error: safeErrorMessage(e, "Não foi possível apagar. Tente novamente."),
      });
    }
  }

  /**
   * Apaga uma ocorrência de série recorrente. Oferece três escolhas via dois confirms:
   * toda a série, deste ponto em diante, ou só esta ocorrência.
   */
  async function handleDeleteSeries(t: TransactionRow) {
    const recId = recurrenceIdOf(t.id);
    if (!recId) {
      dispatchUi({
        type: "actionError",
        error: "Lançamento não pertence a uma série.",
      });
      return;
    }
    const all = window.confirm(
      "Apagar TODA a série recorrente?\n\nOK = apagar a série inteira\nCancela = escolher só este ou os futuros",
    );
    try {
      if (all) {
        await deleteSeriesAll(recId);
      } else {
        const fromHere = window.confirm(
          "OK = apagar este e todos os futuros da série.\nCancela = apagar somente este.",
        );
        if (fromHere) {
          await deleteSeriesFrom(t.id);
        } else {
          await deleteTransaction(t.id);
        }
      }
      invalidateCommands();
      dispatchUi({ type: "reload" });
      dispatchUi({ type: "actionClear" });
    } catch (e) {
      dispatchUi({
        type: "actionError",
        error: safeErrorMessage(e, "Não foi possível apagar. Tente novamente."),
      });
    }
  }

  // React Compiler memoizes; no manual useMemo needed.
  const visible = filterTransactions(transactions, ui.scope, query);

  if (!isTauri) {
    return (
      <div className="dash">
        <div className="dash-hero">
          <div className="dash-hero__txt">
            <div className="dash-hero__line">
              <b>Preview web.</b> Abra o app desktop para ver seus lançamentos.
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
          title="Não foi possível carregar os lançamentos"
          description={error}
          action={
            <Button variant="primary" onClick={handleReload}>
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
          ariaLabel="Filtrar lançamentos por escopo"
          value={ui.scope}
          onChange={(v) =>
            dispatchUi({ type: "setScope", scope: v as TransactionScope })
          }
          options={[
            { value: "all", label: "Todas" },
            { value: "credit", label: "Crédito" },
            { value: "future", label: "Futuro" },
          ]}
        />
        {query && (
          <Badge tone="secondary">
            <span style={{ color: "var(--text-muted)" }}>Busca:</span> {query}
          </Badge>
        )}
        <span className="txs-tools__sp" />
        <Badge tone="secondary">
          {visible.length} {visible.length === 1 ? "exibida" : "exibidas"}
        </Badge>
        <Button
          size="sm"
          variant={ui.showForm ? "ghost" : "primary"}
          iconLeft={<Plus size={15} strokeWidth={2} />}
          onClick={() => dispatchUi({ type: "toggleForm" })}
        >
          {ui.showForm ? "Fechar" : "Novo lançamento"}
        </Button>
      </div>

      {ui.showForm && (
        <div style={{ marginBottom: "var(--space-4)" }}>
          <NewTransactionForm onCreated={handleCreated} />
        </div>
      )}

      {ui.editingTxn && (
        <div style={EDIT_FORM_WRAP_STYLE}>
          <NewTransactionForm
            initialValues={toEditValues(ui.editingTxn)}
            onSaved={handleSaved}
          />
        </div>
      )}

      <div className="dash-card">
        <div className="dash-card__body" style={{ padding: 0 }}>
          {visible.length === 0 ? (
            <EmptyState
              variant="empty"
              title="Nenhum lançamento encontrado"
              description={
                transactions.length === 0
                  ? "Importe sua planilha em Configurações para começar."
                  : "Nenhum resultado para o filtro atual."
              }
              action={
                transactions.length === 0 ? (
                  <Button variant="secondary" size="sm" onClick={onGoToSettings}>
                    Ir para Configurações
                  </Button>
                ) : undefined
              }
            />
          ) : (
            <LedgerTable
              visible={visible}
              ui={ui}
              allTags={allTags}
              onToggleTagEditor={(id) => dispatchUi({ type: "toggleTagEditor", id })}
              onToggleAction={(id) => dispatchUi({ type: "toggleActionRow", id })}
              onToggleItems={(id) => dispatchUi({ type: "toggleItems", id })}
              onEdit={(t) => dispatchUi({ type: "editTxn", txn: t })}
              onDeleteOne={(t) => void handleDeleteOne(t)}
              onDeleteSeries={(t) => void handleDeleteSeries(t)}
              onToggleTag={(t, tagId) => void toggleTag(t, tagId)}
            />
          )}
        </div>
      </div>
    </div>
  );
}
