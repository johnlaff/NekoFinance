import React from "react";

// LineItemEditor — controlled editor for itemized transaction parts (plano 036).
// Each item: R$ <magnitude> - <description>. Total shown when ≥2 items exist.
// Self-contained: no external imports, no fetch, no Tauri. Inline-style convention.

// ---- helpers ----------------------------------------------------------------

function parseBRLToCents(input) {
  const cleaned = input
    .replace(/[R$\s]/g, "")
    .replace(/\./g, "")
    .replace(",", ".");
  if (!cleaned || !/^-?\d+(\.\d+)?$/.test(cleaned)) return null;
  return Math.round(Number(cleaned) * 100);
}

function formatBRL(cents) {
  const neg = cents < 0;
  const v = Math.abs(cents) / 100;
  const s = v.toLocaleString("pt-BR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return (neg ? "−R$ " : "R$ ") + s;
}

// ---- style constants --------------------------------------------------------

const FIELD_BASE = {
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-body)",
  outline: "none",
  boxSizing: "border-box",
};

const ITEM_AMOUNT = {
  ...FIELD_BASE,
  width: 120,
  fontFamily: "var(--font-money)",
  fontVariantNumeric: "tabular-nums",
  flexShrink: 0,
};

const ITEM_DESC = {
  ...FIELD_BASE,
  flex: 1,
  minWidth: 0,
};

const ROW = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
};

const LIST = {
  display: "grid",
  gap: "var(--space-2)",
};

const REMOVE_BTN = {
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

const ADD_BTN = {
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

const SECTION_LABEL = {
  display: "block",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  marginBottom: "var(--space-2)",
};

const TOTAL_LINE = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "baseline",
  marginTop: "var(--space-1)",
  fontSize: "var(--fs-sm)",
  color: "var(--text-muted)",
};

const TOTAL_VALUE = {
  fontFamily: "var(--font-money)",
  fontVariantNumeric: "tabular-nums",
  fontWeight: "var(--fw-semibold)",
  color: "var(--text)",
};

// ---- default demo items (render nicely with no required props) ---------------

const DEMO_ITEMS = [
  { amount_cents: 8500, description: "Supermercado Pão de Açúcar", position: 0 },
  { amount_cents: 3200, description: "Padaria da esquina", position: 1 },
];

// ---- component --------------------------------------------------------------

export function LineItemEditor({ items: itemsProp, onChange, disabled = false }) {
  // Standalone / demo mode: manage state internally when no onChange is provided
  const isControlled = typeof onChange === "function";
  const [internalItems, setInternalItems] = React.useState(
    itemsProp !== undefined ? itemsProp : DEMO_ITEMS,
  );

  const items = isControlled
    ? itemsProp !== undefined
      ? itemsProp
      : []
    : internalItems;

  function emit(next) {
    if (isControlled) {
      onChange(next);
    } else {
      setInternalItems(next);
    }
  }

  // Raw amount text per row (buffered to avoid cursor fighting while typing)
  const [amountText, setAmountText] = React.useState({});

  function displayAmount(index, cents) {
    const buffered = amountText[index];
    if (buffered !== undefined) return buffered;
    return cents > 0 ? (cents / 100).toFixed(2).replace(".", ",") : "";
  }

  function addItem() {
    emit([...items, { amount_cents: 0, description: "", position: items.length }]);
  }

  function removeItem(index) {
    setAmountText({});
    const next = [];
    for (let i = 0; i < items.length; i++) {
      if (i === index) continue;
      const it = items[i];
      if (it) next.push({ ...it, position: next.length });
    }
    emit(next);
  }

  function setAmount(index, raw) {
    setAmountText((prev) => ({ ...prev, [index]: raw }));
    const cents = parseBRLToCents(raw);
    emit(
      items.map((it, i) =>
        i === index ? { ...it, amount_cents: cents !== null ? cents : 0 } : it,
      ),
    );
  }

  function setDescription(index, value) {
    emit(items.map((it, i) => (i === index ? { ...it, description: value } : it)));
  }

  const total = items.reduce((sum, it) => sum + it.amount_cents, 0);

  return (
    <div style={{ fontFamily: "var(--font-sans)" }}>
      <span style={SECTION_LABEL}>Detalhar em partes</span>

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
                style={
                  disabled
                    ? { ...REMOVE_BTN, opacity: 0.4, cursor: "not-allowed" }
                    : REMOVE_BTN
                }
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
        style={{
          ...ADD_BTN,
          marginTop: items.length > 0 ? "var(--space-2)" : 0,
          ...(disabled ? { opacity: 0.4, cursor: "not-allowed" } : {}),
        }}
      >
        + Adicionar item
      </button>

      {items.length >= 2 && (
        <p style={{ ...TOTAL_LINE, margin: "var(--space-1) 0 0" }}>
          <span>Total das partes</span>
          <span style={TOTAL_VALUE}>{formatBRL(total)}</span>
        </p>
      )}
    </div>
  );
}
