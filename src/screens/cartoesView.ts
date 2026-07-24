import type { Card, CardPurchase, InvoiceSummary, Refund } from "../lib/api";
import { formatBRL } from "../lib/format";

/**
 * View-model puro da tela Cartões: derivações de exibição do sub-ledger de
 * faturas — janela de ciclos, barras, rótulos de estado, progresso de série.
 * Nenhuma matemática de método acontece aqui: todo número financeiro chega
 * pronto dos DTOs; o módulo só compõe, formata e nomeia.
 */

const MONTH_FORMAT = new Intl.DateTimeFormat("pt-BR", {
  month: "long",
  year: "numeric",
});
const MONTH_SHORT_FORMAT = new Intl.DateTimeFormat("pt-BR", { month: "short" });
const DATE_FORMAT = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "short",
});

function noon(iso: string): Date {
  return new Date(`${iso}T12:00:00`);
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function shortMonth(cycleMonth: string): string {
  const label = MONTH_SHORT_FORMAT.format(noon(`${cycleMonth}-01`)).replace(/\.$/, "");
  return capitalize(label);
}

/** "agosto de 2026" — para uso no meio de frase, onde maiúscula seria erro. */
export function monthLabelLower(cycleMonth: string): string {
  return MONTH_FORMAT.format(noon(`${cycleMonth}-01`));
}

/** "10 de ago." para datas humanas da tela. */
export function dateLabel(iso: string): string {
  return DATE_FORMAT.format(noon(iso));
}

/** "agosto" minúsculo — para uso no meio de frase. */
function monthNameLower(cycleMonth: string): string {
  return MONTH_FORMAT.format(noon(`${cycleMonth}-01`)).split(" de ")[0]!;
}

// ------------------------------------------------------------------ ciclos --

export const CYCLE_WINDOW_LIMIT = 6;

/**
 * Janela de exibição: ciclos em ordem velho → novo, `limit` no total. Com uma
 * âncora (a fatura aberta/próxima), a janela a contém sempre — séries longas
 * materializam muitas previstas à frente, e "os últimos 6" deixariam a aberta
 * sem rádio nem barra. A âncora senta na penúltima posição quando há história:
 * até `limit − 2` ciclos vividos antes dela + 1 prevista depois.
 */
export function cycleWindow(
  invoices: InvoiceSummary[],
  anchorId?: string | null,
  limit = CYCLE_WINDOW_LIMIT,
): InvoiceSummary[] {
  const sorted = invoices.toSorted((a, b) =>
    a.cycle_month.localeCompare(b.cycle_month),
  );
  const anchorIdx = anchorId ? sorted.findIndex((i) => i.id === anchorId) : -1;
  if (anchorIdx < 0) return sorted.slice(-limit);
  const start = Math.min(
    Math.max(0, anchorIdx - (limit - 2)),
    Math.max(0, sorted.length - limit),
  );
  return sorted.slice(start, start + limit);
}

export interface CycleOption {
  value: string;
  label: string;
}

/**
 * Opções do seletor de ciclo: "Ago · Aberta"; o ano só aparece quando o ciclo
 * não pertence ao ano do ciclo mais recente da janela ("Dez ’25 · Paga").
 */
export function cycleOptions(window: InvoiceSummary[]): CycleOption[] {
  const latestYear = window.at(-1)?.cycle_month.slice(0, 4);
  return window.map((invoice) => ({
    value: invoice.id,
    label: `${monthWithYear(invoice.cycle_month, latestYear)} · ${capitalize(invoice.status)}`,
  }));
}

/** "Ago", ou "Dez ’25" quando o ciclo não pertence ao ano de referência. */
function monthWithYear(cycleMonth: string, latestYear: string | undefined): string {
  const year = cycleMonth.slice(0, 4);
  const suffix = year === latestYear ? "" : ` ’${year.slice(2)}`;
  return `${shortMonth(cycleMonth)}${suffix}`;
}

/** Seleção default do drill: a aberta, senão a próxima a vencer, senão a mais recente. */
export function defaultInvoiceId(
  card: Pick<Card, "open_invoice" | "next_due">,
  invoices: InvoiceSummary[],
): string | null {
  if (card.open_invoice) return card.open_invoice.id;
  if (card.next_due) return card.next_due.id;
  return cycleWindow(invoices).at(-1)?.id ?? null;
}

// ------------------------------------------------------------------- barras --

export interface BarModel {
  id: string;
  label: string;
  /** Altura 0–100, normalizada pelo maior ciclo da janela. */
  pct: number;
  cents: number;
  selected: boolean;
  /** Valor zero de verdade — a barra não desenha nem o mínimo visual. */
  zero: boolean;
}

export interface BarsModel {
  bars: BarModel[];
  /** Equivalente textual completo (mês → valor) para `role="img"`. */
  aria: string;
  caption: string;
}

export function buildBars(
  window: InvoiceSummary[],
  selectedId: string | null,
): BarsModel {
  const max = Math.max(...window.map((i) => i.effective_total_cents), 0);
  const latestYear = window.at(-1)?.cycle_month.slice(0, 4);
  const bars = window.map((invoice) => ({
    id: invoice.id,
    label: monthWithYear(invoice.cycle_month, latestYear),
    pct: max > 0 ? Math.round((invoice.effective_total_cents / max) * 100) : 0,
    cents: invoice.effective_total_cents,
    selected: invoice.id === selectedId,
    zero: invoice.effective_total_cents === 0,
  }));
  const selected = window.find((i) => i.id === selectedId);
  const accumulating =
    selected?.status === "aberta"
      ? ` — a de ${monthNameLower(selected.cycle_month)} ainda acumula.`
      : ".";
  return {
    bars,
    aria: `Faturas por ciclo: ${window
      .map(
        (i) =>
          `${monthWithYear(i.cycle_month, latestYear)} ${formatBRL(i.effective_total_cents)}`,
      )
      .join(", ")}.`,
    caption: `Faturas dos últimos ${window.length} ciclos${accumulating}`,
  };
}

// -------------------------------------------------------------------- herói --

/** Rótulo da linha-cabeça de Totais — a mesma autoridade do herói. */
export function totalsHeadLabel(statedTotalCents: number | null): string {
  return statedTotalCents != null ? "Total declarado" : "Compras itemizadas";
}

/** O subtítulo diz qual autoridade produziu o número: o total declarado da
    planilha manda quando existe; sem ele, o efetivo é a soma das compras. */
export function heroSubtitle(statedTotalCents: number | null): string {
  return statedTotalCents != null
    ? "Total declarado — autoridade da planilha"
    : "Soma das compras itemizadas";
}

function daysBetween(fromISO: string, toISO: string): number {
  return Math.round((noon(toISO).getTime() - noon(fromISO).getTime()) / 86_400_000);
}

/** Estado do ciclo em uma frase, relativa a `todayISO` (injetado — puro). */
export function cycleStateLabel(invoice: InvoiceSummary, todayISO: string): string {
  switch (invoice.status) {
    case "aberta": {
      const days = Math.max(0, daysBetween(todayISO, invoice.closing_date));
      if (days === 0) return "Fecha hoje";
      if (days === 1) return "Fecha amanhã";
      return `Fecha em ${days} dias`;
    }
    case "fechada":
      return `Fechou em ${dateLabel(invoice.closing_date)}`;
    case "prevista":
      return `Fecha em ${dateLabel(invoice.closing_date)}`;
    case "paga":
      return `Paga em ${dateLabel(invoice.due_date)}`;
  }
}

// -------------------------------------------------------------------- séries --

export interface InstallmentProgress {
  current: number;
  total: number;
  fraction: number;
  remainingCents: number;
  remainingCycles: number;
}

/** Progresso de parcela a partir do rótulo `n/N`; malformado → null, nunca inventa. */
export function installmentProgress(
  label: string | null,
  amountCents: number,
): InstallmentProgress | null {
  if (!label) return null;
  const match = /^(\d+)\/(\d+)$/.exec(label);
  if (!match) return null;
  const current = Number(match[1]);
  const total = Number(match[2]);
  if (current < 1 || total < current) return null;
  const remainingCycles = total - current;
  return {
    current,
    total,
    fraction: current / total,
    remainingCents: remainingCycles * amountCents,
    remainingCycles,
  };
}

/** Cadência de assinatura a partir da ocorrência exibida. */
export function subscriptionCadence(dateISO: string): string {
  const day = noon(dateISO).getDate();
  return `Todo mês, dia ${day} · pré-lança nas faturas futuras`;
}

export interface CardSeries {
  id: string;
  kind: "subscription" | "installment";
  occurrence: CardPurchase;
}

/** Uma linha por série, representada pela primeira ocorrência do ciclo. */
export function groupSeries(purchases: CardPurchase[]): CardSeries[] {
  const series = new Map<string, CardPurchase>();
  purchases.forEach((purchase) => {
    if (purchase.series_id && !series.has(purchase.series_id)) {
      series.set(purchase.series_id, purchase);
    }
  });
  return Array.from(series, ([id, occurrence]) => ({
    id,
    kind: occurrence.installment_label
      ? ("installment" as const)
      : ("subscription" as const),
    occurrence,
  }));
}

/** O vencimento mais próximo entre os titulares — alimenta o veredito da tela. */
export function nextDueAcross(
  cards: Card[],
): { card: Card; invoice: InvoiceSummary } | null {
  return cards.reduce<{ card: Card; invoice: InvoiceSummary } | null>(
    (best, card) =>
      card.next_due && (!best || card.next_due.due_date < best.invoice.due_date)
        ? { card, invoice: card.next_due }
        : best,
    null,
  );
}

/**
 * O veredito da tela: o vencimento mais próximo. O `dateLabel` termina com o
 * ponto da abreviação do mês ("ago.") — removido para o ponto final ser um só.
 */
export function verdictLine(cards: Card[]): string {
  const next = nextDueAcross(cards);
  if (!next) return "Seus cartões, fatura a fatura.";
  return `A próxima fatura vence ${dateLabel(next.invoice.due_date).replace(/\.$/, "")}.`;
}

// ------------------------------------------------------------------- fechos --

/** Líquido de reembolsos — leitura de conferência; as réguas julgam o bruto. */
export function netOfRefunds(detail: {
  effective_total_cents: number;
  refunds: Refund[];
}): number {
  return (
    detail.effective_total_cents -
    detail.refunds.reduce((sum, refund) => sum + refund.amount_cents, 0)
  );
}

/** Linha de meta do cartão: ciclo + limite discreto (nunca barra, nunca cor). */
export function metaLine(
  card: Pick<Card, "closing_day" | "due_day" | "credit_limit_cents">,
): string {
  const cycle = `Fecha dia ${card.closing_day} · vence dia ${card.due_day}`;
  return card.credit_limit_cents != null
    ? `${cycle} · limite ${formatBRL(card.credit_limit_cents, true)}`
    : cycle;
}

export function ownerKind(name: string): "personal" | "partner" | "shared" {
  return name === "Eu"
    ? "personal"
    : name.toLowerCase().includes("compart")
      ? "shared"
      : "partner";
}
