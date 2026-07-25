import type { MonthMetric, MonthEnd } from "../lib/api";

// View-model puro da tela O ano. Consome os DTOs do motor (métricas por mês + saldos de
// fim de mês) e produz a estrutura que a tela renderiza: o veredito, a régua da faixa, os
// dois cenários de dezembro, as doze linhas de mês e a renda por ano. Toda a matemática de
// caixa (renda, economia, performance, saldo) vem do motor; aqui mora só a composição —
// o teste de lastro, a seleção do estado do veredito e as derivações de exibição.

// --------------------------------------------------------------- primitivas --

/** Saída total do mês = tudo que saiu da conta = renda − performance. */
export function outflowCents(m: {
  income_cents: number;
  performance_cents: number;
}): number {
  return m.income_cents - m.performance_cents;
}

/** Mediana de uma janela; 0 para janela vazia. Janela par devolve a média dos dois centrais. */
export function median(values: number[]): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
}

// Um mês futuro só sustenta o veredito do ano se a saída lançada for compatível com a vida
// real: piso de 60% do gasto típico (a mediana das saídas dos meses vividos). Abaixo disso
// o mês é SUSPEITO — tem lançamento, só tem pouco: pode ser mês barato de verdade ou pode
// faltar lançar. Enquanto houver suspeito, o veredito recua para o realizado.
//
// A mesma régua é computada no motor (`forecast::annual_ruler`, `LASTRO_FLOOR_BPS`), que é o
// que a conversa lê. As duas precisam dar o mesmo número: mudou aqui, muda lá.
const LASTRO_FLOOR = 0.6;

// ------------------------------------------------------------------- tipos --

export interface AnoInput {
  /** Ano exibido. */
  year: number;
  /** Data de hoje (ISO `YYYY-MM-DD`) — define o mês corrente e a fronteira vivido/futuro. */
  today: string;
  /** Métricas por mês do ano (do motor; pode vir esparso — o view-model completa 12). */
  months: MonthMetric[];
  /** Saldos de fim de mês do ano (motor: futuros do forecast + históricos do grid). */
  monthEnd: MonthEnd[];
  /** Meses de reserva atuais — distingue "zero por escolha" (≥ 6) de "não guardou nada". */
  reserveMonths: number | null;
}

export interface AnoMonth {
  month: number; // 1..12
  income: number; // ent
  outflow: number; // sai = income − performance
  economia: number; // eco
  performance: number; // perf (resultado do mês)
  endBalance: number | null; // saldo no fim do mês (null se fora do horizonte)
  savedPct: number | null; // eco/income*100 no mês vivido; null no futuro (— na tela)
  lived: boolean;
  current: boolean;
  future: boolean;
  suspect: boolean;
}

export type VerdictKind =
  "no_record" | "zero_by_choice" | "below_band" | "in_band" | "above_band";

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
  suspects: number[]; // meses suspeitos (números), em ordem

  // Agregados anuais
  incomeLived: number; // ENT_R
  economiaLived: number; // ECO_R
  surplusLived: number; // PERF_R (a "sobra" dos meses vividos)
  incomeYear: number; // ENT_A (12 meses)
  economiaYear: number; // ECO_A (12 meses)
  livedPct: number | null; // ECO_R/ENT_R*100
  projectedPct: number | null; // ECO_A/ENT_A*100

  // A régua e o veredito
  rulerPct: number | null; // realizado quando há suspeitos, senão projetado
  rulerScopeLived: boolean; // true → recorte "nos N de 12 vividos"; false → ano inteiro
  estimate: boolean; // há suspeitos → o número anual é projeção sem lastro
  verdict: AnoVerdict;

  // Falta para 20%
  shortfallLivedCents: number; // ENT_R*0.2 − ECO_R
  shortfallYearCents: number; // ENT_A*0.2 − ECO_A
  perMonthShortfallCents: number | null; // falta anual ÷ meses futuros (null se ano fechado)
  futureCount: number;

  // Onde o ano termina
  endMonth: number | null; // mês do saldo final (12 quando o horizonte alcança dezembro)
  endBalanceCents: number | null; // DEZ (cenário lançado)
  endBalanceTypicalCents: number | null; // DEZ_TIPICO (só com suspeitos ≤ endMonth)
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

function hasActivity(m: AnoMonth): boolean {
  return m.income !== 0 || m.economia !== 0 || m.outflow !== 0;
}

// ------------------------------------------------------------- view builder --

export function buildAnoView(input: AnoInput): AnoView {
  const { year, today, reserveMonths } = input;
  const currentYear = yearOf(today);
  const currentMonth = monthOf(today);
  const isCurrentYear = year === currentYear;

  const padded = padMonths(input.months, year);
  const endByMonth = new Map<number, number>();
  for (const e of input.monthEnd) {
    if (e.year === year) endByMonth.set(e.month, e.balance_cents);
  }

  const livedOf = (month: number): boolean => {
    if (year < currentYear) return true;
    if (year > currentYear) return false;
    return month <= currentMonth;
  };

  // Passo 1: gasto típico depende só dos meses vividos.
  const livedOutflows = padded
    .filter((m) => livedOf(m.month))
    .map((m) => outflowCents(m));
  const typicalSpendCents = median(livedOutflows);
  const lastroThreshold = typicalSpendCents * LASTRO_FLOOR;

  // Passo 2: monta cada linha de mês (suspeito depende do gasto típico do passo 1).
  const months: AnoMonth[] = padded.map((m) => {
    const outflow = outflowCents(m);
    const lived = livedOf(m.month);
    const future = !lived;
    const suspect = future && typicalSpendCents > 0 && outflow < lastroThreshold;
    return {
      month: m.month,
      income: m.income_cents,
      outflow,
      economia: m.economia_cents,
      performance: m.performance_cents,
      endBalance: endByMonth.has(m.month) ? endByMonth.get(m.month)! : null,
      savedPct: lived
        ? m.income_cents > 0
          ? (m.economia_cents / m.income_cents) * 100
          : 0
        : null,
      lived,
      current: isCurrentYear && m.month === currentMonth,
      future,
      suspect,
    };
  });

  const livedMonths = months.filter((m) => m.lived);
  const futureMonths = months.filter((m) => m.future);
  const suspects = months.filter((m) => m.suspect).map((m) => m.month);

  const incomeLived = sum(livedMonths, (m) => m.income);
  const economiaLived = sum(livedMonths, (m) => m.economia);
  const surplusLived = sum(livedMonths, (m) => m.performance);
  const incomeYear = sum(months, (m) => m.income);
  const economiaYear = sum(months, (m) => m.economia);

  const livedPct = incomeLived > 0 ? (economiaLived / incomeLived) * 100 : null;
  const projectedPct = incomeYear > 0 ? (economiaYear / incomeYear) * 100 : null;

  const estimate = suspects.length > 0;
  const rulerPct = estimate ? livedPct : projectedPct;
  const rulerScopeLived = estimate;

  const hasData = livedMonths.some(hasActivity);

  // Falta para o piso de 20% (nunca negativa vira mentira: pode ser negativa = já passou).
  const shortfallLivedCents = Math.round(incomeLived * 0.2 - economiaLived);
  const shortfallYearCents = Math.round(incomeYear * 0.2 - economiaYear);
  const futureCount = futureMonths.length;
  const perMonthShortfallCents =
    futureCount > 0 ? Math.round(shortfallYearCents / futureCount) : null;

  // Onde o ano termina: prefere dezembro; se o horizonte não alcança, usa o último mês
  // projetado e a tela o nomeia. Sem nenhum saldo → o bloco não se monta.
  let endMonth: number | null = null;
  let endBalanceCents: number | null = null;
  if (endByMonth.has(12)) {
    endMonth = 12;
    endBalanceCents = endByMonth.get(12)!;
  } else if (endByMonth.size > 0) {
    endMonth = Math.max(...endByMonth.keys());
    endBalanceCents = endByMonth.get(endMonth)!;
  }

  // Cenário do gasto típico: se cada mês suspeito (até o mês final) custasse o típico. O
  // termo (típico − saída lançada) é o `estimated_missing_cents` que o motor já computa.
  let endBalanceTypicalCents: number | null = null;
  if (endBalanceCents !== null && endMonth !== null) {
    const upTo = endMonth;
    const missing = months
      .filter((m) => m.suspect && m.month <= upTo)
      .reduce((acc, m) => acc + (typicalSpendCents - m.outflow), 0);
    if (missing > 0) endBalanceTypicalCents = Math.round(endBalanceCents - missing);
  }

  const verdict: AnoVerdict = {
    kind: verdictKind(hasData, economiaLived, rulerPct, reserveMonths),
  };

  return {
    year,
    isCurrentYear,
    hasData,
    months,
    livedCount: livedMonths.length,
    typicalSpendCents,
    suspects,
    incomeLived,
    economiaLived,
    surplusLived,
    incomeYear,
    economiaYear,
    livedPct,
    projectedPct,
    rulerPct,
    rulerScopeLived,
    estimate,
    verdict,
    shortfallLivedCents,
    shortfallYearCents,
    perMonthShortfallCents,
    futureCount,
    endMonth,
    endBalanceCents,
    endBalanceTypicalCents,
  };
}

function verdictKind(
  hasData: boolean,
  economiaLived: number,
  rulerPct: number | null,
  reserveMonths: number | null,
): VerdictKind {
  if (!hasData) return "no_record";
  // Zerar a economia é a troca CERTA quando a reserva está de fato protegida (≥ 6 meses);
  // sem reserva sadia, economia zero é "não guardou nada", não uma escolha.
  if (economiaLived === 0 && reserveMonths !== null && reserveMonths >= 6) {
    return "zero_by_choice";
  }
  const pct = rulerPct ?? 0;
  if (pct < 20) return "below_band";
  if (pct <= 30) return "in_band";
  return "above_band";
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
