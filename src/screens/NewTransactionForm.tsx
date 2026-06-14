import { useEffect, useState } from "react";
import { Button } from "../design-system/components/Button";
import { MovBadge, type MovKind } from "../design-system/components/MovBadge";
import {
  createTransaction,
  listTags,
  type Frequency,
  type Tag,
} from "../lib/api";
import { parseBRLToCents } from "../lib/format";

/** Os tipos de movimento oferecidos no form (Economia → transfer precisa de conta, fica fora). */
const FORM_KINDS: MovKind[] = ["entrada", "saida", "diario", "cartao"];

/** Mapeia o tipo de movimento do método para (type, is_fixed, payment_method) do schema. */
function kindToFields(kind: MovKind): {
  txnType: "income" | "expense";
  isFixed: boolean;
  paymentMethod: string | null;
} {
  switch (kind) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: "debit" };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: "debit" };
  }
}

const FREQ_LABELS: Record<Frequency, string> = {
  diaria: "por dia",
  semanal: "por semana",
  mensal: "por mês",
};

function todayISO(): string {
  return new Date().toISOString().slice(0, 10);
}

const field: React.CSSProperties = {
  width: "100%",
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border)",
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

export function NewTransactionForm({ onCreated }: { onCreated?: () => void }) {
  const [kind, setKind] = useState<MovKind>("diario");
  const [amount, setAmount] = useState("");
  const [description, setDescription] = useState("");
  const [date, setDate] = useState(todayISO);
  const [tags, setTags] = useState<Tag[]>([]);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [repeat, setRepeat] = useState(false);
  const [frequency, setFrequency] = useState<Frequency>("mensal");
  const [repetitions, setRepetitions] = useState(12);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    listTags()
      .then((t) => alive && setTags(t))
      .catch(() => alive && setTags([]));
    return () => {
      alive = false;
    };
  }, []);

  const amountCents = parseBRLToCents(amount);
  const canSubmit = amountCents != null && amountCents > 0 && !busy;

  function toggleTag(id: string) {
    setSelectedTags((cur) =>
      cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id],
    );
  }

  async function submit() {
    if (amountCents == null || amountCents <= 0) {
      setError("Informe um valor válido.");
      return;
    }
    setBusy(true);
    setError(null);
    const fields = kindToFields(kind);
    // try/catch sem `finally`: o React Compiler não otimiza componentes com try/finally.
    try {
      await createTransaction({
        txnType: fields.txnType,
        amountCents,
        description: description.trim() || null,
        date,
        paymentMethod: fields.paymentMethod,
        isFixed: fields.isFixed,
        tagIds: selectedTags,
        recurrence: repeat ? { frequency, repetitions } : null,
      });
      // Reset dos campos voláteis; mantém tipo e data para lançamentos em sequência.
      setAmount("");
      setDescription("");
      setSelectedTags([]);
      setRepeat(false);
      setBusy(false);
      onCreated?.();
    } catch (e) {
      setError(String(e));
      setBusy(false);
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
        background: "var(--surface-1)",
        border: "var(--bw-hair) solid var(--border)",
        borderRadius: "var(--radius-md)",
      }}
    >
      <div>
        <span style={label}>Tipo de movimento</span>
        <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
          {FORM_KINDS.map((k) => {
            const active = k === kind;
            return (
              <button
                key={k}
                type="button"
                aria-pressed={active}
                onClick={() => setKind(k)}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "var(--space-2)",
                  height: "var(--hit-min)",
                  padding: "0 var(--space-3)",
                  borderRadius: "var(--radius-sm)",
                  cursor: "pointer",
                  background: active ? "var(--surface-selected)" : "transparent",
                  border: `var(--bw-hair) solid ${active ? "var(--primary)" : "var(--border)"}`,
                  color: "var(--text)",
                  fontFamily: "var(--font-sans)",
                  fontSize: "var(--fs-sm)",
                }}
              >
                <MovBadge kind={k} showLabel size={16} />
              </button>
            );
          })}
        </div>
      </div>

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
          <input
            id="ntf-amount"
            inputMode="decimal"
            placeholder="R$ 0,00"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            style={{ ...field, fontFamily: "var(--font-money)" }}
          />
        </div>
        <div>
          <label htmlFor="ntf-date" style={label}>
            Data
          </label>
          <input
            id="ntf-date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            style={field}
          />
        </div>
      </div>

      <div>
        <label htmlFor="ntf-desc" style={label}>
          Descrição
        </label>
        <input
          id="ntf-desc"
          placeholder="Ex.: Mercado, salário, aluguel…"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          style={field}
        />
      </div>

      {tags.length > 0 && (
        <div>
          <span style={label}>Tags</span>
          <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
            {tags.map((t) => {
              const on = selectedTags.includes(t.id);
              return (
                <button
                  key={t.id}
                  type="button"
                  aria-pressed={on}
                  onClick={() => toggleTag(t.id)}
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: "6px",
                    height: 28,
                    padding: "0 10px",
                    borderRadius: "var(--radius-pill)",
                    cursor: "pointer",
                    background: on ? "var(--surface-selected)" : "var(--surface-2)",
                    border: `var(--bw-hair) solid ${on ? t.color : "var(--border)"}`,
                    color: "var(--text)",
                    fontSize: "var(--fs-micro)",
                    fontFamily: "var(--font-sans)",
                  }}
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
      )}

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
            onChange={(e) => setRepeat(e.target.checked)}
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
              onChange={(e) => setRepetitions(Math.max(1, Number(e.target.value) || 1))}
              style={{ ...field, width: 88 }}
            />
            <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
              vezes
            </span>
            <select
              aria-label="Frequência"
              value={frequency}
              onChange={(e) => setFrequency(e.target.value as Frequency)}
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

      {error && (
        <p style={{ color: "var(--danger-400)", fontSize: "var(--fs-sm)", margin: 0 }}>
          {error}
        </p>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <Button type="submit" variant="primary" disabled={!canSubmit}>
          {busy ? "Salvando…" : "Lançar"}
        </Button>
      </div>
    </form>
  );
}
