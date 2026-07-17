import React from "react";

/**
 * TransactionRow — linha de lançamento fiel ao método: data, descrição, método, valor, procedência,
 * titular e nota. Quando o lançamento é um lump de fatura (Saída agregada), expande os itens da
 * nota da célula. Portado do production TransactionRow.tsx; inline-style convention (zero classes).
 */

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Formata centavos BRL → "R$ 1.234,56" (− real, U+2212). */
function formatBRL(cents) {
  const neg = cents < 0;
  const v = Math.abs(cents) / 100;
  const s = v.toLocaleString("pt-BR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return (neg ? "−R$ " : "R$ ") + s;
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

const PROV = {
  importado: { label: "Da planilha", color: "var(--prov-imported)" },
  manual: { label: "Do app", color: "var(--prov-app)" },
  projetado: { label: "Previsto", color: "var(--prov-projected)" },
  conciliado: { label: "Conferido", color: "var(--prov-reconciled)" },
};

function ProvBadge({ provenance }) {
  if (!provenance) return null;
  const g = PROV[provenance];
  if (!g) return null;
  return (
    <span
      title={g.label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        fontSize: "var(--fs-micro)",
        fontWeight: "var(--fw-semibold)",
        color: "var(--text-muted)",
        whiteSpace: "nowrap",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          flex: "none",
          background: g.color,
        }}
      />
      {g.label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Static style objects (defined outside the component to avoid re-creation)
// ---------------------------------------------------------------------------

const PASSTHROUGH_BADGE_STYLE = {
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-bold)",
  textTransform: "uppercase",
  letterSpacing: "0.04em",
  color: "var(--info-400)",
  background: "var(--info-tint)",
  padding: "1px 6px",
  borderRadius: "4px",
  whiteSpace: "nowrap",
};

const LUMP_TOGGLE_BASE = {
  width: 18,
  height: 18,
  display: "grid",
  placeItems: "center",
  border: "none",
  background: "transparent",
  color: "var(--text-faint)",
  borderRadius: "4px",
  cursor: "pointer",
  flexShrink: 0,
  transition: "transform var(--dur-fast) var(--ease-standard)",
};

function moneyStyle(amount) {
  return {
    fontFamily: "var(--font-money)",
    fontVariantNumeric: "tabular-nums",
    fontWeight: "var(--fw-semibold)",
    fontSize: "var(--fs-money-sm)",
    textAlign: "right",
    whiteSpace: "nowrap",
    color: amount > 0 ? "var(--money-pos)" : "var(--text)",
  };
}

function lumpItemKey(it) {
  return `${it.what}:${it.amount}:${it.passthrough ? "repasse" : "normal"}`;
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function TransactionRow({
  date = "21/06",
  desc = "Supermercado Central",
  amount = -38500,
  method = "Débito",
  provenance = "importado",
  owner = null,
  note = null,
  passthrough = false,
  future = false,
  lump = null,
  defaultOpen = false,
  selected = false,
  onClick = null,
  className = "",
}) {
  const [open, setOpen] = React.useState(defaultOpen);
  const hasLump = Array.isArray(lump) && lump.length > 0;

  const toggleStyle = {
    ...LUMP_TOGGLE_BASE,
    transform: open ? "rotate(90deg)" : "none",
  };

  const rowInteractionProps = onClick
    ? {
        onClick,
        onKeyDown: (event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onClick();
          }
        },
        role: "button",
        tabIndex: 0,
      }
    : {};

  const futureBackground = future
    ? "repeating-linear-gradient(135deg, transparent, transparent 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 18px)"
    : "transparent";

  const showMeta = provenance || owner || note;

  return (
    <div
      className={className}
      style={{
        borderBottom: "var(--bw-hair) solid var(--border)",
        fontFamily: "var(--font-sans)",
        background: selected ? "var(--surface-selected)" : futureBackground,
        boxShadow: "none",
      }}
    >
      {/* Main row */}
      <div
        {...rowInteractionProps}
        style={{
          display: "grid",
          gridTemplateColumns: "58px 1fr auto auto",
          alignItems: "center",
          gap: "14px",
          padding: "12px 18px",
        }}
      >
        {/* Date */}
        <span
          style={{
            fontSize: "var(--fs-sm)",
            color: "var(--text-faint)",
            fontFamily: "var(--font-money)",
            whiteSpace: "nowrap",
          }}
        >
          {date}
        </span>

        {/* Desc + meta */}
        <div
          style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: "4px" }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            {hasLump ? (
              <button
                type="button"
                aria-expanded={open}
                aria-label={open ? "Fechar itens" : "Abrir itens"}
                onClick={(e) => {
                  e.stopPropagation();
                  setOpen((o) => !o);
                }}
                style={toggleStyle}
              >
                ›
              </button>
            ) : (
              <span style={{ width: 18, flexShrink: 0 }} />
            )}
            <span
              style={{
                fontSize: "var(--fs-body)",
                color: "var(--text)",
                overflowWrap: "anywhere",
              }}
            >
              {desc}
            </span>
            {passthrough ? <span style={PASSTHROUGH_BADGE_STYLE}>repasse</span> : null}
          </div>

          {showMeta ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                flexWrap: "wrap",
                paddingLeft: 26,
              }}
            >
              <ProvBadge provenance={provenance} />
              {owner}
              {note ? (
                <span
                  style={{
                    fontSize: "var(--fs-micro)",
                    color: "var(--text-faint)",
                    fontStyle: "italic",
                  }}
                >
                  {`"${note}"`}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>

        {/* Method pill */}
        {method ? (
          <span
            style={{
              fontSize: "var(--fs-micro)",
              color: "var(--text-muted)",
              padding: "3px 9px",
              border: "var(--bw-hair) solid var(--border)",
              borderRadius: "var(--radius-pill)",
              whiteSpace: "nowrap",
            }}
          >
            {method}
          </span>
        ) : null}

        {/* Amount */}
        <span style={{ ...moneyStyle(amount), opacity: passthrough ? 0.55 : 1 }}>
          {formatBRL(amount)}
        </span>
      </div>

      {/* Lump expand panel */}
      {hasLump && open ? (
        <div
          style={{
            padding: "4px 18px 14px 76px",
            background: "var(--bg-subtle)",
            borderTop: "1px dashed var(--border)",
          }}
        >
          {lump.map((it) => (
            <div
              key={lumpItemKey(it)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                padding: "7px 0",
                borderBottom: "var(--bw-hair) solid var(--border)",
                fontSize: "var(--fs-sm)",
              }}
            >
              <span
                style={{ color: "var(--text-faint)", fontFamily: "var(--font-money)" }}
              >
                ↳
              </span>
              <span
                style={{
                  flex: 1,
                  color: "var(--text-muted)",
                  minWidth: 0,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {it.what}
              </span>
              {it.owner || null}
              <span style={moneyStyle(it.amount)}>{formatBRL(it.amount)}</span>
            </div>
          ))}
          <p
            style={{
              margin: "10px 0 0",
              fontSize: "var(--fs-micro)",
              color: "var(--text-faint)",
            }}
          >
            Esse detalhe vem das notas da célula da planilha. Cada item é preservado;
            nunca vira um "Saída" genérico.
          </p>
        </div>
      ) : null}
    </div>
  );
}
