import { MES } from "../lib/nkFormat";
import {
  getAnnualMetrics,
  getAnnualRuler,
  getForecast,
  type AnnualMetrics,
  type AnnualRuler,
  type BandVerdict,
  type Forecast,
  type MonthEnd,
  type MonthMetric,
} from "../lib/api";

// View-model puro da tela O ano. Consome a régua anual do motor (`get_annual_ruler`) e as
// métricas por mês, e produz a estrutura que a tela renderiza: o veredito, a régua da faixa, os
// dois cenários de dezembro, as doze linhas de mês e a renda por ano. Nenhuma régua nasce aqui —
// o teste de lastro, o percentual que julga, a falta para o piso e o veredito chegam decididos;
// o que mora neste arquivo é a costura das linhas e as derivações de exibição. A view é também a
// porta inteira do shim para a tela (ADR-0007): tipos reexportados, fetchers estáveis e a
// convenção de chave de cache do `useCommand` — a tela nunca importa `lib/api`.

// Tipos do shim reexportados pela view — a tela e seu teste leem daqui.
export type {
  AnnualMetrics,
  AnnualRuler,
  BandVerdict,
  Forecast,
  MonthEnd,
  MonthMetric,
};

// ------------------------------------------------------------------- tipos --

export interface AnoInput {
  /** Ano exibido. */
  year: number;
  /** Data de hoje (ISO `YYYY-MM-DD`) — define o mês corrente. */
  today: string;
  /** Métricas por mês do ano (do motor; pode vir esparso — o view-model completa 12). */
  months: MonthMetric[];
  /** A régua anual do método, pronta (motor: `forecast::annual_ruler`). */
  ruler: AnnualRuler;
}

export interface AnoMonth {
  month: number; // 1..12
  income: number; // ent
  outflow: number; // sai = income − performance
  economia: number; // eco
  performance: number; // perf (resultado do mês)
  endBalance: number | null; // saldo no fim do mês (null se fora do horizonte)
  savedPct: number | null; // Economizado% do mês vivido; null no futuro (— na tela)
  lived: boolean;
  current: boolean;
  future: boolean;
  suspect: boolean;
}

export type VerdictKind = BandVerdict;

export interface AnoVerdict {
  kind: VerdictKind;
}

export interface AnoView {
  year: number;
  isCurrentYear: boolean;
  hasData: boolean;
  months: AnoMonth[]; // sempre 12
  livedCount: number;
  typicalSpendCents: number; // gasto típico = mediana das saídas vividas
  suspects: number[]; // meses sem lastro (números), em ordem

  // Agregados anuais
  incomeLived: number; // ENT_R
  economiaLived: number; // ECO_R
  surplusLived: number; // PERF_R (a "sobra" dos meses vividos)
  incomeYear: number; // ENT_A (12 meses)
  economiaYear: number; // ECO_A (12 meses)
  livedPct: number | null; // ECO_R/ENT_R*100
  projectedPct: number | null; // ECO_A/ENT_A*100

  // A régua e o veredito
  rulerPct: number | null; // realizado quando há mês sem lastro, senão projetado
  rulerScopeLived: boolean; // true → recorte "nos N de 12 vividos"; false → ano inteiro
  estimate: boolean; // há mês sem lastro → o número anual é projeção sem lastro
  verdict: AnoVerdict;

  // Falta para 20%
  shortfallLivedCents: number; // ENT_R*0.2 − ECO_R
  shortfallYearCents: number; // ENT_A*0.2 − ECO_A
  perMonthShortfallCents: number | null; // falta anual ÷ meses futuros (null se ano fechado)
  futureCount: number;

  // Onde o ano termina
  endMonth: number | null; // mês do saldo final (12 quando o horizonte alcança dezembro)
  endBalanceCents: number | null; // DEZ (cenário lançado)
  endBalanceTypicalCents: number | null; // DEZ_TIPICO (só com mês sem lastro ≤ endMonth)
}

export interface IncomeYear {
  year: number;
  recordedMonths: number; // meses com renda registrada (corrente: só vividos)
  avgIncomeCents: number; // renda média por mês com registro
  savedPct: number | null; // economia registrada ÷ renda no período
}

// --------------------------------------------------------------- helpers ----

function yearOf(iso: string): number {
  return parseInt(iso.slice(0, 4), 10);
}
function monthOf(iso: string): number {
  return parseInt(iso.slice(5, 7), 10);
}

/** Pontos-base do motor → percentual de exibição. */
function pct(bps: number | null): number | null {
  return bps == null ? null : bps / 100;
}

function zeroMonth(year: number, month: number): MonthMetric {
  return {
    year,
    month,
    income_cents: 0,
    income_performance_cents: 0,
    performance_cents: 0,
    cost_of_living_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    daily_avg_out_cents: 0,
    daily_projected_cents: 0,
    cartao_cents: 0,
    real_daily_avg_cents: 0,
    economia_cents: 0,
    patrimonio_cents: 0,
    savings_rate_bps: 0,
  };
}

/** Completa 12 meses do ano, preenchendo lacunas com zeros (o motor pode vir esparso). */
function padMonths(months: MonthMetric[], year: number): MonthMetric[] {
  const byMonth = new Map<number, MonthMetric>();
  for (const m of months) {
    if (m.year === year) byMonth.set(m.month, m);
  }
  return Array.from(
    { length: 12 },
    (_, i) => byMonth.get(i + 1) ?? zeroMonth(year, i + 1),
  );
}

function sum<T>(items: T[], pick: (t: T) => number): number {
  return items.reduce((acc, t) => acc + pick(t), 0);
}

// ------------------------------------------------------------- view builder --

export function buildAnoView(input: AnoInput): AnoView {
  const { year, today, ruler } = input;
  const currentYear = yearOf(today);
  const currentMonth = monthOf(today);
  const isCurrentYear = year === currentYear;

  const padded = padMonths(input.months, year);
  const endByMonth = new Map<number, number>();
  for (const e of ruler.month_end) {
    if (e.year === year) endByMonth.set(e.month, e.balance_cents);
  }
  const rulerByMonth = new Map(ruler.months.map((m) => [m.month, m]));

  // Cada linha costura duas leituras do mesmo mês: as figuras de caixa (entradas, economia,
  // resultado) e a leitura do método (o que saiu, se foi vivido, se tem lastro).
  const months: AnoMonth[] = padded.map((m) => {
    const line = rulerByMonth.get(m.month);
    const lived = line?.lived ?? false;
    return {
      month: m.month,
      income: m.income_cents,
      outflow: line?.outflow_cents ?? 0,
      economia: m.economia_cents,
      performance: m.performance_cents,
      endBalance: endByMonth.has(m.month) ? endByMonth.get(m.month)! : null,
      savedPct: lived ? m.savings_rate_bps / 100 : null,
      lived,
      current: isCurrentYear && m.month === currentMonth,
      future: !lived,
      suspect: line?.suspect ?? false,
    };
  });

  return {
    year,
    isCurrentYear,
    hasData: ruler.has_data,
    months,
    livedCount: ruler.lived_months,
    typicalSpendCents: ruler.typical_spend_cents,
    suspects: ruler.months.flatMap((m) => (m.suspect ? [m.month] : [])),
    incomeLived: ruler.income_lived_cents,
    economiaLived: ruler.economia_lived_cents,
    surplusLived: ruler.surplus_lived_cents,
    incomeYear: ruler.income_year_cents,
    economiaYear: ruler.economia_year_cents,
    livedPct: pct(ruler.lived_bps),
    projectedPct: pct(ruler.projected_bps),
    rulerPct: pct(ruler.bps),
    rulerScopeLived: ruler.scope_lived,
    estimate: ruler.scope_lived,
    verdict: { kind: ruler.verdict },
    shortfallLivedCents: ruler.shortfall_lived_cents,
    shortfallYearCents: ruler.shortfall_year_cents,
    perMonthShortfallCents: ruler.per_month_shortfall_cents,
    futureCount: ruler.future_months,
    endMonth: ruler.year_end.end_month,
    endBalanceCents: ruler.year_end.end_balance_cents,
    endBalanceTypicalCents: ruler.year_end.end_balance_typical_cents,
  };
}

// ---------------------------------------------------- observação da Mia ----

export interface AnoMiaObservation {
  /** O mês observado, capitalizado — o último que o ano viveu. */
  month: string;
  /** O que esse mês fez com a economia ("não guardou nada", "guardou 24%"). */
  clause: string;
  /** O Economizado% da régua, truncado como a tela o imprime. */
  yearPct: string;
  /** Ano em curso → a média SEGUE; ano fechado → a média FICOU. */
  ongoing: boolean;
}

/**
 * A observação da Mia sobre o ano: o mês mais recente contra a média que a régua julga.
 * O dado novo é o MÊS — a média entra como referência de leitura, no zoom do ano (a exceção
 * de detalhe da regra 41), e a frase inteira muda a cada mês vivido.
 *
 * Sem mês vivido não há observação: a leitura devolve `null` em vez de inventar uma.
 */
export function anoMiaObservation(v: AnoView): AnoMiaObservation | null {
  const lived = v.months.filter((m) => m.lived);
  const last = lived[lived.length - 1];
  if (!last) return null;
  const monthPct = last.savedPct == null ? 0 : Math.trunc(last.savedPct);
  return {
    month: MES[last.month - 1] ?? "",
    clause:
      monthPct === 0
        ? "não guardou nada"
        : monthPct < 20
          ? "guardou pouco"
          : `guardou ${monthPct}%`,
    yearPct: v.rulerPct == null ? "—" : `${Math.trunc(v.rulerPct)}%`,
    ongoing: v.isCurrentYear,
  };
}

// ------------------------------------------------- renda ao longo dos anos --

/**
 * Renda média por mês COM registro, ano a ano. No ano corrente conta só os meses vividos —
 * dividir por 12 inventaria uma queda que não existiu num ano ainda em curso; num ano
 * preenchido a partir de um mês tardio, conta só os meses que têm renda.
 */
export function buildIncomeAcrossYears(
  years: { year: number; months: MonthMetric[] }[],
  today: string,
): IncomeYear[] {
  const currentYear = yearOf(today);
  const currentMonth = monthOf(today);
  return years.map(({ year, months }) => {
    const padded = padMonths(months, year);
    const eligible = padded.filter((m) => {
      if (year < currentYear) return true;
      if (year > currentYear) return false;
      return m.month <= currentMonth;
    });
    const recorded = eligible.filter((m) => m.income_cents > 0);
    const recordedMonths = recorded.length;
    const income = sum(recorded, (m) => m.income_cents);
    const economia = sum(recorded, (m) => m.economia_cents);
    return {
      year,
      recordedMonths,
      avgIncomeCents: recordedMonths > 0 ? Math.round(income / recordedMonths) : 0,
      savedPct: income > 0 ? (economia / income) * 100 : null,
    };
  });
}

// ---------------------------------------------------------------- leitura ---
// Fetchers com identidade estável por chave (o contrato do useCommand rejeita closures novas a
// cada render) e a convenção da chave de cache do `useCommand` — a tela só chama estas funções,
// nunca monta a string por conta própria.

export function fetchForecast(): Promise<Forecast> {
  return getForecast();
}

export function annualMetricsCacheKey(year: number): string {
  return `annual_metrics:${year}:ano`;
}

const _annualCache = new Map<number, () => Promise<AnnualMetrics>>();
export function annualMetricsFetcher(year: number): () => Promise<AnnualMetrics> {
  const cached = _annualCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _annualCache.set(year, fn);
  return fn;
}

export function annualRulerCacheKey(year: number): string {
  return `annual_ruler:${year}`;
}

const _rulerCache = new Map<number, () => Promise<AnnualRuler>>();
export function annualRulerFetcher(year: number): () => Promise<AnnualRuler> {
  const cached = _rulerCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualRuler(year);
  _rulerCache.set(year, fn);
  return fn;
}
