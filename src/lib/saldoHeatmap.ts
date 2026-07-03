/**
 * Heatmap da coluna Saldo — termômetro fiel à planilha de referência.
 *
 * A planilha de referência pinta a coluna Saldo (E, K, Q… BS) por formatação condicional com limiares
 * ABSOLUTOS em reais: quanto maior o saldo mais verde; quanto menor, mais perto do vermelho. As
 * faixas abaixo reproduzem exatamente essas regras (operadores e valores conferidos célula a célula):
 *
 *   Saldo > R$ 2.000        → verde forte   (folga)
 *   R$ 1.000 a R$ 2.000     → verde claro    (ok / positivo)
 *   R$ 0 a R$ 1.000         → âmbar          (apertado / atenção)
 *   R$ 0 a −R$ 499,99       → vermelho claro (negativo / alerta)
 *   abaixo de −R$ 500       → vermelho forte (crítico / perigo)
 *
 * São limiares absolutos de propósito: é assim que o usuário lê "estou bem ou não" de relance. Ficam
 * em um único objeto para virar configuráveis por usuário no futuro (escalas de renda diferentes), sem
 * mudar a semântica — o default espelha a planilha de referência.
 */

export type SaldoBand = "critical" | "negative" | "tight" | "ok" | "comfortable";

/** Limiares (em centavos) das faixas do termômetro. */
export interface SaldoBandThresholds {
  /** abaixo disto = vermelho forte (crítico) */
  critical: number;
  /** abaixo de 0 (mas acima de crítico) = vermelho claro (negativo) */
  positive: number;
  /** até aqui = âmbar (apertado) */
  tight: number;
  /** até aqui = verde claro (ok); acima = verde forte (folga) */
  ok: number;
}

/** Default = formatação condicional da planilha de referência. */
const SALDO_BAND_THRESHOLDS_CENTS: SaldoBandThresholds = {
  critical: -50_000, // −R$ 500,00
  positive: 0,
  tight: 100_000, // R$ 1.000,00
  ok: 200_000, // R$ 2.000,00
};

/**
 * Classifica um saldo (em centavos) numa das cinco faixas do termômetro, com os limiares absolutos da
 * planilha. As fronteiras seguem as prioridades das regras da planilha (R$ 1.000 cai em "apertado",
 * R$ 2.000 cai em "ok").
 */
export function saldoBand(
  cents: number,
  t: SaldoBandThresholds = SALDO_BAND_THRESHOLDS_CENTS,
): SaldoBand {
  if (cents < t.critical) return "critical";
  if (cents < t.positive) return "negative";
  if (cents <= t.tight) return "tight";
  if (cents <= t.ok) return "ok";
  return "comfortable";
}

/** Fundo da célula por faixa (tokens dark-first com paridade clara; >=4.5:1 com `--text` por cima). */
export const SALDO_BAND_FILL: Record<SaldoBand, string> = {
  critical: "var(--saldo-band-critical-fill)",
  negative: "var(--saldo-band-negative-fill)",
  tight: "var(--saldo-band-tight-fill)",
  ok: "var(--saldo-band-ok-fill)",
  comfortable: "var(--saldo-band-comfortable-fill)",
};

export const SALDO_BAND_LEGEND: { band: SaldoBand; label: string }[] = [
  { band: "comfortable", label: "folga" },
  { band: "ok", label: "ok" },
  { band: "tight", label: "apertado" },
  { band: "negative", label: "negativo" },
  { band: "critical", label: "crítico" },
];

export const SALDO_BAND_LABEL: Record<SaldoBand, string> = Object.fromEntries(
  SALDO_BAND_LEGEND.map((l) => [l.band, l.label]),
) as Record<SaldoBand, string>;
