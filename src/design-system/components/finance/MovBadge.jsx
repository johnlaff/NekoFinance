import React from "react";

// MovBadge — badge de tipo de movimento (os 5 pilares do método do Neko).
// Inline-style convention (no CSS injection needed — all styles are object literals).
// Accessible: o círculo decorativo é aria-hidden; o nome do tipo é sempre exposto via
// sr-only span (quando showLabel=false) ou via label visível (quando showLabel=true).

const KIND_META = {
  entrada: { token: "var(--type-entrada)", glyph: "E", name: "Entrada" },
  saida: { token: "var(--type-saida)", glyph: "S", name: "Saída" },
  diario: { token: "var(--type-diario)", glyph: "D", name: "Diário" },
  economia: { token: "var(--type-economia)", glyph: "E", name: "Economia" },
  cartao: { token: "var(--type-cartao)", glyph: "C", name: "Cartão" },
};

const SR_ONLY = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clipPath: "inset(50%)",
  whiteSpace: "nowrap",
  border: 0,
};

const GLYPH_BASE = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  borderRadius: "50%",
  color: "var(--text-on-primary)",
  fontWeight: "var(--fw-bold)",
  fontFamily: "var(--font-sans)",
  lineHeight: 1,
  flexShrink: 0,
};

export function MovBadge({
  kind = "saida",
  showLabel = false,
  size = 18,
  className = "",
}) {
  const meta = KIND_META[kind] || KIND_META.saida;
  const glyphStyle = {
    ...GLYPH_BASE,
    width: size,
    height: size,
    background: meta.token,
    fontSize: `${Math.round(size * 0.56)}px`,
  };
  return (
    <span
      className={className}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        fontFamily: "var(--font-sans)",
      }}
    >
      <span aria-hidden="true" style={glyphStyle}>
        {meta.glyph}
      </span>
      {showLabel ? (
        <span style={{ fontSize: "var(--fs-sm)", color: "var(--text)" }}>
          {meta.name}
        </span>
      ) : (
        <span style={SR_ONLY}>{meta.name}</span>
      )}
    </span>
  );
}
