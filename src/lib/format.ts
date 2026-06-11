/** Formats INTEGER cents as localized BRL currency (e.g. 123456 → "R$ 1.234,56"). */
export function fmtBRL(cents: number): string {
  const reais = cents / 100;
  return reais.toLocaleString("pt-BR", {
    style: "currency",
    currency: "BRL",
  });
}

/** Formats an ISO 8601 date (YYYY-MM-DD) as DD/MM/YYYY. Empty input stays empty. */
export function fmtDate(iso: string): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}
