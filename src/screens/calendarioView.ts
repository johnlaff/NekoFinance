import type { TransactionRow } from "../lib/api";
import { MES, fmtBRL, fmtSigned } from "../lib/nkFormat";
import { toMovementType } from "./lancamentosView";

// ---------------------------------------------------------------------------
// Calendário — o mês dia a dia: cada dia carrega o movimento e o saldo que ele
// deixou. Helpers puros — a tela orquestra, este módulo decide.
// ---------------------------------------------------------------------------

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
      isLowest: iso === lowestIso,
    });
  }
  while (cells.length % 7 !== 0) cells.push(null);

  const weeks: CalCell[][] = [];
  for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));
  return { weeks, lowestIso };
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
