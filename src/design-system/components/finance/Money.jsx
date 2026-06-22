import React from "react";

// Money — valor monetário BRL em mono tabular, com sinal de menos real (−) e cor por sinal.
// Inline-style only — sem hooks, sem CSS injection.

/** Replica do formatBRL de src/lib/format.ts, sem dependências externas. */
function formatBRL(cents, hideCents) {
  const neg = cents < 0;
  const v = Math.abs(cents) / 100;
  const s = v.toLocaleString("pt-BR", {
    minimumFractionDigits: hideCents ? 0 : 2,
    maximumFractionDigits: hideCents ? 0 : 2,
  });
  // Sinal de menos tipográfico (U+2212) + NBSP após R$
  return (neg ? "−R$ " : "R$ ") + s;
}

const SIZE_FS = {
  sm: "var(--fs-money-sm)",
  md: "var(--fs-money-md)",
  lg: "var(--fs-money-lg)",
  display: "var(--fs-money-xl)",
};

export function Money({
  cents = -123456,
  size = "md",
  sign = "none",
  hideCents = false,
  ariaLabel,
  className = "",
}) {
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

  const label =
    ariaLabel ?? (cents < 0 ? "negativo " : "") + formatBRL(Math.abs(cents), hideCents);

  return (
    <span
      className={className || undefined}
      aria-label={label}
      style={{
        fontFamily: "var(--font-money)",
        fontVariantNumeric: "tabular-nums",
        fontWeight: heavy ? "var(--fw-bold)" : "var(--fw-semibold)",
        fontSize: SIZE_FS[size] || SIZE_FS.md,
        letterSpacing: size === "display" ? "-0.01em" : "0",
        whiteSpace: "nowrap",
        color,
      }}
    >
      {formatBRL(cents, hideCents)}
    </span>
  );
}
