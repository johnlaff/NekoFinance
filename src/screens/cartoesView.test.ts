import { describe, expect, it } from "vitest";
import type { InvoiceSummary } from "../lib/api";
import {
  buildBars,
  cycleOptions,
  cycleStateLabel,
  cycleWindow,
  defaultInvoiceId,
  groupSeries,
  heroSubtitle,
  installmentProgress,
  metaLine,
  monthLabelLower,
  netOfRefunds,
  nextDueAcross,
  ownerKind,
  subscriptionCadence,
  verdictLine,
} from "./cartoesView";

function inv(over: Partial<InvoiceSummary> & { id: string }): InvoiceSummary {
  return {
    cycle_month: "2026-08",
    closing_date: "2026-07-20",
    due_date: "2026-08-10",
    status: "aberta",
    stated_total_cents: 428_900,
    purchases_sum_cents: 403_900,
    effective_total_cents: 428_900,
    reconciliation_delta_cents: 25_000,
    ...over,
  };
}

const WINDOW: InvoiceSummary[] = [
  inv({
    id: "mar",
    cycle_month: "2026-03",
    status: "paga",
    effective_total_cents: 163_000,
  }),
  inv({
    id: "abr",
    cycle_month: "2026-04",
    status: "paga",
    effective_total_cents: 223_000,
  }),
  inv({
    id: "mai",
    cycle_month: "2026-05",
    status: "paga",
    effective_total_cents: 189_000,
  }),
  inv({
    id: "jun",
    cycle_month: "2026-06",
    status: "paga",
    effective_total_cents: 270_000,
  }),
  inv({
    id: "jul",
    cycle_month: "2026-07",
    status: "fechada",
    effective_total_cents: 249_000,
  }),
  inv({
    id: "ago",
    cycle_month: "2026-08",
    status: "aberta",
    effective_total_cents: 300_000,
  }),
];

describe("cycleWindow", () => {
  it("ordena velho → novo e corta nos últimos 6 ciclos", () => {
    const many = [
      inv({ id: "z", cycle_month: "2026-09" }),
      ...WINDOW,
      inv({ id: "a", cycle_month: "2025-12" }),
    ];
    const win = cycleWindow(many);
    expect(win).toHaveLength(6);
    expect(win[0]!.id).toBe("abr");
    expect(win.at(-1)!.id).toBe("z");
  });
});

describe("cycleOptions", () => {
  it("rotula mês curto capitalizado com o status capitalizado", () => {
    const opts = cycleOptions(WINDOW);
    expect(opts[0]).toEqual({ value: "mar", label: "Mar · Paga" });
    expect(opts.at(-1)).toEqual({ value: "ago", label: "Ago · Aberta" });
  });

  it("acrescenta o ano quando a janela cruza a virada", () => {
    const opts = cycleOptions([
      inv({ id: "dez", cycle_month: "2025-12", status: "paga" }),
      inv({ id: "jan", cycle_month: "2026-01", status: "aberta" }),
    ]);
    expect(opts[0]!.label).toBe("Dez ’25 · Paga");
    expect(opts[1]!.label).toBe("Jan · Aberta");
  });
});

describe("defaultInvoiceId", () => {
  it("prefere a fatura aberta, senão a próxima a vencer, senão a mais recente", () => {
    const open = inv({ id: "ago" });
    const due = inv({ id: "jul", status: "fechada" });
    expect(defaultInvoiceId({ open_invoice: open, next_due: due }, WINDOW)).toBe("ago");
    expect(defaultInvoiceId({ open_invoice: null, next_due: due }, WINDOW)).toBe("jul");
    expect(defaultInvoiceId({ open_invoice: null, next_due: null }, WINDOW)).toBe(
      "ago",
    );
    expect(defaultInvoiceId({ open_invoice: null, next_due: null }, [])).toBeNull();
  });
});

describe("buildBars", () => {
  it("normaliza pelo maior ciclo da janela e marca o selecionado", () => {
    const bars = buildBars(WINDOW, "ago");
    expect(bars.bars).toHaveLength(6);
    const ago = bars.bars.at(-1)!;
    expect(ago.selected).toBe(true);
    expect(ago.pct).toBe(100);
    const jun = bars.bars[3]!;
    expect(jun.pct).toBe(90); // 270000 / 300000
    expect(jun.selected).toBe(false);
  });

  it("carrega equivalente textual completo e legenda honesta com a aberta", () => {
    const bars = buildBars(WINDOW, "ago");
    expect(bars.aria).toContain("Mar");
    // NBSP após "R$" é contrato do formatBRL (cola o símbolo ao número).
    expect(bars.aria).toContain("R$ 1.630,00");
    expect(bars.caption).toBe(
      "Faturas dos últimos 6 ciclos — a de agosto ainda acumula.",
    );
  });

  it("não promete acúmulo quando a selecionada não está aberta", () => {
    const bars = buildBars(WINDOW.slice(0, 5), "jul");
    expect(bars.caption).toBe("Faturas dos últimos 5 ciclos.");
  });

  it("sobrevive à janela de valores zerados", () => {
    const bars = buildBars(
      [inv({ id: "a", effective_total_cents: 0, status: "paga" })],
      "a",
    );
    expect(bars.bars[0]!.pct).toBe(0);
  });
});

describe("heroSubtitle", () => {
  it("declara a autoridade da planilha quando o total declarado existe", () => {
    expect(heroSubtitle(428_900)).toBe("Total declarado — autoridade da planilha");
  });
  it("declara a soma quando o total declarado não existe", () => {
    expect(heroSubtitle(null)).toBe("Soma das compras itemizadas");
  });
});

describe("cycleStateLabel", () => {
  it("conta os dias até o fechamento na aberta", () => {
    const open = inv({ id: "ago", closing_date: "2026-08-20" });
    expect(cycleStateLabel(open, "2026-08-15")).toBe("Fecha em 5 dias");
    expect(cycleStateLabel(open, "2026-08-19")).toBe("Fecha amanhã");
    expect(cycleStateLabel(open, "2026-08-20")).toBe("Fecha hoje");
  });

  it("nomeia o fechamento, a abertura futura e o pagamento", () => {
    expect(
      cycleStateLabel(
        inv({ id: "jul", status: "fechada", closing_date: "2026-07-20" }),
        "2026-08-01",
      ),
    ).toMatch(/^Fechou em 20 de jul/);
    expect(
      cycleStateLabel(
        inv({ id: "set", status: "prevista", closing_date: "2026-09-20" }),
        "2026-08-01",
      ),
    ).toMatch(/^Fecha em 20 de set/);
    expect(
      cycleStateLabel(
        inv({ id: "jun", status: "paga", due_date: "2026-07-10" }),
        "2026-08-01",
      ),
    ).toMatch(/^Paga em 10 de jul/);
  });
});

describe("installmentProgress", () => {
  it("deriva fração, restante em reais e faturas restantes de n/N", () => {
    const p = installmentProgress("2/5", 64_000);
    expect(p).toEqual({
      current: 2,
      total: 5,
      fraction: 0.4,
      remainingCents: 192_000,
      remainingCycles: 3,
    });
  });

  it("recusa rótulos malformados sem inventar número", () => {
    expect(installmentProgress(null, 64_000)).toBeNull();
    expect(installmentProgress("abc", 64_000)).toBeNull();
    expect(installmentProgress("0/5", 64_000)).toBeNull();
    expect(installmentProgress("6/5", 64_000)).toBeNull();
  });
});

describe("subscriptionCadence", () => {
  it("nomeia o dia da cobrança a partir da ocorrência", () => {
    expect(subscriptionCadence("2026-06-15")).toBe(
      "Todo mês, dia 15 · pré-lança nas faturas futuras",
    );
  });
});

describe("groupSeries", () => {
  it("separa assinatura de parcelado pela presença do rótulo n/N", () => {
    const series = groupSeries([
      {
        txn_id: "a",
        date: "2026-06-15",
        description: "Assinatura",
        amount_cents: 4_990,
        owner_name: "Eu",
        series_id: "s1",
        installment_label: null,
        is_projection: false,
      },
      {
        txn_id: "b",
        date: "2026-06-16",
        description: "Notebook",
        amount_cents: 64_000,
        owner_name: "Eu",
        series_id: "s2",
        installment_label: "2/5",
        is_projection: false,
      },
      {
        txn_id: "c",
        date: "2026-07-15",
        description: "Assinatura",
        amount_cents: 4_990,
        owner_name: "Eu",
        series_id: "s1",
        installment_label: null,
        is_projection: true,
      },
    ]);
    expect(series).toHaveLength(2);
    expect(series[0]).toMatchObject({ id: "s1", kind: "subscription" });
    expect(series[1]).toMatchObject({ id: "s2", kind: "installment" });
  });
});

describe("nextDueAcross", () => {
  const base = {
    institution: null,
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
  };

  it("acha o vencimento mais próximo entre os titulares", () => {
    const a = {
      ...base,
      id: "a",
      name: "A",
      open_invoice: null,
      next_due: inv({ id: "ia", due_date: "2026-08-10" }),
    };
    const b = {
      ...base,
      id: "b",
      name: "B",
      open_invoice: null,
      next_due: inv({ id: "ib", due_date: "2026-08-04" }),
    };
    const c = { ...base, id: "c", name: "C", open_invoice: null, next_due: null };
    expect(nextDueAcross([a, b, c])?.invoice.id).toBe("ib");
    expect(nextDueAcross([c])).toBeNull();
    expect(nextDueAcross([])).toBeNull();
  });
});

describe("netOfRefunds", () => {
  it("subtrai os reembolsos do total efetivo", () => {
    expect(
      netOfRefunds({
        effective_total_cents: 428_900,
        refunds: [
          {
            txn_id: "r",
            date: "2026-08-10",
            amount_cents: 35_000,
            description: "Parte",
            is_projection: true,
          },
        ],
      }),
    ).toBe(393_900);
  });
});

describe("metaLine", () => {
  it("compõe ciclo e limite discreto sem centavos", () => {
    expect(
      metaLine({ closing_day: 20, due_day: 10, credit_limit_cents: 1_200_000 }),
    ).toBe("Fecha dia 20 · vence dia 10 · limite R$ 12.000");
    expect(metaLine({ closing_day: 3, due_day: 10, credit_limit_cents: null })).toBe(
      "Fecha dia 3 · vence dia 10",
    );
  });
});

describe("monthLabelLower", () => {
  it("mantém o mês minúsculo para o meio de frase", () => {
    expect(monthLabelLower("2026-06")).toBe("junho de 2026");
  });
});

describe("verdictLine", () => {
  const base = {
    institution: null,
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
  };

  it("nomeia o próximo vencimento com ponto final único", () => {
    const card = {
      ...base,
      id: "a",
      name: "A",
      open_invoice: null,
      next_due: inv({ id: "ia", due_date: "2026-08-10" }),
    };
    expect(verdictLine([card])).toBe("A próxima fatura vence 10 de ago.");
  });

  it("cai na voz neutra sem vencimento à vista", () => {
    expect(verdictLine([])).toBe("Seus cartões, fatura a fatura.");
  });
});

describe("ownerKind", () => {
  it("mapeia Eu, compartilhado e parceiro", () => {
    expect(ownerKind("Eu")).toBe("personal");
    expect(ownerKind("Compartilhado")).toBe("shared");
    expect(ownerKind("Parceiro(a)")).toBe("partner");
  });
});
