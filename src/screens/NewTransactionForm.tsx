import { useEffect, useReducer, useState, type Dispatch } from "react";
import { Button } from "../design-system/components/Button";
import { MovBadge, type MovKind } from "../design-system/components/MovBadge";
import {
  createTransaction,
  getPockets,
  listTags,
  updateSeriesAll,
  updateSeriesFrom,
  updateTransaction,
  updateTransactionItems,
  type Frequency,
  type LineItem,
  type LineItemDraft,
  type PocketAccount,
  type SeriesEdit,
  type Tag,
} from "../lib/api";
import { LineItemEditor } from "../design-system/components/LineItemEditor";
import { safeErrorMessage } from "../lib/errors";
import { parseBRLToCents, todayISO } from "../lib/format";
import { FORM_KINDS, fieldsToKind, kindToFields } from "../lib/movement";

/** Valores de um lançamento existente para pré-preencher o form no modo edição. */
export interface TransactionEditValues {
  id: string;
  type: string;
  amount: number; // centavos, magnitude positiva
  description: string;
  date: string;
  payment_method: string | null;
  is_fixed: boolean;
  /** Prefixo uuid da série (derivado do id "uuid:index"); null = lançamento único. */
  recurrence_id: string | null;
  /** Partes itemizadas já persistidas (plano 036). Ausente/vazio = lançamento sem partes. */
  items?: LineItem[];
}

/** `LineItem` (persistido) → `LineItemDraft` (editável no form). Descarta id/transaction_id. */
function toDrafts(items: LineItem[] | undefined): LineItemDraft[] {
  return (items ?? []).map((it, i) => ({
    amount_cents: it.amount_cents,
    description: it.description,
    position: i,
  }));
}

/** Centavos → string editável pt-BR ("1234,50"), que volta limpo por `parseBRLToCents`. */
function centsToInput(cents: number): string {
  return (cents / 100).toFixed(2).replace(".", ",");
}

const FREQ_LABELS: Record<Frequency, string> = {
  diaria: "por dia",
  semanal: "por semana",
  mensal: "por mês",
};

const field: React.CSSProperties = {
  width: "100%",
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-body)",
};

const label: React.CSSProperties = {
  display: "block",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  marginBottom: "var(--space-1)",
};

// Base estática dos chips de seleção (tipo / tag); fundo+borda do estado ativo entram por merge.
const KIND_BTN_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "var(--space-2)",
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-sm)",
};

const TAG_BTN_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  height: 28,
  padding: "0 10px",
  borderRadius: "var(--radius-pill)",
  cursor: "pointer",
  color: "var(--text)",
  fontSize: "var(--fs-micro)",
  fontFamily: "var(--font-sans)",
};

// Texto auxiliar (dica). Hoisted para o React Compiler não recriar o objeto a cada render.
const HINT_TEXT: React.CSSProperties = {
  margin: "var(--space-2) 0 0",
  fontSize: "var(--fs-micro)",
  color: "var(--text-faint)",
};

const HINT_EMPTY: React.CSSProperties = {
  margin: 0,
  fontSize: "var(--fs-sm)",
  color: "var(--text-faint)",
};

// Estado do formulário agrupado num reducer (uma atualização lógica = um render), em vez de dez
// useState relacionados. A lista de tags disponíveis (carregada por IO) fica num useState à parte.
interface FormState {
  kind: MovKind;
  amount: string;
  description: string;
  date: string;
  selectedTags: string[];
  /** Conta-destino da Economia (transfer): id de uma conta reserve/illiquid. Vazio fora do kind. */
  toAccountId: string;
  repeat: boolean;
  frequency: Frequency;
  repetitions: number;
  /** Partes itemizadas (plano 036). Com ≥1 parte, o Valor vira somente-leitura (= Σ partes). */
  items: LineItemDraft[];
  busy: boolean;
  error: string | null;
}

function makeInitialForm(initial?: TransactionEditValues): FormState {
  if (initial) {
    return {
      kind: fieldsToKind(initial.type, initial.is_fixed, initial.payment_method),
      amount: centsToInput(initial.amount),
      description: initial.description,
      date: initial.date,
      selectedTags: [],
      toAccountId: "",
      repeat: false,
      frequency: "mensal",
      repetitions: 12,
      items: toDrafts(initial.items),
      busy: false,
      error: null,
    };
  }
  return {
    kind: "diario",
    amount: "",
    description: "",
    date: todayISO(),
    selectedTags: [],
    toAccountId: "",
    repeat: false,
    frequency: "mensal",
    repetitions: 12,
    items: [],
    busy: false,
    error: null,
  };
}

type FormAction =
  | { type: "set"; patch: Partial<FormState> }
  | { type: "toggleTag"; id: string }
  | { type: "setItems"; items: LineItemDraft[] }
  | { type: "submitStart" }
  | { type: "submitSuccess" }
  | { type: "fail"; error: string };

function formReducer(s: FormState, a: FormAction): FormState {
  switch (a.type) {
    case "setItems":
      return { ...s, items: a.items };
    case "set":
      // Trocar de tipo de movimento limpa a conta-destino: ela só faz sentido para Economia, e um
      // valor remanescente não deve poluir os outros tipos (o backend já ignora, isto é só higiene).
      return a.patch.kind !== undefined && a.patch.kind !== "economia"
        ? { ...s, ...a.patch, toAccountId: "" }
        : { ...s, ...a.patch };
    case "toggleTag":
      return {
        ...s,
        selectedTags: s.selectedTags.includes(a.id)
          ? s.selectedTags.filter((x) => x !== a.id)
          : [...s.selectedTags, a.id],
      };
    case "submitStart":
      return { ...s, busy: true, error: null };
    case "submitSuccess":
      // Reset dos campos voláteis; mantém tipo e data para lançamentos em sequência.
      return {
        ...s,
        amount: "",
        description: "",
        selectedTags: [],
        repeat: false,
        items: [],
        busy: false,
      };
    case "fail":
      return { ...s, busy: false, error: a.error };
  }
}

/** Seleção de tags (só no modo criação). Cada chip alterna a tag no estado do form. */
function TagPicker({
  tags,
  selectedTags,
  dispatch,
}: {
  tags: Tag[];
  selectedTags: string[];
  dispatch: Dispatch<FormAction>;
}) {
  return (
    <div>
      <span style={label}>Tags</span>
      <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
        {tags.map((t) => {
          const on = selectedTags.includes(t.id);
          const tagBtnStyle: React.CSSProperties = {
            ...TAG_BTN_BASE,
            background: on ? "var(--surface-selected)" : "var(--surface-2)",
            border: `var(--bw-hair) solid ${on ? t.color : "var(--border)"}`,
          };
          return (
            <button
              key={t.id}
              type="button"
              aria-pressed={on}
              onClick={() => dispatch({ type: "toggleTag", id: t.id })}
              style={tagBtnStyle}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: t.color,
                }}
              />
              {t.emoji ? `${t.emoji} ` : ""}
              {t.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}

/** Controle de recorrência ("Repetir" + nº de vezes + frequência), só no modo criação. */
function RepeatControls({
  repeat,
  repetitions,
  frequency,
  dispatch,
}: {
  repeat: boolean;
  repetitions: number;
  frequency: Frequency;
  dispatch: Dispatch<FormAction>;
}) {
  return (
    <div>
      <label
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: "var(--space-2)",
          cursor: "pointer",
          fontSize: "var(--fs-sm)",
          color: "var(--text)",
        }}
      >
        <input
          type="checkbox"
          checked={repeat}
          onChange={(e) =>
            dispatch({ type: "set", patch: { repeat: e.target.checked } })
          }
        />
        Repetir
      </label>
      {repeat && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--space-3)",
            marginTop: "var(--space-3)",
            flexWrap: "wrap",
          }}
        >
          <input
            aria-label="Número de repetições"
            type="number"
            min={1}
            max={120}
            value={repetitions}
            onChange={(e) =>
              dispatch({
                type: "set",
                patch: { repetitions: Math.max(1, Number(e.target.value) || 1) },
              })
            }
            style={{ ...field, width: 88 }}
          />
          <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
            vezes
          </span>
          <select
            aria-label="Frequência"
            value={frequency}
            onChange={(e) =>
              dispatch({
                type: "set",
                patch: { frequency: e.target.value as Frequency },
              })
            }
            style={{ ...field, width: "auto", minWidth: 140 }}
          >
            {(["diaria", "semanal", "mensal"] as Frequency[]).map((f) => (
              <option key={f} value={f}>
                {FREQ_LABELS[f]}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}

/** Seletor da conta-destino da Economia (transfer). Só contas reserve/illiquid; sem nenhuma, guia
 * o usuário a criar uma em Configurações › Bolsos (não criamos conta no submit, sem aprovação). */
function ReserveAccountPicker({
  accounts,
  toAccountId,
  dispatch,
}: {
  accounts: PocketAccount[];
  toAccountId: string;
  dispatch: Dispatch<FormAction>;
}) {
  return (
    <div>
      <label htmlFor="ntf-dest-account" style={label}>
        Conta-destino (reserva)
      </label>
      {accounts.length === 0 ? (
        <p style={HINT_EMPTY}>
          Nenhuma conta de reserva ou ilíquida encontrada. Crie uma em Configurações
          &rsaquo; Bolsos antes de registrar Economia.
        </p>
      ) : (
        <select
          id="ntf-dest-account"
          value={toAccountId}
          onChange={(e) =>
            dispatch({ type: "set", patch: { toAccountId: e.target.value } })
          }
          style={{ ...field, width: "100%" }}
        >
          <option value="">Selecione a conta…</option>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
              {a.liquidity === "illiquid" ? " (ilíquida)" : ""}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

/** Linha Valor + Data. Com partes ativas, o Valor é somente-leitura (= Σ partes). Extraído do
 * form para mantê-lo enxuto (uma unidade visual coesa, sem estado próprio). */
function AmountDateFields({
  amount,
  date,
  itemsActive,
  itemsTotal,
  dispatch,
}: {
  amount: string;
  date: string;
  itemsActive: boolean;
  itemsTotal: number;
  dispatch: Dispatch<FormAction>;
}) {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        gap: "var(--space-4)",
      }}
    >
      <div>
        <label htmlFor="ntf-amount" style={label}>
          Valor
        </label>
        {/* Com partes ativas, o Valor = Σ partes (somente-leitura, auto-calculado). */}
        <input
          id="ntf-amount"
          inputMode="decimal"
          placeholder="R$ 0,00"
          value={itemsActive ? centsToInput(itemsTotal) : amount}
          readOnly={itemsActive}
          onChange={(e) => dispatch({ type: "set", patch: { amount: e.target.value } })}
          style={{
            ...field,
            fontFamily: "var(--font-money)",
            ...(itemsActive
              ? { background: "var(--surface-2)", color: "var(--text-muted)" }
              : null),
          }}
        />
        {itemsActive && (
          <p style={HINT_TEXT}>Total calculado a partir das partes detalhadas.</p>
        )}
      </div>
      <div>
        <label htmlFor="ntf-date" style={label}>
          Data
        </label>
        <input
          id="ntf-date"
          type="date"
          value={date}
          onChange={(e) => dispatch({ type: "set", patch: { date: e.target.value } })}
          style={field}
        />
      </div>
    </div>
  );
}

export function NewTransactionForm({
  onCreated,
  initialValues,
  onSaved,
}: {
  onCreated?: () => void;
  initialValues?: TransactionEditValues;
  onSaved?: () => void;
}) {
  const editing = initialValues != null;
  const [form, dispatch] = useReducer(formReducer, initialValues, makeInitialForm);
  const {
    kind,
    amount,
    description,
    date,
    selectedTags,
    toAccountId,
    repeat,
    frequency,
    repetitions,
    items,
    busy,
    error,
  } = form;
  const [tags, setTags] = useState<Tag[]>([]);
  const [reserveAccounts, setReserveAccounts] = useState<PocketAccount[]>([]);

  useEffect(() => {
    let alive = true;
    listTags()
      .then((t) => alive && setTags(t))
      .catch(() => alive && setTags([]));
    return () => {
      alive = false;
    };
  }, []);

  // Contas-destino possíveis da Economia (reserve/illiquid). Só usado no kind "economia".
  useEffect(() => {
    let alive = true;
    getPockets()
      .then((p) => {
        if (!alive) return;
        setReserveAccounts(
          p.accounts.filter(
            (a) => a.liquidity === "reserve" || a.liquidity === "illiquid",
          ),
        );
      })
      .catch(() => alive && setReserveAccounts([]));
    return () => {
      alive = false;
    };
  }, []);

  // Itemização só faz sentido fora da Economia (a aba Economia não tem nota por-célula).
  const itemsEnabled = kind !== "economia";
  const itemsActive = itemsEnabled && items.length > 0;
  const itemsTotal = items.reduce((sum, it) => sum + it.amount_cents, 0);
  // Com partes ativas, o TOTAL é a SOMA delas (campo Valor somente-leitura); senão, o valor digitado.
  const typedAmount = parseBRLToCents(amount);
  const effectiveAmount = itemsActive ? itemsTotal : typedAmount;

  const canSubmit =
    effectiveAmount != null &&
    effectiveAmount > 0 &&
    !busy &&
    (kind !== "economia" || toAccountId !== "");

  async function submit() {
    if (effectiveAmount == null || effectiveAmount <= 0) {
      dispatch({ type: "fail", error: "Informe um valor válido." });
      return;
    }
    const amountCents = effectiveAmount;
    dispatch({ type: "submitStart" });
    const fields = kindToFields(kind);
    // try/catch sem `finally`: o React Compiler não otimiza componentes com try/finally.
    try {
      if (initialValues) {
        const recId = initialValues.recurrence_id;
        if (recId) {
          // Série recorrente: a escolha "toda a série" vs "deste ponto em diante" (o passado
          // fica intacto em updateSeriesFrom). Itens são por-instância → fora do escopo de série.
          const all = window.confirm(
            "Aplicar a alteração em toda a série?\n\nOK = toda a série\nCancela = este e os futuros",
          );
          const edit: SeriesEdit = {
            amount: amountCents,
            description: description.trim() || null,
            paymentMethod: fields.paymentMethod,
            isFixed: fields.isFixed,
          };
          if (all) {
            await updateSeriesAll(recId, edit);
          } else {
            await updateSeriesFrom(initialValues.id, edit);
          }
        } else {
          await updateTransaction(initialValues.id, {
            txnType: fields.txnType,
            amountCents,
            description: description.trim() || null,
            paymentMethod: fields.paymentMethod,
            isFixed: fields.isFixed,
            date,
          });
          // Plano 036: persiste as partes editadas (ou as limpa quando o dono removeu todas). Dois
          // round-trips (não-atômicos): o pai já foi salvo; o update de itens reaplica o total.
          if (itemsActive) {
            await updateTransactionItems(initialValues.id, items);
          }
        }
        dispatch({ type: "submitSuccess" });
        onSaved?.();
        return;
      }
      const newId = await createTransaction({
        txnType: fields.txnType,
        amountCents,
        description: description.trim() || null,
        date,
        paymentMethod: fields.paymentMethod,
        isFixed: fields.isFixed,
        tagIds: selectedTags,
        recurrence: repeat ? { frequency, repetitions } : null,
        toAccountId: kind === "economia" ? toAccountId : null,
      });
      // Plano 036: itens só em lançamento ÚNICO (não-recorrente; a série é por-instância). Segundo
      // round-trip após o create — STOP documentado: um crash entre os dois deixaria a transação
      // sem partes (recuperável: o dono re-detalha; o total do pai já está correto).
      if (itemsActive && !repeat) {
        await updateTransactionItems(newId, items);
      }
      dispatch({ type: "submitSuccess" });
      onCreated?.();
    } catch (e) {
      dispatch({
        type: "fail",
        error: safeErrorMessage(
          e,
          editing
            ? "Não foi possível salvar. Tente novamente."
            : "Não foi possível lançar. Tente novamente.",
        ),
      });
    }
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      style={{
        display: "grid",
        gap: "var(--space-4)",
        padding: "var(--space-5)",
        background: "var(--surface-elevated)",
        border: "var(--bw-hair) solid var(--border)",
        borderRadius: "var(--radius-md)",
      }}
    >
      <div>
        <span style={label}>Tipo de movimento</span>
        <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
          {FORM_KINDS.map((k) => {
            const active = k === kind;
            const btnStyle: React.CSSProperties = {
              ...KIND_BTN_BASE,
              background: active ? "var(--surface-selected)" : "transparent",
              border: `var(--bw-hair) solid ${active ? "var(--primary)" : "var(--border)"}`,
            };
            return (
              <button
                key={k}
                type="button"
                aria-pressed={active}
                onClick={() => dispatch({ type: "set", patch: { kind: k } })}
                style={btnStyle}
              >
                <MovBadge kind={k} showLabel size={16} />
              </button>
            );
          })}
        </div>
        {kind !== "economia" && (
          <p style={HINT_TEXT}>
            <MovBadge kind="economia" size={14} /> Economia é uma transferência para a
            sua reserva — registre aqui ou importe pela aba Economia da planilha
            (Configurações &rsaquo; Conexão Google Sheets).
          </p>
        )}
      </div>

      <AmountDateFields
        amount={amount}
        date={date}
        itemsActive={itemsActive}
        itemsTotal={itemsTotal}
        dispatch={dispatch}
      />

      <div>
        <label htmlFor="ntf-desc" style={label}>
          Descrição
        </label>
        <input
          id="ntf-desc"
          placeholder="Ex.: Mercado, salário, aluguel…"
          value={description}
          onChange={(e) =>
            dispatch({ type: "set", patch: { description: e.target.value } })
          }
          style={field}
        />
      </div>

      {/* Detalhamento em partes (plano 036): só fora da Economia e fora de séries recorrentes
          (itens são por-instância). Vale para novo, passado e previsto. */}
      {itemsEnabled && !initialValues?.recurrence_id && (
        <LineItemEditor
          items={items}
          onChange={(next) => dispatch({ type: "setItems", items: next })}
          disabled={busy}
        />
      )}

      {!editing && kind === "economia" && (
        <ReserveAccountPicker
          accounts={reserveAccounts}
          toAccountId={toAccountId}
          dispatch={dispatch}
        />
      )}

      {!editing && kind !== "economia" && tags.length > 0 && (
        <TagPicker tags={tags} selectedTags={selectedTags} dispatch={dispatch} />
      )}

      {/* Economia é um lançamento único — a recorrência (transfer) é rejeitada pelo backend. */}
      {!editing && kind !== "economia" && (
        <RepeatControls
          repeat={repeat}
          repetitions={repetitions}
          frequency={frequency}
          dispatch={dispatch}
        />
      )}

      {error && (
        <p
          role="alert"
          style={{ color: "var(--danger-400)", fontSize: "var(--fs-sm)", margin: 0 }}
        >
          {error}
        </p>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <Button type="submit" variant="primary" disabled={!canSubmit}>
          {busy ? "Salvando…" : editing ? "Salvar" : "Lançar"}
        </Button>
      </div>
    </form>
  );
}
