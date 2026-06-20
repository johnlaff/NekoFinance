/**
 * Hoje no fuso LOCAL como ISO `YYYY-MM-DD`. Usa a data de PAREDE (getFullYear/Month/Date), não
 * `toISOString()` (UTC): à noite no Brasil (UTC-3) o UTC já virou o dia seguinte, e um lançamento
 * feito depois das 21h era datado como amanhã — quebrando "Diário de hoje" e marcando um gasto
 * realizado como projeção futura no backend (que compara com a data LOCAL).
 */
export function todayISO(): string {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
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

/** Formats an ISO 8601 date (YYYY-MM-DD) as DD/MM/YYYY. Malformed input is returned as-is (não
 * monta "undefined/...": melhor expor o valor inesperado do que mascará-lo). */
export function fmtDate(iso: string): string {
  const parts = iso.split("-");
  if (parts.length < 3) return iso;
  const [y, m, d] = parts;
  return `${d}/${m}/${y}`;
}

/** Formats an ISO 8601 date (YYYY-MM-DD) as DD/MM. Malformed input is returned as-is. */
export function fmtDayMonth(iso: string): string {
  const parts = iso.split("-");
  if (parts.length < 3) return iso;
  const [, m, d] = parts;
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
