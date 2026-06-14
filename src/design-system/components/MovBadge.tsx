/**
 * MovBadge — o badge de TIPO DE MOVIMENTO (os 5 pilares do método).
 *
 * Reimplementação dos glifos coloridos do app oficial (fonte proprietária IconsTypeMov) com os
 * tokens `--type-*` do Midnight Ledger. Como no app, Entrada e Economia compartilham a letra "E" —
 * a COR distingue (entrada = jade; economia = verde-escuro). Acessível: o círculo é decorativo
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
  return (
    <span
      className={className}
      style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}
    >
      <span
        aria-hidden="true"
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: size,
          height: size,
          borderRadius: "50%",
          background: meta.token,
          color: "var(--text-on-primary)",
          fontSize: `${Math.round(size * 0.56)}px`,
          fontWeight: "var(--fw-bold)",
          lineHeight: 1,
          flexShrink: 0,
        }}
      >
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
