import {
  getForecast,
  getMonthGrid,
  getMonthTransactions,
  type Forecast,
  type MonthGridDay,
  type TransactionRow,
} from "../lib/api";
import { MES, fmtBRL, fmtSigned, saldoBand } from "../lib/nkFormat";
import { toMovementType } from "./lancamentosView";

// ---------------------------------------------------------------------------
// Calendário — o mês dia a dia: cada dia carrega o movimento e o saldo que ele
// deixou. Helpers puros — a tela orquestra, este módulo decide. É também a
// porta inteira do shim para a tela (ADR-0007): tipos reexportados, fetchers
// estáveis e a convenção de chave de cache do `useCommand`.
// ---------------------------------------------------------------------------

// Tipos do shim reexportados pela view — a tela e seu teste leem daqui.
export type { Forecast, MonthGridDay, TransactionRow };

/** Cabeçalhos da grade, semana começando na segunda (gramática da direção). */
export const CAL_DOW = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];

/** Linha de um dia vinda de qualquer corrente (grid realizado ou projeção).
 *  `economia_cents` só existe na projeção — por isso o movimento do dia deriva
 *  do delta da corrente, nunca da soma de componentes. */
export interface DayRow {
  date: string;
  income_cents: number;
  fixed_out_cents: number;
  daily_out_cents: number;
  economia_cents?: number;
  balance_cents: number | null;
}

export interface CalDayCell {
  day: number;
  iso: string;
  /** Saldo que o dia deixou; null = sem corrente conhecida para o dia. */
  balanceCents: number | null;
  /** Delta contra a véspera; null quando qualquer ponta é desconhecida. */
  movementCents: number | null;
  isToday: boolean;
  isFuture: boolean;
  hasIncome: boolean;
  /** Quanto entrou no dia — o valor que "O que marca o mês" imprime. */
  incomeCents: number;
  isLowest: boolean;
}

/** null = célula fora do mês (preenchimento da semana). */
export type CalCell = CalDayCell | null;

export interface CalendarMonth {
  weeks: CalCell[][];
  lowestIso: string | null;
}

function isoOf(year: number, month0: number, day: number): string {
  return `${year}-${String(month0 + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

/** Soma meses a um "YYYY-MM" (delta negativo volta; cruza anos). */
export function addMonths(ym: string, delta: number): string {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(y ?? 0, (m ?? 1) - 1 + delta, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** Soma dias a um ISO (a aritmética de Date absorve fronteiras). */
export function shiftIso(iso: string, days: number): string {
  const [y, m, d] = iso.split("-").map(Number);
  const next = new Date(y ?? 0, (m ?? 1) - 1, (d ?? 1) + days);
  return isoOf(next.getFullYear(), next.getMonth(), next.getDate());
}

/** Véspera de um ISO — a aritmética de Date absorve fronteiras de mês e ano. */
function prevIso(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  const prev = new Date(y ?? 0, (m ?? 1) - 1, (d ?? 1) - 1);
  return isoOf(prev.getFullYear(), prev.getMonth(), prev.getDate());
}

export function buildCalendarMonth(opts: {
  year: number;
  /** Mês 0-based. */
  month0: number;
  /** ISO de hoje — a costura das correntes: antes lê o realizado, dali em diante a projeção. */
  today: string;
  realized: DayRow[];
  forecast: DayRow[];
}): CalendarMonth {
  const { year, month0, today, realized, forecast } = opts;
  const realizedMap = new Map(realized.map((r) => [r.date, r]));
  const forecastMap = new Map(forecast.map((r) => [r.date, r]));

  const rowAt = (iso: string): DayRow | undefined =>
    iso < today ? realizedMap.get(iso) : forecastMap.get(iso);
  const balanceAt = (iso: string): number | null => rowAt(iso)?.balance_cents ?? null;

  const daysInMonth = new Date(year, month0 + 1, 0).getDate();

  // Menor saldo do mês entre dias com corrente; empate fica com o primeiro dia.
  let lowestIso: string | null = null;
  let lowest = Infinity;
  for (let d = 1; d <= daysInMonth; d++) {
    const iso = isoOf(year, month0, d);
    const b = balanceAt(iso);
    if (b != null && b < lowest) {
      lowest = b;
      lowestIso = iso;
    }
  }

  // Matriz Seg-first: getDay() é Dom-first, o deslocamento (+6)%7 rebaixa o domingo.
  const leading = (new Date(year, month0, 1).getDay() + 6) % 7;
  const cells: CalCell[] = Array.from({ length: leading }, () => null);
  for (let d = 1; d <= daysInMonth; d++) {
    const iso = isoOf(year, month0, d);
    const row = rowAt(iso);
    const balance = row?.balance_cents ?? null;
    const prevBalance = balanceAt(prevIso(iso));
    cells.push({
      day: d,
      iso,
      balanceCents: balance,
      movementCents:
        balance != null && prevBalance != null ? balance - prevBalance : null,
      isToday: iso === today,
      isFuture: iso > today,
      hasIncome: (row?.income_cents ?? 0) > 0,
      incomeCents: row?.income_cents ?? 0,
      isLowest: iso === lowestIso,
    });
  }
  while (cells.length % 7 !== 0) cells.push(null);

  const weeks: CalCell[][] = [];
  for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));
  return { weeks, lowestIso };
}

/** Os dias do mês em ordem, sem as células de preenchimento da semana. */
function daysOf(month: CalendarMonth): CalDayCell[] {
  return month.weeks.flat().filter((c): c is CalDayCell => c != null);
}

// ---------------------------------------------------------------------------
// A leitura do mês — manchete, marcos, faixa da grade e trilho
// ---------------------------------------------------------------------------

/** A faixa do termômetro que a GRADE pinta. Os limiares seguem absolutos: o que
 *  muda é onde a cor é gasta — um dia na faixa boa fica neutro, porque num mês
 *  saudável 30 células tingidas não distinguem nada. A faixa cheia continua no
 *  veredito e no dia aberto, onde a palavra acompanha a cor. */
export function gridBand(
  cents: number | null | undefined,
): "tight" | "negative" | "critical" | null {
  if (cents == null) return null;
  const band = saldoBand(cents).key;
  return band === "tight" || band === "negative" || band === "critical" ? band : null;
}

export type MarkKind = "lowest" | "out" | "income" | "lowest-out";

export interface MonthMark {
  kind: MarkKind;
  /** O papel do dia, já resolvido — inclusive quando ele acumula dois. */
  label: string;
  iso: string;
  /** O valor principal: saldo no vale, movimento na saída, entrada na entrada. */
  cents: number;
  /** O segundo valor, só quando a linha carrega dois papéis. */
  extraCents?: number;
}

/** Os dois extremos do mês, fundidos numa linha só quando caem no mesmo dia:
 *  repetir a data infla o bloco sem informação nova (regra 41). */
function extremeMarks(
  lowest: MonthMark | null | undefined,
  out: MonthMark | null | undefined,
): MonthMark[] {
  if (!lowest) return out ? [out] : [];
  if (!out) return [lowest];
  if (lowest.iso !== out.iso) return [lowest, out];
  return [
    {
      ...lowest,
      kind: "lowest-out",
      label: "Menor saldo e maior saída",
      extraCents: out.cents,
    },
  ];
}

/** Os dias que decidem o mês: o vale, a maior saída e as entradas. */
export function monthMarks(month: CalendarMonth): MonthMark[] {
  const days = daysOf(month);
  // Os predicados provam o que o tipo sozinho não prova — sem eles, cada leitura
  // de saldo ou movimento precisaria de um `!`.
  const withBalance = (d: CalDayCell): d is CalDayCell & { balanceCents: number } =>
    d.balanceCents != null;
  const falling = (d: CalDayCell): d is CalDayCell & { movementCents: number } =>
    d.movementCents != null && d.movementCents < 0;

  const lowest = days.filter(withBalance).find((d) => d.isLowest);
  const out = days
    .filter(falling)
    .reduce<(CalDayCell & { movementCents: number }) | null>(
      (a, b) => (a == null || b.movementCents < a.movementCents ? b : a),
      null,
    );

  const marks: MonthMark[] = extremeMarks(
    lowest && {
      kind: "lowest",
      label: "Menor saldo",
      iso: lowest.iso,
      cents: lowest.balanceCents,
    },
    out && {
      kind: "out",
      label: "Maior saída",
      iso: out.iso,
      cents: out.movementCents,
    },
  );

  for (const d of days) {
    if (d.hasIncome) {
      marks.push({
        kind: "income",
        label: "Entradas",
        iso: d.iso,
        cents: d.incomeCents,
      });
    }
  }
  return marks;
}

/** A forma do mês em uma frase: onde ele afunda e onde respira, na ordem em que
 *  acontece. Sem entrada no mês, só o vale; sem corrente, não há o que dizer. */
export function monthHeadline(month: CalendarMonth, monthLabel: string): string | null {
  const days = daysOf(month);
  const lowest = days.find((d) => d.isLowest && d.balanceCents != null);
  if (!lowest) return null;
  // "Respirar" é a recuperação: a maior entrada DEPOIS do vale. Quando o mês só
  // tem entrada antes dele, ela ainda nomeia o alívio — e a ordem cronológica
  // inverte a frase.
  const biggest = (pool: CalDayCell[]) =>
    pool.reduce<CalDayCell | null>(
      (a, b) => (a == null || b.incomeCents > a.incomeCents ? b : a),
      null,
    );
  const incomes = days.filter((d) => d.hasIncome);
  const breath = biggest(incomes.filter((d) => d.iso > lowest.iso)) ?? biggest(incomes);

  if (!breath) return `${monthLabel} afunda no dia ${lowest.day}.`;
  const [first, second] =
    lowest.iso <= breath.iso
      ? [`afunda no dia ${lowest.day}`, `respira no ${breath.day}`]
      : [`respira no dia ${breath.day}`, `afunda no ${lowest.day}`];
  return `${monthLabel} ${first} e ${second}.`;
}

export interface RailPoint {
  iso: string;
  /** Posição no eixo do tempo, 0 no primeiro ponto e 1 no último. */
  x: number;
  /** Posição do valor, 0 no menor saldo do mês e 1 no maior. */
  v: number;
  isFuture: boolean;
  /** O evento do dia viaja com o ponto: a tela desenha, não reanda o mês. */
  hasIncome: boolean;
}

/** A série do trilho: os saldos conhecidos do mês, normalizados. A tela decide
 *  os pixels; aqui mora só a forma. */
export function railSeries(
  month: CalendarMonth,
): { points: RailPoint[]; lowestIndex: number; todayIndex: number } | null {
  const days = daysOf(month).filter((d) => d.balanceCents != null);
  if (days.length === 0) return null;

  const values = days.map((d) => d.balanceCents!);
  const min = Math.min(...values);
  const span = Math.max(...values) - min;
  const lastX = days.length - 1;

  const points = days.map((d, i) => ({
    iso: d.iso,
    x: lastX === 0 ? 0 : i / lastX,
    // Faixa de valor nula (um ponto, ou mês inteiro no mesmo saldo): o traço
    // fica no meio da caixa em vez de colapsar na borda.
    v: span === 0 ? 0.5 : (d.balanceCents! - min) / span,
    isFuture: d.isFuture,
    hasIncome: d.hasIncome,
  }));
  return {
    points,
    lowestIndex: days.findIndex((d) => d.isLowest),
    todayIndex: days.findIndex((d) => d.isToday),
  };
}

// ---------------------------------------------------------------------------
// Agenda do dia
// ---------------------------------------------------------------------------

export interface AgendaComponent {
  key: "entrada" | "saida" | "diario" | "economia";
  label: string;
  cents: number;
}

/** Componentes não-zerados do dia, na ordem canônica das colunas. Zerados são
 *  omitidos: a agenda lê como frase, não como formulário. */
export function dayComponents(row: DayRow | null | undefined): AgendaComponent[] {
  if (!row) return [];
  const all: AgendaComponent[] = [
    { key: "entrada", label: "Entrada", cents: row.income_cents },
    { key: "saida", label: "Saídas fixas", cents: row.fixed_out_cents },
    { key: "diario", label: "Diário", cents: row.daily_out_cents },
    { key: "economia", label: "Economia", cents: row.economia_cents ?? 0 },
  ];
  return all.filter((c) => c.cents !== 0);
}

/** Sinal de exibição de um lançamento — a regra do Livro-razão: entrada
 *  positiva, todo o resto negativo (o `amount` cru não carrega o sinal). */
export function agendaSignedCents(t: TransactionRow): number {
  const abs = Math.abs(t.amount);
  return toMovementType(t) === "entrada" ? abs : -abs;
}

/** Lançamentos do dia, na ordem em que o Livro-razão os entrega. */
export function agendaTransactions(
  rows: TransactionRow[],
  iso: string,
): TransactionRow[] {
  return rows.filter((t) => t.date === iso);
}

// ---------------------------------------------------------------------------
// Rótulos e formatação de célula
// ---------------------------------------------------------------------------

/** "12 de julho" — a data no rótulo acessível da célula. */
function dayMonthLabel(iso: string): string {
  const [, m, d] = iso.split("-").map(Number);
  return `${d} de ${(MES[(m ?? 1) - 1] ?? "").toLowerCase()}`;
}

/** "10/06" — a data curta do olho do veredito, onde a linha é estreita. */
export function shortDate(iso: string): string {
  const [, m, d] = iso.split("-");
  return `${d}/${m}`;
}

/** Rótulo acessível completo da célula: data, saldo, movimento e eventos.
 *  A cor nunca é o único canal — tudo que a borda diz, o rótulo repete. */
export function cellLabel(cell: CalDayCell): string {
  const parts = [dayMonthLabel(cell.iso)];
  if (cell.balanceCents == null) {
    parts.push(cell.isFuture ? "Projeção indisponível" : "Sem dados");
  } else {
    parts.push(`Saldo ${fmtBRL(cell.balanceCents)}`);
    if (cell.movementCents != null && cell.movementCents !== 0) {
      parts.push(`Movimento ${fmtSigned(cell.movementCents)}`);
    }
  }
  if (cell.isToday) parts.push("Hoje");
  if (cell.hasIncome) parts.push("Entrada");
  if (cell.isLowest) parts.push("Menor saldo do mês");
  if (cell.isFuture) parts.push("Previsto — ainda não aconteceu");
  return parts.join(" · ");
}

/** Reais inteiros com milhar, sem "R$" — a leitura rápida da célula (31 células
 *  com "R$" viram ruído); a precisão cheia mora no rótulo acessível e na agenda. */
export function cellMoney(cents: number): string {
  const whole = Math.round(Math.abs(cents) / 100);
  return `${cents < 0 ? "−" : ""}${whole.toLocaleString("pt-BR")}`;
}

/** Movimento da célula, com sinal; abaixo de meio real (arredonda a zero) cala —
 *  o rótulo acessível segue dizendo o valor exato. */
export function cellSigned(cents: number): string {
  const whole = Math.round(Math.abs(cents) / 100);
  if (whole === 0) return "";
  return `${cents < 0 ? "−" : "+"}${whole.toLocaleString("pt-BR")}`;
}

// ---------------------------------------------------------------------------
// Leitura — fetchers com identidade estável por chave (o contrato do useCommand rejeita
// closures novas a cada render) e a convenção da chave de cache do `useCommand`.
// ---------------------------------------------------------------------------

export function fetchForecast(): Promise<Forecast> {
  return getForecast();
}

export function monthGridCacheKey(ym: string): string {
  return `get_month_grid:${ym}`;
}

const _gridFetchers = new Map<string, () => Promise<MonthGridDay[]>>();
export function monthGridFetcher(ym: string): () => Promise<MonthGridDay[]> {
  const cached = _gridFetchers.get(ym);
  if (cached) return cached;
  const fn = () => getMonthGrid(Number(ym.slice(0, 4)), Number(ym.slice(5, 7)));
  _gridFetchers.set(ym, fn);
  return fn;
}

export function monthTransactionsCacheKey(ym: string): string {
  return `month_transactions:${ym}`;
}

const _txFetchers = new Map<string, () => Promise<TransactionRow[]>>();
export function monthTransactionsFetcher(ym: string): () => Promise<TransactionRow[]> {
  const cached = _txFetchers.get(ym);
  if (cached) return cached;
  const fn = () => getMonthTransactions(ym);
  _txFetchers.set(ym, fn);
  return fn;
}
