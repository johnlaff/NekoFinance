import { describe, expect, it } from "vitest";
import type { ForecastDay, UpcomingInvoice } from "../lib/api";
import {
  dueLabel,
  eyebrowDate,
  faturaDayLabel,
  greetingForHour,
  joinNames,
  localTodayIso,
  monthInsight,
  openInvoicesView,
  saldoBandPhrase,
  spendCapReason,
  saldoGaugeFraction,
  upcomingIncome,
} from "./hojeView";

function invoice(overrides: Partial<UpcomingInvoice>): UpcomingInvoice {
  return {
    account_id: "card-1",
    card_name: "Cartão",
    due_date: "2026-08-10",
    amount_cents: 100_00,
    status: "aberta",
    owner_name: "Eu",
    has_refund_expectation: false,
    ...overrides,
  };
}

function day(overrides: Partial<ForecastDay> & { date: string }): ForecastDay {
  return {
    income_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    economia_cents: 0,
    balance_cents: 0,
    ...overrides,
  };
}

describe("greetingForHour", () => {
  it("cobre as três faixas do dia, com a madrugada como noite", () => {
    expect(greetingForHour(5)).toBe("Bom dia.");
    expect(greetingForHour(11)).toBe("Bom dia.");
    expect(greetingForHour(12)).toBe("Boa tarde.");
    expect(greetingForHour(17)).toBe("Boa tarde.");
    expect(greetingForHour(18)).toBe("Boa noite.");
    expect(greetingForHour(23)).toBe("Boa noite.");
    expect(greetingForHour(0)).toBe("Boa noite.");
    expect(greetingForHour(4)).toBe("Boa noite.");
  });
});

describe("datas", () => {
  it("formata a data por extenso e o vencimento curto", () => {
    expect(eyebrowDate("2026-07-15")).toBe("Quarta-feira, 15 de julho");
    expect(faturaDayLabel("2026-08-10")).toBe("10 de agosto");
  });

  it("localTodayIso usa o relógio local, com zero à esquerda", () => {
    expect(localTodayIso(new Date(2026, 6, 5, 23, 30))).toBe("2026-07-05");
  });
});

describe("openInvoicesView", () => {
  it("agrupa por vencimento em ordem cronológica, maior primeiro dentro do grupo", () => {
    const view = openInvoicesView([
      invoice({ account_id: "amazon", amount_cents: 195_62, due_date: "2026-08-10" }),
      invoice({
        account_id: "bradesco",
        amount_cents: 1_631_72,
        due_date: "2026-08-12",
      }),
      invoice({ account_id: "itau", amount_cents: 1_747_39, due_date: "2026-08-10" }),
    ]);
    expect(view.groups.map((g) => g.dueDate)).toEqual(["2026-08-10", "2026-08-12"]);
    expect(view.groups[0]!.invoices.map((i) => i.account_id)).toEqual([
      "itau",
      "amazon",
    ]);
    expect(view.totalCents).toBe(195_62 + 1_631_72 + 1_747_39);
    expect(view.count).toBe(3);
    expect(view.largestAccountId).toBe("itau");
  });

  it("em aberto = aberta ou fechada; prevista e paga ficam fora", () => {
    const view = openInvoicesView([
      invoice({ account_id: "a", status: "aberta", amount_cents: 10_00 }),
      invoice({ account_id: "b", status: "fechada", amount_cents: 20_00 }),
      invoice({ account_id: "c", status: "prevista", amount_cents: 40_00 }),
      invoice({ account_id: "d", status: "paga", amount_cents: 80_00 }),
    ]);
    expect(view.count).toBe(2);
    expect(view.totalCents).toBe(30_00);
    expect(view.largestAccountId).toBe("b");
  });

  it("sem faturas em aberto: vazio honesto, sem destaque", () => {
    const view = openInvoicesView([invoice({ status: "prevista" })]);
    expect(view.count).toBe(0);
    expect(view.groups).toEqual([]);
    expect(view.largestAccountId).toBeNull();
  });
});

describe("joinNames", () => {
  it("junta em linguagem natural", () => {
    expect(joinNames([])).toBe("");
    expect(joinNames(["Inter"])).toBe("Inter");
    expect(joinNames(["Inter", "BB"])).toBe("Inter e BB");
    expect(joinNames(["Inter", "Nubank", "BB"])).toBe("Inter, Nubank e BB");
  });
});

describe("monthInsight", () => {
  const july: ForecastDay[] = [
    day({ date: "2026-07-11", balance_cents: 13_671_23 }),
    day({ date: "2026-07-12", balance_cents: 5_569_65 }),
    day({ date: "2026-07-15", balance_cents: 5_569_65 }),
    day({ date: "2026-07-20", income_cents: 1_998_65, balance_cents: 7_568_30 }),
    day({ date: "2026-07-30", income_cents: 6_012_73, balance_cents: 12_995_20 }),
    day({ date: "2026-07-31", balance_cents: 12_995_20 }),
  ];

  it("deriva fechamento, mínimo (primeiro dia) e a próxima entrada", () => {
    const insight = monthInsight(july, "2026-07-15");
    expect(insight).not.toBeNull();
    expect(insight!.endBalanceCents).toBe(12_995_20);
    expect(insight!.minCents).toBe(5_569_65);
    expect(insight!.minDate).toBe("2026-07-12");
    expect(insight!.minIsOngoing).toBe(true);
    expect(insight!.nextIncomeDate).toBe("2026-07-20");
    expect(insight!.nextIncomeCents).toBe(1_998_65);
    expect(insight!.deficitDaysAhead).toBe(0);
  });

  it("mínimo à frente não é 'hoje'; dias no vermelho contam só de hoje em diante", () => {
    const series: ForecastDay[] = [
      day({ date: "2026-07-01", balance_cents: -50_00 }),
      day({ date: "2026-07-10", balance_cents: 300_00 }),
      day({ date: "2026-07-20", balance_cents: -120_00 }),
      day({ date: "2026-07-31", balance_cents: 80_00 }),
    ];
    const insight = monthInsight(series, "2026-07-10");
    expect(insight!.minDate).toBe("2026-07-20");
    expect(insight!.minIsOngoing).toBe(false);
    // O vermelho do dia 1 já passou; só o dia 20 está à vista.
    expect(insight!.deficitDaysAhead).toBe(1);
  });

  it("mês sem série devolve null — nunca números fabricados", () => {
    expect(monthInsight([], "2026-07-15")).toBeNull();
  });
});

describe("datas relativas dos próximos movimentos", () => {
  it("rotula vencimentos próximos em dias corridos e distantes por extenso", () => {
    expect(dueLabel("2026-07-15", "2026-07-15")).toEqual({ label: "Hoje", soon: true });
    expect(dueLabel("2026-07-15", "2026-07-16")).toEqual({
      label: "Amanhã",
      soon: true,
    });
    expect(dueLabel("2026-07-15", "2026-07-26")).toEqual({
      label: "Em 11 dias — 26 de julho",
      soon: true,
    });
    expect(dueLabel("2026-07-15", "2026-08-10")).toEqual({
      label: "10 de agosto",
      soon: false,
    });
  });

  it("acha a primeira entrada futura dentro da janela — e só dentro dela", () => {
    const daily: ForecastDay[] = [
      day({ date: "2026-07-15", income_cents: 999_00 }),
      day({ date: "2026-07-20", income_cents: 1_998_65 }),
      day({ date: "2026-09-30", income_cents: 6_012_73 }),
    ];
    expect(upcomingIncome(daily, "2026-07-15", 45)).toEqual({
      date: "2026-07-20",
      cents: 1_998_65,
    });
    // A entrada de hoje não é "próxima"; a de setembro está fora dos 45 dias.
    expect(upcomingIncome([daily[0]!, daily[2]!], "2026-07-15", 45)).toBeNull();
  });
});

describe("termômetro do saldo", () => {
  it("toda faixa tem frase com a fronteira absoluta e preenchimento decrescente", () => {
    expect(saldoBandPhrase("comfortable")).toContain("R$ 2.000");
    expect(saldoBandPhrase("ok")).toContain("R$ 1.000");
    expect(saldoBandPhrase("critical")).toContain("R$ 500");
    expect(saldoGaugeFraction("comfortable")).toBe(1);
    expect(saldoGaugeFraction("ok")).toBeLessThan(1);
    expect(saldoGaugeFraction("tight")).toBeLessThan(saldoGaugeFraction("ok"));
    expect(saldoGaugeFraction("negative")).toBeLessThan(saldoGaugeFraction("tight"));
    expect(saldoGaugeFraction("critical")).toBeLessThan(saldoGaugeFraction("negative"));
  });
});

// --- por que o teto do dia é o que é -------------------------------------------------------

describe("spendCapReason", () => {
  const base = {
    bindingGuardrail: "cash" as const,
    deepestBalanceCents: 734_608,
    deepestDate: "2026-08-12",
  };

  it("saldo que se segura acima do zero é leitura de caixa comum", () => {
    // A reserva incompleta NÃO aperta o teto: no método ela socorre o vermelho, não o proíbe.
    expect(spendCapReason(base).kind).toBe("cash");
  });

  it("saldo que abre o bico vira déficit, com o tamanho e o dia", () => {
    const reason = spendCapReason({
      ...base,
      deepestBalanceCents: -120_000,
      deepestDate: "2026-09-14",
    });
    if (reason.kind !== "deficit")
      throw new Error(`esperava deficit, veio ${reason.kind}`);
    expect(reason.shortfallCents).toBe(120_000);
    expect(reason.date).toBe("2026-09-14");
  });

  it("a régua da economia continua tendo a palavra quando é ela que morde", () => {
    expect(spendCapReason({ ...base, bindingGuardrail: "savings" }).kind).toBe(
      "savings",
    );
  });

  it("sem dia de menor saldo não há déficit a nomear", () => {
    expect(
      spendCapReason({ ...base, deepestBalanceCents: -5_000, deepestDate: null }).kind,
    ).toBe("cash");
  });
});
