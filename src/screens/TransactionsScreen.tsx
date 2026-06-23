import "./lancamentos.css";
import { useState, useReducer } from "react";
import {
  CalendarRange,
  ChevronRight,
  Pencil,
  Plus,
  Search,
  Tags,
  Trash2,
} from "lucide-react";
import { Button } from "../design-system/components/Button";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import {
  deleteTransaction,
  getRecentTransactions,
  isTauri,
  listTags,
  setTransactionTags,
  type LineItemKind,
  type Tag,
  type TransactionRow,
} from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import {
  fmtBRL,
  fmtSigned,
  MES,
  monthOf,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { todayISO, fmtDayMonth } from "../lib/format";
import { useNekoApp } from "../shell/appContext";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FETCH_LIMIT = 400;

/** ISO today, computed once per module load (stable for a day). */
const TODAY = todayISO();

type FilterKey = "todos" | MovementType;
type ViewMode = "monthOnly" | "anchor";
interface LineItemKindMeta {
  name: string;
  color: string;
}

// ---------------------------------------------------------------------------
// Type mapping: TransactionRow → MovementType (the 5 pillars of the method)
// ---------------------------------------------------------------------------

/**
 * Maps a raw TransactionRow to one of the 5 MovementTypes used by the method.
 * income → entrada
 * transfer → economia
 * expense + is_fixed → saida
 * expense + payment_method === "credit" → cartao
 * expense + everything else → diario
 */
function toMovementType(t: TransactionRow): MovementType {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

function monthKey(iso: string): string {
  return iso.slice(0, 7);
}

function monthLabel(key: string): string {
  const [y, m] = key.split("-");
  const mIdx = parseInt(m ?? "1", 10) - 1;
  return `${MES[mIdx] ?? m} ${y}`;
}

/** Net signed cents for a row (income = positive, everything else = negative). */
function signedCents(t: TransactionRow): number {
  const abs = Math.abs(t.amount);
  return toMovementType(t) === "entrada" ? abs : -abs;
}

/** Group a list of rows into descending month order [["2026-06", [...]], ...]. */
function groupByMonth(rows: TransactionRow[]): [string, TransactionRow[]][] {
  const map = new Map<string, TransactionRow[]>();
  for (const t of rows) {
    const k = monthKey(t.date);
    if (!map.has(k)) map.set(k, []);
    map.get(k)!.push(t);
  }
  return Array.from(map.entries()).toSorted((a, b) => (a[0] < b[0] ? 1 : -1));
}

/** Current month index (0-based), relative to the current year month. */
function currentMonthIndex(): number {
  return new Date().getMonth();
}

/** Delete a transaction row, refreshing on success. Module-scope so React Compiler sees it as stable. */
function handleDelete(t: TransactionRow): void {
  if (t.provenance === "importado") return; // guard: backend also rejects this
  const confirmed = window.confirm(
    `Apagar "${t.description || "lançamento"}"? Esta ação não pode ser desfeita.`,
  );
  if (!confirmed) return;
  void deleteTransaction(t.id)
    .then(() => invalidateCommands())
    .catch((err: unknown) => {
      console.error("Falha ao apagar lançamento:", err);
    });
}

// ---------------------------------------------------------------------------
// Filter chip definitions
// ---------------------------------------------------------------------------

const FILTER_CHIPS: { key: FilterKey; label: string; color: string }[] = [
  { key: "todos", label: "Todos", color: "var(--text-faint)" },
  { key: "entrada", label: "Entradas", color: "var(--type-entrada)" },
  { key: "saida", label: "Saídas", color: "var(--type-saida)" },
  { key: "cartao", label: "Cartão", color: "var(--type-cartao)" },
  { key: "diario", label: "Diário", color: "var(--type-diario)" },
  { key: "economia", label: "Economia", color: "var(--type-economia)" },
];

const LINE_ITEM_KIND_META: Record<LineItemKind, LineItemKindMeta> = {
  saida: { name: TYPE_META.saida.name, color: TYPE_META.saida.color },
  cartao: { name: TYPE_META.cartao.name, color: TYPE_META.cartao.color },
  diario: { name: TYPE_META.diario.name, color: TYPE_META.diario.color },
  economia: { name: TYPE_META.economia.name, color: TYPE_META.economia.color },
  patrimonio: { name: "Patrimônio", color: "var(--text-muted)" },
  ajuste: { name: "Ajuste", color: "var(--warning-400)" },
};

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Inline tag picker shown within the row's expanded panel. */
function TagPicker({
  transactionId,
  currentTagIds,
  allTags,
  onDone,
}: {
  transactionId: string;
  currentTagIds: string[];
  allTags: Tag[];
  onDone: () => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(() => new Set(currentTagIds));
  const [saving, setSaving] = useState(false);

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function save() {
    setSaving(true);
    setTransactionTags(transactionId, Array.from(selected))
      .then(() => {
        invalidateCommands();
        onDone();
      })
      .catch(() => undefined)
      .finally(() => setSaving(false));
  }

  return (
    <div className="lc-tagpicker">
      <div className="lc-tagpicker__chips">
        {allTags.length === 0 ? (
          <span style={{ fontSize: 12, color: "var(--text-faint)" }}>
            Nenhuma tag criada ainda.
          </span>
        ) : (
          allTags.map((tag) => {
            const on = selected.has(tag.id);
            return (
              <button
                key={tag.id}
                type="button"
                className={"lc-tagchip" + (on ? " is-on" : "")}
                style={
                  on
                    ? {
                        background: `color-mix(in srgb, ${tag.color} 20%, transparent)`,
                        borderColor: tag.color,
                        color: tag.color,
                      }
                    : undefined
                }
                onClick={() => toggle(tag.id)}
                aria-pressed={on}
              >
                {tag.emoji ? `${tag.emoji} ` : ""}
                {tag.name}
              </button>
            );
          })
        )}
      </div>
      <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
        <Button
          size="sm"
          variant="primary"
          onClick={() => void save()}
          disabled={saving}
        >
          Aplicar
        </Button>
        <Button size="sm" variant="ghost" onClick={onDone}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

/** A single transaction row with optional expanded parts panel. */
function Row({
  t,
  open,
  onToggle,
  onEdit,
  onDelete,
  allTags,
}: {
  t: TransactionRow;
  open: boolean;
  onToggle: () => void;
  onEdit: (t: TransactionRow) => void;
  onDelete: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const mvType = toMovementType(t);
  const tm = TYPE_META[mvType];
  const totalCents = Math.abs(t.amount);
  const isFuture = t.date > TODAY;
  const isToday = t.date === TODAY;
  const isEntrada = mvType === "entrada";
  const hasItems = t.line_items.length > 1;
  const lineItemsTotal = t.line_items.reduce(
    (sum, item) => sum + Math.abs(item.amount_cents),
    0,
  );
  const lineItemsDiverge =
    t.line_items.length > 0 && Math.abs(lineItemsTotal - totalCents) > 1;
  const isImported = t.provenance === "importado";
  const [showTagPicker, setShowTagPicker] = useState(false);

  const installmentLabel =
    t.installment_index != null && t.installment_total != null
      ? `${t.installment_index}/${t.installment_total}`
      : null;

  return (
    <>
      <button
        type="button"
        className={"lc-row" + (isFuture ? " lc-row--future" : "")}
        onClick={onToggle}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggle();
          }
        }}
        aria-expanded={open}
        aria-label={t.description || "Lançamento"}
      >
        <span className="lc-row__date">{fmtDayMonth(t.date)}</span>
        <span className="lc-row__type" style={{ color: tm.color }}>
          <span className="dot" style={{ background: tm.color }}>
            {tm.glyph}
          </span>
          {tm.name}
        </span>
        <span className="lc-row__desc">
          <ChevronRight
            size={13}
            strokeWidth={1.75}
            className={"lc-chev" + (open ? " is-open" : "")}
          />
          <span className="lc-row__t" title={t.description}>
            {t.description || "—"}
          </span>
          {hasItems && <span className="lc-tag">{`${t.line_items.length} itens`}</span>}
          {installmentLabel && <span className="lc-tag">{installmentLabel}</span>}
          {t.tags.map((tag) => (
            <span
              key={tag.id}
              className="lc-tag"
              style={{
                borderColor: `color-mix(in srgb, ${tag.color} 40%, transparent)`,
                color: tag.color,
              }}
            >
              {tag.emoji ? `${tag.emoji} ` : ""}
              {tag.name}
            </span>
          ))}
          {isFuture && (
            <span
              className="lc-pill"
              style={{ background: "var(--brass-tint)", color: "var(--brass-400)" }}
            >
              Previsto
            </span>
          )}
          {isToday && (
            <span
              className="lc-pill"
              style={{ background: "var(--surface-selected)", color: "var(--primary)" }}
            >
              Hoje
            </span>
          )}
        </span>
        <span
          className="lc-row__amt"
          style={{
            color: isEntrada
              ? "var(--money-pos)"
              : isFuture
                ? "var(--text-faint)"
                : "var(--money-neg)",
          }}
        >
          {isEntrada ? "+" : "−"}
          {fmtBRL(totalCents)}
        </span>
      </button>
      {open && (
        <div className="lc-parts">
          {t.line_items.length > 0 ? (
            <>
              <p className="lc-parts__note">
                <Pencil size={11} strokeWidth={1.75} />
                {`${t.line_items.length} ${t.line_items.length === 1 ? "item" : "itens"}`}
                {hasItems ? " · viram a nota da célula na planilha" : ""}
                {lineItemsDiverge && (
                  <span className="lc-parts__warn">itens não batem</span>
                )}
              </p>
              {t.line_items.map((li, i) => {
                const kind = LINE_ITEM_KIND_META[li.kind];
                return (
                  <div className="lc-part" key={li.id ?? `li-${i}`}>
                    <span className="lc-part__desc">
                      <span
                        className="lc-kind"
                        aria-label={`Item classificado como ${kind.name}`}
                        style={{
                          color: kind.color,
                          borderColor: `color-mix(in srgb, ${kind.color} 34%, transparent)`,
                          background: `color-mix(in srgb, ${kind.color} 10%, transparent)`,
                        }}
                      >
                        <span
                          className="lc-kind__dot"
                          style={{ background: kind.color }}
                        />
                        {kind.name}
                      </span>
                      <span className="lc-part__text">{li.description}</span>
                    </span>
                    <span
                      className="lc-part__amt"
                      style={{
                        color: isEntrada ? "var(--money-pos)" : "var(--money-neg)",
                      }}
                    >
                      {isEntrada ? "+" : "−"}
                      {fmtBRL(Math.abs(li.amount_cents))}
                    </span>
                  </div>
                );
              })}
            </>
          ) : (
            <p className="lc-parts__note">
              <Pencil size={11} strokeWidth={1.75} />
              Lançamento simples · sem itens detalhados
            </p>
          )}

          {isImported && (
            <p className="lc-parts__note" style={{ color: "var(--brass-400)" }}>
              Lançamento importado · edição e exclusão via planilha
            </p>
          )}

          <div className="lc-part-actions">
            <Button
              size="sm"
              variant="ghost"
              iconLeft={<Pencil size={13} strokeWidth={1.75} />}
              disabled={isImported}
              onClick={(e?: React.MouseEvent) => {
                e?.stopPropagation();
                onEdit(t);
              }}
            >
              Editar
            </Button>
            <Button
              size="sm"
              variant="ghost"
              iconLeft={<Tags size={13} strokeWidth={1.75} />}
              onClick={(e?: React.MouseEvent) => {
                e?.stopPropagation();
                setShowTagPicker((v) => !v);
              }}
            >
              Tags
            </Button>
            <Button
              size="sm"
              variant="ghost"
              iconLeft={<Trash2 size={13} strokeWidth={1.75} />}
              disabled={isImported}
              onClick={(e?: React.MouseEvent) => {
                e?.stopPropagation();
                onDelete(t);
              }}
            >
              Apagar
            </Button>
          </div>

          {showTagPicker && (
            <TagPicker
              transactionId={t.id}
              currentTagIds={t.tags.map((tag) => tag.id)}
              allTags={allTags}
              onDone={() => setShowTagPicker(false)}
            />
          )}
        </div>
      )}
    </>
  );
}

/** A sticky day-group header with net sum. */
function GroupHeader({
  title,
  today,
  sum,
}: {
  title: string;
  today: boolean;
  sum: number;
}) {
  return (
    <div className={"lc-gh" + (today ? " lc-gh--today" : "")}>
      <span className="lc-gh__t">{title}</span>
      <span className="lc-gh__sum">{fmtSigned(sum)}</span>
    </div>
  );
}

/** A group of rows under a single GroupHeader. */
function Group({
  title,
  today,
  rows,
  openIds,
  toggle,
  onEdit,
  onDelete,
  allTags,
}: {
  title: string;
  today: boolean;
  rows: TransactionRow[];
  openIds: ReadonlySet<string>;
  toggle: (id: string) => void;
  onEdit: (t: TransactionRow) => void;
  onDelete: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const sum = rows.reduce((s, t) => s + signedCents(t), 0);
  return (
    <>
      <GroupHeader title={title} today={today} sum={sum} />
      {rows.map((t) => (
        <Row
          key={t.id}
          t={t}
          open={openIds.has(t.id)}
          onToggle={() => toggle(t.id)}
          onEdit={onEdit}
          onDelete={onDelete}
          allTags={allTags}
        />
      ))}
    </>
  );
}

/** Skeleton placeholder while loading. */
function Skeleton() {
  return (
    <div className="lc-card">
      <div className="lc-skeleton">
        {Array.from({ length: 7 }).map((_, i) => (
          <div
            key={i}
            className="lc-skel-row"
            style={{ width: `${60 + (i % 3) * 15}%` }}
          />
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Toolbar sub-component
// ---------------------------------------------------------------------------

function LcToolbar({
  view,
  mOffset,
  search,
  onViewChange,
  onMonthPrev,
  onMonthNext,
  onSearchChange,
  onNew,
}: {
  view: ViewMode;
  mOffset: number;
  search: string;
  onViewChange: (v: ViewMode) => void;
  onMonthPrev: () => void;
  onMonthNext: () => void;
  onSearchChange: (q: string) => void;
  onNew: () => void;
}) {
  // mOffset is read only for determining rendered label when needed; no direct use here
  void mOffset;
  return (
    <div className="lc-tools">
      <SegmentedControl
        size="sm"
        ariaLabel="Modo de visualização"
        value={view}
        onChange={(v) => onViewChange(v as ViewMode)}
        options={[
          { value: "monthOnly", label: "Por mês" },
          { value: "anchor", label: "Linha do tempo" },
        ]}
      />
      {view === "monthOnly" && (
        <div style={{ display: "inline-flex", gap: 4 }}>
          <button
            className="sh-iconbtn"
            onClick={onMonthPrev}
            aria-label="Mês anterior"
            type="button"
          >
            <ChevronRight
              size={15}
              strokeWidth={1.75}
              style={{ transform: "rotate(180deg)" }}
            />
          </button>
          <button
            className="sh-iconbtn"
            onClick={onMonthNext}
            aria-label="Próximo mês"
            type="button"
          >
            <ChevronRight size={15} strokeWidth={1.75} />
          </button>
        </div>
      )}
      <span className="lc-tools__sp" />
      <label className="lc-search">
        <Search size={14} strokeWidth={1.75} />
        <input
          placeholder="Buscar…"
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          aria-label="Buscar lançamentos"
        />
      </label>
      <Button
        size="sm"
        variant="primary"
        iconLeft={<Plus size={14} strokeWidth={1.75} />}
        onClick={onNew}
      >
        Novo
      </Button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Future banner sub-component
// ---------------------------------------------------------------------------

function FutureBanner({
  count,
  sum,
  expanded,
  onToggle,
}: {
  count: number;
  sum: number;
  expanded: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className="lc-future"
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      aria-expanded={expanded}
      aria-label="Lançamentos futuros já previstos"
    >
      <span className="lc-future__ic">
        <CalendarRange size={16} strokeWidth={1.75} />
      </span>
      <div>
        <div className="lc-future__t">{count} lançamentos futuros já previstos</div>
        <div className="lc-future__s">
          {expanded ? "Toque para recolher" : "Toque para ver. Não atrapalham aqui."}
        </div>
      </div>
      <span className="lc-future__amt">
        {fmtSigned(sum)}
        <br />
        <ChevronRight
          size={14}
          strokeWidth={1.75}
          style={expanded ? { transform: "rotate(90deg)" } : undefined}
        />
      </span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Anchor view sub-component
// ---------------------------------------------------------------------------

function AnchorView({
  futureRows,
  todayRows,
  pastRows,
  todayLabel,
  allRows,
  transactions,
  openIds,
  toggle,
  onEdit,
  onDelete,
  allTags,
}: {
  futureRows: TransactionRow[];
  todayRows: TransactionRow[];
  pastRows: TransactionRow[];
  todayLabel: string;
  allRows: TransactionRow[];
  transactions: TransactionRow[] | undefined;
  openIds: ReadonlySet<string>;
  toggle: (id: string) => void;
  onEdit: (t: TransactionRow) => void;
  onDelete: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const [showFuture, setShowFuture] = useState(false);
  const futureSum = futureRows.reduce((s, t) => s + signedCents(t), 0);

  return (
    <div className="lc-card">
      {futureRows.length > 0 && (
        <>
          <FutureBanner
            count={futureRows.length}
            sum={futureSum}
            expanded={showFuture}
            onToggle={() => setShowFuture((v) => !v)}
          />
          {showFuture &&
            groupByMonth(futureRows)
              .slice()
              .reverse()
              .map(([k, rows]) => (
                <Group
                  key={k}
                  title={"Futuro · " + monthLabel(k)}
                  today={false}
                  rows={rows.slice().sort((a, b) => (a.date < b.date ? -1 : 1))}
                  openIds={openIds}
                  toggle={toggle}
                  onEdit={onEdit}
                  onDelete={onDelete}
                  allTags={allTags}
                />
              ))}
        </>
      )}

      {/* Today section */}
      {todayRows.length > 0 ? (
        <Group
          title={todayLabel}
          today
          rows={todayRows}
          openIds={openIds}
          toggle={toggle}
          onEdit={onEdit}
          onDelete={onDelete}
          allTags={allTags}
        />
      ) : (
        <div className="lc-gh lc-gh--today">
          <span className="lc-gh__t">{todayLabel}</span>
          <span className="lc-gh__sum" style={{ color: "var(--text-faint)" }}>
            sem lançamentos
          </span>
        </div>
      )}

      {/* Past months */}
      {groupByMonth(pastRows).map(([k, rows]) => (
        <Group
          key={k}
          title={monthLabel(k)}
          today={false}
          rows={rows}
          openIds={openIds}
          toggle={toggle}
          onEdit={onEdit}
          onDelete={onDelete}
          allTags={allTags}
        />
      ))}

      {allRows.length === 0 && (
        <div className="lc-empty">
          {transactions?.length === 0
            ? "Importe sua planilha em Configurações para começar."
            : "Nenhum resultado para o filtro atual."}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Month-only view sub-component
// ---------------------------------------------------------------------------

function MonthView({
  targetKey,
  inMonthRows,
  openIds,
  toggle,
  onEdit,
  onDelete,
  allTags,
}: {
  targetKey: string;
  inMonthRows: TransactionRow[];
  openIds: ReadonlySet<string>;
  toggle: (id: string) => void;
  onEdit: (t: TransactionRow) => void;
  onDelete: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  return (
    <div className="lc-card">
      <Group
        title={monthLabel(targetKey)}
        today={targetKey === TODAY.slice(0, 7)}
        rows={inMonthRows}
        openIds={openIds}
        toggle={toggle}
        onEdit={onEdit}
        onDelete={onDelete}
        allTags={allTags}
      />
      {inMonthRows.length === 0 && (
        <div className="lc-empty">Nenhum lançamento neste mês para o filtro atual.</div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component state
// ---------------------------------------------------------------------------

interface LcState {
  view: ViewMode;
  filter: FilterKey;
  openIds: ReadonlySet<string>;
  mOffset: number;
  search: string;
}

type LcAction =
  | { type: "SET_VIEW"; view: ViewMode }
  | { type: "SET_FILTER"; filter: FilterKey }
  | { type: "TOGGLE_OPEN"; id: string }
  | { type: "SET_M_OFFSET"; delta: number }
  | { type: "SET_SEARCH"; search: string };

function lcReducer(state: LcState, action: LcAction): LcState {
  switch (action.type) {
    case "SET_VIEW":
      return { ...state, view: action.view };
    case "SET_FILTER":
      return { ...state, filter: action.filter };
    case "TOGGLE_OPEN": {
      const next = new Set(state.openIds);
      if (next.has(action.id)) next.delete(action.id);
      else next.add(action.id);
      return { ...state, openIds: next };
    }
    case "SET_M_OFFSET":
      return { ...state, mOffset: state.mOffset + action.delta };
    case "SET_SEARCH":
      return { ...state, search: action.search };
    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function TransactionsScreen() {
  const { openCompose } = useNekoApp();

  const {
    data: transactions,
    loading,
    error,
  } = useCommand("get_recent_transactions:lc", () =>
    getRecentTransactions(FETCH_LIMIT),
  );

  const { data: allTags } = useCommand("list_tags:lc", listTags);

  const [state, dispatch] = useReducer(lcReducer, {
    view: "monthOnly",
    filter: "todos",
    openIds: new Set<string>(),
    mOffset: 0,
    search: "",
  });

  const { view, filter, openIds, mOffset, search } = state;

  function toggle(id: string) {
    dispatch({ type: "TOGGLE_OPEN", id });
  }

  function handleEdit(t: TransactionRow) {
    openCompose({
      mode: "edit",
      transactionId: t.id,
      type: toMovementType(t),
      date: t.date,
      description: t.description,
      amountCents: Math.abs(t.amount),
      provenance: t.provenance,
    });
  }

  function handleNew() {
    openCompose({ mode: "new" });
  }

  // Derive filtered + searched rows (React Compiler caches these)
  const rows = transactions ?? [];
  const filteredByType =
    filter !== "todos" ? rows.filter((t) => toMovementType(t) === filter) : rows;
  const allRows: TransactionRow[] = search.trim()
    ? filteredByType.filter((t) => {
        const q = search.trim().toLowerCase();
        return t.description?.toLowerCase().includes(q) || t.date.includes(q);
      })
    : filteredByType;

  const futureRows = allRows.filter((t) => t.date > TODAY);
  const todayRows = allRows.filter((t) => t.date === TODAY);
  const pastRows = allRows.filter((t) => t.date < TODAY);

  // Today label for group header
  const todayParts = TODAY.split("-");
  const todayDay = parseInt(todayParts[2] ?? "1", 10);
  const todayMonthName = MES[monthOf(TODAY)]?.toLowerCase() ?? "";
  const todayLabel = `Hoje · ${todayDay} de ${todayMonthName}`;

  // Month-only mode: which month to show
  const currentMonthIdx = currentMonthIndex();
  const targetMonthIdx = currentMonthIdx + mOffset;
  // Wrap around 0-11 for the month, adjust year for offset
  const targetDate = new Date(new Date().getFullYear(), targetMonthIdx, 1);
  const targetYear = String(targetDate.getFullYear());
  const targetMonth = String(targetDate.getMonth() + 1).padStart(2, "0");
  const targetKey = `${targetYear}-${targetMonth}`;
  const inMonthRows = allRows.filter((t) => monthKey(t.date) === targetKey);

  // Web-preview fallback
  if (!isTauri) {
    return (
      <div className="lc">
        <div className="lc-card">
          <div className="lc-empty">
            Preview web — abra o app desktop para ver seus lançamentos.
          </div>
        </div>
      </div>
    );
  }

  // -------------------------------------------------------------------------
  // Content area
  // -------------------------------------------------------------------------

  let content: React.ReactNode;

  if (loading && !transactions) {
    content = <Skeleton />;
  } else if (error && !transactions) {
    content = (
      <div className="lc-card">
        <div className="lc-empty">
          Não foi possível carregar os lançamentos.{" "}
          <button
            type="button"
            className="lc-retry-btn"
            onClick={() => {
              invalidateCommands();
            }}
          >
            Tentar novamente
          </button>
        </div>
      </div>
    );
  } else if (view === "monthOnly") {
    content = (
      <MonthView
        targetKey={targetKey}
        inMonthRows={inMonthRows}
        openIds={openIds}
        toggle={toggle}
        onEdit={handleEdit}
        onDelete={handleDelete}
        allTags={allTags ?? []}
      />
    );
  } else {
    content = (
      <AnchorView
        futureRows={futureRows}
        todayRows={todayRows}
        pastRows={pastRows}
        todayLabel={todayLabel}
        allRows={allRows}
        transactions={transactions}
        openIds={openIds}
        toggle={toggle}
        onEdit={handleEdit}
        onDelete={handleDelete}
        allTags={allTags ?? []}
      />
    );
  }

  return (
    <div className="lc">
      {/* Toolbar */}
      <LcToolbar
        view={view}
        mOffset={mOffset}
        search={search}
        onViewChange={(v) => dispatch({ type: "SET_VIEW", view: v })}
        onMonthPrev={() => dispatch({ type: "SET_M_OFFSET", delta: -1 })}
        onMonthNext={() => dispatch({ type: "SET_M_OFFSET", delta: 1 })}
        onSearchChange={(q) => dispatch({ type: "SET_SEARCH", search: q })}
        onNew={handleNew}
      />

      {/* Filter chips */}
      <div className="lc-filters">
        {FILTER_CHIPS.map((f) => (
          <button
            key={f.key}
            type="button"
            className={"lc-fchip" + (filter === f.key ? " is-on" : "")}
            onClick={() => dispatch({ type: "SET_FILTER", filter: f.key })}
            style={
              filter === f.key
                ? { background: `color-mix(in srgb,${f.color} 16%, transparent)` }
                : undefined
            }
          >
            <span className="lc-fchip__dot" style={{ background: f.color }} />
            {f.label}
          </button>
        ))}
      </div>

      {content}

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
