/**
 * Money — valor monetário BRL em mono tabular, com sinal de menos REAL (−) e cor por sinal.
 *
 * Valor monetário do Design System do Neko, com inline-style (convenção do Neko: Badge/MovBadge)
 * em vez do hook de injeção de CSS — assim é puro, sem hooks/estado/efeito. O formatador `formatBRL`
 * mora em `lib/format` (módulo puro); importado aqui para o rótulo e o conteúdo.
 */
import { formatBRL } from "../../lib/format";

export type MoneySize = "sm" | "md" | "lg" | "display";
export type MoneySign = "none" | "auto" | "negative";

const SIZE_FS: Record<MoneySize, string> = {
  sm: "var(--fs-money-sm)",
  md: "var(--fs-money-md)",
  lg: "var(--fs-money-lg)",
  display: "var(--fs-money-xl)",
};

interface MoneyProps {
  cents: number;
  size?: MoneySize;
  /** `auto` colore por sinal; `negative` força vermelho; `none` herda a cor. */
  sign?: MoneySign;
  hideCents?: boolean;
  ariaLabel?: string;
  className?: string;
}

export function Money({
  cents,
  size = "md",
  sign = "none",
  hideCents = false,
  ariaLabel,
  className = "",
}: MoneyProps) {
  const color =
    sign === "negative"
      ? "var(--money-neg)"
      : sign === "auto"
        ? cents < 0
          ? "var(--money-neg)"
          : cents > 0
            ? "var(--money-pos)"
            : "var(--money-neutral)"
        : undefined;
  const heavy = size === "lg" || size === "display";
  return (
    <span
      className={className}
      aria-label={
        ariaLabel ??
        (cents < 0 ? "negativo " : "") + formatBRL(Math.abs(cents), hideCents)
      }
      style={{
        fontFamily: "var(--font-money)",
        fontVariantNumeric: "tabular-nums",
        fontWeight: heavy ? "var(--fw-bold)" : "var(--fw-semibold)",
        fontSize: SIZE_FS[size],
        letterSpacing: size === "display" ? "-0.01em" : "0",
        whiteSpace: "nowrap",
        color,
      }}
    >
      {formatBRL(cents, hideCents)}
    </span>
  );
}
