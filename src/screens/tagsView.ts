import type {
  TagRulerEffects,
  TagRulerFlags,
  TagsScreenDto,
  TagsScreenTag,
  TagsScreenThirdParty,
} from "../lib/api";
import { formatBRL } from "../lib/format";
import { MES } from "../lib/nkFormat";

// ---------------------------------------------------------------------------
// A tag é um interruptor de contabilidade: 4 réguas independentes (Performance,
// Custo de vida, Economia, Diário médio) que ela liga/desliga. O Saldo nunca tem
// interruptor — garantia estrutural, não configuração. Este módulo decide TUDO que
// não é fetch: a manchete (6 estados), a frase de cada régua, o agrupamento
// exceção×rótulo e a leitura de "dinheiro de terceiros". A tela só monta.
// ---------------------------------------------------------------------------

export type RulerKey = keyof TagRulerFlags;

/** Ordem canônica das 4 réguas — a mesma do DTO e do protótipo aprovado. */
export const RULER_ORDER: RulerKey[] = [
  "performance",
  "cost_of_living",
  "savings",
  "daily_avg",
];

export const RULER_LABEL: Record<RulerKey, string> = {
  performance: "Performance",
  cost_of_living: "Custo de vida",
  savings: "Economia",
  daily_avg: "Diário médio",
};

/** A metade FIXA de cada frase — sempre visível, ligada ou desligada: ensina o
 * vocabulário do método a quem chega hoje. */
const RULER_MEASURES: Record<RulerKey, string> = {
  performance: "Quanto sobrou no mês.",
  cost_of_living: "Quanto você gasta para viver por mês.",
  savings: "Quanto você guarda da sua renda.",
  daily_avg: "Quanto você pode gastar por dia.",
};

export function rulerMeasures(ruler: RulerKey): string {
  return RULER_MEASURES[ruler];
}

/** Nome acessível ESTÁVEL do switch — quem anuncia o estado é `aria-checked`, nunca o nome. */
export function rulerSwitchLabel(ruler: RulerKey, tagName: string): string {
  return `${RULER_LABEL[ruler]} · tag ${tagName}`;
}

// ---------------------------------------------------------------------------
// Efeito em reais quando a régua está desligada — a metade VARIÁVEL da frase.
// Sinal deriva do que o motor de fato recomputou (a contribuição marginal do
// flag invertido); nunca um número reformulado ou estimado no frontend.
// ---------------------------------------------------------------------------

export type RulerEffect =
  { kind: "text"; text: string } | { kind: "money"; cents: number; suffix: string };

/** Performance mede o líquido. O delta do motor é a contribuição da tag
 * (contando − excluído): POSITIVO = entra mais do que sai — fora da régua, o
 * resultado mostra esse tanto A MENOS ("devolve mais do que gasta" explica por
 * quê); NEGATIVO = sai mais do que entra — fora, o resultado infla A MAIS. */
function performanceEffect(deltaCents: number): RulerEffect {
  if (deltaCents === 0)
    return { kind: "text", text: "entrou e saiu: o resultado não muda." };
  if (deltaCents > 0) {
    return {
      kind: "money",
      cents: deltaCents,
      suffix: " a menos no resultado do mês — entra mais do que sai.",
    };
  }
  return {
    kind: "money",
    cents: Math.abs(deltaCents),
    suffix: " a mais no resultado do mês — sai mais do que entra.",
  };
}

/** Custo de vida soma saída. Delta POSITIVO = a tag contribuía com esse gasto —
 * fora da régua, o custo mostra esse tanto A MENOS. O negativo (resíduo com
 * sinal reduzindo a célula) fica coberto por honestidade. */
function costOfLivingEffect(deltaCents: number): RulerEffect {
  if (deltaCents === 0) return { kind: "text", text: "não pesa no seu custo de vida." };
  const cents = Math.abs(deltaCents);
  return deltaCents > 0
    ? { kind: "money", cents, suffix: " a menos no seu custo de vida." }
    : { kind: "money", cents, suffix: " a mais no seu custo de vida." };
}

/** Economia tem DUAS pernas (base/renda × economia registrada, reconciliada com a
 * anotação da aba) — a frase usa a que domina em magnitude. Quando as duas são
 * zero (dinheiro de terceiro que a régua nunca contou), o texto explica o porquê
 * em vez de mentir com "R$ 0,00 a menos". */
function savingsEffect(effects: TagRulerEffects): RulerEffect {
  const base = effects.savings_base_delta_cents;
  const amount = effects.savings_amount_delta_cents;
  const baseDominant = Math.abs(base) >= Math.abs(amount);
  const dominant = baseDominant ? base : amount;
  if (dominant === 0) return { kind: "text", text: "não entra na sua renda." };
  const cents = Math.abs(dominant);
  const where = baseDominant ? "na base da economia" : "na economia registrada";
  // Delta POSITIVO = a tag contribuía com esse valor — fora da régua, a perna
  // dominante mostra esse tanto A MENOS.
  return dominant > 0
    ? { kind: "money", cents, suffix: ` a menos ${where}.` }
    : { kind: "money", cents, suffix: ` a mais ${where}.` };
}

/** Diário médio nunca mostra número (decisão do desenho): é uma régua de ritmo,
 * não de figura isolada — o efeito exato mudaria a cada dia decorrido. */
function dailyAvgEffect(): RulerEffect {
  return { kind: "text", text: "não pesa no seu teto do dia a dia." };
}

/** Efeito da régua quando desligada; `null` quando ligada (só a frase fixa vale). */
export function rulerEffect(
  ruler: RulerKey,
  on: boolean,
  effects: TagRulerEffects,
): RulerEffect | null {
  if (on) return null;
  switch (ruler) {
    case "performance":
      return performanceEffect(effects.performance_delta_cents);
    case "cost_of_living":
      return costOfLivingEffect(effects.cost_delta_cents);
    case "savings":
      return savingsEffect(effects);
    case "daily_avg":
      return dailyAvgEffect();
  }
}

// ---------------------------------------------------------------------------
// Resumo "fora de N de 4 réguas" e agrupamento exceção × rótulo (derivado, nunca
// um campo próprio: 4 réguas ligadas = rótulo; qualquer uma desligada = exceção).
// ---------------------------------------------------------------------------

export function offRulerCount(counts: TagRulerFlags): number {
  return RULER_ORDER.filter((r) => !counts[r]).length;
}

export function exceptionSummary(counts: TagRulerFlags): string {
  const off = offRulerCount(counts);
  return off === 0
    ? "Conta em todas as réguas"
    : `Fora de ${off} de ${RULER_ORDER.length} réguas`;
}

export function isException(tag: Pick<TagsScreenTag, "counts_in">): boolean {
  return offRulerCount(tag.counts_in) > 0;
}

function byTotalDesc(a: TagsScreenTag, b: TagsScreenTag): number {
  return b.month_total_cents - a.month_total_cents;
}

export interface TagGroups {
  exceptions: TagsScreenTag[];
  labels: TagsScreenTag[];
}

/** Separa `tags[]` em Exceções × Movimentação por rótulo, maior movimento primeiro
 * em cada lista — a mesma ordenação que a Meter de rótulo usa como referência. */
export function splitExceptionsAndLabels(tags: TagsScreenTag[]): TagGroups {
  const exceptions = tags.filter(isException).toSorted(byTotalDesc);
  const labels = tags.filter((t) => !isException(t)).toSorted(byTotalDesc);
  return { exceptions, labels };
}

export function maxLabelTotal(labels: TagsScreenTag[]): number {
  return labels.reduce((max, t) => Math.max(max, t.month_total_cents), 0);
}

/** Fração 0–1 do maior rótulo — nunca % do total (a relação é N:N; as partes não
 * somam o todo, já que uma tag pode carregar mais de um rótulo). */
export function labelFraction(tag: TagsScreenTag, maxCents: number): number {
  return maxCents > 0 ? tag.month_total_cents / maxCents : 0;
}

// ---------------------------------------------------------------------------
// Manchete — a máquina de 6 estados (A–F) do desenho. E (carregando) e o erro
// duro sem cache são plumbing genérico, iguais a toda tela — resolvidos aqui só
// D/A/B/C/F, que dependem do conteúdo do DTO.
// ---------------------------------------------------------------------------

export type TagsHeadline =
  | { kind: "empty-tags" }
  | { kind: "exceptions"; costCents: number; excludedCents: number; allOnCents: number }
  | { kind: "third-party"; avgCents: number; peopleCount: number }
  | { kind: "clean"; costCents: number }
  | { kind: "stale"; costCents: number; staleAt: string | null };

/** `fetchFailed` = a leitura ATUAL falhou (erro da query com DTO em cache): o número
 * fica, com a idade da última sincronização (`last_sync_at`) — manchete F. O DTO
 * sempre carrega `last_sync_at`; sozinho ele não significa falha nenhuma. */
export function resolveHeadline(
  dto: TagsScreenDto,
  fetchFailed = false,
): TagsHeadline {
  if (dto.tags.length === 0) return { kind: "empty-tags" };
  if (fetchFailed) {
    return {
      kind: "stale",
      costCents: dto.verdict.cost_current_cents,
      staleAt: dto.last_sync_at,
    };
  }
  if (dto.verdict.has_exceptions) {
    return {
      kind: "exceptions",
      costCents: dto.verdict.cost_current_cents,
      excludedCents: dto.verdict.cost_all_on_cents - dto.verdict.cost_current_cents,
      allOnCents: dto.verdict.cost_all_on_cents,
    };
  }
  if (dto.verdict.third_party_avg_cents != null) {
    return {
      kind: "third-party",
      avgCents: dto.verdict.third_party_avg_cents,
      peopleCount: dto.verdict.third_party_people,
    };
  }
  return { kind: "clean", costCents: dto.verdict.cost_current_cents };
}

export function monthLabelLower(monthKey: string): string {
  const m = Number(monthKey.slice(5, 7));
  return (MES[m - 1] ?? "").toLowerCase();
}

/** "Custo de vida · julho" — vlabel do veredito, o mesmo em qualquer estado. */
export function verdictLabel(monthKey: string): string {
  return `Custo de vida · ${monthLabelLower(monthKey)}`;
}

export function pluralLancamentos(n: number): string {
  return n === 1 ? "1 lançamento" : `${n} lançamentos`;
}

export function pluralPessoas(n: number): string {
  return n === 1 ? "1 pessoa" : `${n} pessoas`;
}

export function pluralRotulos(n: number): string {
  return n === 1 ? "1 rótulo" : `${n} rótulos`;
}

// ---------------------------------------------------------------------------
// Dinheiro de terceiros — leitura por pessoa a partir do estado epistêmico
// (favor/em aberto/série/quitado/sem registro). Nunca fabrica "parcela"/"em
// aberto" fora do que o vínculo real (marcador, split, cartão adicional,
// expectativa) sustenta — os 5 estados vêm prontos do DTO.
// ---------------------------------------------------------------------------

export interface PersonRowView {
  personId: string;
  name: string;
  /** Até 2 letras (iniciais das primeiras palavras do nome) para o avatar-monograma. */
  initials: string;
  detail: string;
  value: { kind: "money"; cents: number } | { kind: "text"; text: string };
  tail: string;
}

export function personInitials(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .map((w) => w[0] ?? "")
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

function pluralDias(n: number): string {
  return n === 1 ? "1 dia" : `${n} dias`;
}

function pluralParcelas(n: number): string {
  return n === 1 ? "1 parcela" : `${n} parcelas`;
}

/** "4 de julho" — qualificador curto de data (mesmo padrão de Lançamentos). */
function shortDayMonth(iso: string): string {
  const [, m, d] = iso.split("-").map(Number);
  if (!m || !d) return iso;
  return `${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

export function personRow(p: TagsScreenThirdParty, monthLabel: string): PersonRowView {
  const base = {
    personId: p.person_id,
    name: p.name,
    initials: personInitials(p.name),
  };
  switch (p.state) {
    case "favor":
      return {
        ...base,
        detail: `Saiu ${formatBRL(p.out_cents)} · voltou ${formatBRL(p.back_cents)}`,
        value: { kind: "money", cents: p.back_cents - p.out_cents },
        tail: "A seu favor",
      };
    case "series": {
      const total = p.series_total ?? 0;
      const done = p.series_done ?? 0;
      const remaining = Math.max(0, total - done);
      return {
        ...base,
        detail: `Voltou ${formatBRL(p.back_cents)} · parcela ${done} de ${total}`,
        value: { kind: "money", cents: p.back_cents },
        tail: remaining > 0 ? `Falta ${pluralParcelas(remaining)}` : "Série concluída",
      };
    }
    case "open": {
      const days = p.open_since_days ?? 0;
      return {
        ...base,
        detail:
          p.back_cents > 0
            ? `Saiu ${formatBRL(p.out_cents)} · voltou ${formatBRL(p.back_cents)} até agora`
            : `Saiu ${formatBRL(p.out_cents)} · sem retorno`,
        value: { kind: "money", cents: p.out_cents - p.back_cents },
        tail: `Em aberto há ${pluralDias(days)}`,
      };
    }
    case "settled":
      return {
        ...base,
        detail: `Saiu ${formatBRL(p.out_cents)} · voltou ${formatBRL(p.back_cents)}`,
        value: { kind: "text", text: "Quitado" },
        tail: p.settled_date ? `Em ${shortDayMonth(p.settled_date)}` : "",
      };
    case "none":
      return {
        ...base,
        detail: `Nenhum lançamento em ${monthLabel}.`,
        value: { kind: "text", text: "—" },
        tail: "Sem registro",
      };
  }
}
