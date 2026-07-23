import type { LineItem, LineItemKind, TagRef, TransactionRow } from "../lib/api";
import { MES, TYPE_META, type MovementType } from "../lib/nkFormat";
import { eyebrowDate } from "./hojeView";

// ---------------------------------------------------------------------------
// O modelo célula×nota do Livro-razão: a célula (dia, coluna) é a autoridade do
// total; a nota itemiza; a diferença célula×nota é linha sintética, nunca item.
// Helpers puros — a tela orquestra, este módulo decide.
// ---------------------------------------------------------------------------

/** Filtro por tipo da tela: os 5 tipos do método + "todos". */
export type FilterKey = "todos" | MovementType;

/**
 * Mapeia um TransactionRow para um dos 5 tipos de movimento do método.
 * income → entrada · transfer → economia · expense fixa → saída ·
 * expense no crédito → cartão · demais expenses → diário.
 */
export function toMovementType(t: TransactionRow): MovementType {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
}

export interface LineItemKindMeta {
  name: string;
  color: string;
}

/** Identidade visual dos kinds de item de nota (os 5 tipos + os kinds só-de-nota). */
export const LINE_ITEM_KIND_META: Record<LineItemKind, LineItemKindMeta> = {
  entrada: { name: TYPE_META.entrada.name, color: TYPE_META.entrada.color },
  saida: { name: TYPE_META.saida.name, color: TYPE_META.saida.color },
  cartao: { name: TYPE_META.cartao.name, color: TYPE_META.cartao.color },
  diario: { name: TYPE_META.diario.name, color: TYPE_META.diario.color },
  economia: { name: TYPE_META.economia.name, color: TYPE_META.economia.color },
  patrimonio: { name: "Patrimônio", color: "var(--text-muted)" },
  ajuste: { name: "Ajuste", color: "var(--warning-400)" },
};

// ---------------------------------------------------------------------------
// Rótulos
// ---------------------------------------------------------------------------

/** "Julho de 2026" — título do mês visto (crumb da appbar e MonthNav). */
export function monthTitle(key: string): string {
  const [y, m] = key.split("-");
  const name = MES[parseInt(m ?? "1", 10) - 1] ?? "";
  return `${name} de ${y}`;
}

/** "Domingo, 12 de julho" — o daymark. Delegado ao formatador canônico de data. */
export function daymarkLabel(iso: string): string {
  return eyebrowDate(iso);
}

/** "12 de julho" — qualificador curto de vencimento. */
function shortDayMonth(iso: string): string {
  const [, m, d] = iso.split("-").map(Number);
  if (!m || !d) return iso;
  return `${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

/**
 * Normaliza o cabeçalho de seção cru da nota ("CONTAS |", "FATURAS:") para
 * exibição na coluna de contexto — a gramática do dono, sem a pontuação.
 */
export function sectionLabel(raw: string | null): string | null {
  if (!raw) return null;
  const clean = raw.replace(/[|:\s]+$/g, "").trim();
  if (!clean) return null;
  return clean.charAt(0).toUpperCase() + clean.slice(1).toLowerCase();
}

/**
 * Rótulo do cabeçalho de célula. Entrada/Saída/Diário têm célula na planilha
 * (a autoridade); Cartão pendura na fatura; Economia vive em aba própria —
 * o rótulo nunca alega uma célula que não existe.
 */
export function cellHeadLabel(type: MovementType): string {
  if (type === "cartao") return "Cartão — Soma na fatura";
  if (type === "economia") return "Economia — Total do dia";
  return `${TYPE_META[type].name} — Total da célula`;
}

// ---------------------------------------------------------------------------
// Busca
// ---------------------------------------------------------------------------

/** Normaliza para busca: minúsculas e sem diacríticos. */
export function normalizeQuery(s: string): string {
  return s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}

// ---------------------------------------------------------------------------
// Grupos de exibição
// ---------------------------------------------------------------------------

export interface RowPills {
  /** "n/N" quando a linha pertence a uma série de parcelas. */
  installment: string | null;
  /** Ainda não aconteceu (projeção ou data futura). */
  previsto: boolean;
  /** Há dinheiro que volta vinculado à linha. */
  refund: boolean;
  /** Tags do lançamento (vazio em linha de item — as tags do pai vivem no cel-head). */
  tags: TagRef[];
}

export interface DisplayRow {
  key: string;
  /** Kind do item (linha de nota) ou tipo do movimento (linha simples) — cor do ícone. */
  kind: LineItemKind;
  name: string;
  context: string;
  /** Centavos com sinal (entrada positiva, resto negativo). */
  signedCents: number;
  /** Lançamento dono das ações (editar/tags/apagar). */
  txn: TransactionRow;
  /** Item da nota quando a linha é explodida; null em linha simples. */
  item: LineItem | null;
  pills: RowPills;
}

export interface CellGroup {
  key: string;
  type: MovementType;
  label: string;
  /** Σ|amount| dos lançamentos do grupo — o valor da célula quando importada. */
  totalCents: number;
  /** Célula − Σ itens (com sinal), somada sobre os lançamentos itemizados divergentes. */
  diffCents: number;
  /** Tags dos lançamentos itemizados (o cel-head representa a célula importada). */
  tags: TagRef[];
  /** Reembolso vinculado a um lançamento itemizado do grupo. */
  refund: boolean;
  rows: DisplayRow[];
}

export interface DayGroup {
  date: string;
  cells: CellGroup[];
}

/** Ordem canônica dos 5 tipos dentro do dia (a mesma dos seletores do app). */
const CELL_ORDER: MovementType[] = ["entrada", "saida", "diario", "economia", "cartao"];

/** Divergência célula×nota conta a partir de 2 centavos (1 centavo é arredondamento). */
const DIFF_TOLERANCE_CENTS = 1;

function contextOfItem(item: LineItem): string {
  return sectionLabel(item.section) ?? LINE_ITEM_KIND_META[item.kind].name;
}

function contextOfTxn(t: TransactionRow, mv: MovementType, todayIso: string): string {
  const base = TYPE_META[mv].name;
  if (t.due_date) {
    const verb = t.due_date >= todayIso ? "vence" : "venceu";
    return `${base} · ${verb} ${shortDayMonth(t.due_date)}`;
  }
  return base;
}

function pillsOfTxn(t: TransactionRow, todayIso: string): RowPills {
  return {
    installment:
      t.installment_index != null && t.installment_total != null
        ? `${t.installment_index}/${t.installment_total}`
        : null,
    previsto: t.is_projection || t.date > todayIso,
    refund: t.has_refund_link,
    tags: t.tags,
  };
}

/** Pílulas de linha de item: só o estado temporal desce do lançamento-pai
 *  (uma célula futura precisa marcar cada linha); parcela/reembolso/tags do
 *  pai vivem no cabeçalho da célula. */
function itemPills(t: TransactionRow, todayIso: string): RowPills {
  return {
    installment: null,
    previsto: t.is_projection || t.date > todayIso,
    refund: false,
    tags: [],
  };
}

/**
 * Agrupa lançamentos em dias (ordem crescente de data) e, dentro do dia, em
 * grupos-célula por tipo (ordem canônica). Lançamentos itemizados explodem em
 * linhas de item; a divergência célula×nota acumula em `diffCents` por grupo.
 */
export function buildDayGroups(rows: TransactionRow[], todayIso: string): DayGroup[] {
  const byDay = new Map<string, Map<MovementType, TransactionRow[]>>();
  for (const t of rows) {
    if (!byDay.has(t.date)) byDay.set(t.date, new Map());
    const cells = byDay.get(t.date)!;
    const mv = toMovementType(t);
    if (!cells.has(mv)) cells.set(mv, []);
    cells.get(mv)!.push(t);
  }

  const days: DayGroup[] = [];
  for (const [date, cells] of byDay) {
    const groups: CellGroup[] = [];
    for (const type of CELL_ORDER) {
      const txns = cells.get(type);
      if (!txns) continue;
      const isEntrada = type === "entrada";
      let totalCents = 0;
      let diffCents = 0;
      const tags: TagRef[] = [];
      let refund = false;
      const displayRows: DisplayRow[] = [];
      for (const t of txns) {
        const totalAbs = Math.abs(t.amount);
        totalCents += totalAbs;
        if (t.line_items.length > 0) {
          const itemsAbs = t.line_items.reduce(
            (sum, item) => sum + Math.abs(item.amount_cents),
            0,
          );
          const diff = totalAbs - itemsAbs;
          if (Math.abs(diff) > DIFF_TOLERANCE_CENTS) diffCents += diff;
          tags.push(...t.tags);
          refund = refund || t.has_refund_link;
          for (const item of t.line_items) {
            const abs = Math.abs(item.amount_cents);
            displayRows.push({
              key: item.id ?? `${t.id}:${item.position}`,
              kind: item.kind,
              name: item.description,
              context: contextOfItem(item),
              signedCents: item.kind === "entrada" ? abs : -abs,
              txn: t,
              item,
              pills: itemPills(t, todayIso),
            });
          }
        } else {
          displayRows.push({
            key: t.id,
            kind: type,
            name: t.description || "—",
            context: contextOfTxn(t, type, todayIso),
            signedCents: isEntrada ? totalAbs : -totalAbs,
            txn: t,
            item: null,
            pills: pillsOfTxn(t, todayIso),
          });
        }
      }
      groups.push({
        key: `${date}:${type}`,
        type,
        label: cellHeadLabel(type),
        totalCents,
        diffCents,
        tags,
        refund,
        rows: displayRows,
      });
    }
    days.push({ date, cells: groups });
  }
  return days.toSorted((a, b) => (a.date < b.date ? -1 : 1));
}

/**
 * Filtra as linhas pela busca (nome + contexto, sem acento/caixa). Com busca
 * ativa a linha de reconciliação some (`diffCents` zera): os itens visíveis são
 * um subconjunto — compará-los com o total da célula mentiria.
 */
export function applySearch(days: DayGroup[], query: string): DayGroup[] {
  const q = normalizeQuery(query.trim());
  if (!q) return days;
  const out: DayGroup[] = [];
  for (const day of days) {
    const cells: CellGroup[] = [];
    for (const cell of day.cells) {
      // O palheiro inclui o lançamento-pai (descrição e data): achar "Fatura X"
      // pelo item da nota, ou um dia inteiro por "2026-07-12".
      const rows = cell.rows.filter((r) =>
        normalizeQuery(
          `${r.name} ${r.context} ${r.txn.description} ${r.txn.date}`,
        ).includes(q),
      );
      if (rows.length > 0) cells.push({ ...cell, rows, diffCents: 0 });
    }
    if (cells.length > 0) out.push({ ...day, cells });
  }
  return out;
}

export interface SplitDays {
  /** Dias após hoje, em ordem crescente (o próximo primeiro). */
  future: DayGroup[];
  /** Hoje e o passado, em ordem decrescente (o mais recente primeiro). */
  past: DayGroup[];
}

/** Divide os dias em torno de hoje — "distância de hoje" define a ordem de leitura. */
export function splitAroundToday(days: DayGroup[], todayIso: string): SplitDays {
  const future = days.filter((d) => d.date > todayIso);
  const past = days.filter((d) => d.date <= todayIso).toReversed();
  return { future, past };
}

export interface DaysSummary {
  /** Lançamentos distintos (nunca itens de nota). */
  txnCount: number;
  /** Σ com sinal por CÉLULA (a autoridade) — nunca pela nota, que pode divergir. */
  sumCents: number;
}

/** Resumo honesto de um conjunto de dias (o disclosure de futuros). */
export function daysSummary(days: DayGroup[]): DaysSummary {
  const txns = new Set<string>();
  let sumCents = 0;
  for (const day of days) {
    for (const cell of day.cells) {
      sumCents += cell.type === "entrada" ? cell.totalCents : -cell.totalCents;
      for (const row of cell.rows) txns.add(row.txn.id);
    }
  }
  return { txnCount: txns.size, sumCents };
}

/** Total de linhas exibíveis (itens contam; reconciliação não). */
export function countRows(days: DayGroup[]): number {
  let n = 0;
  for (const day of days) for (const cell of day.cells) n += cell.rows.length;
  return n;
}

// ---------------------------------------------------------------------------
// Copy dos estados vazios
// ---------------------------------------------------------------------------

export function emptyListCopy(opts: {
  query: string;
  filterName: string | null;
  monthName: string;
  cardMode: boolean;
}): string {
  const q = opts.query.trim();
  if (q)
    return `Nada em ${opts.monthName} para "${q}". Limpe a busca ou troque o filtro.`;
  if (opts.filterName) {
    const base = `Nenhum lançamento de ${opts.filterName.toLowerCase()} em ${opts.monthName}.`;
    if (opts.filterName === "Diário" && opts.cardMode)
      return `${base} No modo cartão, o variável vive nas faturas.`;
    return base;
  }
  return `Nenhum lançamento em ${opts.monthName}.`;
}
