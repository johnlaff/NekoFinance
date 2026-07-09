/** Helpers puros do cenário "e se" (plano 072, fatia C) — separados de `screens/scenarios.tsx`
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
