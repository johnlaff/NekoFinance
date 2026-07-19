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

/** Centavos → valor editável pt-BR, aceito por `parseBRLToCents`. */
export function centsToBRLInput(cents: number): string {
  return (cents / 100).toFixed(2).replace(".", ",");
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

/**
 * R$ compacto SEM decimais para rótulos de PONTO num gráfico SVG apertado (mín/máx do
 * `BalanceTrajectory`) — "R$ 31 mil", "R$ 1 mi", nunca "k"/"M". `fmtCompactBRL` de
 * `nkFormat.ts` formata manchetes com uma casa decimal; `fmtAxisBRL` mantém rótulos de gráfico
 * sem decimal, com a precisão cheia no tooltip e no `aria-label` do próprio gráfico.
 *
 * A faixa é decidida DEPOIS do arredondamento na precisão da própria faixa (mesma regra do
 * `fmtCompactBRL` de `nkFormat.ts`): arredondar pode cruzar o limiar da faixa seguinte, e sem
 * a promoção o resultado sai no registro errado — R$ 999.500,00 arredondado a milhares vira
 * "R$ 1.000 mil" (absurdo; o certo é "R$ 1 mi") e R$ 999,50 arredondado a reais inteiros vira
 * "R$ 1.000" enquanto R$ 1.000,00 exato vira "R$ 1 mil" — mesma magnitude, dois registros.
 */
export function fmtAxisBRL(cents: number): string {
  const neg = cents < 0;
  const abs = Math.abs(cents);
  const sign = neg ? "−" : "";
  if (abs >= 100_000_000)
    return `${sign}R$ ${Math.round(abs / 100_000_000).toLocaleString("pt-BR")} mi`;
  if (abs >= 100_000) {
    // Promoção pós-arredondamento: milhares que arredondam para 1.000 mil pertencem à
    // faixa "mi" — nunca "R$ 1.000 mil".
    const mil = Math.round(abs / 100_000);
    if (mil >= 1_000)
      return `${sign}R$ ${Math.round(abs / 100_000_000).toLocaleString("pt-BR")} mi`;
    return `${sign}R$ ${mil.toLocaleString("pt-BR")} mil`;
  }
  // Promoção pós-arredondamento: reais que arredondam para R$ 1.000 rendem no registro
  // "mil" — nunca dois registros para a mesma magnitude.
  const reais = Math.round(abs / 100);
  if (reais >= 1_000)
    return `${sign}R$ ${Math.round(abs / 100_000).toLocaleString("pt-BR")} mil`;
  return `${sign}R$ ${reais.toLocaleString("pt-BR")}`;
}
