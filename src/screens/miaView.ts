import type { DashboardSummary, Forecast } from "../lib/api";
import { GLOSSARY } from "../design-system/glossary";
import { formatBRL } from "../lib/format";
import { MES } from "../lib/nkFormat";
import { saldoBand } from "../lib/saldoHeatmap";
import { faturaDayLabel, localTodayIso, openInvoicesView } from "./hojeView";
import type { Screen } from "../shell/screens";

// View-model puro da tela Mia — a conversa. Aqui moram o roteamento da pergunta, as
// respostas determinísticas e as recusas honestas. Nenhuma regra de método nasce aqui:
// cada número vem pronto dos DTOs do motor e o recibo imprime a operação que ELE fez.
// Quando o runtime do copiloto entrar, ele publica nesta mesma forma — o recibo é o
// contrato visual da resposta, não um detalhe da implementação de hoje.

// ------------------------------------------------------------------- tipos --

/** Trecho de texto de uma resposta: prosa, ênfase ou dinheiro (que rende tabular). */
export type Span =
  { t: "text"; s: string } | { t: "strong"; s: string } | { t: "money"; cents: number };

/** Operação impressa entre os operandos do recibo. */
export type ReceiptOp = "min" | "minus" | "div" | "eq";

/** Tom do método: paz, atenção, alerta. Nunca segue o acento da marca. */
export type Tone = "ok" | "warn" | "bad";

export interface ReceiptLine {
  label: string;
  /** Valor monetário (renderiza tabular); exclusivo com `text`. */
  cents?: number;
  text?: string;
  op?: ReceiptOp;
  result?: boolean;
  tone?: Tone;
  /** Selo epistêmico: número derivado sai marcado, com a didática do ritual que o
   *  tornaria veredito. */
  mark?: { kind: "estimate"; term: { title?: string; body: string } };
}

/** De onde a resposta vem — a linha de proveniência do pé da bolha. */
export type Provenance = "calculo" | "metodo";

/** Motivo da recusa (taxonomia do contrato do copiloto). */
export type Refusal = "sem_dado" | "capacidade" | "ambigua" | "nao_ligada";

export interface AnswerCta {
  label: string;
  /** Tela de destino, ou o gesto global de registrar. */
  target: Screen | "compose";
}

export interface MiaAnswer {
  text: Span[];
  receipt?: ReceiptLine[];
  /** Parágrafo após o recibo — o fato que não entra na conta. */
  note?: Span[];
  provenance: Provenance;
  refusal?: Refusal;
  cta?: AnswerCta;
  /** Perguntas oferecidas (recusa ambígua e conversa ainda não ligada). */
  options?: string[];
}

export interface MiaFacts {
  summary: DashboardSummary;
  forecast: Forecast;
  today: string;
}

// -------------------------------------------------------------- repertório --

export type IntentId =
  | "gastar_hoje"
  | "mes"
  | "economia_ano"
  | "reserva"
  | "buraco"
  | "faturas"
  | "gasto_por_categoria"
  | "registrar"
  | "editar"
  | "prelancar"
  | `termo_${string}`;

export type Route =
  | { kind: "intent"; id: IntentId }
  | { kind: "ambiguous"; ids: IntentId[] }
  | { kind: "unknown" };

/** As perguntas que viram pílulas: exemplos do repertório, nunca contrato de capacidade. */
export const SUGGESTIONS = [
  "Quanto posso gastar hoje?",
  "Como o mês está indo?",
  "Tem buraco na estrada?",
  "O que é buraco do futuro?",
  "Como está a economia do ano?",
  "Como está a reserva?",
  "Quando vence a próxima fatura?",
];

/** Rótulo curto de cada intenção — usado quando a Mia devolve as opções de uma ambiguidade. */
const INTENT_QUESTION: Record<string, string> = {
  gastar_hoje: "Quanto posso gastar hoje?",
  mes: "Como o mês está indo?",
  economia_ano: "Como está a economia do ano?",
  reserva: "Como está a reserva?",
  buraco: "Tem buraco na estrada?",
  faturas: "Quando vence a próxima fatura?",
};

/** Termos que a Mia ensina — a chave é a do glossário da UI (vocabulário único). */
const TEACH_TERMS: { key: string; patterns: string[] }[] = [
  { key: "buraco_do_futuro", patterns: ["buraco"] },
  { key: "termometro", patterns: ["termometro"] },
  { key: "diario", patterns: ["diario"] },
  { key: "performance", patterns: ["performance"] },
  { key: "custo_de_vida", patterns: ["custo de vida"] },
  { key: "economizado", patterns: ["economizado", "economia"] },
  { key: "reserva", patterns: ["reserva"] },
  { key: "pode_gastar", patterns: ["pode gastar", "posso gastar"] },
  { key: "cartao", patterns: ["cartao", "fatura"] },
  { key: "colchao", patterns: ["colchao"] },
  { key: "previsibilidade", patterns: ["previsibilidade"] },
];

/** Gatilhos por intenção. Palavra isolada casa com limite (`\b`); frase casa como substring. */
const INTENT_PATTERNS: { id: IntentId; patterns: string[] }[] = [
  {
    id: "gastar_hoje",
    patterns: [
      "posso gastar",
      "pode gastar",
      "gastar hoje",
      "quanto posso",
      "sobra hoje",
    ],
  },
  { id: "mes", patterns: ["mes", "performance", "como o mes", "sobra este mes"] },
  {
    id: "economia_ano",
    patterns: ["economia", "economizado", "poupanca", "do ano", "no ano", "guardei"],
  },
  { id: "reserva", patterns: ["reserva", "emergencia"] },
  {
    id: "buraco",
    patterns: ["buraco", "estrada", "horizonte", "vermelho", "negativo", "furo"],
  },
  { id: "faturas", patterns: ["fatura", "cartao", "vence", "vencimento", "credito"] },
  {
    id: "gasto_por_categoria",
    patterns: ["onde gastei", "gastei mais", "categoria", "gastei com", "maior gasto"],
  },
  {
    id: "registrar",
    patterns: [
      "registra",
      "registrar",
      "lanca",
      "lancar",
      "anota",
      "anotar",
      "cadastra",
    ],
  },
  {
    id: "editar",
    patterns: ["apaga", "apagar", "edita", "editar", "corrige", "exclui", "deleta"],
  },
  {
    id: "prelancar",
    patterns: ["pre lancar", "prelancar", "em lote", "pre lancamento"],
  },
];

/** Pergunta de definição: muda o destino de "buraco" (o cálculo) para "o buraco" (o conceito). */
const TEACH_MARKERS = [
  "o que e",
  "o que sao",
  "o que significa",
  "que significa",
  "como funciona",
  "me explica",
  "explica",
  "explique",
];

/** Minúsculas, sem acento e sem pontuação — a forma em que os gatilhos são escritos. */
function normalize(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .replace(/[^a-z0-9\s]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function hits(haystack: string, pattern: string): boolean {
  if (pattern.includes(" ")) return haystack.includes(pattern);
  return new RegExp(`\\b${pattern}\\b`).test(haystack);
}

export function routeQuestion(question: string): Route {
  const text = normalize(question);
  if (!text) return { kind: "unknown" };

  if (TEACH_MARKERS.some((m) => hits(text, m))) {
    const term = TEACH_TERMS.find((t) => t.patterns.some((p) => hits(text, p)));
    if (term) return { kind: "intent", id: `termo_${term.key}` };
  }

  const scored: { id: IntentId; score: number }[] = [];
  for (const { id, patterns } of INTENT_PATTERNS) {
    let score = 0;
    for (const pattern of patterns) {
      if (hits(text, pattern)) score += 1;
    }
    if (score > 0) scored.push({ id, score });
  }

  if (scored.length === 0) return { kind: "unknown" };
  const top = Math.max(...scored.map((s) => s.score));
  const winners: IntentId[] = [];
  for (const scoredIntent of scored) {
    if (scoredIntent.score === top) winners.push(scoredIntent.id);
  }
  if (winners.length === 1) return { kind: "intent", id: winners[0]! };
  // Empate: perguntar é sempre mais barato que supor — só as intenções com pergunta
  // própria viram opções (uma capacidade não suportada não é alternativa a nada).
  const answerable = winners.filter((id) => INTENT_QUESTION[id]);
  if (answerable.length >= 2) return { kind: "ambiguous", ids: answerable };
  return { kind: "intent", id: winners[0]! };
}

// ------------------------------------------------------------- formatadores --

const t = (s: string): Span => ({ t: "text", s });
const b = (s: string): Span => ({ t: "strong", s });
const m = (cents: number): Span => ({ t: "money", cents });

/** Texto corrido de uma resposta — a base do `aria-label` e das asserções de copy. */
export function plainText(spans: Span[]): string {
  return spans.map((s) => (s.t === "money" ? formatBRL(s.cents) : s.s)).join("");
}

/** "Hoje" · "Ontem" · "2 de julho" — o marco que separa os blocos da conversa. */
export function dayMarkLabel(dateISO: string, todayISO: string): string {
  if (dateISO === todayISO) return "Hoje";
  const [y, mo, d] = todayISO.split("-").map(Number);
  if (y && mo && d) {
    const prev = new Date(Date.UTC(y, mo - 1, d - 1)).toISOString().slice(0, 10);
    if (dateISO === prev) return "Ontem";
  }
  return faturaDayLabel(dateISO);
}

/**
 * Carimbo local de uma mensagem ("2026-07-15T22:41"). O relógio da conversa é o do
 * usuário: `toISOString()` carimbaria em UTC e a mensagem das 22h41 apareceria como 01h41
 * do dia seguinte — com o marco de dia errado junto.
 */
export function localStamp(now: Date = new Date()): string {
  const hh = String(now.getHours()).padStart(2, "0");
  const mm = String(now.getMinutes()).padStart(2, "0");
  return `${localTodayIso(now)}T${hh}:${mm}`;
}

/** "22h41" — a hora de uma mensagem, no formato do app. */
export function timeLabel(iso: string): string {
  const time = iso.slice(11, 16);
  return time ? time.replace(":", "h") : "";
}

/** Percentual TRUNCADO: exibir 21% de 21,9% nunca promete o que o motor não mediu. */
function pct(bps: number): string {
  return `${Math.trunc(bps / 100)}%`;
}

/** "4,5 meses" · "6 meses" — truncado, para a cobertura nunca parecer maior do que é. */
function monthsLabel(months: number): string {
  const truncated = Math.trunc(months * 10) / 10;
  const text = Number.isInteger(truncated)
    ? String(truncated)
    : truncated.toFixed(1).replace(".", ",");
  return `${text} ${truncated === 1 ? "mês" : "meses"}`;
}

/** Mês por extenso ("julho") a partir de "YYYY-MM-DD". */
function monthName(iso: string): string {
  const month = Number(iso.slice(5, 7));
  return (MES[month - 1] ?? "").toLowerCase();
}

function toneForBalance(cents: number): Tone {
  const band = saldoBand(cents);
  if (band === "negative" || band === "critical") return "bad";
  if (band === "tight") return "warn";
  return "ok";
}

// ---------------------------------------------------------------- respostas --

function teach(key: string): MiaAnswer {
  const entry = GLOSSARY[key];
  if (!entry) return notLinked();
  return {
    text: entry.title ? [b(entry.title), t(" — " + entry.body)] : [t(entry.body)],
    provenance: "metodo",
  };
}

function notLinked(linked = false): MiaAnswer {
  return {
    text: [
      t(
        "A conversa aberta ainda não está ligada — quando estiver, eu respondo qualquer pergunta sobre os seus números. Hoje eu já sei responder estas:",
      ),
    ],
    provenance: "metodo",
    refusal: "nao_ligada",
    options: SUGGESTIONS.slice(0, 6),
    ...(linked ? {} : { cta: { label: "Autorizar a conversa", target: "config" } }),
  };
}

function noData(text: Span[], cta?: AnswerCta): MiaAnswer {
  return { text, provenance: "calculo", refusal: "sem_dado", ...(cta ? { cta } : {}) };
}

function podeGastar(facts: MiaFacts): MiaAnswer {
  const { forecast, summary } = facts;
  const safe = forecast.safe_to_spend_today_cents;
  const savingsBound = forecast.binding_guardrail === "savings";
  const receipt: ReceiptLine[] = [
    { label: "Limite do caixa", cents: forecast.cash_headroom_cents },
  ];
  if (forecast.savings_headroom_cents !== null) {
    receipt.push({
      label: "Limite da economia",
      cents: forecast.savings_headroom_cents,
      op: "min",
    });
  }
  receipt.push({
    // Guardrail negativo (o dia já passou dos dois limites) clampa só na EXIBIÇÃO, como no
    // resto do app: o veredito é segurar, e "−R$ 50,00 por gastar" não é uma quantia.
    label: "Pode gastar hoje",
    cents: Math.max(0, safe),
    op: "eq",
    result: true,
    tone: safe > 0 ? "ok" : "warn",
  });

  const head: Span[] =
    safe > 0
      ? [t("Hoje você pode gastar até "), m(safe), t(". ")]
      : [t("Hoje o melhor é segurar: o limite está em "), m(0), t(". ")];
  const rule = savingsBound
    ? "Sem tocar na economia planejada do ano."
    : "Sem deixar nenhum dia no vermelho.";

  // O teto NÃO entra no mín do motor — é o segundo limite do dia, e dizer o contrário
  // seria fabricar um `min` que o guardrail não computa.
  const ceiling = summary.daily_budget;
  const spent = summary.daily_spend_today;
  const left = ceiling - spent;
  const source = summary.daily_ceiling_source;
  let note: Span[];
  let cta: AnswerCta | undefined;
  if (source === "none") {
    note = [
      t(
        "Você ainda não estipulou um teto do diário — hoje o dia corre só pelo caixa e pela economia.",
      ),
    ];
    cta = { label: "Estipular o teto", target: "teto" };
  } else {
    const mark = source === "estimate" ? " (estimativa da média)" : "";
    note = [
      t("O teto do diário é o segundo limite do dia: "),
      m(ceiling),
      t(` por dia${mark}, `),
      m(spent),
      t(" já gasto hoje — "),
      left > 0 ? m(left) : m(0),
      t(
        left > 0
          ? " ainda cabem por ele."
          : " pelo teto: o dia já fechou a conta dele.",
      ),
    ];
  }

  return {
    text: [...head, b(rule)],
    receipt,
    note,
    provenance: "calculo",
    ...(cta ? { cta } : {}),
  };
}

function mesAnswer(facts: MiaFacts): MiaAnswer {
  const { forecast, summary, today } = facts;
  const key = today.slice(0, 7);
  const month = forecast.months.find(
    (mm) => `${mm.year}-${String(mm.month).padStart(2, "0")}` === key,
  );
  if (!month) {
    return noData(
      [
        t(
          "Ainda não tenho um mês fechado o bastante para ler. Assim que os lançamentos do mês entrarem, eu faço a conta.",
        ),
      ],
      { label: "Abrir Lançamentos", target: "lancamentos" },
    );
  }
  const income = month.income_performance_cents;
  const performance = month.performance_cents;
  // A decomposição por bucket vive na tela do mês; aqui a conta impressa fecha SEMPRE —
  // as réguas do motor podem mascarar buckets de formas diferentes, e uma soma que não
  // fecha é uma fórmula mentindo em prosa.
  const outflow = income - performance;
  return {
    text: [
      t("Este mês entrou "),
      m(income),
      t(" e, depois de tudo o que saiu e do que foi guardado, a performance está em "),
      m(performance),
      t("."),
    ],
    receipt: [
      { label: "Entradas do mês", cents: income },
      { label: "Saídas e economia do mês", cents: outflow, op: "minus" },
      {
        label: "Performance do mês",
        cents: performance,
        op: "eq",
        result: true,
        tone: performance >= 0 ? "ok" : "warn",
      },
    ],
    note: [
      t("O custo de vida — Saídas, Diário e Cartão — está em "),
      m(month.cost_of_living_cents),
      t(", e o saldo previsto para o fim de "),
      t(monthName(today)),
      t(" é "),
      m(summary.balance),
      t("."),
    ],
    provenance: "calculo",
    cta: { label: "Abrir Este mês", target: "mes" },
  };
}

function economiaAno(facts: MiaFacts): MiaAnswer {
  const a = facts.forecast.annual_savings;
  if (a.economia_state === "no_record") {
    return {
      text: [
        t(
          "A planilha ainda não registra Economia neste ano, então não há Economizado para julgar. O que dá para ver é o colchão — a sobra que ficou em conta: ",
        ),
        m(a.realized_savings_cents),
        t("."),
      ],
      receipt: [
        {
          label: "Colchão do ano",
          cents: a.realized_savings_cents,
          mark: { kind: "estimate", term: GLOSSARY["colchao"]! },
          result: true,
        },
      ],
      note: [
        t(
          "Colchão não é Economia: a régua do método conta o que sai da conta para a reserva. Quando esse lançamento existir, o Economizado aparece aqui.",
        ),
      ],
      provenance: "calculo",
      refusal: "sem_dado",
      cta: { label: "Abrir O ano", target: "ano" },
    };
  }
  const rate = a.economia_ruler_rate_bps;
  const alive = rate >= 2_000;
  return {
    text: [
      t("A régua do ano está em "),
      b(pct(rate)),
      t(
        alive
          ? " — dentro da faixa de 20 a 30 que o método pede. "
          : " — abaixo dos 20 que o método pede. ",
      ),
      t("Ela é anual: um mês fraco não reprova o ano."),
    ],
    receipt: [
      { label: "Economia da régua", cents: a.economia_ruler_cents },
      { label: "Renda realizada", cents: a.realized_income_cents, op: "div" },
      {
        label: "Economizado no ano",
        text: pct(rate),
        op: "eq",
        result: true,
        tone: alive ? "ok" : "warn",
      },
    ],
    note: [
      t(
        a.includes_previdencia
          ? "A previdência entra na régua porque a reserva já cobre 6 meses do custo de vida."
          : "A previdência só entra nessa régua quando a reserva cobre 6 meses do custo de vida — o método faz liquidez primeiro.",
      ),
    ],
    provenance: "calculo",
    cta: { label: "Abrir O ano", target: "ano" },
  };
}

/** Didática do retrato vivo: por que a cobertura ainda não é veredito. */
const LIVE_PORTRAIT = {
  title: "Retrato vivo",
  body: "A cobertura sai do custo de vida dos meses já vividos. Com menos de 6 meses completos ela retrata o agora, não o veredito da régua — ele chega quando o histórico fechar a janela.",
};

function reservaAnswer(facts: MiaFacts): MiaAnswer {
  const s = facts.summary;
  if (s.reserve_state === "no_record") {
    return noData(
      [
        t(
          "Nenhum bolso está marcado como reserva, então não dá para dizer quantos meses ela cobre. Marque em Configurações qual conta guarda a reserva e eu passo a acompanhar.",
        ),
      ],
      { label: "Abrir Configurações", target: "config" },
    );
  }
  if (s.reserve_state === "zero") {
    return {
      text: [
        b("Sem reserva"),
        t(
          " — as contas marcadas como reserva estão zeradas. O método começa por ela: 6 meses do custo de vida num lugar separado, antes de qualquer investimento.",
        ),
      ],
      receipt: [
        { label: "Meta do método", text: "6 meses" },
        { label: "Reserva de hoje", text: monthsLabel(0), result: true, tone: "warn" },
      ],
      provenance: "calculo",
      cta: { label: "Abrir Configurações", target: "config" },
    };
  }
  const months = s.reserve_months;
  const estimate = s.reserve_state === "estimate";
  const tone: Tone = months >= 6 ? "ok" : "warn";
  return {
    text: estimate
      ? [
          t("Com "),
          t(monthsLabel(s.reserve_basis_months)),
          t(" de custo de vida vividos, o retrato de agora aponta "),
          b(monthsLabel(months)),
          t(
            " de reserva — ainda é uma estimativa: a régua pede 6 meses completos para virar veredito.",
          ),
        ]
      : [
          t("A reserva cobre "),
          b(monthsLabel(months)),
          t(" do seu custo de vida. O método pede 6 — e a partir de 12 é a paz."),
        ],
    receipt: [
      { label: "Meta do método", text: "6 meses" },
      {
        label: "Reserva de hoje",
        text: monthsLabel(months),
        result: true,
        tone,
        ...(estimate
          ? { mark: { kind: "estimate" as const, term: LIVE_PORTRAIT } }
          : {}),
      },
    ],
    provenance: "calculo",
    cta: { label: "Abrir Configurações", target: "config" },
  };
}

function buracoAnswer(facts: MiaFacts): MiaAnswer {
  const { forecast } = facts;
  const low = forecast.deepest_deficit;
  if (!low) {
    return noData(
      [
        t(
          "A projeção ainda não tem dias suficientes para desenhar a estrada. Com os lançamentos dos próximos meses, eu acho o ponto mais baixo.",
        ),
      ],
      { label: "Abrir o Horizonte", target: "horizonte" },
    );
  }
  const tone = toneForBalance(low.balance_cents);
  const negative = low.balance_cents < 0;
  return {
    text: negative
      ? [
          b("Tem buraco na estrada"),
          t(": o saldo chega a "),
          m(low.balance_cents),
          t(" em "),
          t(faturaDayLabel(low.date)),
          t(
            ". Não é sentença — dá para atravessar antecipando uma entrada, adiando uma saída que caiba, ou cruzando com a reserva e repondo depois.",
          ),
        ]
      : [
          t("Nenhum buraco até "),
          t(faturaDayLabel(forecast.horizon_end)),
          t(": o ponto mais baixo da estrada é "),
          m(low.balance_cents),
          t(" em "),
          t(faturaDayLabel(low.date)),
          t(" — é ele que sustenta o quanto você pode gastar hoje."),
        ],
    receipt: [
      {
        label: "Ponto mais baixo da estrada",
        cents: low.balance_cents,
        result: true,
        tone,
      },
      { label: "No dia", text: faturaDayLabel(low.date) },
      { label: "Estrada até", text: faturaDayLabel(forecast.horizon_end) },
    ],
    provenance: "calculo",
    cta: { label: "Abrir o Horizonte", target: "horizonte" },
  };
}

function faturasAnswer(facts: MiaFacts): MiaAnswer {
  const { summary } = facts;
  const view = openInvoicesView(summary.upcoming_invoices);
  const next = view.groups[0];
  if (!next) {
    return noData(
      [
        t(
          "Nenhuma fatura em aberto agora. Quando um cartão estiver cadastrado e a planilha trouxer a fatura do ciclo, eu aviso o vencimento aqui.",
        ),
      ],
      { label: "Abrir Cartões", target: "cartoes" },
    );
  }
  const total = next.invoices.reduce((sum, i) => sum + i.amount_cents, 0);
  const receipt: ReceiptLine[] = next.invoices.map((i) => ({
    label: i.card_name,
    cents: i.amount_cents,
  }));
  receipt.push({
    label: "Total do vencimento",
    cents: total,
    op: "eq",
    result: true,
  });
  const others = view.count - next.invoices.length;
  // Fatura em aberto com vencimento no passado é fatura VENCIDA — anunciá-la como "a
  // próxima a vencer" seria a tela mentindo sobre o calendário.
  const overdue = next.dueDate < facts.today;
  const head = overdue
    ? next.invoices.length > 1
      ? `${next.invoices.length} faturas venceram em `
      : "A fatura venceu em "
    : next.invoices.length > 1
      ? `As próximas ${next.invoices.length} faturas vencem em `
      : "A próxima fatura vence em ";
  return {
    text: [
      t(head),
      b(faturaDayLabel(next.dueDate)),
      t(overdue ? " e segue em aberto: " : ": "),
      m(total),
      t("."),
    ],
    receipt,
    ...(others > 0
      ? {
          note: [
            t("Em aberto no total: "),
            m(view.totalCents),
            t(` em ${view.count} faturas.`),
          ],
        }
      : {}),
    provenance: "calculo",
    cta: { label: "Abrir Cartões", target: "cartoes" },
  };
}

function capability(id: IntentId): MiaAnswer {
  if (id === "gasto_por_categoria") {
    return {
      text: [
        t(
          "O método não organiza gasto por categoria — ele separa só o que é fixo (Saídas) do que é variável (Diário), e a tag é um ",
        ),
        b("interruptor"),
        t(
          " de régua: ela decide em quais contas o lançamento entra, não vira relatório de categoria. Em Tags você vê exatamente o que cada régua enxerga.",
        ),
      ],
      provenance: "metodo",
      refusal: "capacidade",
      cta: { label: "Abrir Tags", target: "tags" },
    };
  }
  if (id === "registrar") {
    return {
      text: [
        t(
          "Registrar pela conversa ainda não existe. Use Registrar lançamento: você preenche, revisa o impacto e aprova — nada entra sem a sua aprovação.",
        ),
      ],
      provenance: "metodo",
      refusal: "capacidade",
      cta: { label: "Registrar lançamento", target: "compose" },
    };
  }
  if (id === "editar") {
    return {
      text: [
        t(
          "Editar ou apagar um lançamento pela conversa ainda não existe. Em Lançamentos você faz isso direto na linha, com o efeito visível na hora.",
        ),
      ],
      provenance: "metodo",
      refusal: "capacidade",
      cta: { label: "Abrir Lançamentos", target: "lancamentos" },
    };
  }
  return {
    text: [
      t(
        "Pré-lançar um mês inteiro pela conversa ainda não existe. Em Lançamentos você pré-lança o previsto e o Horizonte mostra o efeito na estrada.",
      ),
    ],
    provenance: "metodo",
    refusal: "capacidade",
    cta: { label: "Abrir Lançamentos", target: "lancamentos" },
  };
}

const CALC_INTENTS = new Set<string>([
  "gastar_hoje",
  "mes",
  "economia_ano",
  "reserva",
  "buraco",
  "faturas",
]);

export function answerFor(
  route: Route,
  facts: MiaFacts | null,
  linked = false,
): MiaAnswer {
  if (route.kind === "unknown") return notLinked(linked);
  if (route.kind === "ambiguous") {
    const options = route.ids.map((id) => INTENT_QUESTION[id]!).slice(0, 2);
    return {
      text: [t("Consigo responder as duas — qual delas você quer?")],
      provenance: "metodo",
      refusal: "ambigua",
      options,
    };
  }
  const { id } = route;
  if (id.startsWith("termo_")) return teach(id.slice("termo_".length));
  if (!CALC_INTENTS.has(id)) return capability(id);
  if (!facts) {
    return noData([
      t(
        "Ainda estou lendo os seus dados — assim que a planilha carregar, eu respondo com os números na mesa.",
      ),
    ]);
  }
  switch (id) {
    case "gastar_hoje":
      return podeGastar(facts);
    case "mes":
      return mesAnswer(facts);
    case "economia_ano":
      return economiaAno(facts);
    case "reserva":
      return reservaAnswer(facts);
    case "buraco":
      return buracoAnswer(facts);
    default:
      return faturasAnswer(facts);
  }
}

// ------------------------------------------------------------------ conversa --

export interface MiaMessage {
  id: number;
  author: "voce" | "mia";
  /** Carimbo local "YYYY-MM-DDTHH:mm". */
  atISO: string;
  /** Texto da pessoa (autor `voce`). */
  question?: string;
  /** Resposta da Mia (autor `mia`). */
  answer?: MiaAnswer;
}

export type TimelineItem =
  | { kind: "daymark"; key: string; label: string }
  | { kind: "message"; key: string; message: MiaMessage };

/** Conversa com os marcos de dia intercalados — derivação pura, sem estado escondido. */
export function buildTimeline(log: MiaMessage[], todayISO: string): TimelineItem[] {
  const items: TimelineItem[] = [];
  let lastDay = "";
  for (const message of log) {
    const day = message.atISO.slice(0, 10);
    if (day !== lastDay) {
      items.push({
        kind: "daymark",
        key: `day-${day}`,
        label: dayMarkLabel(day, todayISO),
      });
      lastDay = day;
    }
    items.push({ kind: "message", key: `msg-${message.id}`, message });
  }
  return items;
}

// -------------------------------------------------------- os números por trás --

export interface ContextFact {
  key: string;
  label: string;
  cents?: number;
  text?: string;
  tone?: Tone;
  /** A pergunta que esta linha faz quando tocada — o painel é atalho de conversa. */
  question: string;
}

/**
 * Os fatos do painel são o índice do repertório com os valores vivos: cada linha faz a
 * pergunta que a explica. É isso que permite ao painel ser ergonomia de desktop sem
 * esconder informação — nada aqui é alcançável só por ele.
 */
export function contextFacts(facts: MiaFacts | null): ContextFact[] {
  if (!facts) return [];
  const { summary, forecast } = facts;
  const annual = forecast.annual_savings;
  const invoices = openInvoicesView(summary.upcoming_invoices);
  const low = forecast.deepest_deficit;

  const out: ContextFact[] = [
    {
      key: "pode_gastar",
      label: "Pode gastar hoje",
      cents: forecast.safe_to_spend_today_cents,
      tone: forecast.safe_to_spend_today_cents > 0 ? "ok" : "warn",
      question: INTENT_QUESTION["gastar_hoje"]!,
    },
    {
      key: "fim_mes",
      label: "Fim do mês previsto",
      cents: summary.balance,
      tone: toneForBalance(summary.balance),
      question: INTENT_QUESTION["mes"]!,
    },
    {
      key: "economia",
      label: "Economizado no ano",
      ...(annual.economia_state === "no_record"
        ? { text: "Sem registro" }
        : {
            text: pct(annual.economia_ruler_rate_bps),
            tone: annual.economia_ruler_rate_bps >= 2_000 ? "ok" : "warn",
          }),
      question: INTENT_QUESTION["economia_ano"]!,
    },
    {
      key: "reserva",
      label: "Reserva",
      ...(summary.reserve_state === "no_record"
        ? { text: "Sem registro" }
        : {
            text: monthsLabel(summary.reserve_months),
            tone: summary.reserve_months >= 6 ? "ok" : "warn",
          }),
      question: INTENT_QUESTION["reserva"]!,
    },
  ];

  if (low) {
    out.push({
      key: "estrada",
      label: "Ponto mais baixo da estrada",
      cents: low.balance_cents,
      tone: toneForBalance(low.balance_cents),
      question: INTENT_QUESTION["buraco"]!,
    });
  }
  if (invoices.count > 0) {
    out.push({
      key: "faturas",
      label: "Faturas em aberto",
      cents: invoices.totalCents,
      question: INTENT_QUESTION["faturas"]!,
    });
  }
  return out;
}
