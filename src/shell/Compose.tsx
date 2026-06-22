import { useEffect, useMemo, useState } from "react";
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
import { fmtBRL, TYPE_META, type MovementType } from "../lib/nkFormat";
import type { ComposeOptions } from "./appContext";

interface Part {
  desc: string;
  amt: string;
}

const TYPES: MovementType[] = ["entrada", "saida", "diario", "cartao", "economia"];

/** txnType + flags por tipo de movimento (espelha NewTransactionForm). */
function mapType(t: MovementType): {
  txnType: "income" | "expense" | "transfer";
  isFixed: boolean;
  paymentMethod: string | null;
} {
  switch (t) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: null };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credito" };
    case "economia":
      return { txnType: "transfer", isFixed: false, paymentMethod: null };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: null };
  }
}

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

  // App remonta o Compose (via `key`) a cada abertura → os inicializadores leem as opções frescas.
  const [type, setType] = useState<MovementType>(options.type ?? "diario");
  const [date, setDate] = useState(options.date ?? todayISO());
  const [desc, setDesc] = useState(options.description ?? "");
  const [composed, setComposed] = useState(false);
  const [single, setSingle] = useState(
    options.amountCents != null && options.amountCents > 0
      ? centsToInput(options.amountCents)
      : "",
  );
  const [parts, setParts] = useState<Part[]>([{ desc: "", amt: "" }]);
  const [toAccountId, setToAccountId] = useState("");
  const [saving, setSaving] = useState(false);
  const [loadingItems, setLoadingItems] = useState(false);

  const pocketsQ = useCommand("get_pockets", getPockets);
  const reserveAccounts: PocketAccount[] = (pocketsQ.data?.accounts ?? []).filter(
    (a) => a.liquidity === "reserve" || a.liquidity === "illiquid",
  );

  // In edit mode: load existing line items and pre-fill composed state.
  useEffect(() => {
    if (!isEditMode || !options.transactionId || !isTauri) return;
    // Wrap in Promise.resolve so setState calls happen inside async callbacks,
    // avoiding the react-hooks/set-state-in-effect synchronous-in-body constraint.
    void Promise.resolve().then(() => {
      setLoadingItems(true);
      return getLineItems(options.transactionId!)
        .then((items) => {
          if (items.length >= 2) {
            setComposed(true);
            setParts(
              items
                .slice()
                .sort((a, b) => a.position - b.position)
                .map((li) => ({
                  desc: li.description,
                  amt: centsToInput(li.amount_cents),
                })),
            );
          }
          // If 0 or 1 item, keep single-value mode (already set from amountCents in options).
        })
        .catch(() => {
          // Non-fatal: keep the simple single-value pre-fill.
        })
        .finally(() => setLoadingItems(false));
    });
  }, [isEditMode, options.transactionId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const meta = TYPE_META[type];
  const total = composed
    ? parts.reduce((s, p) => s + (parseBRLToCents(p.amt) ?? 0), 0)
    : (parseBRLToCents(single) ?? 0);
  const sign = type === "entrada" ? "+" : "−";
  const totalColor = type === "entrada" ? "var(--money-pos)" : "var(--money-neg)";

  const effectiveParts = useMemo(() => {
    if (composed) return parts.filter((p) => (parseBRLToCents(p.amt) ?? 0) > 0);
    const c = parseBRLToCents(single) ?? 0;
    return c > 0 ? [{ desc: desc || meta.name, amt: single }] : [];
  }, [composed, parts, single, desc, meta.name]);

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

  const setPart = (i: number, k: keyof Part, v: string) =>
    setParts((ps) => ps.map((p, j) => (j === i ? { ...p, [k]: v } : p)));
  const addPart = () => setParts((ps) => [...ps, { desc: "", amt: "" }]);
  const rmPart = (i: number) =>
    setParts((ps) => (ps.length > 1 ? ps.filter((_, j) => j !== i) : ps));

  function save() {
    if (!canSave) return;
    const m = mapType(type);
    const items =
      composed && effectiveParts.length >= 1
        ? effectiveParts.map((p, i) => ({
            amount_cents: parseBRLToCents(p.amt) ?? 0,
            description: p.desc || "",
            position: i,
          }))
        : null;
    setSaving(true);
    persistLancamento({
      transactionId:
        isEditMode && options.transactionId ? options.transactionId : undefined,
      fields: {
        txnType: m.txnType,
        amountCents: total,
        description: desc || null,
        date,
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
      })
      .catch(() => undefined)
      .finally(() => setSaving(false));
  }

  return (
    <>
      <div className={"cmp-scrim" + (open ? " is-open" : "")} onClick={onClose} />
      <aside
        className={"cmp neko-app" + (open ? " is-open" : "")}
        role="dialog"
        aria-label={isEditMode ? "Editar lançamento" : "Novo lançamento"}
        aria-hidden={!open}
      >
        <div className="cmp-head">
          <span
            style={{
              width: 34,
              height: 34,
              borderRadius: 9,
              background: meta.color,
              color: "var(--text-on-primary)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontWeight: 800,
              fontSize: 16,
              flex: "none",
            }}
          >
            {meta.glyph}
          </span>
          <div style={{ flex: 1 }}>
            <div className="cmp-head__t">
              {isEditMode ? "Editar lançamento" : "Novo lançamento"}
            </div>
            <div className="cmp-head__s">
              {isEditMode
                ? "Atualiza o lançamento local · reflita no write-back"
                : "Grava 1:1 na planilha · precisa da sua confirmação"}
            </div>
          </div>
          <button className="sh-iconbtn" onClick={onClose} aria-label="Fechar">
            <X size={17} strokeWidth={1.75} />
          </button>
        </div>

        <div className="cmp-body">
          <div>
            <span className="cmp-label">Tipo de movimento</span>
            <div className="cmp-types">
              {TYPES.map((k) => {
                const tm = TYPE_META[k];
                const on = type === k;
                return (
                  <button
                    key={k}
                    className={"cmp-type" + (on ? " is-on" : "")}
                    onClick={() => setType(k)}
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

          <div style={{ display: "flex", gap: 12 }}>
            <div style={{ width: 160 }}>
              <span className="cmp-label">Data</span>
              <input
                type="date"
                className="cmp-field"
                value={date}
                onChange={(e) => setDate(e.target.value)}
                aria-label="Data"
              />
            </div>
            <div style={{ flex: 1 }}>
              <span className="cmp-label">Descrição</span>
              <input
                className="cmp-field"
                placeholder="Ex.: Contas fixas, Salário…"
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
              />
            </div>
          </div>

          {type === "economia" && (
            <div>
              <span className="cmp-label">Conta de reserva (destino)</span>
              <select
                className="cmp-field"
                value={toAccountId}
                onChange={(e) => setToAccountId(e.target.value)}
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
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                marginBottom: 10,
              }}
            >
              <span className="cmp-label" style={{ margin: 0 }}>
                Valor
              </span>
              <div className="cmp-modeswitch">
                <button
                  className={!composed ? "is-on" : ""}
                  onClick={() => setComposed(false)}
                >
                  Único
                </button>
                <button
                  className={composed ? "is-on" : ""}
                  onClick={() => setComposed(true)}
                >
                  Compor por itens
                </button>
              </div>
            </div>

            {!composed ? (
              <input
                className="cmp-field cmp-field--money"
                inputMode="decimal"
                placeholder="R$ 0,00"
                value={single}
                onChange={(e) => setSingle(e.target.value)}
                style={{ fontSize: 18, height: 48 }}
              />
            ) : (
              <div className="cmp-parts">
                {parts.map((p, i) => (
                  <div className="cmp-part" key={i}>
                    <input
                      className="cmp-field cmp-field--money cmp-part__amt"
                      inputMode="decimal"
                      placeholder="R$ 0,00"
                      value={p.amt}
                      onChange={(e) => setPart(i, "amt", e.target.value)}
                      aria-label={`Valor do item ${i + 1}`}
                    />
                    <input
                      className="cmp-field cmp-part__desc"
                      placeholder="O que é esse item?"
                      value={p.desc}
                      onChange={(e) => setPart(i, "desc", e.target.value)}
                      aria-label={`Descrição do item ${i + 1}`}
                    />
                    <button
                      className="cmp-part__rm"
                      onClick={() => rmPart(i)}
                      aria-label={`Remover item ${i + 1}`}
                    >
                      <X size={15} strokeWidth={1.75} />
                    </button>
                  </div>
                ))}
                <button className="cmp-add" onClick={addPart}>
                  <Plus size={14} strokeWidth={2} />
                  Adicionar item
                </button>
              </div>
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

          <div className="cmp-prev">
            <div className="cmp-prev__head">
              <Table2 size={13} strokeWidth={1.75} />
              Como cai na planilha
            </div>
            <div className="cmp-prev__cell">
              <span
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  letterSpacing: ".05em",
                  textTransform: "uppercase",
                  color: "var(--text-faint)",
                }}
              >
                Valor da célula
              </span>
              <span
                style={{
                  fontFamily: "var(--font-money)",
                  fontVariantNumeric: "tabular-nums",
                  fontWeight: 700,
                  fontSize: 18,
                  color: "var(--text-strong)",
                }}
              >
                {fmtBRL(total)}
              </span>
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
        </div>

        <div className="cmp-foot">
          <Button
            variant="primary"
            iconLeft={<Check size={15} strokeWidth={2} />}
            onClick={() => void save()}
            disabled={!canSave}
          >
            {isEditMode ? "Salvar alterações" : "Salvar lançamento"}
          </Button>
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
        </div>
      </aside>
    </>
  );
}
