/**
 * MovBadge — o badge de TIPO DE MOVIMENTO (os 5 pilares do método).
 *
 * Glifos próprios dos tipos de movimento do método, com os tokens `--type-*` do Midnight
 * Ledger. No método, Entrada e Economia compartilham a letra "E" — a COR distingue (entrada = jade; economia = verde-escuro). Acessível: o círculo é decorativo
 * (`aria-hidden`) e o nome do tipo é sempre exposto a leitores de tela.
 */
export type MovKind = "entrada" | "saida" | "diario" | "economia" | "cartao";

const KIND_META: Record<MovKind, { token: string; glyph: string; name: string }> = {
  entrada: { token: "var(--type-entrada)", glyph: "E", name: "Entrada" },
  saida: { token: "var(--type-saida)", glyph: "S", name: "Saída" },
  diario: { token: "var(--type-diario)", glyph: "D", name: "Diário" },
  economia: { token: "var(--type-economia)", glyph: "E", name: "Economia" },
  cartao: { token: "var(--type-cartao)", glyph: "C", name: "Cartão" },
};

const SR_ONLY: React.CSSProperties = {
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

// Base estática do glifo (não recria por render); tamanho/cor/fonte entram por merge.
const GLYPH_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  borderRadius: "50%",
  color: "var(--text-on-primary)",
  fontWeight: "var(--fw-bold)",
  lineHeight: 1,
  flexShrink: 0,
};

interface MovBadgeProps {
  kind: MovKind;
  /** Mostra o nome do tipo ao lado do glifo (senão fica só para leitores de tela). */
  showLabel?: boolean;
  /** Diâmetro do glifo em px. */
  size?: number;
  className?: string;
}

export function MovBadge({
  kind,
  showLabel = false,
  size = 18,
  className = "",
}: MovBadgeProps) {
  const meta = KIND_META[kind];
  const glyphStyle: React.CSSProperties = {
    ...GLYPH_BASE,
    width: size,
    height: size,
    background: meta.token,
    fontSize: `${Math.round(size * 0.56)}px`,
  };
  return (
    <span
      className={className}
      style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}
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
