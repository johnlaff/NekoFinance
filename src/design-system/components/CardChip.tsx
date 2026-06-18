import type { CSSProperties } from "react";
import { formatBRL } from "../../lib/format";

/**
 * CardChip — cartão de crédito (o 5º tipo: gasto com cartão / Régua 2). Face com gradiente da
 * marca, apelido, final, e a fatura acumulada. Portado do novo DS em inline-style (puro). `brand`
 * default = token do tipo Cartão.
 */

// Base estática do cartão (não recria por render); borda/sombra do estado ativo entram por merge.
const CARD_CHIP_BASE: CSSProperties = {
  flex: "0 0 auto",
  width: 168,
  display: "flex",
  flexDirection: "column",
  gap: "8px",
  padding: "12px",
  borderRadius: "var(--radius-md)",
  background: "var(--surface)",
  textAlign: "left",
  cursor: "pointer",
  fontFamily: "var(--font-sans)",
  transition: "transform var(--dur-fast) var(--ease-standard)",
};

const ADDITIONAL_BADGE_STYLE: CSSProperties = {
  display: "inline-block",
  width: "fit-content",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-bold)",
  textTransform: "uppercase",
  letterSpacing: "0.04em",
  color: "var(--info-400)",
  background: "var(--info-tint)",
  padding: "1px 6px",
  borderRadius: "4px",
};

interface CardChipProps {
  brand?: string;
  /** Nome impresso (banco/bandeira). */
  mono: string;
  last4: string;
  nick: string;
  /** Fatura acumulada em centavos (negativo = a pagar). */
  total: number;
  /** Cartão adicional (pago por outro titular). */
  additional?: boolean;
  ownerLabel?: string;
  active?: boolean;
  onClick?: () => void;
  ariaLabel?: string;
  className?: string;
}

export function CardChip({
  brand = "var(--type-cartao)",
  mono,
  last4,
  nick,
  total,
  additional = false,
  ownerLabel,
  active = false,
  onClick,
  ariaLabel,
  className = "",
}: CardChipProps) {
  const totalColor =
    total < 0
      ? "var(--money-neg)"
      : total > 0
        ? "var(--money-pos)"
        : "var(--money-neutral)";
  const chipStyle: CSSProperties = {
    ...CARD_CHIP_BASE,
    border: active
      ? "var(--bw-default) solid var(--border-focus)"
      : "var(--bw-hair) solid var(--border)",
    boxShadow: active ? "var(--shadow-focus)" : "var(--shadow-1)",
  };
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      aria-label={ariaLabel ?? `${nick}, final ${last4}, fatura ${formatBRL(total)}`}
      className={className}
      style={chipStyle}
    >
      <span
        aria-hidden="true"
        style={{
          height: 52,
          borderRadius: "var(--radius-sm)",
          background: `linear-gradient(135deg, ${brand}, color-mix(in srgb, ${brand} 62%, #000))`,
          padding: "9px 11px",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
        }}
      >
        <span
          style={{
            fontFamily: "var(--font-money)",
            fontWeight: "var(--fw-bold)",
            fontSize: "13px",
            color: "#fff",
            opacity: 0.95,
            // O nome cai na ponta CLARA do gradiente (135deg) — a sombra garante leitura do branco.
            textShadow: "0 1px 3px rgba(0, 0, 0, 0.5)",
          }}
        >
          {mono}
        </span>
        <span
          style={{
            fontFamily: "var(--font-money)",
            fontSize: "var(--fs-label)",
            color: "#fff",
            opacity: 0.85,
            letterSpacing: "0.04em",
          }}
        >
          •• {last4}
        </span>
      </span>
      <span
        style={{
          fontSize: "var(--fs-sm)",
          fontWeight: "var(--fw-semibold)",
          color: "var(--text)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {nick}
      </span>
      <span
        style={{
          fontFamily: "var(--font-money)",
          fontVariantNumeric: "tabular-nums",
          fontWeight: "var(--fw-semibold)",
          fontSize: "var(--fs-money-sm)",
          color: totalColor,
        }}
      >
        {formatBRL(total)}
      </span>
      {additional ? (
        <span style={ADDITIONAL_BADGE_STYLE}>
          {ownerLabel ? `paga ${ownerLabel}` : "adicional"}
        </span>
      ) : null}
    </button>
  );
}
