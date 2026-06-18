import type { MonthMetric } from "../lib/api";
import type { HealthLevel } from "../design-system/components/HealthBadge";

export interface Status {
  level: HealthLevel;
  label: string;
}

// Piso 20% para o badge MENSAL "Dentro do ideal" — um mês pode variar dentro da faixa 20–30% do
// método. O guardrail ANUAL "pode gastar hoje" usa 25% (alvo médio da faixa) em
// src-tauri/src/commands.rs (SAVINGS_TARGET_BPS). Divergência deliberada: indicador mensal leniente,
// gate anual mais firme; ambos dentro da faixa canônica 20–30%.
const SAVINGS_TARGET_BPS = 2000;

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
  // 20–30% é o alvo; abaixo de 20% fica aquém. "Dentro do ideal" é verbatim; "Acima/Abaixo" são
  // copy do Neko para nomear os estados que o método só descreve.
  if (bps > 3000) return { level: "steady", label: "Acima do ideal" };
  if (bps >= SAVINGS_TARGET_BPS) return { level: "strong", label: "Dentro do ideal" };
  return { level: "watch", label: "Abaixo do ideal" };
}

export function custoVidaStatus(cost: number, income: number): Status {
  return cost <= income
    ? { level: "steady", label: "Dentro da renda" } // verbatim do método
    : { level: "watch", label: "Acima da renda" }; // copy do Neko (estado vermelho)
}
