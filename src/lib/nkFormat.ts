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

/**
 * R$ compacto para a MANCHETE de um card de KPI (plano 074, fatia A) — palavras por extenso
 * ("mil"/"mi"), nunca abreviação em letra ("k"/"M"): esse é o registro BR correto para um
 * valor de destaque na UI. Não confundir com `fmtAxisBRL` de `lib/format.ts` (plano 074,
 * fatia C — renomeado do antigo `fmtCompactBRL` daquele módulo, que violava o registro BR
 * com estilo "k": "R$ 5.8k") — aquele é o rótulo de PONTO do `BalanceTrajectory`, sem casa
 * decimal (o gráfico disputa espaço com a própria linha); este aqui tem 1 casa decimal,
 * apropriado ao respiro de um card. Nomes diferentes agora, mas o mesmo cuidado: nunca "k"/"M".
 *
 * Faixas (arredondamento meio-para-cima, nunca bancário):
 * - ≥ R$ 1.000.000 → 1 casa decimal + "mi" ("R$ 1,2 mi")
 * - ≥ R$ 10.000    → 1 casa decimal + "mil" ("R$ 30,8 mil")
 * - ≥ R$ 1.000      → reais inteiros, sem centavos ("R$ 7.632")
 * - abaixo de R$ 1.000 → valor cheio, mas sem ",00" quando o centavo é zero ("R$ 180"
 *   em vez de "R$ 180,00"; "R$ 193,22" quando há centavo real)
 *
 * A faixa é decidida DEPOIS do arredondamento na precisão da própria faixa: arredondar pode
 * cruzar o limiar da faixa seguinte, e sem a promoção o resultado sai no registro errado —
 * R$ 999.950,00 arredondado a décimos de mil vira "1.000,0 mil" (absurdo; o certo é
 * "R$ 1,0 mi") e R$ 9.999,99 arredondado a reais inteiros vira "R$ 10.000" enquanto
 * R$ 10.000,00 exato vira "R$ 10,0 mil" — mesma magnitude, dois registros. A faixa "abaixo de
 * R$ 1.000" nunca arredonda (centavos exatos), então não tem promoção.
 *
 * A precisão total nunca se perde: ela mora na linha de evidência (`real → cenário`, ambos em
 * `<Money>` de precisão cheia) e no `aria-label` do card — este formatador é só a leitura
 * visual rápida.
 */
export function fmtCompactBRL(cents: number): string {
  const abs = Math.abs(cents);
  if (abs >= 100_000_000) return `${fmtScaledUnit(cents, 10_000_000)} mi`;
  if (abs >= 1_000_000) {
    // Promoção pós-arredondamento: se o arredondamento a décimos de mil alcança 1.000,0 mil
    // (mesma conta de `fmtScaledUnit(cents, 10_000)`), o valor pertence à faixa "mi".
    if (Math.round(abs / 10_000) >= 10_000)
      return `${fmtScaledUnit(cents, 10_000_000)} mi`;
    return `${fmtScaledUnit(cents, 10_000)} mil`;
  }
  if (abs >= 100_000) {
    // Promoção pós-arredondamento: reais inteiros que arredondam para R$ 10.000 rendem no
    // registro "mil" — nunca dois registros para a mesma magnitude.
    if (Math.round(abs / 100) >= 10_000) return `${fmtScaledUnit(cents, 10_000)} mil`;
    return formatBRL(cents, true);
  }
  // Abaixo de R$1.000: valor cheio, mas sem ",00" quando os centavos são zero.
  return formatBRL(cents, abs % 100 === 0);
}

/** "R$ 30,8"/"−R$ 1,2" — divide `cents` por `unitCents` para chegar num inteiro de DÉCIMOS
 *  (arredondado meio-para-cima) antes de formatar como texto. Trabalhar em inteiro evita as
 *  armadilhas de ponto-flutuante do `toFixed` (ex.: `(1.005).toFixed(2)` vira "1.00" no V8). */
function fmtScaledUnit(cents: number, unitCents: number): string {
  const neg = cents < 0;
  const tenths = Math.round(Math.abs(cents) / unitCents);
  const whole = Math.trunc(tenths / 10);
  const frac = tenths % 10;
  // NBSP (U+00A0) cola "R$" ao número — mesma convenção de `formatBRL` (nunca quebra linha
  // entre o símbolo e o valor).
  return `${neg ? "−" : ""}R$\u00A0${whole.toLocaleString("pt-BR")},${frac}`;
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
