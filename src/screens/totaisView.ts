import {
  getAnnualMetrics,
  getDashboardSummary,
  getForecast,
  ownerTotalsForMonth,
  type AnnualMetrics,
  type DashboardSummary,
  type Forecast,
  type MonthMetric,
  type OwnerTotal,
} from "../lib/api";
import type { HealthLevel } from "../design-system/components/HealthBadge";
import { MES, MES_ABBR } from "../lib/nkFormat";

// View-model puro da tela Este mês (Totais): os status de método por régua (Performance,
// Economizado, Custo de vida), a leitura da série histórica e a costura do mês visto entre o
// realizado (annual metrics) e a projeção (forecast). É também a porta inteira do shim para a
// tela (ADR-0007): tipos reexportados, fetchers estáveis e a convenção de chave de cache do
// `useCommand` — a tela nunca importa `lib/api`.

// Tipos do shim reexportados pela view — a tela e seu teste leem daqui.
export type { AnnualMetrics, DashboardSummary, Forecast, MonthMetric, OwnerTotal };

export interface Status {
  level: HealthLevel;
  label: string;
}

// Piso de 20% (2000 bps) do método — fonte única para os indicadores e visuais MENSAIS e ANUAIS:
// badge "Dentro do ideal" (este arquivo) e cor da visão anual (anoView.ts). Um mês pode variar
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

// ---------------------------------------------------------------------------
// Costura do mês visto: o realizado (annual metrics, meses antes de hoje) com a projeção
// (forecast, hoje em diante) — a mesma linha nunca vem das duas fontes.
// ---------------------------------------------------------------------------

/** "YYYY-MM" a partir de uma MonthMetric. */
export function ymOf(m: { year: number; month: number }): string {
  return `${m.year}-${String(m.month).padStart(2, "0")}`;
}

export function mergePastAnnualWithForecastMonths(
  annualMonths: MonthMetric[],
  forecastMonths: MonthMetric[],
  today: string,
): MonthMetric[] {
  const todayYm = today.slice(0, 7);
  const byMonth = new Map<string, MonthMetric>();
  for (const month of annualMonths) {
    if (ymOf(month) < todayYm) byMonth.set(ymOf(month), month);
  }
  for (const month of forecastMonths) {
    byMonth.set(ymOf(month), month);
  }
  return Array.from(byMonth.values()).toSorted(
    (a, b) => a.year - b.year || a.month - b.month,
  );
}

// ---------------------------------------------------------------------------
// Leitura — fetchers com identidade estável por chave (o contrato do useCommand rejeita
// closures novas a cada render) e a convenção da chave de cache do `useCommand`.
// ---------------------------------------------------------------------------

export function fetchForecast(): Promise<Forecast> {
  return getForecast();
}

export function fetchDashboardSummary(): Promise<DashboardSummary> {
  return getDashboardSummary();
}

export function annualMetricsCacheKey(year: number): string {
  return `annual_metrics:${year}:totais`;
}

const _annualFetcherCache = new Map<number, () => Promise<AnnualMetrics>>();
export function annualMetricsFetcher(year: number): () => Promise<AnnualMetrics> {
  const cached = _annualFetcherCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _annualFetcherCache.set(year, fn);
  return fn;
}

export function ownerTotalsCacheKey(year: number, month: number): string {
  return `owner_totals_for_month:${year}:${month}`;
}

const _ownerTotalsFetchers = new Map<string, () => Promise<OwnerTotal[]>>();
export function ownerTotalsFetcher(
  year: number,
  month: number,
): () => Promise<OwnerTotal[]> {
  const key = ownerTotalsCacheKey(year, month);
  const cached = _ownerTotalsFetchers.get(key);
  if (cached) return cached;
  const fn = () => ownerTotalsForMonth(year, month);
  _ownerTotalsFetchers.set(key, fn);
  return fn;
}
