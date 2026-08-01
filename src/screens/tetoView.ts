import type {
  CeilingProposal,
  DailyBudget,
  DailyBudgetCategoryInput,
  DashboardSummary,
} from "../lib/api";
import { parseBRLToCents } from "../lib/format";
import { MES } from "../lib/nkFormat";

// View-model puro da tela Teto do diário. A tela é o registro de uma decisão com prova: aqui
// vive a composição — qual estado a manchete assume, a aritmética da cerimônia (itens ÷ dias,
// arredondado para cima), a idade que convida à recalibração e a guarda do teto que baixa.
// Nenhum número de caixa nasce aqui: o teto vigente e a estimativa vêm do motor.

/** Uma linha editável do rito (identidade estável para o React). */
export interface DraftItem {
  key: string;
  name: string;
  amountText: string;
}

/** A cerimônia que produziu o teto, pronta para ser exibida como prova. */
export interface TetoProof {
  items: { id: string; name: string; amountCents: number }[];
  totalCents: number;
  divisorDays: number;
  /** O teto que ESTES itens produzem — recalculado, nunca copiado do registro. */
  perDayCents: number;
  /** Nota crua da planilha, quando o teto nasceu de uma proposta aceita. */
  sourceNote: string | null;
}

export type TetoStateKind = "loading" | "chosen" | "proposal" | "estimate" | "none";

export interface TetoInput {
  budget: DailyBudget | undefined;
  proposal: CeilingProposal | null | undefined;
  summary: DashboardSummary | undefined;
  /** Hoje em ISO (`YYYY-MM-DD`) — define a idade da cerimônia. */
  today: string;
}

export interface TetoView {
  kind: TetoStateKind;
  /** O número da manchete: o teto escolhido, o proposto ou a estimativa. 0 = nada a mostrar. */
  perDayCents: number;
  /** Teto vigente, independente da manchete — o confronto que a proposta precisa fazer. */
  currentPerDayCents: number;
  mode: "debit" | "card";
  ceremonyMonth: string | null;
  ageMonths: number | null;
  /** A cadência do método: refazer de três em três meses. */
  recalibrationDue: boolean;
  proof: TetoProof | null;
  /** A prova fecha com o teto vigente? `true` quando não há prova a confrontar. */
  proofMatchesVerdict: boolean;
  proposal: CeilingProposal | null;
  /** Operandos da estimativa, para a tela imprimir a conta. Só existe no estado `estimate`. */
  estimateBasis: { variableCents: number; days: number; month: string } | null;
}

/** Cadência do método: a cerimônia se refaz de três em três meses. */
const RECALIBRATION_MONTHS = 3;

const POR_EXTENSO = [
  "zero",
  "um",
  "dois",
  "três",
  "quatro",
  "cinco",
  "seis",
  "sete",
  "oito",
  "nove",
  "dez",
  "onze",
  "doze",
];

/**
 * O teto do dia a partir da cerimônia: `total ÷ dias`, sempre **para cima** — teto é teto, e
 * um centavo a menos já seria um teto que a cerimônia não autorizou. Mesma regra do núcleo Rust,
 * para o aceite da proposta e o rito nunca divergirem.
 */
export function ceilingPerDayCents(totalCents: number, divisorDays: number): number {
  if (!Number.isFinite(divisorDays) || divisorDays <= 0) return 0;
  return Math.ceil(totalCents / divisorDays);
}

/** Soma das linhas do rascunho; linha sem valor legível vale zero. */
export function draftTotalCents(items: DraftItem[]): number {
  return items.reduce((sum, it) => sum + (parseBRLToCents(it.amountText) ?? 0), 0);
}

/**
 * As linhas preenchidas do rascunho viram categorias para gravar, na ordem em que aparecem.
 * Linha sem nome ou sem valor é descarte (o rito sempre tem uma linha em branco no fim), e a
 * posição é reatribuída em sequência para o registro nunca guardar buracos.
 */
export function categoriesFromDraft(items: DraftItem[]): DailyBudgetCategoryInput[] {
  const categories: DailyBudgetCategoryInput[] = [];
  for (const item of items) {
    const name = item.name.trim();
    const amountCents = parseBRLToCents(item.amountText) ?? 0;
    if (name === "" || amountCents <= 0) continue;
    categories.push({
      name,
      amount_cents: amountCents,
      position: categories.length,
    });
  }
  return categories;
}

/** O divisor só existe como inteiro positivo — vazio, zero e lixo devolvem `null`. */
export function divisorFromText(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const n = Number.parseInt(trimmed, 10);
  return n > 0 ? n : null;
}

/** Meses completos entre a cerimônia (`YYYY-MM`) e hoje. */
export function ceremonyAgeMonths(
  ceremonyMonth: string | null,
  today: string,
): number | null {
  if (!ceremonyMonth) return null;
  const [cy, cm] = ceremonyMonth.split("-").map((p) => Number.parseInt(p, 10));
  if (!cy || !cm) return null;
  const ty = Number.parseInt(today.slice(0, 4), 10);
  const tm = Number.parseInt(today.slice(5, 7), 10);
  return Math.max(0, (ty - cy) * 12 + (tm - cm));
}

/**
 * "setembro de 2025" — o mês por extenso, em minúscula (nome de mês no meio da frase é
 * minúscula em pt-BR; a maiúscula da constante serve a rótulos). `null` em mês inválido.
 */
export function monthYearLabel(ym: string | null): string | null {
  if (!ym) return null;
  const [year, month] = ym.split("-").map((p) => Number.parseInt(p, 10));
  const name = MES[(month ?? 0) - 1];
  if (!name || !year) return null;
  return `${name.toLowerCase()} de ${year}`;
}

/** "Estipulado em setembro de 2025" — a procedência acima da manchete. */
export function ceremonyMonthLabel(ceremonyMonth: string | null): string {
  const label = monthYearLabel(ceremonyMonth);
  return label ? `Estipulado em ${label}` : "Estipulado por você";
}

/** A idade em português: por extenso até doze meses, em anos depois disso. */
export function ceremonyAgeLabel(months: number): string {
  if (months <= 0) return "A cerimônia é deste mês";
  if (months === 1) return "A cerimônia fez um mês";
  if (months <= 12) return `A cerimônia fez ${POR_EXTENSO[months]} meses`;
  const years = Math.floor(months / 12);
  if (years === 1) return "A cerimônia fez mais de um ano";
  return `A cerimônia fez mais de ${POR_EXTENSO[years] ?? years} anos`;
}

/**
 * A guarda do "vença o dia": só intercepta quem BAIXA um teto vigente. Baixar por esperança
 * pinta a planilha de verde sem mudar o extrato — a guarda ensina a consequência e libera a
 * escolha.
 */
export function guardTriggered(
  currentPerDayCents: number,
  newPerDayCents: number,
): boolean {
  return (
    currentPerDayCents > 0 && newPerDayCents > 0 && newPerDayCents < currentPerDayCents
  );
}

/** As cinco perguntas do método, na voz da casa — a cerimônia de quem ainda não tem teto. */
export const GUIDED_QUESTIONS = [
  {
    category: "Comida",
    question: "Quanto você gasta por mês com comida?",
    hint: "Mercado, padaria, delivery, almoço fora. Não precisa ser exato — o ajuste vem com a observação.",
  },
  {
    category: "Transporte",
    question: "E com transporte?",
    hint: "Combustível, aplicativo, ônibus, estacionamento.",
  },
  {
    category: "Saúde",
    question: "Quanto costuma sair com saúde?",
    hint: "Farmácia, consultas, exames — o que você paga do bolso todo mês.",
  },
  {
    category: "Lazer",
    question: "E com lazer?",
    hint: "Bar, cinema, viagem curta, assinaturas de entretenimento.",
  },
  {
    category: "Compras",
    question: "Por último: quanto vai por mês em compras?",
    hint: "Roupa, casa, presentes — o gasto variável que não cabe nas outras quatro.",
  },
] as const;

function proofFrom(budget: DailyBudget): TetoProof | null {
  if (budget.categories.length === 0 || !budget.divisor_days) return null;
  const items = budget.categories.map((c) => ({
    id: c.id,
    name: c.name,
    amountCents: c.amount_cents,
  }));
  const totalCents = items.reduce((sum, it) => sum + it.amountCents, 0);
  return {
    items,
    totalCents,
    divisorDays: budget.divisor_days,
    perDayCents: ceilingPerDayCents(totalCents, budget.divisor_days),
    sourceNote: budget.source_note,
  };
}

export function buildTetoView(input: TetoInput): TetoView {
  const { budget, proposal, summary, today } = input;
  const mode = summary?.spending_mode ?? "debit";

  if (!budget) {
    return {
      kind: "loading",
      perDayCents: 0,
      currentPerDayCents: 0,
      mode,
      ceremonyMonth: null,
      ageMonths: null,
      recalibrationDue: false,
      proof: null,
      proofMatchesVerdict: true,
      proposal: null,
      estimateBasis: null,
    };
  }

  const currentPerDayCents = budget.per_day_cents;
  const proof = currentPerDayCents > 0 ? proofFrom(budget) : null;
  const ageMonths = ceremonyAgeMonths(budget.ceremony_month, today);
  const base = {
    currentPerDayCents,
    mode,
    ceremonyMonth: budget.ceremony_month,
    ageMonths,
    // A cadência só vale para um teto vigente: sem teto, o convite é estipular, não recalibrar.
    recalibrationDue:
      currentPerDayCents > 0 && ageMonths != null && ageMonths >= RECALIBRATION_MONTHS,
    proof,
    proofMatchesVerdict: proof == null || proof.perDayCents === currentPerDayCents,
    proposal: proposal ?? null,
    // Só o estado `estimate` tem conta a mostrar; os outros sobrescrevem quando têm.
    estimateBasis: null,
  };

  // A proposta é uma decisão esperando o dono — ela toma a manchete mesmo com teto vigente, que
  // segue visível no confronto ("substitui o atual de R$ …"). Nada é gravado sem o aceite.
  if (proposal) {
    return { ...base, kind: "proposal", perDayCents: proposal.per_day_cents };
  }
  if (currentPerDayCents > 0) {
    return { ...base, kind: "chosen", perDayCents: currentPerDayCents };
  }
  if (summary?.daily_ceiling_source === "estimate" && summary.daily_budget > 0) {
    const basis = summary.daily_ceiling_estimate;
    return {
      ...base,
      kind: "estimate",
      perDayCents: summary.daily_budget,
      estimateBasis: basis
        ? { variableCents: basis.variable_cents, days: basis.days, month: basis.month }
        : null,
    };
  }
  return { ...base, kind: "none", perDayCents: 0 };
}
