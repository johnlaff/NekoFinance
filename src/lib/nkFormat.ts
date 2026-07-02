/**
 * Neko Finance — Redesign shared helpers.
 *
 * Pure, method-canonical helpers reused by the redesigned screens (Hoje, Este mês,
 * O ano, Calendário, Horizonte, Tags, Mia, Lançamentos, Compose). Mirrors the
 * vocabulary the design prototype (window.NK) used, but lives in the app and reuses
 * `lib/format` for money. No data fetching here — screens wire real data from `lib/api`.
 */
import { formatBRL } from "./format";
import {
  saldoBand as classifySaldoBand,
  SALDO_BAND_FILL,
  type SaldoBand as SaldoBandKey,
} from "./saldoHeatmap";

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

export interface SaldoBand {
  key: "none" | "critical" | "negative" | "tight" | "ok" | "comfortable";
  fill: string;
  text: string;
  label: string;
}

/** Cor de texto e rótulo pt-BR por faixa do termômetro (apresentação; a classificação
 *  canônica vive em `saldoHeatmap.ts`). */
const SALDO_BAND_UI: Record<SaldoBandKey, { text: string; label: string }> = {
  critical: { text: "var(--danger-400)", label: "Crítico" },
  negative: { text: "var(--danger-400)", label: "Negativo" },
  tight: { text: "var(--warning-400)", label: "Apertado" },
  ok: { text: "var(--success-400)", label: "OK" },
  comfortable: { text: "var(--success-400)", label: "Folga" },
};

/**
 * Termômetro de saldo — limiares ABSOLUTOS em centavos (canônico da planilha de ensino):
 * < −R$500 crítico · < R$0 negativo · ≤ R$1000 apertado · ≤ R$2000 OK · > R$2000 folga.
 * Fronteiras inclusivas (R$1.000 exato = apertado; R$2.000 exato = OK), como na formatação
 * condicional da planilha. A classificação delega para `saldoHeatmap.saldoBand` — única
 * fonte da regra. NÃO relativizar ao baseline.
 */
export function saldoBand(cents: number | null | undefined): SaldoBand {
  if (cents == null)
    return { key: "none", fill: "transparent", text: "var(--text-faint)", label: "" };
  const key = classifySaldoBand(cents);
  return { key, fill: SALDO_BAND_FILL[key], ...SALDO_BAND_UI[key] };
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
