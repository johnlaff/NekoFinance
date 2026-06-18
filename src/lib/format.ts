/** Formats INTEGER cents as localized BRL currency (e.g. 123456 → "R$ 1.234,56"). */
export function fmtBRL(cents: number): string {
  const reais = cents / 100;
  return reais.toLocaleString("pt-BR", {
    style: "currency",
    currency: "BRL",
  });
}

/**
 * Parses a pt-BR money string ("1.234,56", "R$ 950", "42,5") into INTEGER cents.
 * Returns null when the input has no parseable amount.
 */
export function parseBRLToCents(input: string): number | null {
  const cleaned = input
    .replace(/[R$\s]/g, "")
    .replace(/\./g, "")
    .replace(",", ".");
  if (!cleaned || !/^-?\d+(\.\d+)?$/.test(cleaned)) return null;
  return Math.round(Number(cleaned) * 100);
}

/** Formats an ISO 8601 date (YYYY-MM-DD) as DD/MM/YYYY. Empty input stays empty. */
export function fmtDate(iso: string): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}

/** Formats an ISO 8601 date (YYYY-MM-DD) as DD/MM. Empty input stays empty. */
export function fmtDayMonth(iso: string): string {
  if (!iso) return "";
  const [, m, d] = iso.split("-");
  return `${d}/${m}`;
}

/** Lower-case pt-BR month name ("junho") for an ISO 8601 date. */
export function monthNamePtBR(iso: string): string {
  const [y, m] = iso.split("-").map(Number);
  if (!y || !m) return "";
  return new Date(Date.UTC(y, m - 1, 1)).toLocaleDateString("pt-BR", {
    month: "long",
    timeZone: "UTC",
  });
}

/**
 * Formata centavos BRL → "R$ 1.234,56" com sinal de menos real (−, U+2212).
 * Formatador próprio do Design System (usado por `<Money>`, TransactionRow, CardChip…).
 */
export function formatBRL(cents: number, hideCents = false): string {
  const neg = cents < 0;
  const v = Math.abs(cents) / 100;
  const s = v.toLocaleString("pt-BR", {
    minimumFractionDigits: hideCents ? 0 : 2,
    maximumFractionDigits: hideCents ? 0 : 2,
  });
  // Espaco apos R$ e um NBSP (U+00A0), que cola o simbolo ao numero; menos real e U+2212.
  return (neg ? "−R$ " : "R$ ") + s;
}

/** R$ compacto para rótulos de gráfico: "R$ 5.8k", "−R$ 320". Minus tipográfico (U+2212). */
export function fmtCompactBRL(cents: number): string {
  const v = cents / 100;
  const abs = Math.abs(v);
  const sign = v < 0 ? "−" : "";
  if (abs >= 1000) return `${sign}R$ ${(abs / 1000).toFixed(abs >= 10000 ? 0 : 1)}k`;
  return `${sign}R$ ${abs.toFixed(0)}`;
}
