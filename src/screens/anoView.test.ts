import { describe, it, expect } from "vitest";
import type { MonthMetric, MonthEnd } from "../lib/api";
import {
  outflowCents,
  median,
  buildAnoView,
  buildIncomeAcrossYears,
  type AnoInput,
} from "./anoView";

// ---------------------------------------------------------------- fixtures --
// Planilha real de 2026 (a mesma que ancora o desenho aprovado do #200), em centavos.
// Cada mês carrega income/performance/economia; a saída total sai = income − performance.
// `end` é o saldo projetado no fim do mês (do motor). Julho é o mês corrente.
interface Row {
  m: number;
  income: number;
  perf: number;
  eco: number;
  end: number;
}
const REAL_2026: Row[] = [
  { m: 1, income: 965132, perf: -99751, eco: 0, end: 957349 },
  { m: 2, income: 1623670, perf: 492689, eco: 0, end: 1450038 },
  { m: 3, income: 1042963, perf: -50308, eco: 0, end: 1399730 },
  { m: 4, income: 1342641, perf: -135002, eco: 0, end: 1264728 },
  { m: 5, income: 1274701, perf: 189619, eco: 0, end: 1454347 },
  { m: 6, income: 1018860, perf: -124321, eco: 0, end: 1330026 },
  { m: 7, income: 1211421, perf: -30506, eco: 0, end: 1299520 },
  { m: 8, income: 1015607, perf: 168517, eco: 0, end: 1468037 },
  { m: 9, income: 740808, perf: 357286, eco: 0, end: 1825323 },
  { m: 10, income: 739857, perf: 386966, eco: 0, end: 2212289 },
  { m: 11, income: 739857, perf: 392711, eco: 0, end: 2605000 },
  { m: 12, income: 736867, perf: 392711, eco: 0, end: 2997711 },
];

function monthMetric(r: Row, year = 2026): MonthMetric {
  return {
    year,
    month: r.m,
    income_cents: r.income,
    income_performance_cents: r.income,
    performance_cents: r.perf,
    cost_of_living_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    daily_avg_out_cents: 0,
    daily_projected_cents: 0,
    cartao_cents: 0,
    real_daily_avg_cents: 0,
    economia_cents: r.eco,
    patrimonio_cents: 0,
    savings_rate_bps: r.income > 0 ? Math.round((r.eco / r.income) * 10000) : 0,
  };
}

function realInput(overrides: Partial<AnoInput> = {}): AnoInput {
  const months = REAL_2026.map((r) => monthMetric(r));
  const monthEnd: MonthEnd[] = REAL_2026.map((r) => ({
    year: 2026,
    month: r.m,
    balance_cents: r.end,
  }));
  return {
    year: 2026,
    today: "2026-07-15",
    months,
    monthEnd,
    reserveMonths: null,
    ...overrides,
  };
}

// ------------------------------------------------------------------ helpers --

describe("anoView — helpers puros", () => {
  it("outflowCents = income − performance", () => {
    expect(outflowCents({ income_cents: 965132, performance_cents: -99751 })).toBe(
      1064883,
    );
    expect(outflowCents({ income_cents: 1623670, performance_cents: 492689 })).toBe(
      1130981,
    );
  });

  it("median de janela ímpar e par", () => {
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 3, 2])).toBe(2.5);
    expect(median([])).toBe(0);
  });
});

// ------------------------------------------------------------ teste de lastro --

describe("anoView — teste de lastro (planilha real 2026)", () => {
  it("gasto típico é a mediana das saídas dos meses vividos (jan–jul)", () => {
    const v = buildAnoView(realInput());
    // mediana de [1064883,1085082,1093271,1130981,1143181,1241927,1477643] = 1130981
    expect(v.typicalSpendCents).toBe(1130981);
  });

  it("piso de 60%: set–dez reprovam, ago passa", () => {
    const v = buildAnoView(realInput());
    // 0.6 × 1.130.981 = 678.588,6 → ago (847.090) passa; set–dez (< 386k) reprovam
    expect(v.suspects).toEqual([9, 10, 11, 12]);
    const ago = v.months.find((x) => x.month === 8)!;
    expect(ago.suspect).toBe(false);
    const set = v.months.find((x) => x.month === 9)!;
    expect(set.suspect).toBe(true);
  });
});

// ------------------------------------------------------------ agregados anuais --

describe("anoView — agregados anuais e veredito", () => {
  it("realizado: 7 meses vividos, economia zero, sobra R$ 2.424,20", () => {
    const v = buildAnoView(realInput());
    expect(v.livedCount).toBe(7);
    expect(v.incomeLived).toBe(8479388); // ENT_R
    expect(v.economiaLived).toBe(0); // ECO_R
    expect(v.surplusLived).toBe(242420); // PERF_R = R$ 2.424,20
    expect(v.livedPct).toBe(0);
  });

  it("ano inteiro: entradas R$ 124.523,84; falta anual para 20% = R$ 24.904,77", () => {
    const v = buildAnoView(realInput());
    expect(v.incomeYear).toBe(12452384); // ENT_A
    expect(v.economiaYear).toBe(0); // ECO_A
    // ENT_R*0.2 − ECO_R = 1.695.877,6 → arredonda p/ 1.695.878 (R$ 16.958,78)
    expect(v.shortfallLivedCents).toBe(1695878);
    // ENT_A*0.2 − ECO_A = 2.490.476,8 → 2.490.477 (R$ 24.904,77)
    expect(v.shortfallYearCents).toBe(2490477);
  });

  it("com meses suspeitos, o veredito recua para o realizado (estimativa) e a régua declara o recorte vivido", () => {
    const v = buildAnoView(realInput());
    expect(v.estimate).toBe(true);
    expect(v.rulerScopeLived).toBe(true);
    expect(v.rulerPct).toBe(0);
    // economia zero + reserva desconhecida → "não guardou nada"
    expect(v.verdict.kind).toBe("below_band");
  });

  it("onde dezembro termina: lançado R$ 29.977,11; cenário típico −R$ 984,98", () => {
    const v = buildAnoView(realInput());
    expect(v.endMonth).toBe(12);
    expect(v.endBalanceCents).toBe(2997711); // DEZ
    expect(v.endBalanceTypicalCents).toBe(-98498); // DEZ_TIPICO = −R$ 984,98
  });
});

// ------------------------------------------------------- estados epistêmicos --

describe("anoView — estados epistêmicos do veredito", () => {
  it("dentro da faixa quando a taxa fecha entre 20% e 30%, sem suspeitos", () => {
    // Ano fechado (passado): todos vividos, economia 25% em cada mês, sem suspeitos.
    const months = REAL_2026.map((r) =>
      monthMetric({ ...r, eco: Math.round(r.income * 0.25) }, 2025),
    );
    const v = buildAnoView({
      year: 2025,
      today: "2026-07-15",
      months,
      monthEnd: REAL_2026.map((r) => ({
        year: 2025,
        month: r.m,
        balance_cents: r.end,
      })),
      reserveMonths: 8,
    });
    expect(v.livedCount).toBe(12);
    expect(v.estimate).toBe(false);
    expect(v.rulerScopeLived).toBe(false);
    expect(v.rulerPct).toBe(25);
    expect(v.verdict.kind).toBe("in_band");
  });

  it("zero por escolha: economia zero mas reserva ≥ 6 meses", () => {
    const v = buildAnoView(realInput({ reserveMonths: 8 }));
    expect(v.verdict.kind).toBe("zero_by_choice");
  });

  it("sem registro: nenhum mês vivido tem atividade", () => {
    const empty = Array.from({ length: 12 }, (_, i) =>
      monthMetric({ m: i + 1, income: 0, perf: 0, eco: 0, end: 0 }, 2024),
    );
    const v = buildAnoView({
      year: 2024,
      today: "2026-07-15",
      months: empty,
      monthEnd: [],
      reserveMonths: null,
    });
    expect(v.hasData).toBe(false);
    expect(v.verdict.kind).toBe("no_record");
  });

  it("meses futuros exibem savedPct null (— na tela), nunca 0%", () => {
    const v = buildAnoView(realInput());
    const dez = v.months.find((x) => x.month === 12)!;
    expect(dez.future).toBe(true);
    expect(dez.savedPct).toBeNull();
    const jan = v.months.find((x) => x.month === 1)!;
    expect(jan.lived).toBe(true);
    expect(jan.savedPct).toBe(0);
  });
});

// -------------------------------------------------------------- robustez ----

describe("anoView — robustez (achados da revisão externa do desenho)", () => {
  it("sem meses suspeitos: sem cenário alternativo de dezembro", () => {
    // Todos os futuros com saída ≥ 60% do típico (usa o realizado de jan em todos).
    const rows = REAL_2026.map((r) =>
      r.m > 7 ? { ...r, income: 1211421, perf: -30506 } : r,
    );
    const v = buildAnoView(realInput({ months: rows.map((r) => monthMetric(r)) }));
    expect(v.suspects).toEqual([]);
    expect(v.endBalanceTypicalCents).toBeNull();
    expect(v.estimate).toBe(false);
  });

  it("horizonte curto: dezembro sem saldo → usa o último mês projetado e o nomeia", () => {
    // month_end só vai até setembro (mês 9).
    const monthEnd: MonthEnd[] = REAL_2026.filter((r) => r.m <= 9).map((r) => ({
      year: 2026,
      month: r.m,
      balance_cents: r.end,
    }));
    const v = buildAnoView(realInput({ monthEnd }));
    expect(v.endMonth).toBe(9);
    expect(v.endBalanceCents).toBe(1825323);
    // só o suspeito de setembro entra no cenário típico (≤ endMonth)
    expect(Number.isFinite(v.endBalanceTypicalCents!)).toBe(true);
  });

  it("ano fechado sem futuro: sem divisão por zero em 'por mês'", () => {
    const months = REAL_2026.map((r) => monthMetric(r, 2025));
    const v = buildAnoView({
      year: 2025,
      today: "2026-07-15",
      months,
      monthEnd: REAL_2026.map((r) => ({
        year: 2025,
        month: r.m,
        balance_cents: r.end,
      })),
      reserveMonths: null,
    });
    expect(v.months.filter((x) => x.future)).toHaveLength(0);
    expect(v.perMonthShortfallCents).toBeNull();
  });
});

// --------------------------------------------------- renda ao longo dos anos --

describe("anoView — renda ao longo dos anos", () => {
  it("média por meses com registro: corrente conta vividos, passado conta meses com dado", () => {
    const y2025 = Array.from({ length: 12 }, (_, i) => {
      const filled = i + 1 >= 7; // preenchido a partir de julho: 6 meses
      return monthMetric(
        { m: i + 1, income: filled ? 1013923 : 0, perf: 0, eco: 0, end: 0 },
        2025,
      );
    });
    const y2026 = REAL_2026.map((r) => monthMetric(r));
    const rows = buildIncomeAcrossYears(
      [
        { year: 2025, months: y2025 },
        { year: 2026, months: y2026 },
      ],
      "2026-07-15",
    );
    const r25 = rows.find((r) => r.year === 2025)!;
    const r26 = rows.find((r) => r.year === 2026)!;
    expect(r25.recordedMonths).toBe(6);
    expect(r25.avgIncomeCents).toBe(1013923);
    expect(r26.recordedMonths).toBe(7); // só meses vividos no ano corrente
    expect(r26.avgIncomeCents).toBe(Math.round(8479388 / 7));
    expect(r26.savedPct).toBe(0);
  });
});
