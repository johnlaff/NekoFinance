import { describe, it, expect } from "vitest";
import type { AnnualRuler, MonthMetric } from "../lib/api";
import { buildAnoView, buildIncomeAcrossYears, type AnoInput } from "./anoView";

// ---------------------------------------------------------------- fixtures --
// Planilha real de 2026 (a mesma que ancora o desenho aprovado do #200), em centavos, com a
// régua que o motor devolve sobre ela. Os números da régua são fato do motor (provados em
// `forecast::annual_ruler`); aqui eles são ENTRADA — o que está sob teste é a composição da tela.
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

/** Gasto típico do ano: mediana das saídas de jan–jul. */
const TIPICO = 1130981;

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
    savings_rate_bps: r.income > 0 ? Math.trunc((r.eco / r.income) * 10000) : 0,
  };
}

/**
 * A régua do motor sobre a planilha real, com hoje em 15/07/2026: sete meses vividos, gasto
 * típico de R$ 11.309,81 e setembro a dezembro sem lastro (agosto passa o piso de 60%).
 */
function realRuler(overrides: Partial<AnnualRuler> = {}): AnnualRuler {
  const outflow = (r: Row): number => r.income - r.perf;
  const suspects = [9, 10, 11, 12];
  return {
    year: 2026,
    lived_months: 7,
    future_months: 5,
    typical_spend_cents: TIPICO,
    income_lived_cents: 8479388,
    economia_lived_cents: 0,
    surplus_lived_cents: 242420,
    income_year_cents: 12452384,
    economia_year_cents: 0,
    recorded_months: 7,
    avg_income_cents: Math.trunc(8479388 / 7),
    lived_bps: 0,
    projected_bps: 0,
    bps: 0,
    scope_lived: true,
    has_data: true,
    shortfall_lived_cents: 1695878,
    shortfall_year_cents: 2490477,
    per_month_shortfall_cents: 498095,
    verdict: "below_band",
    band: { floor_bps: 2000, ceiling_bps: 3000 },
    months: REAL_2026.map((r) => ({
      month: r.m,
      outflow_cents: outflow(r),
      lived: r.m <= 7,
      suspect: suspects.includes(r.m),
      missing_cents: suspects.includes(r.m) ? TIPICO - outflow(r) : 0,
    })),
    month_end: REAL_2026.map((r) => ({
      year: 2026,
      month: r.m,
      balance_cents: r.end,
    })),
    year_end: {
      end_month: 12,
      end_balance_cents: 2997711,
      // 2.997.711 − 3.096.209 de silêncio nos quatro meses sem lastro.
      end_balance_typical_cents: -98498,
    },
    ...overrides,
  };
}

function realInput(overrides: Partial<AnoInput> = {}): AnoInput {
  return {
    year: 2026,
    today: "2026-07-15",
    months: REAL_2026.map((r) => monthMetric(r)),
    ruler: realRuler(),
    ...overrides,
  };
}

// --------------------------------------------------------- as doze linhas ----

describe("anoView — as doze linhas do ano", () => {
  it("costura as figuras de caixa do mês com a leitura do método", () => {
    const v = buildAnoView(realInput());

    const fev = v.months.find((m) => m.month === 2)!;
    expect(fev.income).toBe(1623670);
    expect(fev.economia).toBe(0);
    expect(fev.performance).toBe(492689);
    expect(fev.outflow).toBe(1130981); // do motor: renda − performance
    expect(fev.endBalance).toBe(1450038);
    expect(fev.lived).toBe(true);
    expect(fev.suspect).toBe(false);

    const set = v.months.find((m) => m.month === 9)!;
    expect(set.future).toBe(true);
    expect(set.suspect).toBe(true);
  });

  it("o mês corrente é o de hoje, e só ele", () => {
    const v = buildAnoView(realInput());
    expect(v.months.filter((m) => m.current).map((m) => m.month)).toEqual([7]);

    const passado = buildAnoView({
      ...realInput(),
      year: 2025,
      months: REAL_2026.map((r) => monthMetric(r, 2025)),
      ruler: realRuler({ year: 2025 }),
    });
    expect(passado.isCurrentYear).toBe(false);
    expect(passado.months.some((m) => m.current)).toBe(false);
  });

  it("mês futuro exibe savedPct null (— na tela), nunca 0%", () => {
    const v = buildAnoView(realInput());
    expect(v.months.find((m) => m.month === 12)!.savedPct).toBeNull();
    expect(v.months.find((m) => m.month === 1)!.savedPct).toBe(0);
  });

  it("mês sem saldo importado fica sem saldo, nunca com zero", () => {
    const v = buildAnoView(
      realInput({
        ruler: realRuler({
          month_end: REAL_2026.filter((r) => r.m <= 9).map((r) => ({
            year: 2026,
            month: r.m,
            balance_cents: r.end,
          })),
        }),
      }),
    );
    expect(v.months.find((m) => m.month === 9)!.endBalance).toBe(1825323);
    expect(v.months.find((m) => m.month === 10)!.endBalance).toBeNull();
  });

  it("mês ausente do motor entra como mês de zeros (a entrada pode vir esparsa)", () => {
    const v = buildAnoView(
      realInput({
        months: REAL_2026.filter((r) => r.m !== 3).map((r) => monthMetric(r)),
      }),
    );
    const mar = v.months.find((m) => m.month === 3)!;
    expect(v.months).toHaveLength(12);
    expect(mar.income).toBe(0);
    expect(mar.economia).toBe(0);
  });
});

// ------------------------------------------------------- a régua já pronta ---

describe("anoView — a régua chega pronta do motor", () => {
  it("o percentual que julga vem em pontos-base e a tela só o exibe", () => {
    const v = buildAnoView(realInput());
    expect(v.rulerPct).toBe(0);
    expect(v.rulerScopeLived).toBe(true);
    expect(v.estimate).toBe(true);

    const fechado = buildAnoView(
      realInput({
        ruler: realRuler({
          bps: 2537,
          lived_bps: 2537,
          projected_bps: 2537,
          scope_lived: false,
          verdict: "in_band",
          months: realRuler().months.map((m) => ({
            ...m,
            suspect: false,
            missing_cents: 0,
          })),
        }),
      }),
    );
    expect(fechado.rulerPct).toBeCloseTo(25.37, 5);
    expect(fechado.livedPct).toBeCloseTo(25.37, 5);
    expect(fechado.estimate).toBe(false);
    expect(fechado.verdict.kind).toBe("in_band");
  });

  it("sem renda para dividir, o percentual é nulo — nunca um zero que passaria por veredito", () => {
    const v = buildAnoView(
      realInput({
        ruler: realRuler({
          bps: null,
          lived_bps: null,
          projected_bps: null,
          has_data: false,
          verdict: "no_record",
        }),
      }),
    );
    expect(v.rulerPct).toBeNull();
    expect(v.livedPct).toBeNull();
    expect(v.hasData).toBe(false);
    expect(v.verdict.kind).toBe("no_record");
  });

  it("agregados, falta para os 20% e gasto típico saem da régua sem recomposição", () => {
    const v = buildAnoView(realInput());

    expect(v.livedCount).toBe(7);
    expect(v.futureCount).toBe(5);
    expect(v.incomeLived).toBe(8479388);
    expect(v.economiaLived).toBe(0);
    expect(v.surplusLived).toBe(242420);
    expect(v.incomeYear).toBe(12452384);
    expect(v.typicalSpendCents).toBe(TIPICO);
    expect(v.suspects).toEqual([9, 10, 11, 12]);
    expect(v.shortfallLivedCents).toBe(1695878);
    expect(v.shortfallYearCents).toBe(2490477);
    expect(v.perMonthShortfallCents).toBe(498095);
  });

  it("onde o ano termina e o cenário do gasto típico vêm decididos", () => {
    const v = buildAnoView(realInput());
    expect(v.endMonth).toBe(12);
    expect(v.endBalanceCents).toBe(2997711);
    expect(v.endBalanceTypicalCents).toBe(-98498);

    const semSuspeito = buildAnoView(
      realInput({
        ruler: realRuler({
          year_end: {
            end_month: 12,
            end_balance_cents: 2997711,
            end_balance_typical_cents: null,
          },
        }),
      }),
    );
    expect(semSuspeito.endBalanceTypicalCents).toBeNull();
  });

  it("ano sem saldo nenhum não monta o bloco do fim do ano", () => {
    const v = buildAnoView(
      realInput({
        ruler: realRuler({
          month_end: [],
          year_end: {
            end_month: null,
            end_balance_cents: null,
            end_balance_typical_cents: null,
          },
        }),
      }),
    );
    expect(v.endMonth).toBeNull();
    expect(v.endBalanceCents).toBeNull();
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
