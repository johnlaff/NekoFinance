/** Helpers puros do cenário "e se" — separados de `screens/scenarios.tsx`
 * (arquivo de componentes) para o Fast Refresh preservar estado (react-doctor
 * `only-export-components`) e para ficarem testáveis isoladamente, no mesmo espírito de
 * `lib/nkFormat.ts`/`lib/movement.ts`. */

/** Remove os sufixos de marca (`#loan:<...>`/`#repl:<...>`) do FIM da descrição — a UI nunca
 * mostra o marcador cru (ver convenções em `src-tauri/src/scenarios.rs`). Ancorado ao fim,
 * como o parser do backend (`parse_loan_marker`/`parse_repl_marker`): um "#loan:" literal
 * digitado pelo usuário no MEIO do texto é dado dele e fica intacto. Vários marcadores no
 * fim são removidos um a um — regex sem quantificador aninhado (linear, sem backtracking
 * exponencial em entrada adversarial). */
const TRAILING_MARKER = /\s*#(?:loan|repl):\S+$/;
export function stripScenarioMarker(description: string): string {
  let out = description;
  while (TRAILING_MARKER.test(out)) {
    out = out.replace(TRAILING_MARKER, "");
  }
  return out.trim();
}

export interface LoanMarker {
  groupId: string;
  rateBps: number;
}

/** O inverso de `stripScenarioMarker` para o marcador `#loan`: INTERPRETA o sufixo
 * (` #loan:<groupId>:<rateBps>`) em vez de apagá-lo, para a UI reconhecer as linhas irmãs de
 * um mesmo empréstimo e agrupá-las. Ancorado ao FIM como o parser do backend — um "#loan:"
 * no meio do texto é dado do usuário, não marcador. */
const LOAN_MARKER = /\s#loan:([^\s:]+):(\d+)$/;
export function parseLoanMarker(description: string): LoanMarker | null {
  const m = LOAN_MARKER.exec(description);
  if (!m) return null;
  return { groupId: m[1]!, rateBps: parseInt(m[2]!, 10) };
}

/** Espaço mínimo (px, no espaço do viewBox) entre as linhas de base dos rótulos "Real" e
 *  "Simulação" do gráfico de trajetória — abaixo disso as duas legendas coladas ficam
 *  ilegíveis mesmo com o halo. */
export const CHART_LABEL_MIN_GAP = 14;

export interface EndLabelYs {
  realLabelY: number;
  scenarioLabelY: number;
}

/**
 * Posiciona os rótulos de fim de linha ("Real"/"Simulação") do gráfico de trajetória.
 *
 * Direction-aware: a linha que termina visualmente mais alta (y menor) ganha o rótulo ACIMA
 * do próprio traço; a outra fica abaixo — em vez de "Real sempre em cima", que colidia quando
 * o cenário terminava mais alto que o real.
 *
 * O clamp aos limites verticais é do PAR, não de cada rótulo isolado: clampar cada um por
 * conta própria pode comprimir o vão de volta (ex.: traços em y=22/y=20 perto do topo — o
 * rótulo de cima é empurrado para baixo pelo limite superior e cola no de baixo). Aqui o
 * rótulo superior é clampado primeiro, o inferior deriva dele mantendo o vão mínimo; se o
 * inferior estourar o limite de baixo, o PAR sobe junto — e se a janela não comportar os
 * dois, o vão de 14px vence o limite (rótulo levemente fora da moldura é legível; dois
 * rótulos fundidos não são).
 */
export function placeChartEndLabels(
  realY: number,
  scenarioY: number,
  minY: number,
  maxY: number,
): EndLabelYs {
  const realIsUpper = realY <= scenarioY;
  let upper: number;
  let lower: number;
  if (realIsUpper) {
    upper = realY - 8;
    lower = Math.max(scenarioY + 14, upper + CHART_LABEL_MIN_GAP);
  } else {
    upper = scenarioY - 8;
    lower = Math.max(realY + 14, upper + CHART_LABEL_MIN_GAP);
  }
  // Clamp do par: superior ao teto primeiro, inferior re-derivado preservando o vão.
  upper = Math.max(upper, minY);
  lower = Math.max(lower, upper + CHART_LABEL_MIN_GAP);
  if (lower > maxY) {
    // Estourou embaixo: o par inteiro sobe junto (o vão não comprime).
    lower = maxY;
    upper = lower - CHART_LABEL_MIN_GAP;
    if (upper < minY) {
      // Janela menor que o vão: prioriza os 14px (o inferior pode passar do limite).
      upper = minY;
      lower = upper + CHART_LABEL_MIN_GAP;
    }
  }
  return realIsUpper
    ? { realLabelY: upper, scenarioLabelY: lower }
    : { realLabelY: lower, scenarioLabelY: upper };
}

export interface ChartScale {
  /** Piso do domínio (centavos) — sempre ≤ menor dado. */
  min: number;
  /** Teto do domínio (centavos) — sempre ≥ maior dado. */
  max: number;
  /** Ticks igualmente espaçados, múltiplos 1–2–5 de potência de 10, ancorados no zero. */
  ticks: number[];
}

/** Passo "nice" (1–2–5 × 10^n) imediatamente ≥ `rough`. */
function niceStep(rough: number): number {
  const pow = 10 ** Math.floor(Math.log10(rough));
  const m = rough / pow;
  if (m <= 1) return pow;
  if (m <= 2) return 2 * pow;
  if (m <= 5) return 5 * pow;
  return 10 * pow;
}

/** Range mínimo para série constante: 5% do valor, nunca menos que R$1.000 — uma linha reta
 *  precisa de moldura em volta, não de um domínio de altura zero. */
const CONSTANT_HALF_FLOOR = 100_000;

/**
 * Domínio "nice" do gráfico de comparação (valores em centavos): ticks 1–2–5 legíveis e a
 * regra do ZERO CONDICIONAL — o zero entra quando a série o cruza/toca, ou quando ele cai a
 * até UM passo do domínio (aí o eixo estende até 0 em vez de truncar a um triz dele); longe
 * disso o domínio fica truncado nos dados. Um gráfico de LINHA existe para mostrar variação,
 * não magnitude — forçar o 0 num saldo que vive em R$ 30 mil esmaga as duas linhas numa
 * faixa de pixels (o defeito medido); mas o 0 de um saldo é o limiar "fura o caixa", então
 * quando ele está perto/na série, escondê-lo mentiria por omissão.
 */
export function niceChartScale(values: number[], targetTicks = 3): ChartScale {
  // Série vazia rende o mesmo domínio da série constante em 0 — nunca um eixo
  // degenerado (max sem tick correspondente).
  if (values.length === 0) return niceChartScale([0], targetTicks);
  let dataMin = Math.min(...values);
  let dataMax = Math.max(...values);

  if (dataMin === dataMax) {
    const half = Math.max(Math.round(Math.abs(dataMin) * 0.05), CONSTANT_HALF_FLOOR);
    dataMin -= half;
    dataMax += half;
  }

  // Padding pequeno ANTES do arredondamento: entrada da função de ticks, não o domínio final
  // (linhas coladas na borda são o sintoma; ticks estranhos seriam o efeito colateral).
  const pad = (dataMax - dataMin) * 0.05;
  const lo = dataMin - pad;
  const hi = dataMax + pad;

  let step = niceStep((hi - lo) / targetTicks);
  let min = Math.floor(lo / step) * step;
  let max = Math.ceil(hi / step) * step;

  // Zero condicional sobre os limites já arredondados:
  if (dataMin >= 0 && min < 0) min = 0; // padding nunca inventa domínio negativo
  if (dataMax <= 0 && max > 0) max = 0; // …nem positivo, para série toda negativa
  if (dataMin >= 0 && min > 0 && min <= step) min = 0; // zero a ≤1 passo → estende
  if (dataMax <= 0 && max < 0 && -max <= step) max = 0;

  if (min === max) max = min + step;

  // Guarda de densidade: nunca mais que 6 ticks (re-arredonda o passo pro range final).
  while ((max - min) / step > 5) {
    step = niceStep(step * 1.5);
    min = Math.floor(min / step) * step;
    max = Math.ceil(max / step) * step;
  }

  const ticks: number[] = [];
  for (let t = min; t <= max; t += step) ticks.push(t === 0 ? 0 : t); // normaliza -0
  return { min: min === 0 ? 0 : min, max: max === 0 ? 0 : max, ticks };
}

/** Soma `n` meses a uma data ISO ("YYYY-MM-DD"), preservando o dia quando possível (satura no
 * último dia do mês de destino — ex.: 31/jan + 1 mês = 28 ou 29/fev). */
export function addMonthsISO(iso: string, n: number): string {
  const [y, m, d] = iso.split("-").map((s) => parseInt(s, 10));
  const base = new Date(Date.UTC(y ?? 1970, (m ?? 1) - 1 + n, 1));
  const daysInMonth = new Date(
    Date.UTC(base.getUTCFullYear(), base.getUTCMonth() + 1, 0),
  ).getUTCDate();
  base.setUTCDate(Math.min(d ?? 1, daysInMonth));
  const yy = base.getUTCFullYear();
  const mm = String(base.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(base.getUTCDate()).padStart(2, "0");
  return `${yy}-${mm}-${dd}`;
}
