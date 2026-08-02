import type { ForecastDay, UpcomingInvoice } from "../lib/api";
import { MES } from "../lib/nkFormat";
import type { SaldoBand } from "../lib/saldoHeatmap";

const WEEKDAYS = [
  "Domingo",
  "Segunda-feira",
  "Terça-feira",
  "Quarta-feira",
  "Quinta-feira",
  "Sexta-feira",
  "Sábado",
];

/** Data ISO (YYYY-MM-DD) do relógio local — a data que a appbar da Hoje exibe. */
export function localTodayIso(now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/** "Quarta-feira, 15 de julho" — cabeçalho de data da tela e crumb da appbar. */
export function eyebrowDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return "";
  const wd = new Date(y, m - 1, d).getDay();
  return `${WEEKDAYS[wd] ?? ""}, ${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

/** "10 de agosto" — rótulo curto de vencimento. */
export function faturaDayLabel(iso: string): string {
  const [, m, d] = iso.split("-").map(Number);
  if (!m || !d) return iso;
  return `${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

/**
 * Saudação por hora local. Sem nome: o app não tem fonte de nome de usuário,
 * e veredito nunca nasce de dado fabricado.
 */
export function greetingForHour(hour: number): string {
  if (hour >= 5 && hour < 12) return "Bom dia.";
  if (hour >= 12 && hour < 18) return "Boa tarde.";
  return "Boa noite.";
}

export interface InvoiceDueGroup {
  dueDate: string;
  /** Faturas do vencimento, maior primeiro. */
  invoices: UpcomingInvoice[];
}

export interface OpenInvoicesView {
  /** Σ líquida das faturas em aberto, depois da parte esperada de reembolso; só leituras marcadas a exibem. */
  totalCents: number;
  /** Σ bruta das faturas em aberto; recibos auditáveis permanecem neste regime não marcado. */
  grossTotalCents: number;
  /** Σ da parte esperada de reembolso, limitada ao total de cada fatura. */
  refundedCents: number;
  /** Faturas em aberto com uma parte esperada de reembolso. */
  refundedCount: number;
  count: number;
  /** Grupos por vencimento, em ordem cronológica. */
  groups: InvoiceDueGroup[];
  /** A maior fatura em aberto de todas — recebe o contexto de destaque. */
  largestAccountId: string | null;
}

/**
 * A parte da fatura que volta como reembolso. O teto no total da própria fatura é o que impede
 * uma Entrada que mistura outras origens de anular um compromisso que ainda é do dono.
 */
export function refundExpectedCents(invoice: UpcomingInvoice): number {
  const expected = Number.isFinite(invoice.refund_expected_cents)
    ? invoice.refund_expected_cents
    : 0;
  return Math.min(Math.max(expected, 0), invoice.amount_cents);
}

/**
 * Corpo do bloco do dia no modo cartão: faturas em aberto agrupadas por vencimento.
 * "Em aberto" = acumulando (`aberta`) ou fechada aguardando pagamento (`fechada`);
 * `prevista` é ciclo futuro (voz do Horizonte) e `paga` já saiu do caixa.
 */
export function openInvoicesView(invoices: UpcomingInvoice[]): OpenInvoicesView {
  const open = invoices.filter(
    (i) => (i.status === "aberta" || i.status === "fechada") && i.amount_cents !== 0,
  );
  const byDue = new Map<string, UpcomingInvoice[]>();
  for (const invoice of open) {
    const group = byDue.get(invoice.due_date) ?? [];
    group.push(invoice);
    byDue.set(invoice.due_date, group);
  }
  const groups = [...byDue.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([dueDate, list]) => ({
      dueDate,
      // A ordem e o destaque leem o compromisso líquido; a linha preserva o bruto declarado pela fatura.
      invoices: list.toSorted(
        (a, b) =>
          b.amount_cents -
          refundExpectedCents(b) -
          (a.amount_cents - refundExpectedCents(a)),
      ),
    }));
  const largest = open.reduce<UpcomingInvoice | null>(
    (acc, invoice) =>
      acc === null ||
      invoice.amount_cents - refundExpectedCents(invoice) >
        acc.amount_cents - refundExpectedCents(acc)
        ? invoice
        : acc,
    null,
  );
  const { totalCents, grossTotalCents, refundedCents, refundedCount } = open.reduce(
    (totals, invoice) => {
      const refundExpected = refundExpectedCents(invoice);
      return {
        totalCents: totals.totalCents + invoice.amount_cents - refundExpected,
        grossTotalCents: totals.grossTotalCents + invoice.amount_cents,
        refundedCents: totals.refundedCents + refundExpected,
        refundedCount: totals.refundedCount + Number(refundExpected > 0),
      };
    },
    { totalCents: 0, grossTotalCents: 0, refundedCents: 0, refundedCount: 0 },
  );
  return {
    totalCents,
    grossTotalCents,
    refundedCents,
    refundedCount,
    count: open.length,
    groups,
    largestAccountId: largest?.account_id ?? null,
  };
}

/** "Inter, Nubank e BB" — junção em linguagem natural. */
export function joinNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} e ${names[names.length - 1]}`;
}

export interface MonthInsight {
  /** Saldo projetado do último dia do mês. */
  endBalanceCents: number;
  /** O menor saldo do mês e o primeiro dia em que ele acontece. */
  minCents: number;
  minDate: string;
  /** O ponto mais apertado está valendo agora (saldo de hoje já é o mínimo). */
  minIsOngoing: boolean;
  /** Primeira entrada estritamente após hoje, dentro do mês. */
  nextIncomeDate: string | null;
  nextIncomeCents: number;
  /** Dias de hoje em diante com saldo projetado negativo (buraco do futuro). */
  deficitDaysAhead: number;
}

/** Dias corridos entre duas datas ISO (b − a). */
export function daysBetweenIso(a: string, b: string): number {
  const [ya, ma, da] = a.split("-").map(Number);
  const [yb, mb, db] = b.split("-").map(Number);
  if (!ya || !ma || !da || !yb || !mb || !db) return 0;
  const ms = Date.UTC(yb, mb - 1, db) - Date.UTC(ya, ma - 1, da);
  return Math.round(ms / 86_400_000);
}

/** "Hoje" · "Amanhã" · "Em N dias — 26 de julho" (≤ 14 dias) · "26 de julho". */
export function dueLabel(today: string, due: string): { label: string; soon: boolean } {
  const days = daysBetweenIso(today, due);
  if (days <= 0) return { label: "Hoje", soon: true };
  if (days === 1) return { label: "Amanhã", soon: true };
  if (days <= 14)
    return { label: `Em ${days} dias — ${faturaDayLabel(due)}`, soon: true };
  return { label: faturaDayLabel(due), soon: false };
}

/** Primeira entrada estritamente após hoje dentro da janela (dias corridos). */
export function upcomingIncome(
  daily: ForecastDay[],
  today: string,
  horizonDays: number,
): { date: string; cents: number } | null {
  const hit = daily.find(
    (d) =>
      d.date > today &&
      d.income_cents > 0 &&
      daysBetweenIso(today, d.date) <= horizonDays,
  );
  return hit ? { date: hit.date, cents: hit.income_cents } : null;
}

/**
 * Referência da faixa do termômetro em linguagem natural — os limiares são ABSOLUTOS
 * (canônicos da planilha de ensino), então a frase cita o R$ da fronteira.
 */
export function saldoBandPhrase(key: SaldoBand): string {
  switch (key) {
    case "comfortable":
      return "acima dos R$ 2.000 da régua da planilha";
    case "ok":
      return "entre R$ 1.000 e R$ 2.000 na régua da planilha";
    case "tight":
      return "entre zero e R$ 1.000 — a faixa de atenção da régua";
    case "negative":
      return "abaixo de zero — a régua pede socorro ao caixa";
    case "critical":
      return "mais de R$ 500 abaixo de zero";
  }
}

/** Preenchimento qualitativo da régua do termômetro (a frase é o dado; a barra reforça). */
export function saldoGaugeFraction(key: SaldoBand): number {
  switch (key) {
    case "comfortable":
      return 1;
    case "ok":
      return 0.72;
    case "tight":
      return 0.45;
    case "negative":
      return 0.18;
    case "critical":
      return 0.08;
  }
}

/** Leitura do mês para o insight da Mia — derivação pura da corrente de saldo. */
export function monthInsight(
  monthDaily: ForecastDay[],
  today: string,
): MonthInsight | null {
  if (monthDaily.length === 0) return null;
  const endBalanceCents = monthDaily[monthDaily.length - 1]!.balance_cents;
  let min = monthDaily[0]!;
  for (const day of monthDaily) {
    if (day.balance_cents < min.balance_cents) min = day;
  }
  const todayBalance = monthDaily.find((d) => d.date === today)?.balance_cents;
  const nextIncome = monthDaily.find((d) => d.date > today && d.income_cents > 0);
  return {
    endBalanceCents,
    minCents: min.balance_cents,
    minDate: min.date,
    minIsOngoing: min.date <= today && todayBalance === min.balance_cents,
    nextIncomeDate: nextIncome?.date ?? null,
    nextIncomeCents: nextIncome?.income_cents ?? 0,
    deficitDaysAhead: monthDaily.filter((d) => d.date >= today && d.balance_cents < 0)
      .length,
  };
}

/**
 * Por que o teto do dia é o que é.
 *
 * O método tem duas réguas para o dia: o caixa (não abrir o bico — o Saldo e o termômetro) e a
 * economia do ano. A reserva não é uma delas: ela é o amortecedor que se ACIONA quando o saldo
 * fica negativo, e é por isso que o déficit tem leitura própria aqui — é o momento em que o
 * método manda usar a reserva, não o momento de proibir o gasto.
 */
export type SpendCapReason =
  | { kind: "savings" }
  | { kind: "cash"; date: string | null; inCurrentMonth: boolean }
  | { kind: "deficit"; shortfallCents: number; date: string };

export function spendCapReason(input: {
  bindingGuardrail: "cash" | "savings";
  /** Menor saldo projetado do horizonte, e o dia em que ele acontece. */
  deepestBalanceCents: number;
  deepestDate: string | null;
  today: string;
}): SpendCapReason {
  if (input.bindingGuardrail === "savings") return { kind: "savings" };
  if (input.deepestBalanceCents < 0 && input.deepestDate) {
    return {
      kind: "deficit",
      shortfallCents: -input.deepestBalanceCents,
      date: input.deepestDate,
    };
  }
  return {
    kind: "cash",
    date: input.deepestDate,
    inCurrentMonth:
      input.deepestDate !== null &&
      input.deepestDate.slice(0, 7) === input.today.slice(0, 7),
  };
}

/**
 * A economia do ano rompeu a faixa do método?
 *
 * A régua de 20–30% é MÉDIA ANUAL — "tem mês que é mais, tem mês que é menos" —, então ela
 * protege a faixa enquanto está viva e para de morder depois de rompida: o déficit é do ano que
 * passou, e nenhum gasto de hoje o desfaz. Some do teto, não da tela: o diagnóstico continua
 * visível, porque é ele que aponta o caminho (subir a performance do mês).
 */
export function savingsBandBroken(savingsHeadroomCents: number | null): boolean {
  return savingsHeadroomCents != null && savingsHeadroomCents < 0;
}
