import { describe, it, expect } from "vitest";
import {
  GUIDED_QUESTIONS,
  buildTetoView,
  categoriesFromDraft,
  ceilingPerDayCents,
  ceremonyAgeLabel,
  ceremonyAgeMonths,
  ceremonyMonthLabel,
  divisorFromText,
  draftTotalCents,
  guardTriggered,
  recalibrationCaption,
  recalibrationDueMonth,
} from "./tetoView";
import type { CeilingProposal, DailyBudget, DashboardSummary } from "../lib/api";
import { SUMMARY } from "../test/commands";

// A cerimônia real da planilha de referência: cinco itens mensais somando R$ 1.250,00,
// divididos por 31 dias, arredondados PARA CIMA — R$ 40,33.
const NOTA_REAL = [
  "Mensal  R$ 300,00  Transporte",
  "Mensal  R$ 200,00  Farmácia",
  "Mensal  R$ 300,00  Alimentação",
  "Mensal  R$ 200,00  Lazer",
  "Mensal  R$ 250,00  Compras",
  "Total = R$ 1250,00",
  "R$ 1250,00 / 31 Dias = R$ 40,33",
].join("\n");

const ITENS_REAIS = [
  { id: "c1", name: "Transporte", amount_cents: 30_000, position: 0 },
  { id: "c2", name: "Farmácia", amount_cents: 20_000, position: 1 },
  { id: "c3", name: "Alimentação", amount_cents: 30_000, position: 2 },
  { id: "c4", name: "Lazer", amount_cents: 20_000, position: 3 },
  { id: "c5", name: "Compras", amount_cents: 25_000, position: 4 },
];

function budget(over: Partial<DailyBudget> = {}): DailyBudget {
  return {
    per_day_cents: 4_033,
    divisor_days: 31,
    ceremony_month: "2025-09",
    source_note: NOTA_REAL,
    categories: ITENS_REAIS,
    ...over,
  };
}

function summary(over: Partial<DashboardSummary> = {}): DashboardSummary {
  return { ...SUMMARY, ...over };
}

const PROPOSTA: CeilingProposal = {
  id: "cp-1",
  per_day_cents: 4_033,
  divisor_days: 31,
  source_month: "2025-09",
  raw_note: NOTA_REAL,
  items: [{ name: "Transporte", amount_cents: 30_000 }],
};

describe("a aritmética da cerimônia", () => {
  it("divide o total pelos dias e arredonda para cima — a nota real fecha no centavo", () => {
    expect(ceilingPerDayCents(125_000, 31)).toBe(4_033); // 4032,25… → 4033
  });

  it("arredonda para cima também quando a divisão é exata por um centavo a menos", () => {
    expect(ceilingPerDayCents(90_000, 30)).toBe(3_000);
    expect(ceilingPerDayCents(90_001, 30)).toBe(3_001);
  });

  it("sem divisor válido não inventa teto", () => {
    expect(ceilingPerDayCents(125_000, 0)).toBe(0);
    expect(ceilingPerDayCents(125_000, -3)).toBe(0);
  });

  it("soma os itens do rascunho ignorando linhas em branco", () => {
    expect(
      draftTotalCents([
        { key: "a", name: "Transporte", amountText: "300,00" },
        { key: "b", name: "", amountText: "" },
        { key: "c", name: "Lazer", amountText: "250" },
      ]),
    ).toBe(55_000);
  });

  it("só as linhas preenchidas viram categoria, com posição em sequência", () => {
    expect(
      categoriesFromDraft([
        { key: "a", name: " Transporte ", amountText: "300,00" },
        { key: "b", name: "", amountText: "120,00" }, // sem nome
        { key: "c", name: "Lazer", amountText: "" }, // sem valor
        { key: "d", name: "Compras", amountText: "250,00" },
      ]),
    ).toEqual([
      { name: "Transporte", amount_cents: 30_000, position: 0 },
      { name: "Compras", amount_cents: 25_000, position: 1 },
    ]);
  });

  it("lê o divisor só quando é inteiro positivo", () => {
    expect(divisorFromText("31")).toBe(31);
    expect(divisorFromText(" 30 ")).toBe(30);
    expect(divisorFromText("")).toBeNull();
    expect(divisorFromText("0")).toBeNull();
    expect(divisorFromText("-4")).toBeNull();
    expect(divisorFromText("abc")).toBeNull();
  });
});

describe("a idade da cerimônia", () => {
  it("conta os meses completos entre a cerimônia e hoje", () => {
    expect(ceremonyAgeMonths("2025-09", "2026-07-23")).toBe(10);
    expect(ceremonyAgeMonths("2026-07", "2026-07-23")).toBe(0);
    expect(ceremonyAgeMonths(null, "2026-07-23")).toBeNull();
  });

  it("nomeia o mês da cerimônia com inicial maiúscula na abertura da linha", () => {
    expect(ceremonyMonthLabel("2025-09")).toBe("Estipulado em setembro de 2025");
    expect(ceremonyMonthLabel(null)).toBe("Estipulado por você");
  });

  it("fala a idade em português, e recua para anos quando passa de doze meses", () => {
    expect(ceremonyAgeLabel(0)).toBe("A cerimônia é deste mês");
    expect(ceremonyAgeLabel(1)).toBe("A cerimônia fez um mês");
    expect(ceremonyAgeLabel(10)).toBe("A cerimônia fez dez meses");
    expect(ceremonyAgeLabel(14)).toBe("A cerimônia fez mais de um ano");
    expect(ceremonyAgeLabel(30)).toBe("A cerimônia fez mais de dois anos");
  });

  it("o prazo da cadência fecha três meses depois da cerimônia, virando o ano quando preciso", () => {
    expect(recalibrationDueMonth("2025-09")).toBe("2025-12");
    expect(recalibrationDueMonth("2025-11")).toBe("2026-02");
    expect(recalibrationDueMonth(null)).toBeNull();
  });

  it("a legenda do cartão de idade cruza o prazo com o veredito da cadência", () => {
    expect(recalibrationCaption("2025-09", true)).toBe(
      "Prazo vencido em dezembro de 2025.",
    );
    expect(recalibrationCaption("2025-09", false)).toBe("Prazo até dezembro de 2025.");
    expect(recalibrationCaption(null, false)).toBe(
      "Sem cerimônia registrada para calcular o prazo.",
    );
  });
});

describe("a guarda do vença o dia", () => {
  it("dispara só quando o teto novo é menor que o vigente", () => {
    expect(guardTriggered(4_033, 3_226)).toBe(true);
    expect(guardTriggered(4_033, 4_355)).toBe(false);
    expect(guardTriggered(4_033, 4_033)).toBe(false);
    expect(guardTriggered(0, 3_226)).toBe(false); // sem teto vigente não há o que baixar
  });
});

describe("o estado da manchete", () => {
  it("carrega enquanto o orçamento não chegou", () => {
    const v = buildTetoView({
      budget: undefined,
      proposal: undefined,
      summary: undefined,
      today: "2026-07-23",
    });
    expect(v.kind).toBe("loading");
    expect(v.perDayCents).toBe(0);
  });

  it("veredito escolhido: o teto, o modo detectado e a prova completa", () => {
    const v = buildTetoView({
      budget: budget(),
      proposal: null,
      summary: summary({ spending_mode: "card" }),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("chosen");
    expect(v.perDayCents).toBe(4_033);
    expect(v.mode).toBe("card");
    expect(v.ageMonths).toBe(10);
    expect(v.recalibrationDue).toBe(true);
    expect(v.proof).not.toBeNull();
    expect(v.proof?.totalCents).toBe(125_000);
    expect(v.proof?.divisorDays).toBe(31);
    expect(v.proof?.perDayCents).toBe(4_033);
    expect(v.proof?.sourceNote).toBe(NOTA_REAL);
  });

  it("teto direto (sem itens nem divisor) é veredito sem prova a exibir", () => {
    const v = buildTetoView({
      budget: budget({
        categories: [],
        divisor_days: null,
        source_note: null,
        ceremony_month: "2026-07",
      }),
      proposal: null,
      summary: summary(),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("chosen");
    expect(v.proof).toBeNull();
    expect(v.recalibrationDue).toBe(false);
  });

  it("a prova recalcula o teto a partir dos itens — não repete o número gravado", () => {
    // Itens editados fora do app (ou importados) que não fecham com o total gravado: a prova
    // mostra a conta REAL dos itens, e a tela pode confrontar os dois números.
    const v = buildTetoView({
      budget: budget({ per_day_cents: 9_999 }),
      proposal: null,
      summary: summary(),
      today: "2026-07-23",
    });
    expect(v.perDayCents).toBe(9_999);
    expect(v.proof?.perDayCents).toBe(4_033);
    expect(v.proofMatchesVerdict).toBe(false);
  });

  it("a proposta pendente manda na manchete e carrega o teto vigente para o confronto", () => {
    const v = buildTetoView({
      budget: budget(),
      proposal: PROPOSTA,
      summary: summary(),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("proposal");
    expect(v.perDayCents).toBe(4_033);
    expect(v.proposal?.id).toBe("cp-1");
    expect(v.currentPerDayCents).toBe(4_033);
  });

  it("estimativa: a média do histórico nunca se veste de veredito", () => {
    const v = buildTetoView({
      budget: budget({ per_day_cents: 0, categories: [], divisor_days: null }),
      proposal: null,
      summary: summary({ daily_ceiling_source: "estimate", daily_budget: 4_600 }),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("estimate");
    expect(v.perDayCents).toBe(4_600);
    expect(v.proof).toBeNull();
  });

  // A tela IMPRIME a conta da estimativa: sem os operandos do motor ela não pode inventá-los,
  // e com eles a divisão precisa fechar no número da manchete.
  it("estimativa carrega os operandos que o motor usou, e eles fecham", () => {
    const v = buildTetoView({
      budget: budget({ per_day_cents: 0, categories: [], divisor_days: null }),
      proposal: null,
      summary: summary({
        daily_ceiling_source: "estimate",
        daily_budget: 2_000,
        daily_ceiling_estimate: { variable_cents: 62_000, days: 31, month: "2026-05" },
      }),
      today: "2026-07-23",
    });
    expect(v.estimateBasis).toEqual({
      variableCents: 62_000,
      days: 31,
      month: "2026-05",
    });
    expect(v.estimateBasis!.variableCents / v.estimateBasis!.days).toBe(v.perDayCents);
  });

  // Sem base vinda do motor, a tela fica sem conta a mostrar — nunca com uma conta inventada.
  it("estimativa sem base do motor não fabrica operandos", () => {
    const v = buildTetoView({
      budget: budget({ per_day_cents: 0, categories: [], divisor_days: null }),
      proposal: null,
      summary: summary({ daily_ceiling_source: "estimate", daily_budget: 4_600 }),
      today: "2026-07-23",
    });
    expect(v.estimateBasis).toBeNull();
  });

  // Teto escolhido é decisão do dono: número digitado não tem conta a mostrar.
  it("teto escolhido não carrega base de estimativa", () => {
    const v = buildTetoView({
      budget: budget({ per_day_cents: 5_000 }),
      proposal: null,
      summary: summary({ daily_ceiling_source: "chosen", daily_budget: 5_000 }),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("chosen");
    expect(v.estimateBasis).toBeNull();
  });

  it("sem registro: nem teto escolhido, nem histórico para estimar", () => {
    const v = buildTetoView({
      budget: budget({ per_day_cents: 0, categories: [], divisor_days: null }),
      proposal: null,
      summary: summary({ daily_ceiling_source: "none", daily_budget: 0 }),
      today: "2026-07-23",
    });
    expect(v.kind).toBe("none");
    expect(v.perDayCents).toBe(0);
  });

  it("sem resumo do dia ainda, o modo cai no débito e o teto escolhido continua veredito", () => {
    const v = buildTetoView({
      budget: budget(),
      proposal: null,
      summary: undefined,
      today: "2026-07-23",
    });
    expect(v.kind).toBe("chosen");
    expect(v.mode).toBe("debit");
  });
});

describe("a cerimônia guiada", () => {
  it("faz as cinco perguntas do método, na ordem, com categoria própria", () => {
    expect(GUIDED_QUESTIONS.map((q) => q.category)).toEqual([
      "Comida",
      "Transporte",
      "Saúde",
      "Lazer",
      "Compras",
    ]);
    for (const q of GUIDED_QUESTIONS) {
      expect(q.question.endsWith("?")).toBe(true);
      expect(q.question[0]).toBe(q.question[0]?.toUpperCase());
    }
  });
});
