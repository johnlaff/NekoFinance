/**
 * Money — valor monetário BRL em mono tabular, com sinal de menos REAL (−) e cor por sinal.
 *
 * Valor monetário do Design System do Neko, com inline-style (convenção do Neko: Badge/MovBadge)
 * em vez do hook de injeção de CSS — assim é puro, sem hooks/estado/efeito. O formatador `formatBRL`
 * mora em `lib/format` (módulo puro); importado aqui para o rótulo e o conteúdo.
 */
import type { CSSProperties } from "react";
import { formatBRL } from "../../lib/format";

/**
 * `inherit` não impõe font-size/font-weight/letter-spacing — herda do wrapper. É o modo
 * certo quando a classe envolvente já define a tipografia do valor (tabelas, chips, prosa):
 * o componente contribui só a semântica (a11y, tabular-nums, nowrap) sem brigar com o CSS.
 */
export type MoneySize = "sm" | "md" | "lg" | "display" | "inherit";
export type MoneySign = "none" | "auto" | "negative";

const SIZE_FS: Record<Exclude<MoneySize, "inherit">, string> = {
  sm: "var(--fs-money-sm)",
  md: "var(--fs-money-md)",
  lg: "var(--fs-money-lg)",
  display: "var(--fs-money-xl)",
};

/** Estilos tipográficos por tamanho; `inherit` deixa size/weight/tracking com o wrapper. */
function sizeStyles(size: MoneySize): CSSProperties {
  if (size === "inherit") return {};
  return {
    fontWeight:
      size === "lg" || size === "display" ? "var(--fw-bold)" : "var(--fw-semibold)",
    fontSize: SIZE_FS[size],
    letterSpacing: size === "display" ? "-0.01em" : "0",
  };
}

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
        whiteSpace: "nowrap",
        ...sizeStyles(size),
        color,
      }}
    >
      {formatBRL(cents, hideCents)}
    </span>
  );
}

interface SignedMoneyProps {
  cents: number;
  size?: MoneySize;
  hideCents?: boolean;
  className?: string;
}

/**
 * SignedMoney — como `Money`, mas força o "+" visual em positivos (deltas, chips de
 * comparação). `Money` não tem esse modo de sinal (seu `sign="auto"` só troca a COR), então
 * este componente monta o próprio texto e replica o padrão de a11y do `Money`: o rótulo
 * anuncia "negativo"/"positivo" por extenso em vez de deixar o leitor de tela ler o símbolo.
 */
export function SignedMoney({
  cents,
  size = "md",
  hideCents = false,
  className = "",
}: SignedMoneyProps) {
  const text =
    cents > 0 ? "+" + formatBRL(cents, hideCents) : formatBRL(cents, hideCents);
  const label =
    (cents < 0 ? "negativo " : cents > 0 ? "positivo " : "") +
    formatBRL(Math.abs(cents), hideCents);
  return (
    <span
      className={className}
      aria-label={label}
      style={{
        fontFamily: "var(--font-money)",
        fontVariantNumeric: "tabular-nums",
        whiteSpace: "nowrap",
        ...sizeStyles(size),
      }}
    >
      {text}
    </span>
  );
}
