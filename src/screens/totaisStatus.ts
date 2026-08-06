import type { MonthMetric } from "../lib/api";
import type { HealthLevel } from "../design-system/components/HealthBadge";
import { MES, MES_ABBR } from "../lib/nkFormat";

export interface Status {
  level: HealthLevel;
  label: string;
}

// Piso de 20% (2000 bps) do método — fonte única para os indicadores e visuais MENSAIS e ANUAIS:
// badge "Dentro do ideal" (este arquivo) e cor da visão anual (AnnualScreen). Um mês pode variar
// dentro da faixa 20–30%, então estes são lenientes.
// É o MESMO critério do guardrail anual "pode gastar hoje" (`SAVINGS_FLOOR_BPS` em
// src-tauri/src/forecast/mod.rs): uma barra só, porque a faixa 20–30% é média anual e é o piso
// que diz se o ano ainda está dentro dela.
export const SAVINGS_MIN_BPS = 2000;

/** Encontra a métrica do mês corrente a partir do `today` do forecast. */
export function currentMonthMetric(
  months: MonthMetric[],
  today: string,
): MonthMetric | null {
  const [y, m] = today.split("-").map(Number);
  return months.find((x) => x.year === y && x.month === m) ?? null;
}

// Proveniência dos rótulos (fidelidade ao método):
// - Performance: "Sobrou dinheiro" / "Faltou dinheiro" — AMBOS verbatim do método (par confirmado).
// - "Dentro do ideal" (economizado) e "Dentro da renda" (custo de vida): os ESTADOS POSITIVOS são
//   verbatim do método. Os estados negativos abaixo ("Abaixo do ideal", "Acima da renda") são copy
//   PRÓPRIA do Neko para o estado vermelho — o método só registra o rótulo positivo. Mantidos
//   porque a UI precisa nomear o estado ruim; não os atribua ao método.
export function performanceStatus(cents: number): Status {
  return cents >= 0
    ? { level: "strong", label: "Sobrou dinheiro" }
    : { level: "risk", label: "Faltou dinheiro" };
}

export function economizadoStatus(bps: number): Status {
  // Faixa do método "20 a 30": acima de 30% é guardar além do ideal (pode alocar em outro lugar);
  // 20–30% é o alvo; abaixo de 20% fica aquém; zero tem nome próprio — "Nada guardado" é estado
  // distinto de "Abaixo do ideal" (guardou algo, só que menos que o piso). "Dentro do ideal",
  // "Acima do ideal" e "Nada guardado" são verbatim do método; "Abaixo do ideal" é copy do Neko.
  if (bps > 3000) return { level: "steady", label: "Acima do ideal" };
  if (bps >= SAVINGS_MIN_BPS) return { level: "strong", label: "Dentro do ideal" };
  if (bps > 0) return { level: "watch", label: "Abaixo do ideal" };
  return { level: "watch", label: "Nada guardado" };
}

export function custoVidaStatus(cost: number, income: number): Status {
  return cost <= income
    ? { level: "steady", label: "Dentro da renda" } // verbatim do método
    : { level: "watch", label: "Acima da renda" }; // copy do Neko (estado vermelho)
}

/** Percentual do método em exibição: TRUNCA (nunca arredonda para cima do veredito). */
export function pctDisplay(bps: number): number {
  return Math.trunc(bps / 100);
}

/**
 * A leitura da série histórica diz o FATO da janela, nunca julga um mês isolado
 * (a régua do método é a média anual; mês fraco não é veredito).
 */
export function serieLeitura(trend: MonthMetric[]): string {
  if (trend.length <= 1) return "Sem meses anteriores para comparar ainda.";
  if (trend.every((t) => t.savings_rate_bps === 0)) {
    return `O economizado está em zero nos últimos ${trend.length} meses — é o mesmo zero em todos, não uma queda.`;
  }
  const best = trend.reduce((a, b) =>
    b.savings_rate_bps >= a.savings_rate_bps ? b : a,
  );
  const min = Math.min(...trend.map((t) => t.savings_rate_bps));
  const max = Math.max(...trend.map((t) => t.savings_rate_bps));
  const first = trend[0]!;
  const last = trend[trend.length - 1]!;
  return `Entre ${MES_ABBR[first.month - 1]} e ${MES_ABBR[last.month - 1]}, o economizado foi de ${pctDisplay(min)}% a ${pctDisplay(max)}% — o melhor mês foi ${MES[best.month - 1]}.`;
}
