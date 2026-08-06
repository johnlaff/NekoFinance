import { describe, it, expect } from "vitest";
import type { MonthMetric } from "../lib/api";
import { FORECAST } from "../test/commands";
import {
  currentMonthMetric,
  custoVidaStatus,
  economizadoStatus,
  performanceStatus,
  SAVINGS_MIN_BPS,
  serieLeitura,
} from "./totaisView";

function monthFixture(overrides: Partial<MonthMetric>): MonthMetric {
  return { ...FORECAST.months[0]!, ...overrides };
}

describe("TotaisScreen — regras do método (puro)", () => {
  it("Performance: >=0 Sobrou, <0 Faltou", () => {
    expect(performanceStatus(100).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(0).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(-1).label).toBe("Faltou dinheiro");
    expect(performanceStatus(-1).level).toBe("risk");
  });

  it("Economizado: >=20% (2000bps) dentro do ideal", () => {
    expect(economizadoStatus(2500).label).toBe("Dentro do ideal");
    expect(economizadoStatus(2000).label).toBe("Dentro do ideal");
    expect(economizadoStatus(1900).label).toBe("Abaixo do ideal");
  });

  it("Economizado: zero tem nome próprio — Nada guardado", () => {
    expect(economizadoStatus(0).label).toBe("Nada guardado");
    expect(economizadoStatus(0).level).toBe("watch");
    // Guardou algo (mesmo 1 bps) → já não é "Nada guardado".
    expect(economizadoStatus(1).label).toBe("Abaixo do ideal");
  });

  it("Economizado: acima de 30% é Acima do ideal", () => {
    expect(economizadoStatus(3001).label).toBe("Acima do ideal");
    expect(economizadoStatus(3000).label).toBe("Dentro do ideal");
  });

  it("SAVINGS_MIN_BPS é a constante canônica de 20% (compartilhada entre as telas), travada contra o piso publicado pelo DTO", () => {
    expect(SAVINGS_MIN_BPS).toBe(FORECAST.savings_band.floor_bps);
    expect(economizadoStatus(SAVINGS_MIN_BPS).label).toBe("Dentro do ideal");
    expect(economizadoStatus(SAVINGS_MIN_BPS - 1).label).toBe("Abaixo do ideal");
  });

  it("Custo de vida: custo<=renda dentro da renda", () => {
    expect(custoVidaStatus(500, 1000).label).toBe("Dentro da renda");
    expect(custoVidaStatus(1200, 1000).label).toBe("Acima da renda");
  });

  it("currentMonthMetric acha o mês do `today`", () => {
    const months = FORECAST.months.filter((month) => month.month === 6);
    expect(currentMonthMetric(months, "2026-06-13")?.month).toBe(6);
    expect(currentMonthMetric(months, "2026-07-01")).toBeNull();
  });
});

describe("serieLeitura — a leitura diz o fato, nunca julga mês isolado", () => {
  it("sem meses anteriores não fabrica comparação", () => {
    expect(serieLeitura([monthFixture({ month: 6 })])).toBe(
      "Sem meses anteriores para comparar ainda.",
    );
  });

  it("todos zero: é o mesmo zero em todos, não uma queda", () => {
    const trend = [4, 5, 6].map((month) =>
      monthFixture({ month, savings_rate_bps: 0 }),
    );
    expect(serieLeitura(trend)).toBe(
      "O economizado está em zero nos últimos 3 meses — é o mesmo zero em todos, não uma queda.",
    );
  });

  it("caso geral: faixa da janela + melhor mês (percentuais truncados)", () => {
    const trend = [
      monthFixture({ month: 5, savings_rate_bps: 1590 }),
      monthFixture({ month: 6, savings_rate_bps: 2540 }),
    ];
    expect(serieLeitura(trend)).toBe(
      "Entre Mai e Jun, o economizado foi de 15% a 25% — o melhor mês foi Junho.",
    );
  });
});
