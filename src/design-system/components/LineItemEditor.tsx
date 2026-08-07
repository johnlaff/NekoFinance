import { useState, type CSSProperties } from "react";
import { formatBRL, parseBRLToCents } from "../../lib/format";

/** Forma genérica de uma parte itemizada — o design system não conhece o DTO `LineItemDraft` do
 *  shim, só esta forma estrutural. Quem chama passa o tipo de domínio, que já a satisfaz. */
export interface LineItemEditorItem {
  amount_cents: number;
  description: string;
  position: number;
}

/**
 * Editor das PARTES de um lançamento itemizado. Componente puro/controlado: o pai é
 * dono da lista; toda adição/remoção/edição emite `onChange` com a nova lista (sem estado interno,
 * sem fetch, sem chamada Tauri). O TOTAL do lançamento é a SOMA das partes — quando há ≥1 parte, o
 * campo de Valor do form vira somente-leitura (auto-calculado); sem partes, o Valor é editável.
 *
 * Gramática (espelha a nota da célula): cada parte é `R$ <valor> - <descrição>`. O valor é magnitude
 * positiva em centavos; a descrição é texto livre. A descrição NUNCA vira fórmula na planilha (o
 * write-back monta `=SUM(...)` só dos valores numéricos — ver `build_itemized_cell_value`).
 */
const FIELD_BASE: CSSProperties = {
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-body)",
};

const ITEM_AMOUNT: CSSProperties = {
  ...FIELD_BASE,
  width: 120,
  fontFamily: "var(--font-money)",
};

const ITEM_DESC: CSSProperties = {
  ...FIELD_BASE,
  flex: 1,
  minWidth: 0,
};

const ROW: CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
};

const LIST: CSSProperties = {
  display: "grid",
  gap: "var(--space-2)",
};

const REMOVE_BTN: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  width: "var(--hit-min)",
  height: "var(--hit-min)",
  flexShrink: 0,
  borderRadius: "var(--radius-xs)",
  border: "var(--bw-hair) solid var(--border)",
  background: "transparent",
  color: "var(--text-muted)",
  cursor: "pointer",
  fontSize: "var(--fs-body)",
  lineHeight: 1,
};

const ADD_BTN: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "var(--space-2)",
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) dashed var(--border)",
  background: "transparent",
  color: "var(--text)",
  cursor: "pointer",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-sm)",
  width: "fit-content",
};

const TOTAL_LINE: CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "baseline",
  marginTop: "var(--space-1)",
  fontSize: "var(--fs-sm)",
  color: "var(--text-muted)",
};

const TOTAL_VALUE: CSSProperties = {
  fontFamily: "var(--font-money)",
  fontWeight: "var(--fw-semibold)",
  color: "var(--text)",
};

export function LineItemEditor({
  items,
  onChange,
  disabled = false,
}: {
  items: LineItemEditorItem[];
  onChange: (items: LineItemEditorItem[]) => void;
  disabled?: boolean;
}) {
  const total = items.reduce((sum, it) => sum + it.amount_cents, 0);

  // Texto CRU do campo de valor por linha (chave = posição). Mantemos a string como o dono digita
  // (ex.: "50," durante a digitação) sem reformatá-la a cada tecla — só convertemos para centavos no
  // `onChange`. Sem isto, formatar a string a cada keystroke brigaria com o cursor (50 → 5,00 → …).
  const [amountText, setAmountText] = useState<Record<number, string>>({});

  function displayAmount(index: number, cents: number): string {
    const buffered = amountText[index];
    if (buffered !== undefined) return buffered;
    return cents > 0 ? (cents / 100).toFixed(2).replace(".", ",") : "";
  }

  function addItem() {
    onChange([...items, { amount_cents: 0, description: "", position: items.length }]);
  }

  function removeItem(index: number) {
    setAmountText({}); // posições mudam ao remover → descarta o buffer (re-deriva dos centavos).
    // Uma passada só: remove o índice e reindexa as posições restantes (sem filter+map encadeados).
    const next: LineItemEditorItem[] = [];
    for (let i = 0; i < items.length; i++) {
      if (i === index) continue;
      const it = items[i];
      if (it) next.push({ ...it, position: next.length });
    }
    onChange(next);
  }

  function setAmount(index: number, raw: string) {
    setAmountText((prev) => ({ ...prev, [index]: raw }));
    const cents = parseBRLToCents(raw);
    onChange(
      items.map((it, i) => (i === index ? { ...it, amount_cents: cents ?? 0 } : it)),
    );
  }

  function setDescription(index: number, value: string) {
    onChange(items.map((it, i) => (i === index ? { ...it, description: value } : it)));
  }

  return (
    <div>
      <span
        style={{
          display: "block",
          fontSize: "var(--fs-label)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--ls-label)",
          textTransform: "uppercase",
          color: "var(--text-muted)",
          marginBottom: "var(--space-2)",
        }}
      >
        Detalhar em partes
      </span>

      {items.length > 0 && (
        <div style={LIST}>
          {items.map((it, i) => (
            <div key={it.position} style={ROW}>
              <input
                aria-label={`Valor do item ${i + 1}`}
                inputMode="decimal"
                placeholder="R$ 0,00"
                value={displayAmount(i, it.amount_cents)}
                onChange={(e) => setAmount(i, e.target.value)}
                disabled={disabled}
                style={ITEM_AMOUNT}
              />
              <input
                aria-label={`Descrição do item ${i + 1}`}
                placeholder="Descrição da parte…"
                value={it.description}
                onChange={(e) => setDescription(i, e.target.value)}
                disabled={disabled}
                style={ITEM_DESC}
              />
              <button
                type="button"
                aria-label={`Remover item ${i + 1}`}
                onClick={() => removeItem(i)}
                disabled={disabled}
                style={REMOVE_BTN}
              >
                &times;
              </button>
            </div>
          ))}
        </div>
      )}

      <button
        type="button"
        onClick={addItem}
        disabled={disabled}
        style={{ ...ADD_BTN, marginTop: items.length > 0 ? "var(--space-2)" : 0 }}
      >
        + Adicionar item
      </button>

      {items.length >= 2 && (
        <p style={TOTAL_LINE}>
          <span>Total das partes</span>
          <span style={TOTAL_VALUE}>{formatBRL(total)}</span>
        </p>
      )}
    </div>
  );
}
