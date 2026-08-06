import { describe, it, expect } from "vitest";
import { GLOSSARY } from "../design-system/glossary";
import {
  answerFor,
  buildTimeline,
  contextFacts,
  dayMarkLabel,
  localStamp,
  plainText as rawPlainText,
  routeQuestion,
  SUGGESTIONS,
  timeLabel,
  type MiaFacts,
  type Span,
} from "./miaView";
import type { DashboardSummary, Forecast, MonthMetric } from "../lib/api";

// ---- fixtures --------------------------------------------------------------

const MONTH: MonthMetric = {
  year: 2026,
  month: 7,
  income_cents: 1_200_000,
  income_performance_cents: 1_200_000,
  performance_cents: 150_000,
  cost_of_living_cents: 900_000,
  fixed_out_cents: 400_000,
  daily_out_cents: 200_000,
  daily_avg_out_cents: 200_000,
  daily_projected_cents: 100_000,
  cartao_cents: 300_000,
  real_daily_avg_cents: 20_000,
  economia_cents: 50_000,
  patrimonio_cents: 0,
  savings_rate_bps: 417,
};

const SUMMARY: DashboardSummary = {
  balance: 1_299_520,
  daily_budget: 4_300,
  daily_ceiling_source: "chosen",
  daily_ceiling_estimate: null,
  ceiling_proposal_pending: false,
  daily_spend_today: 3_800,
  card_spend_today_cents: 0,
  reserve_months: 4.5,
  reserve_state: "verdict",
  reserve_basis_months: 6,
  reserve_target_cents: 0,
  reserve_surplus_cents: null,
  reserve_trend: "flat",
  spending_mode: "debit",
  spending_mode_detected: true,
  card_gate: "unknown",
  card_gate_economy: "unknown",
  card_gate_economy_bps: null,
  card_gate_reserve: "unknown",
  cartao_month_cents: 300_000,
  next_fatura_date: null,
  next_fatura_amount_cents: 0,
  upcoming_invoices: [],
  transaction_count: 120,
  last_real_tx_date: "2026-07-14",
};

const FORECAST: Forecast = {
  today: "2026-07-15",
  horizon_end: "2026-12-31",
  annual_savings: {
    realized_income_cents: 8_400_000,
    realized_savings_cents: 300_000,
    realized_rate_bps: 357,
    registered_economia_cents: 1_800_000,
    patrimonio_cents: 0,
    economia_ruler_cents: 1_800_000,
    economia_ruler_rate_bps: 2_142,
    economia_state: "verdict",
    projected_income_cents: 0,
    projected_savings_cents: 0,
    projected_rate_bps: 0,
  },
  coverage: [],
  baseline_outflow_cents: 1_100_000,
  trusted_through_month: "2026-09",
  total_missing_cents: 0,
  safe_to_spend_today_cents: 53_720,
  cash_headroom_cents: 80_000,
  savings_headroom_cents: 53_720,
  binding_guardrail: "savings",
  deepest_deficit: { date: "2026-08-12", balance_cents: 156_965 },
  daily: [],
  month_end: [],
  months: [MONTH],
};

const FACTS: MiaFacts = {
  summary: SUMMARY,
  forecast: FORECAST,
  today: "2026-07-15",
};

/** `formatBRL` cola o valor ao "R$" com NBSP; as asserções escrevem espaço normal. */
function plainText(spans: Span[]): string {
  return rawPlainText(spans).replace(/\u00a0/g, " ");
}

function ask(question: string, facts: MiaFacts | null = FACTS) {
  return answerFor(routeQuestion(question), facts);
}

/** Os números marcados como estimativa na frase, com o termo que abre a didática. */
function markedSpans(answer: { text: Span[] }) {
  return answer.text
    .filter((span) => span.t !== "text" && span.mark)
    .map((span) => ({
      value: span.t === "money" ? span.cents : span.t === "strong" ? span.s : "",
      term: span.t === "text" ? undefined : span.mark?.term.title,
    }));
}

/** Texto corrido da resposta (parágrafo + nota) para asserções de copy. */
function said(question: string, facts: MiaFacts | null = FACTS) {
  const a = ask(question, facts);
  return plainText(a.text) + (a.note ? " " + plainText(a.note) : "");
}

// ---- roteamento ------------------------------------------------------------

describe("routeQuestion", () => {
  it("casa as perguntas do repertório, com e sem acento", () => {
    expect(routeQuestion("Quanto posso gastar hoje?")).toEqual({
      kind: "intent",
      id: "gastar_hoje",
    });
    expect(routeQuestion("como o mes esta indo")).toEqual({
      kind: "intent",
      id: "mes",
    });
    expect(routeQuestion("Como está a economia do ano?")).toEqual({
      kind: "intent",
      id: "economia_ano",
    });
    expect(routeQuestion("Como está a reserva?")).toEqual({
      kind: "intent",
      id: "reserva",
    });
    expect(routeQuestion("Tem buraco na estrada?")).toEqual({
      kind: "intent",
      id: "buraco",
    });
    expect(routeQuestion("Quando vence a próxima fatura?")).toEqual({
      kind: "intent",
      id: "faturas",
    });
  });

  it("marcador de definição manda para a didática, não para o cálculo", () => {
    expect(routeQuestion("O que é buraco do futuro?")).toEqual({
      kind: "intent",
      id: "termo_buraco_do_futuro",
    });
    // Sem o marcador, a mesma palavra é a pergunta do horizonte.
    expect(routeQuestion("tenho buraco?")).toEqual({ kind: "intent", id: "buraco" });
  });

  it("texto que não casa com nada é desconhecido", () => {
    expect(routeQuestion("me conta uma piada")).toEqual({ kind: "unknown" });
    expect(routeQuestion("   ")).toEqual({ kind: "unknown" });
  });

  it("empate entre duas intenções vira ambiguidade, nunca suposição", () => {
    const route = routeQuestion("e a fatura deste mês?");
    expect(route.kind).toBe("ambiguous");
    if (route.kind === "ambiguous") {
      expect(route.ids).toEqual(expect.arrayContaining(["faturas", "mes"]));
    }
  });

  it("reconhece capacidades que a tela não tem", () => {
    expect(routeQuestion("registra R$ 4,50 do café")).toEqual({
      kind: "intent",
      id: "registrar",
    });
    expect(routeQuestion("apaga o lançamento de ontem")).toEqual({
      kind: "intent",
      id: "editar",
    });
    expect(routeQuestion("onde gastei mais?")).toEqual({
      kind: "intent",
      id: "gasto_por_categoria",
    });
  });
});

// ---- recibos ---------------------------------------------------------------

describe("pode gastar hoje", () => {
  it("guardrail negativo vira 'segure hoje' — a exibição clampa, o veredito não mente", () => {
    const a = ask("Quanto posso gastar hoje?", {
      ...FACTS,
      forecast: { ...FORECAST, safe_to_spend_today_cents: -5_000 },
    });
    expect(plainText(a.text)).toContain("segurar");
    const result = a.receipt?.find((l) => l.result);
    expect(result?.cents).toBe(0);
    expect(result?.tone).toBe("warn");
  });

  it("promove o guardrail que morde e imprime o mín das duas réguas", () => {
    const a = ask("Quanto posso gastar hoje?");
    expect(a.provenance).toBe("calculo");
    expect(plainText(a.text)).toContain("R$ 537,20");
    // Guardrail de poupança: a frase é a mesma da tela Hoje.
    expect(plainText(a.text)).toContain("Sem tocar na economia planejada do ano.");
    expect(a.receipt).toEqual([
      { label: "Limite do caixa", cents: 80_000 },
      { label: "Limite da economia", cents: 53_720, op: "min" },
      {
        label: "Pode gastar hoje",
        cents: 53_720,
        op: "eq",
        result: true,
        tone: "ok",
      },
    ]);
  });

  it("sem régua de poupança ativa, a conta é só o caixa", () => {
    const a = ask("Quanto posso gastar hoje?", {
      ...FACTS,
      forecast: {
        ...FORECAST,
        savings_headroom_cents: null,
        binding_guardrail: "cash",
        safe_to_spend_today_cents: 80_000,
      },
    });
    expect(plainText(a.text)).toContain("Sem deixar nenhum dia no vermelho.");
    expect(a.receipt).toHaveLength(2);
    expect(a.receipt?.some((l) => l.label === "Limite da economia")).toBe(false);
  });

  it("o teto é o SEGUNDO limite do dia — nunca entra no mín do motor", () => {
    const a = ask("Quanto posso gastar hoje?");
    // O teto não é linha do recibo (o motor não o computa no guardrail)…
    expect(a.receipt?.some((l) => l.label.includes("Teto"))).toBe(false);
    // …e aparece como nota, com o já gasto e o que resta por ele.
    const note = plainText(a.note ?? []);
    expect(note).toContain("segundo limite do dia");
    expect(note).toContain("R$ 43,00");
    expect(note).toContain("R$ 38,00");
    expect(note).toContain("R$ 5,00");
  });

  it("sem teto estipulado, convida a estipular em vez de fabricar um número", () => {
    const a = ask("Quanto posso gastar hoje?", {
      ...FACTS,
      summary: { ...SUMMARY, daily_budget: 0, daily_ceiling_source: "none" },
    });
    expect(plainText(a.note ?? [])).toContain("ainda não estipulou");
    expect(plainText(a.note ?? [])).not.toContain("R$ 0,00");
    expect(a.cta).toEqual({ label: "Estipular o teto", target: "teto" });
  });
});

describe("como o mês está indo", () => {
  it("imprime uma conta que fecha com a performance do motor", () => {
    const a = ask("Como o mês está indo?");
    const receipt = a.receipt ?? [];
    expect(receipt[0]).toEqual({ label: "Entradas do mês", cents: 1_200_000 });
    expect(receipt[1]).toEqual({
      label: "Saídas e economia do mês",
      cents: 1_050_000,
      op: "minus",
    });
    expect(receipt[2]).toEqual({
      label: "Performance do mês",
      cents: 150_000,
      op: "eq",
      result: true,
      tone: "ok",
    });
    // A soma impressa fecha: entradas − saídas = performance.
    expect(receipt[0]!.cents! - receipt[1]!.cents!).toBe(receipt[2]!.cents);
  });

  it("a nota traz o custo de vida e o fim do mês previsto", () => {
    const note = plainText(ask("Como o mês está indo?").note ?? []);
    expect(note).toContain("R$ 9.000,00");
    expect(note).toContain("R$ 12.995,20");
  });

  it("performance negativa não vira julgamento", () => {
    const a = ask("Como o mês está indo?", {
      ...FACTS,
      forecast: {
        ...FORECAST,
        months: [{ ...MONTH, performance_cents: -80_000 }],
      },
    });
    expect(a.receipt?.[2]?.tone).toBe("warn");
    expect(said("Como o mês está indo?")).not.toMatch(/ruim|errado|péssim/i);
  });
});

describe("economia do ano", () => {
  it("imprime economia ÷ renda = Economizado, com a faixa anual", () => {
    const a = ask("Como está a economia do ano?");
    expect(a.receipt).toEqual([
      { label: "Economia da régua", cents: 1_800_000 },
      { label: "Entradas do ano até aqui", cents: 8_400_000, op: "div" },
      {
        label: "Economizado no ano",
        text: "21%",
        op: "eq",
        result: true,
        tone: "ok",
      },
    ]);
    expect(plainText(a.text)).toContain("21%");
    expect(plainText(a.text)).toContain("anual");
  });

  it("trunca o percentual — nunca arredonda para cima do que o motor mediu", () => {
    const a = ask("Como está a economia do ano?", {
      ...FACTS,
      forecast: {
        ...FORECAST,
        annual_savings: { ...FORECAST.annual_savings, economia_ruler_rate_bps: 1_999 },
      },
    });
    expect(a.receipt?.[2]?.text).toBe("19%");
    expect(a.receipt?.[2]?.tone).toBe("warn");
  });

  it("sem Economia registrada, mostra o colchão como estimativa marcada", () => {
    const a = ask("Como está a economia do ano?", {
      ...FACTS,
      forecast: {
        ...FORECAST,
        annual_savings: {
          ...FORECAST.annual_savings,
          economia_state: "no_record",
          registered_economia_cents: 0,
          economia_ruler_cents: 0,
          economia_ruler_rate_bps: 0,
        },
      },
    });
    expect(a.refusal).toBe("sem_dado");
    expect(plainText(a.text)).toContain("R$ 3.000,00");
    expect(markedSpans(a)).toEqual([
      { value: 300000, term: GLOSSARY["colchao"]!.title },
    ]);
    // Zero fabricado é proibido: não existe "0%" como veredito aqui.
    expect(plainText(a.text)).not.toContain("0%");
  });
});

describe("reserva", () => {
  it("veredito compara com a meta de 6 meses", () => {
    const a = ask("Como está a reserva?");
    expect(a.receipt).toEqual([
      { label: "Meta do método", text: "6 meses" },
      { label: "Reserva de hoje", text: "4,5 meses", result: true, tone: "warn" },
    ]);
    expect(plainText(a.text)).toContain("4,5 meses");
  });

  it("retrato vivo marca o número da frase, não a linha do recibo", () => {
    const a = ask("Como está a reserva?", {
      ...FACTS,
      summary: { ...SUMMARY, reserve_state: "estimate", reserve_basis_months: 2 },
    });
    // O selo mora colado ao número que ele qualifica — assim ele sobrevive ao recolhimento
    // da conta sem deixar dúvida sobre a qual valor se refere.
    expect(markedSpans(a)).toEqual([{ value: "4,5 meses", term: "Retrato vivo" }]);
    expect(a.receipt?.some((l) => l.mark)).toBe(false);
    expect(plainText(a.text)).toContain("estimativa");
  });

  it("veredito fechado não marca nada", () => {
    expect(markedSpans(ask("Como está a reserva?"))).toEqual([]);
  });

  it("sem bolso de reserva mapeado, recusa com o caminho de registro", () => {
    const a = ask("Como está a reserva?", {
      ...FACTS,
      summary: { ...SUMMARY, reserve_state: "no_record", reserve_months: 0 },
    });
    expect(a.refusal).toBe("sem_dado");
    expect(a.receipt).toBeUndefined();
    expect(plainText(a.text)).not.toContain("0 meses");
    expect(a.cta?.target).toBe("config");
  });

  it("reserva zerada é palavra própria, não travessão", () => {
    const a = ask("Como está a reserva?", {
      ...FACTS,
      summary: { ...SUMMARY, reserve_state: "zero", reserve_months: 0 },
    });
    expect(plainText(a.text)).toContain("Sem reserva");
    expect(a.refusal).toBeUndefined();
  });

  it("resposta que já entrega o veredito não empurra Configurações", () => {
    expect(ask("Como está a reserva?").cta).toBeUndefined();
    expect(
      ask("Como está a reserva?", {
        ...FACTS,
        summary: { ...SUMMARY, reserve_state: "zero", reserve_months: 0 },
      }).cta,
    ).toBeUndefined();
  });
});

describe("buraco na estrada", () => {
  it("sem buraco, o menor ponto é a prova do 'pode gastar'", () => {
    const a = ask("Tem buraco na estrada?");
    expect(plainText(a.text)).toContain("R$ 1.569,65");
    expect(plainText(a.text)).toContain("12 de agosto");
    expect(a.receipt?.[0]?.tone).toBe("ok");
    expect(a.cta).toEqual({ label: "Abrir o Horizonte", target: "horizonte" });
  });

  it("com buraco, nomeia a travessia em vez de sentenciar", () => {
    const a = ask("Tem buraco na estrada?", {
      ...FACTS,
      forecast: {
        ...FORECAST,
        deepest_deficit: { date: "2026-09-03", balance_cents: -120_000 },
      },
    });
    const text = plainText(a.text);
    expect(text).toContain("Tem buraco");
    expect(text).toContain("atravessar");
    expect(a.receipt?.[0]?.tone).toBe("bad");
  });

  it("sem projeção, não inventa um menor ponto", () => {
    const a = ask("Tem buraco na estrada?", {
      ...FACTS,
      forecast: { ...FORECAST, deepest_deficit: null },
    });
    expect(a.refusal).toBe("sem_dado");
    expect(a.receipt).toBeUndefined();
  });
});

describe("faturas", () => {
  const invoices = [
    {
      account_id: "a",
      card_name: "Cartão azul",
      due_date: "2026-07-20",
      amount_cents: 400_066,
      status: "aberta" as const,
      owner_name: "Ana",
      has_refund_expectation: false,
      refund_expected_cents: 0,
    },
    {
      account_id: "b",
      card_name: "Cartão verde",
      due_date: "2026-07-20",
      amount_cents: 22_298,
      status: "fechada" as const,
      owner_name: "Ana",
      has_refund_expectation: false,
      refund_expected_cents: 0,
    },
  ];

  it("agrupa o vencimento e soma o total", () => {
    const a = ask("Quando vence a próxima fatura?", {
      ...FACTS,
      summary: { ...SUMMARY, upcoming_invoices: invoices },
    });
    expect(a.receipt).toEqual([
      { label: "Cartão azul", cents: 400_066 },
      { label: "Cartão verde", cents: 22_298 },
      {
        label: "Total do vencimento",
        cents: 422_364,
        op: "eq",
        result: true,
      },
    ]);
    expect(plainText(a.text)).toContain("20 de julho");
  });

  it("mantém recibo e total em aberto no regime bruto", () => {
    const a = ask("Quando vence a próxima fatura?", {
      ...FACTS,
      summary: {
        ...SUMMARY,
        upcoming_invoices: [
          {
            ...invoices[0]!,
            amount_cents: 400_000,
            refund_expected_cents: 100_000,
          },
          {
            ...invoices[1]!,
            due_date: "2026-07-25",
            amount_cents: 200_000,
            refund_expected_cents: 50_000,
          },
        ],
      },
    });

    expect(a.receipt).toEqual([
      { label: "Cartão azul", cents: 400_000 },
      { label: "Total do vencimento", cents: 400_000, op: "eq", result: true },
    ]);
    expect(plainText(a.note ?? [])).toBe(
      "Em aberto no total: R$ 6.000,00 em 2 faturas.",
    );
  });

  it("fatura vencida em aberto não vira 'a próxima a vencer'", () => {
    const a = ask("Quando vence a próxima fatura?", {
      ...FACTS,
      summary: {
        ...SUMMARY,
        upcoming_invoices: invoices.map((i) => ({ ...i, due_date: "2026-07-10" })),
      },
    });
    const text = plainText(a.text);
    expect(text).toContain("venceram em");
    expect(text).toContain("segue em aberto");
    expect(text).not.toContain("próxim");
  });

  it("sem fatura em aberto, não fabrica R$ 0,00", () => {
    const a = ask("Quando vence a próxima fatura?");
    expect(a.refusal).toBe("sem_dado");
    expect(plainText(a.text)).not.toContain("R$ 0,00");
    expect(a.cta?.target).toBe("cartoes");
  });
});

// ---- recusas ---------------------------------------------------------------

describe("recusa honesta", () => {
  it("fora do repertório: diz que a conversa aberta ainda não está ligada e oferece o que sabe", () => {
    const a = ask("me conta uma piada");
    expect(a.refusal).toBe("nao_ligada");
    expect(a.provenance).toBe("metodo");
    expect(a.options?.length).toBeGreaterThan(2);
    expect(a.receipt).toBeUndefined();
  });

  it("oferece ligar a conversa só enquanto esse gesto ainda existe", () => {
    const route = routeQuestion("me conta uma piada");

    expect(answerFor(route, FACTS).cta).toEqual({
      label: "Autorizar a conversa",
      target: "config",
    });
    expect(answerFor(route, FACTS, true).cta).toBeUndefined();
  });

  it("capacidade não suportada nomeia o caminho certo", () => {
    const registrar = ask("registra R$ 4,50 do café");
    expect(registrar.refusal).toBe("capacidade");
    expect(registrar.cta).toEqual({
      label: "Registrar lançamento",
      target: "compose",
    });

    const editar = ask("apaga o lançamento de ontem");
    expect(editar.cta?.target).toBe("lancamentos");
  });

  it("gasto por categoria ensina o método em vez de recusar seco", () => {
    const a = ask("onde gastei mais?");
    expect(a.refusal).toBe("capacidade");
    const text = plainText(a.text);
    expect(text).toContain("categoria");
    expect(text).toContain("interruptor");
    expect(a.cta?.target).toBe("tags");
  });

  it("ambiguidade pergunta em vez de supor", () => {
    const a = ask("e a fatura deste mês?");
    expect(a.refusal).toBe("ambigua");
    expect(a.options).toHaveLength(2);
    expect(a.receipt).toBeUndefined();
  });

  it("sem dados carregados, nenhuma pergunta inventa número", () => {
    const a = ask("Quanto posso gastar hoje?", null);
    expect(a.refusal).toBe("sem_dado");
    expect(a.receipt).toBeUndefined();
  });
});

describe("didática", () => {
  it("explica o termo do glossário e declara a proveniência", () => {
    const a = ask("O que é buraco do futuro?");
    expect(a.provenance).toBe("metodo");
    expect(a.receipt).toBeUndefined();
    expect(plainText(a.text)).toContain("menor ponto");
  });

  it("didática responde igual sem dados carregados", () => {
    const a = ask("o que é termômetro?", null);
    expect(a.refusal).toBeUndefined();
    expect(plainText(a.text).length).toBeGreaterThan(40);
  });
});

// ---- marcos de dia e hora --------------------------------------------------

describe("marcos de dia e hora", () => {
  it("nomeia hoje, ontem e a data por extenso", () => {
    expect(dayMarkLabel("2026-07-15", "2026-07-15")).toBe("Hoje");
    expect(dayMarkLabel("2026-07-14", "2026-07-15")).toBe("Ontem");
    expect(dayMarkLabel("2026-07-02", "2026-07-15")).toBe("2 de julho");
  });

  it("hora no formato do app", () => {
    expect(timeLabel("2026-07-15T22:41:00")).toBe("22h41");
    expect(timeLabel("2026-07-15T09:05:00")).toBe("09h05");
  });

  it("o carimbo da mensagem é o relógio local, não UTC", () => {
    // 22h41 de 15/07 em São Paulo seria 01h41 de 16/07 em UTC — dia e hora errados.
    expect(localStamp(new Date(2026, 6, 15, 22, 41))).toBe("2026-07-15T22:41");
    expect(timeLabel(localStamp(new Date(2026, 6, 15, 9, 5)))).toBe("09h05");
  });
});

describe("linha do tempo", () => {
  const log = [
    { id: 1, author: "voce" as const, atISO: "2026-07-14T22:41", question: "oi" },
    { id: 2, author: "mia" as const, atISO: "2026-07-14T22:41" },
    { id: 3, author: "voce" as const, atISO: "2026-07-15T09:05", question: "e hoje?" },
  ];

  it("abre um marco por dia, na ordem da conversa", () => {
    const items = buildTimeline(log, "2026-07-15");
    expect(items.map((i) => i.kind)).toEqual([
      "daymark",
      "message",
      "message",
      "daymark",
      "message",
    ]);
    expect(items[0]).toMatchObject({ label: "Ontem" });
    expect(items[3]).toMatchObject({ label: "Hoje" });
  });

  it("conversa vazia não tem marco", () => {
    expect(buildTimeline([], "2026-07-15")).toEqual([]);
  });
});

// ---- painel de contexto ----------------------------------------------------

describe("os números por trás", () => {
  it("cada fato do painel é uma pergunta do repertório", () => {
    const facts = contextFacts(FACTS);
    expect(facts.length).toBeGreaterThanOrEqual(4);
    for (const fact of facts) {
      const route = routeQuestion(fact.question);
      expect(route.kind).toBe("intent");
    }
  });

  it("fato fora da régua carrega o tom de atenção, nunca vermelho de castigo", () => {
    const facts = contextFacts({
      ...FACTS,
      forecast: {
        ...FORECAST,
        annual_savings: { ...FORECAST.annual_savings, economia_ruler_rate_bps: 500 },
      },
    });
    const economia = facts.find((f) => f.key === "economia");
    expect(economia?.tone).toBe("warn");
  });

  it("fatura com reembolso mantém no painel o bruto publicado pela resposta", () => {
    const facts: MiaFacts = {
      ...FACTS,
      summary: {
        ...SUMMARY,
        upcoming_invoices: [
          {
            account_id: "visa",
            card_name: "Visa",
            due_date: "2026-07-20",
            amount_cents: 600_000,
            status: "aberta",
            owner_name: "Eu",
            has_refund_expectation: true,
            refund_expected_cents: 150_000,
          },
        ],
      },
    };

    const footer = contextFacts(facts).find((fact) => fact.key === "faturas");
    const answer = ask("Quando vence a próxima fatura?", facts);
    expect(footer?.cents).toBe(600_000);
    expect(plainText(answer.text)).toContain("R$ 6.000,00");
  });

  it("sem dados, o painel não existe", () => {
    expect(contextFacts(null)).toEqual([]);
  });
});

// ---- selo epistêmico da resposta -------------------------------------------

describe("selo epistêmico", () => {
  it("nenhuma resposta do repertório marca a linha de resultado", () => {
    // A linha do recibo guarda o selo só para operando estimado entre operandos vividos; o
    // número que a resposta afirma leva o selo na frase.
    for (const question of SUGGESTIONS) {
      const marked = ask(question).receipt?.filter((l) => l.mark && l.result) ?? [];
      expect(marked).toEqual([]);
    }
  });
});
