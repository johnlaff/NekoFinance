/**
 * Neko Finance — Redesign shared helpers.
 *
 * Pure, method-canonical helpers reused by the redesigned screens (Hoje, Este mês,
 * O ano, Calendário, Horizonte, Tags, Mia, Lançamentos, Compose). Mirrors the
 * vocabulary the design prototype (window.NK) used, but lives in the app and reuses
 * `lib/format` for money. No data fetching here — screens wire real data from `lib/api`.
 */
import { formatBRL } from "./format";

export type MovementType = "entrada" | "saida" | "diario" | "economia" | "cartao";

export const MES = [
  "Janeiro",
  "Fevereiro",
  "Março",
  "Abril",
  "Maio",
  "Junho",
  "Julho",
  "Agosto",
  "Setembro",
  "Outubro",
  "Novembro",
  "Dezembro",
];
export const MES_ABBR = [
  "Jan",
  "Fev",
  "Mar",
  "Abr",
  "Mai",
  "Jun",
  "Jul",
  "Ago",
  "Set",
  "Out",
  "Nov",
  "Dez",
];

export const fmtBRL = formatBRL;

/** "+R$ …" para positivos; mantém o "−" real para negativos. */
export function fmtSigned(cents: number, hideCents = false): string {
  return cents > 0 ? "+" + formatBRL(cents, hideCents) : formatBRL(cents, hideCents);
}

/** R$ compacto no estilo do redesign: "R$ 1,2 mil", "R$ 38,6 mil", "−R$ 1,4M". */
export function fmtCompact(cents: number): string {
  const neg = cents < 0;
  const abs = Math.abs(cents);
  const sign = neg ? "−" : "";
  if (abs >= 100_000_000)
    return `${sign}R$ ${(abs / 100_000_000).toFixed(1).replace(".", ",")}M`;
  if (abs >= 100_000)
    return `${sign}R$ ${(abs / 100_000).toFixed(1).replace(".", ",")} mil`;
  return formatBRL(cents);
}

/** Mês (0–11) e dia a partir de um ISO YYYY-MM-DD. */
export function monthOf(iso: string): number {
  const p = iso.split("-")[1];
  return p ? parseInt(p, 10) - 1 : 0;
}
export function dayOf(iso: string): number {
  const p = iso.split("-")[2];
  return p ? parseInt(p, 10) : 0;
}

export interface SaldoBand {
  key: "none" | "critical" | "negative" | "tight" | "ok" | "comfortable";
  fill: string;
  text: string;
  label: string;
}

/**
 * Termômetro de saldo — limiares ABSOLUTOS em centavos (canônico da planilha de ensino):
 * < −R$500 crítico · < R$0 negativo · < R$1000 apertado · < R$2000 OK · ≥ R$2000 folga.
 * NÃO relativizar ao baseline.
 */
export function saldoBand(cents: number | null | undefined): SaldoBand {
  if (cents == null)
    return { key: "none", fill: "transparent", text: "var(--text-faint)", label: "" };
  if (cents < -50000)
    return {
      key: "critical",
      fill: "var(--saldo-band-critical-fill)",
      text: "var(--danger-400)",
      label: "Crítico",
    };
  if (cents < 0)
    return {
      key: "negative",
      fill: "var(--saldo-band-negative-fill)",
      text: "var(--danger-400)",
      label: "Negativo",
    };
  if (cents < 100000)
    return {
      key: "tight",
      fill: "var(--saldo-band-tight-fill)",
      text: "var(--warning-400)",
      label: "Apertado",
    };
  if (cents < 200000)
    return {
      key: "ok",
      fill: "var(--saldo-band-ok-fill)",
      text: "var(--success-400)",
      label: "OK",
    };
  return {
    key: "comfortable",
    fill: "var(--saldo-band-comfortable-fill)",
    text: "var(--success-400)",
    label: "Folga",
  };
}

export interface TypeMeta {
  name: string;
  color: string;
  glyph: string;
}

/** Os 5 tipos de movimento do método, com cor (token) e glifo. */
export const TYPE_META: Record<MovementType, TypeMeta> = {
  entrada: { name: "Entrada", color: "var(--type-entrada)", glyph: "E" },
  saida: { name: "Saída", color: "var(--type-saida)", glyph: "S" },
  diario: { name: "Diário", color: "var(--type-diario)", glyph: "D" },
  economia: { name: "Economia", color: "var(--type-economia)", glyph: "P" },
  cartao: { name: "Cartão", color: "var(--type-cartao)", glyph: "C" },
};
