import { useEffect, useReducer, useRef } from "react";
import { Check, Pencil, Plus, Table2, X } from "lucide-react";
import { Button } from "../design-system/components/Button";
import {
  createTransaction,
  getLineItems,
  getPockets,
  isTauri,
  updateTransaction,
  updateTransactionItems,
  type PocketAccount,
} from "../lib/api";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { formatBRL, parseBRLToCents, todayISO } from "../lib/format";
import { safeErrorMessage } from "../lib/errors";
import { kindToFields } from "../lib/movement";
import { fmtBRL, TYPE_META, type MovementType } from "../lib/nkFormat";
import type { ComposeOptions } from "./appContext";

interface Part {
  desc: string;
  amt: string;
}

const TYPES: MovementType[] = ["entrada", "saida", "diario", "cartao", "economia"];

/** Converts magnitude cents → pt-BR input string ("1.234,56"). */
function centsToInput(cents: number): string {
  // formatBRL returns "R$ 1.234,56"; strip the currency prefix for the input field.
  return formatBRL(Math.abs(cents))
    .replace(/^[R$\s−]+/, "")
    .trim();
}

interface LineItemInput {
  amount_cents: number;
  description: string;
  position: number;
}

/** Persiste o lançamento (edição ou criação) + itens. Module-level de propósito: o React
 *  Compiler não compila `try` dentro de componentes, então a sequência de awaits vive aqui e
 *  o componente só encadeia .then/.catch/.finally. */
async function persistLancamento(p: {
  transactionId?: string | undefined;
  fields: {
    txnType: "income" | "expense" | "transfer";
    amountCents: number;
    description: string | null;
    date: string;
    paymentMethod: string | null;
    isFixed: boolean;
  };
  toAccountId: string | null;
  items: LineItemInput[] | null;
}): Promise<void> {
  if (p.transactionId) {
    await updateTransaction(p.transactionId, p.fields);
    if (p.items) await updateTransactionItems(p.transactionId, p.items);
  } else {
    const newId = await createTransaction({
      ...p.fields,
      tagIds: [],
      recurrence: null,
      toAccountId: p.toAccountId,
    });
    if (p.items) await updateTransactionItems(newId, p.items);
  }
}

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

interface ComposeState {
  type: MovementType;
  date: string;
  desc: string;
  composed: boolean;
  single: string;
  parts: Part[];
  toAccountId: string;
  saving: boolean;
  loadingItems: boolean;
  error: string | null;
}

type ComposeAction =
  | { kind: "set_type"; value: MovementType }
  | { kind: "set_date"; value: string }
  | { kind: "set_desc"; value: string }
  | { kind: "set_composed"; value: boolean }
  | { kind: "set_single"; value: string }
  | { kind: "set_parts"; parts: Part[] }
  | { kind: "set_to_account"; value: string }
  | { kind: "set_saving"; value: boolean }
  | { kind: "items_loaded"; parts: Part[]; composed: boolean }
  | { kind: "save_started" }
  | { kind: "save_failed"; message: string };

function composeReducer(state: ComposeState, action: ComposeAction): ComposeState {
  switch (action.kind) {
    case "set_type":
      return { ...state, type: action.value };
    case "set_date":
      return { ...state, date: action.value };
    case "set_desc":
      return { ...state, desc: action.value };
    case "set_composed":
      return { ...state, composed: action.value };
    case "set_single":
      return { ...state, single: action.value };
    case "set_parts":
      return { ...state, parts: action.parts };
    case "set_to_account":
      return { ...state, toAccountId: action.value };
    case "set_saving":
      return { ...state, saving: action.value };
    case "save_started":
      return { ...state, saving: true, error: null };
    case "save_failed":
      return { ...state, saving: false, error: action.message };
    case "items_loaded":
      // Combines loadingItems=false + composed + parts in one update (no-cascading-set-state).
      return {
        ...state,
        loadingItems: false,
        composed: action.composed,
        parts: action.parts,
      };
    default:
      return state;
  }
}

function makeInitialState(options: ComposeOptions): ComposeState {
  return {
    type: options.type ?? "diario",
    date: options.date ?? todayISO(),
    desc: options.description ?? "",
    composed: false,
    single:
      options.amountCents != null && options.amountCents > 0
        ? centsToInput(options.amountCents)
        : "",
    parts: [{ desc: "", amt: "" }],
    toAccountId: "",
    saving: false,
    loadingItems: false,
    error: null,
  };
}

// ---------------------------------------------------------------------------
// Sub-components (same file — splits no-giant-component)
// ---------------------------------------------------------------------------

function ComposeTypeRow({
  type,
  onSelect,
}: {
  type: MovementType;
  onSelect: (k: MovementType) => void;
}) {
  return (
    <div>
      <span className="cmp-label">Tipo de movimento</span>
      <div className="cmp-types">
        {TYPES.map((k) => {
          const tm = TYPE_META[k];
          const on = type === k;
          return (
            <button
              type="button"
              key={k}
              className={"cmp-type" + (on ? " is-on" : "")}
              onClick={() => onSelect(k)}
              style={
                on
                  ? {
                      background: `color-mix(in srgb, ${tm.color} 18%, transparent)`,
                      color: "var(--text-strong)",
                    }
                  : undefined
              }
            >
              <span className="cmp-type__dot" style={{ background: tm.color }} />
              {tm.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function ComposePartsEditor({
  parts,
  onChangePart,
  onAdd,
  onRemove,
}: {
  parts: Part[];
  onChangePart: (i: number, k: keyof Part, v: string) => void;
  onAdd: () => void;
  onRemove: (i: number) => void;
}) {
  return (
    <div className="cmp-parts">
      {parts.map((p, i) => {
        // Stable key: prefer a non-empty field; fall back to positional suffix.
        const stableKey = p.desc || p.amt ? `${p.desc}-${p.amt}-${i}` : `part-${i}`;
        return (
          <div className="cmp-part" key={stableKey}>
            <input
              className="cmp-field cmp-field--money cmp-part__amt"
              inputMode="decimal"
              placeholder="R$ 0,00"
              value={p.amt}
              onChange={(e) => onChangePart(i, "amt", e.target.value)}
              aria-label={`Valor do item ${i + 1}`}
            />
            <input
              className="cmp-field cmp-part__desc"
              placeholder="O que é esse item?"
              value={p.desc}
              onChange={(e) => onChangePart(i, "desc", e.target.value)}
              aria-label={`Descrição do item ${i + 1}`}
            />
            <button
              type="button"
              className="cmp-part__rm"
              onClick={() => onRemove(i)}
              aria-label={`Remover item ${i + 1}`}
            >
              <X size={15} strokeWidth={1.75} />
            </button>
          </div>
        );
      })}
      <button type="button" className="cmp-add" onClick={onAdd}>
        <Plus size={14} strokeWidth={2} />
        Adicionar item
      </button>
    </div>
  );
}

function ComposePrevPanel({ total, noteText }: { total: number; noteText: string }) {
  return (
    <div className="cmp-prev">
      <div className="cmp-prev__head">
        <Table2 size={13} strokeWidth={1.75} />
        Como cai na planilha
      </div>
      <div className="cmp-prev__cell">
        <span className="cmp-prev__cell-label">Valor da célula</span>
        <span className="cmp-prev__cell-val">{fmtBRL(total)}</span>
      </div>
      <div style={{ padding: "12px 14px" }}>
        <div className="cmp-prev__noteh">
          <Pencil size={12} strokeWidth={1.75} />
          Nota da célula
        </div>
        <div
          className="cmp-prev__note"
          style={{ background: "transparent", padding: 0 }}
        >
          {noteText || "—"}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Drawer shell sub-component (splits no-giant-component from Compose)
// ---------------------------------------------------------------------------

function ComposeDrawer({
  open,
  isEditMode,
  state,
  reserveAccounts,
  effectiveParts,
  total,
  sign,
  totalColor,
  noteText,
  canSave,
  dispatch,
  onSave,
  onClose,
}: {
  open: boolean;
  isEditMode: boolean;
  state: ComposeState;
  reserveAccounts: PocketAccount[];
  effectiveParts: Part[];
  total: number;
  sign: string;
  totalColor: string;
  noteText: string;
  canSave: boolean;
  dispatch: React.Dispatch<ComposeAction>;
  onSave: () => void;
  onClose: () => void;
}) {
  const { type, date, desc, composed, single, parts, toAccountId } = state;
  const meta = TYPE_META[type];

  const setPart = (i: number, k: keyof Part, v: string) =>
    dispatch({
      kind: "set_parts",
      parts: parts.map((p, j) => (j === i ? { ...p, [k]: v } : p)),
    });
  const addPart = () =>
    dispatch({ kind: "set_parts", parts: [...parts, { desc: "", amt: "" }] });
  const rmPart = (i: number) =>
    dispatch({
      kind: "set_parts",
      parts: parts.length > 1 ? parts.filter((_, j) => j !== i) : parts,
    });

  return (
    <aside className={"cmp" + (open ? " is-open" : "")}>
      <div className="cmp-head">
        <span className="cmp-head__icon" style={{ background: meta.color }}>
          {meta.glyph}
        </span>
        <div style={{ flex: 1 }}>
          <div className="cmp-head__t">
            {isEditMode ? "Editar lançamento" : "Novo lançamento"}
          </div>
          <div className="cmp-head__s">
            {isEditMode
              ? "Atualiza o lançamento local · Leve à planilha pelo write-back"
              : "Salva no app · Leve à planilha pelo write-back em Configurações"}
          </div>
        </div>
        <button
          type="button"
          className="sh-iconbtn"
          onClick={onClose}
          aria-label="Fechar"
        >
          <X size={17} strokeWidth={1.75} />
        </button>
      </div>

      <div className="cmp-body">
        <ComposeTypeRow
          type={type}
          onSelect={(k) => dispatch({ kind: "set_type", value: k })}
        />

        <div className="cmp-date-desc-row">
          <div className="cmp-date-col">
            <span className="cmp-label">Data</span>
            <input
              type="date"
              className="cmp-field"
              value={date}
              onChange={(e) => dispatch({ kind: "set_date", value: e.target.value })}
              aria-label="Data"
            />
          </div>
          <div style={{ flex: 1 }}>
            <span className="cmp-label">Descrição</span>
            <input
              className="cmp-field"
              placeholder="Ex.: Contas fixas, Salário…"
              value={desc}
              onChange={(e) => dispatch({ kind: "set_desc", value: e.target.value })}
              aria-label="Descrição"
            />
          </div>
        </div>

        {type === "economia" && (
          <div>
            <span className="cmp-label">Conta de reserva (destino)</span>
            <select
              className="cmp-field"
              value={toAccountId}
              onChange={(e) =>
                dispatch({ kind: "set_to_account", value: e.target.value })
              }
              aria-label="Conta de reserva"
            >
              <option value="">Escolha a conta…</option>
              {reserveAccounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </div>
        )}

        <div>
          <div className="cmp-valor-row">
            <span className="cmp-label" style={{ margin: 0 }}>
              Valor
            </span>
            <div className="cmp-modeswitch">
              <button
                type="button"
                className={!composed ? "is-on" : ""}
                onClick={() => dispatch({ kind: "set_composed", value: false })}
              >
                Único
              </button>
              <button
                type="button"
                className={composed ? "is-on" : ""}
                onClick={() => dispatch({ kind: "set_composed", value: true })}
              >
                Compor por itens
              </button>
            </div>
          </div>

          {!composed ? (
            <input
              className="cmp-field cmp-field--money cmp-field--big"
              inputMode="decimal"
              placeholder="R$ 0,00"
              value={single}
              onChange={(e) => dispatch({ kind: "set_single", value: e.target.value })}
              aria-label="Valor único"
            />
          ) : (
            <ComposePartsEditor
              parts={parts}
              onChangePart={setPart}
              onAdd={addPart}
              onRemove={rmPart}
            />
          )}
        </div>

        <div className="cmp-total">
          <span className="cmp-total__l">
            {composed && effectiveParts.length >= 2
              ? `Soma de ${effectiveParts.length} itens`
              : "Total do lançamento"}
          </span>
          <span className="cmp-total__v" style={{ color: totalColor }}>
            {sign} {fmtBRL(total)}
          </span>
        </div>

        <ComposePrevPanel total={total} noteText={noteText} />
      </div>

      {state.error && (
        <p role="alert" className="cmp-error">
          {state.error}
        </p>
      )}
      <div className="cmp-foot">
        <Button
          variant="primary"
          iconLeft={<Check size={15} strokeWidth={2} />}
          onClick={onSave}
          disabled={!canSave}
        >
          {isEditMode ? "Salvar alterações" : "Salvar lançamento"}
        </Button>
        <Button variant="ghost" onClick={onClose}>
          Cancelar
        </Button>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// Main component (state management + effects only; markup delegated to sub-components)
// ---------------------------------------------------------------------------

export function Compose({
  open,
  options,
  onClose,
  onSaved,
}: {
  open: boolean;
  options: ComposeOptions;
  onClose: () => void;
  onSaved: () => void;
}) {
  const isEditMode = options.mode === "edit";
  const dialogRef = useRef<HTMLDialogElement>(null);

  // App remonta o Compose (via `key`) a cada abertura → os inicializadores leem as opções frescas.
  const [state, dispatch] = useReducer(composeReducer, options, makeInitialState);
  const { type, composed, single, parts, desc, toAccountId, saving, loadingItems } =
    state;

  const pocketsQ = useCommand("get_pockets", getPockets);
  const reserveAccounts: PocketAccount[] = (pocketsQ.data?.accounts ?? []).filter(
    (a) => a.liquidity === "reserve" || a.liquidity === "illiquid",
  );

  // Drive the native dialog open/close state.
  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    // react-doctor-disable-next-line react-doctor/no-event-handler -- showModal/close are imperative-only DOM APIs; no declarative React equivalent exists for <dialog>
    if (open && !el.open) el.showModal();
    // react-doctor-disable-next-line react-doctor/no-event-handler -- same as above: close() is the only API to dismiss a native dialog imperatively
    if (!open && el.open) el.close();
  }, [open]);

  // Handle native cancel (Esc) and backdrop click → onClose.
  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    const onCancel = (e: Event) => {
      e.preventDefault();
      onClose();
    };
    const onClick = (e: MouseEvent) => {
      if (e.target === el) onClose();
    };
    el.addEventListener("cancel", onCancel);
    el.addEventListener("click", onClick);
    return () => {
      el.removeEventListener("cancel", onCancel);
      el.removeEventListener("click", onClick);
    };
  }, [onClose]);

  // In edit mode: load existing line items and pre-fill composed state.
  useEffect(() => {
    if (!isEditMode || !options.transactionId || !isTauri) return;
    void Promise.resolve().then(() => {
      dispatch({
        kind: "items_loaded",
        composed: false,
        parts: [{ desc: "", amt: "" }],
      });
      return getLineItems(options.transactionId!)
        .then((items) => {
          if (items.length >= 2) {
            const loadedParts = items
              .slice()
              .sort((a, b) => a.position - b.position)
              .map((li) => ({
                desc: li.description,
                amt: centsToInput(li.amount_cents),
              }));
            dispatch({ kind: "items_loaded", composed: true, parts: loadedParts });
          } else {
            dispatch({
              kind: "items_loaded",
              composed: false,
              parts: [{ desc: "", amt: "" }],
            });
          }
        })
        .catch(() => {
          dispatch({
            kind: "items_loaded",
            composed: false,
            parts: [{ desc: "", amt: "" }],
          });
        });
    });
  }, [isEditMode, options.transactionId]);

  const meta = TYPE_META[type];
  const total = composed
    ? parts.reduce((s, p) => s + (parseBRLToCents(p.amt) ?? 0), 0)
    : (parseBRLToCents(single) ?? 0);
  const sign = type === "entrada" ? "+" : "−";
  const totalColor = type === "entrada" ? "var(--money-pos)" : "var(--money-neg)";

  // React Compiler caches this — no useMemo needed (react-compiler-no-manual-memoization).
  const effectiveParts = composed
    ? parts.filter((p) => (parseBRLToCents(p.amt) ?? 0) > 0)
    : (() => {
        const c = parseBRLToCents(single) ?? 0;
        return c > 0 ? [{ desc: desc || meta.name, amt: single }] : [];
      })();

  const noteText =
    effectiveParts.length > 0
      ? effectiveParts
          .map(
            (p) =>
              `${fmtBRL(parseBRLToCents(p.amt) ?? 0)} - ${p.desc || "(sem descrição)"}`,
          )
          .join("\n") +
        (effectiveParts.length >= 2 ? `\n\nTotal = ${fmtBRL(total)}` : "")
      : "";

  const economiaNeedsAccount = type === "economia" && toAccountId === "";
  const canSave =
    isTauri && total > 0 && !economiaNeedsAccount && !saving && !loadingItems;

  function save() {
    if (!canSave) return;
    const m = kindToFields(type);
    const items =
      composed && effectiveParts.length >= 1
        ? effectiveParts.map((p, i) => ({
            amount_cents: parseBRLToCents(p.amt) ?? 0,
            description: p.desc || "",
            position: i,
          }))
        : null;
    dispatch({ kind: "save_started" });
    persistLancamento({
      transactionId:
        isEditMode && options.transactionId ? options.transactionId : undefined,
      fields: {
        txnType: m.txnType,
        amountCents: total,
        description: desc || null,
        date: state.date,
        paymentMethod: m.paymentMethod,
        isFixed: m.isFixed,
      },
      toAccountId: type === "economia" ? toAccountId : null,
      items,
    })
      .then(() => {
        invalidateCommands();
        onSaved();
        onClose();
        dispatch({ kind: "set_saving", value: false });
      })
      .catch((e) =>
        dispatch({
          kind: "save_failed",
          message: safeErrorMessage(
            e,
            "Não foi possível salvar o lançamento. Tente novamente.",
          ),
        }),
      );
  }

  return (
    <dialog
      ref={dialogRef}
      className="cmp-dialog neko-app"
      aria-label={isEditMode ? "Editar lançamento" : "Novo lançamento"}
    >
      <ComposeDrawer
        open={open}
        isEditMode={isEditMode}
        state={state}
        reserveAccounts={reserveAccounts}
        effectiveParts={effectiveParts}
        total={total}
        sign={sign}
        totalColor={totalColor}
        noteText={noteText}
        canSave={canSave}
        dispatch={dispatch}
        onSave={() => void save()}
        onClose={onClose}
      />
    </dialog>
  );
}
