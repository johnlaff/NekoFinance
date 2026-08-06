import { describe, it, expect } from "vitest";
import { buildHorizonteView, type HorizonteInput } from "./horizonteView";
import type { Forecast, MonthEnd, TransactionRow, ForecastDay } from "../lib/api";

// ---- fixtures --------------------------------------------------------------

const ann = {
  realized_income_cents: 0,
  realized_savings_cents: 0,
  realized_rate_bps: 0,
  registered_economia_cents: 0,
  patrimonio_cents: 0,
  economia_ruler_cents: 0,
  economia_ruler_rate_bps: 0,
  economia_state: "no_record" as const,
  projected_income_cents: 0,
  projected_savings_cents: 0,
  projected_rate_bps: 0,
};

function day(date: string, balance: number): ForecastDay {
  return {
    date,
    income_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    economia_cents: 0,
    balance_cents: balance,
  };
}

const MONTH_END: MonthEnd[] = [
  { year: 2026, month: 7, balance_cents: 1299520 },
  { year: 2026, month: 8, balance_cents: 1468037 },
  { year: 2026, month: 9, balance_cents: 1825323 },
  { year: 2026, month: 10, balance_cents: 2212289 },
  { year: 2026, month: 11, balance_cents: 2605000 },
  { year: 2026, month: 12, balance_cents: 2997711 },
];

// Agosto tem lastro (76% ≥ 60%) → não entra no traçado típico; setembro a dezembro, sem lastro.
const COVERAGE = [
  {
    year: 2026,
    month: 8,
    projected_outflow_cents: 800000,
    baseline_outflow_cents: 1112126,
    coverage_bps: 7600,
    is_complete: false,
    estimated_missing_cents: 312126,
  },
  {
    year: 2026,
    month: 9,
    projected_outflow_cents: 383522,
    baseline_outflow_cents: 1112126,
    coverage_bps: 3300,
    is_complete: false,
    estimated_missing_cents: 728604,
  },
  {
    year: 2026,
    month: 10,
    projected_outflow_cents: 352891,
    baseline_outflow_cents: 1112126,
    coverage_bps: 3300,
    is_complete: false,
    estimated_missing_cents: 759235,
  },
  {
    year: 2026,
    month: 11,
    projected_outflow_cents: 347146,
    baseline_outflow_cents: 1112126,
    coverage_bps: 3300,
    is_complete: false,
    estimated_missing_cents: 764980,
  },
  {
    year: 2026,
    month: 12,
    projected_outflow_cents: 344156,
    baseline_outflow_cents: 1112126,
    coverage_bps: 3300,
    is_complete: false,
    estimated_missing_cents: 767970,
  },
];

function forecast(over: Partial<Forecast> = {}): Forecast {
  return {
    today: "2026-07-22",
    horizon_end: "2026-12-31",
    annual_savings: ann,
    coverage: COVERAGE,
    baseline_outflow_cents: 1112126,
    trusted_through_month: "2026-08",
    total_missing_cents: 3020789,
    safe_to_spend_today_cents: 20000,
    cash_headroom_cents: 752086,
    savings_headroom_cents: 20000,
    binding_guardrail: "savings",
    savings_band_verdict: "in_band",
    savings_band: { floor_bps: 2000, ceiling_bps: 3000 },
    savings_band_scope_lived: true,
    deepest_deficit: null,
    daily: [
      day("2026-07-22", 756830),
      day("2026-07-26", 752086), // menor ponto
      day("2026-08-15", 1468037),
      day("2026-09-10", 1825323),
      day("2026-10-12", 2212289),
      day("2026-11-20", 2605000),
      day("2026-12-31", 2997711),
    ],
    month_end: MONTH_END,
    months: [],
    ...over,
  };
}

function tx(
  over: Partial<TransactionRow> & { id: string; date: string },
): TransactionRow {
  return {
    type: "expense",
    amount: 10000,
    description: "",
    payment_method: "debit",
    is_projection: true,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "projetado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    has_refund_link: false,
    ...over,
  };
}

function input(over: Partial<HorizonteInput> = {}): HorizonteInput {
  return { forecast: forecast(), rowsByMonth: {}, syncLabel: "22h14", ...over };
}

// ---- veredito --------------------------------------------------------------

describe("buildHorizonteView — veredito", () => {
  it("sem forecast: carregando, sem estrada nem grade", () => {
    const v = buildHorizonteView(input({ forecast: undefined }));
    expect(v.voice).toBe("loading");
    expect(v.road).toBeNull();
    expect(v.grid).toEqual([]);
    expect(v.baselineOutflowCents).toBe(0);
  });

  it("caminho livre: lançado nunca negativo → menor ponto e mês de confiança", () => {
    const v = buildHorizonteView(input());
    expect(v.voice).toBe("livre");
    expect(v.minPoint).toEqual({ dateISO: "2026-07-26", cents: 752086 });
    expect(v.trustedThroughMonth).toBe("2026-08");
    expect(v.trustedMonthLabel).toBe("agosto");
  });

  it("gêmeo honesto: o traçado típico raspa o zero no fim do horizonte", () => {
    const v = buildHorizonteView(input());
    // Dez lançado 2.997.711 − Σ custo (728604+759235+764980+767970 = 3.020.789) = −23.078
    expect(v.endLaunchedCents).toBe(2997711);
    expect(v.endTypicalCents).toBe(-23078);
    expect(v.typicalHitsZero).toBe(true);
  });

  it("aperto: o lançado cruza o zero → voz de aperto com o mês do buraco", () => {
    const v = buildHorizonteView(
      input({
        forecast: forecast({
          deepest_deficit: { date: "2026-10-12", balance_cents: -124000 },
        }),
      }),
    );
    expect(v.voice).toBe("aperto");
    expect(v.deficit).toEqual({ dateISO: "2026-10-12", cents: -124000 });
    expect(v.deficitMonthLabel).toBe("outubro");
  });

  it("déficit não-negativo não vira aperto (o campo do motor pode vir positivo)", () => {
    const v = buildHorizonteView(
      input({
        forecast: forecast({
          deepest_deficit: { date: "2026-07-26", balance_cents: 752086 },
        }),
      }),
    );
    expect(v.voice).toBe("livre");
    expect(v.deficit).toBeNull();
  });

  it("vazio: sem gasto típico o radar não tem veredito", () => {
    const v = buildHorizonteView(
      input({ forecast: forecast({ baseline_outflow_cents: 0 }) }),
    );
    expect(v.voice).toBe("vazio");
  });

  it("vazio: sem estrada (só um ponto diário) também é radar sem futuro", () => {
    const v = buildHorizonteView(
      input({ forecast: forecast({ daily: [day("2026-07-22", 756830)] }) }),
    );
    expect(v.voice).toBe("vazio");
    expect(v.road).toBeNull();
  });
});

// ---- estrada ---------------------------------------------------------------

describe("buildHorizonteView — estrada", () => {
  it("pontos, menor índice e fronteira do lastro", () => {
    const v = buildHorizonteView(input());
    const road = v.road!;
    expect(road.points).toHaveLength(7);
    expect(road.minIndex).toBe(1); // 2026-07-26
    // Fronteira: primeiro ponto cujo mês é posterior a 2026-08 → 2026-09-10 (índice 3).
    expect(road.fogFromIndex).toBe(3);
  });

  it("traçado típico: ancora no lançado enquanto há lastro e diverge depois", () => {
    const road = buildHorizonteView(input()).road!;
    const byMonth = Object.fromEntries(
      road.typicalPath.map((p) => [p.dateISO.slice(0, 7), p.cents]),
    );
    expect(byMonth["2026-07"]).toBe(1299520); // sem custo → == lançado
    expect(byMonth["2026-08"]).toBe(1468037); // Ago tem lastro → == lançado
    expect(byMonth["2026-09"]).toBe(1096719);
    expect(byMonth["2026-12"]).toBe(-23078);
  });

  it("fim lançado é o saldo de dezembro (month_end), não o último ponto diário", () => {
    // Série diária esparsa que para em julho, mas month_end alcança dezembro.
    const sparse = forecast({
      daily: [day("2026-07-22", 756830), day("2026-07-31", 1299520)],
    });
    const road = buildHorizonteView(input({ forecast: sparse })).road!;
    expect(road.endLaunchedCents).toBe(2997711); // dezembro do month_end, não julho (1.299.520)
  });

  it("o eixo Y acomoda o topo do traçado típico, não só o do lançado", () => {
    // Lançado diário baixo (≤ 1,3 mi), mas o típico sobe a ~2,3 mi — o eixo precisa contê-lo.
    const sparse = forecast({
      daily: [day("2026-07-22", 700000), day("2026-07-31", 1299520)],
    });
    const road = buildHorizonteView(input({ forecast: sparse })).road!;
    const typMax = Math.max(...road.typicalPath.map((p) => p.cents));
    expect(road.yMax).toBeGreaterThanOrEqual(typMax);
  });

  it("eixo Y nice-scaled sempre inclui o zero", () => {
    const road = buildHorizonteView(input()).road!;
    expect(road.yTicks).toContain(0);
    expect(road.yMax).toBeGreaterThanOrEqual(2997711);
    expect(road.yMin).toBeLessThanOrEqual(-23078); // acomoda o pior caso do típico
  });

  it("rótulos de mês na virada de cada mês", () => {
    const road = buildHorizonteView(input()).road!;
    expect(road.monthTicks.map((t) => t.label)).toEqual([
      "Jul",
      "Ago",
      "Set",
      "Out",
      "Nov",
      "Dez",
    ]);
  });
});

// ---- grade -----------------------------------------------------------------

describe("buildHorizonteView — grade dos 12 meses", () => {
  it("mês corrente + 11, com os estados epistêmicos", () => {
    const g = buildHorizonteView(input()).grid;
    expect(g).toHaveLength(12);
    expect(g[0]).toMatchObject({ year: 2026, month: 7, state: "vivido", todayDay: 22 });
    expect(g[1]).toMatchObject({ month: 8, state: "prev" }); // ≤ fronteira
    expect(g[2]).toMatchObject({ month: 9, state: "conf" }); // > fronteira, com dado
    // Jan/2027 em diante não tem month_end → sem registro, não navegável.
    const jan = g.find((m) => m.year === 2027 && m.month === 1)!;
    expect(jan.state).toBe("sem");
    expect(jan.navMonth).toBeNull();
    expect(jan.band).toBeNull();
  });

  it("cor da grade vem da banda do saldo de fim de mês (dado real: folga)", () => {
    const g = buildHorizonteView(input()).grid;
    expect(g[0]!.band).toBe("comfortable"); // 1.299.520 = R$ 12.995,20 > R$ 2.000 → folga
  });

  it("dataset sintético apertado/negativo pinta âmbar/vermelho na grade", () => {
    const stressed = forecast({
      month_end: [
        { year: 2026, month: 7, balance_cents: 1299520 },
        { year: 2026, month: 8, balance_cents: 60000 }, // R$ 600 → apertado (âmbar)
        { year: 2026, month: 9, balance_cents: -80000 }, // −R$ 800 → crítico (vermelho)
      ],
    });
    const g = buildHorizonteView(input({ forecast: stressed })).grid;
    expect(g[1]!.band).toBe("tight");
    expect(g[2]!.band).toBe("critical");
  });

  it("dias no mês e dia da semana do 1º dia derivados do calendário", () => {
    const g = buildHorizonteView(input()).grid;
    // Julho/2026: 31 dias, 1º cai numa quarta (dow 3).
    expect(g[0]!.daysInMonth).toBe(31);
    expect(g[0]!.firstDow).toBe(3);
  });
});

// ---- compromissos ----------------------------------------------------------

describe("buildHorizonteView — compromissos", () => {
  const agosto: TransactionRow[] = [
    tx({
      id: "a1",
      date: "2026-08-01",
      description: "Fatura Vivo",
      amount: 6528,
      is_fixed: true,
    }),
    tx({
      id: "a2",
      date: "2026-08-07",
      description: "Financiamento do carro",
      amount: 100651,
      installment_index: 11,
      installment_total: 36,
    }),
    tx({
      id: "a3",
      date: "2026-08-07",
      description: "Reembolso do financiamento",
      amount: 100651,
      type: "income",
      has_refund_link: true,
    }),
    tx({
      id: "a4",
      date: "2026-08-27",
      description: "Salário",
      amount: 601273,
      type: "income",
      is_fixed: true,
    }),
  ];

  it("agrupa por mês, deriva subtítulo/parcela e soma entra/sai/dias", () => {
    const v = buildHorizonteView(input({ rowsByMonth: { "2026-08": agosto } }));
    expect(v.commitments).toHaveLength(1);
    const ago = v.commitments[0]!;
    expect(ago.label).toBe("Agosto");
    expect(ago.monthKey).toBe("2026-08");
    // Entra = reembolso + salário; sai = fatura + financiamento.
    expect(ago.inCents).toBe(100651 + 601273);
    expect(ago.outCents).toBe(6528 + 100651);
    // Dias distintos: 01, 07, 27 → 3.
    expect(ago.days).toBe(3);
  });

  it("subtítulos e sinais derivam dos campos reais, nunca de rótulo fabricado", () => {
    const v = buildHorizonteView(input({ rowsByMonth: { "2026-08": agosto } }));
    const items = v.commitments[0]!.items;
    const fin = items.find((i) => i.title === "Financiamento do carro")!;
    expect(fin.subtitle).toBe("Parcela");
    expect(fin.installment).toBe("11/36");
    expect(fin.signedCents).toBe(-100651);
    const reemb = items.find((i) => i.title === "Reembolso do financiamento")!;
    expect(reemb.subtitle).toBe("Entrada vinculada");
    expect(reemb.isIn).toBe(true);
    expect(reemb.signedCents).toBe(100651);
    const fatura = items.find((i) => i.title === "Fatura Vivo")!;
    expect(fatura.subtitle).toBe("Conta fixa");
    expect(fatura.dayLabel).toBe("1º");
  });

  it("total soma todos os meses; vazio → total nulo", () => {
    const setembro: TransactionRow[] = [
      tx({
        id: "s1",
        date: "2026-09-10",
        description: "Aluguel",
        amount: 179636,
        is_fixed: true,
      }),
    ];
    const v = buildHorizonteView(
      input({ rowsByMonth: { "2026-08": agosto, "2026-09": setembro } }),
    );
    expect(v.commitments).toHaveLength(2);
    expect(v.commitmentsTotal).toEqual({
      inCents: 100651 + 601273,
      outCents: 6528 + 100651 + 179636,
      days: 3 + 1,
    });
    const empty = buildHorizonteView(input({ rowsByMonth: {} }));
    expect(empty.commitmentsTotal).toBeNull();
  });
});
