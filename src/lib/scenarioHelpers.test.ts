import { describe, expect, it } from "vitest";
import {
  CHART_LABEL_MIN_GAP,
  niceChartScale,
  parseLoanMarker,
  placeChartEndLabels,
} from "./scenarioHelpers";

// ---------------------------------------------------------------------------
// parseLoanMarker — o inverso do stripScenarioMarker para o marcador #loan: em
// vez de APAGAR o sufixo, INTERPRETA (groupId + taxa) para a UI agrupar as
// linhas de um mesmo empréstimo. Mesma âncora de fim do parser do backend.
// ---------------------------------------------------------------------------

describe("parseLoanMarker", () => {
  it("extrai groupId e rateBps do sufixo #loan", () => {
    expect(parseLoanMarker("Empréstimo #loan:abc-123:250")).toEqual({
      groupId: "abc-123",
      rateBps: 250,
    });
    expect(parseLoanMarker("Empréstimo parcela 3/12 #loan:g1:200")).toEqual({
      groupId: "g1",
      rateBps: 200,
    });
  });

  it("'#loan:' no MEIO do texto é dado do usuário — não é marcador", () => {
    expect(parseLoanMarker("Pagamento #loan:xyz do consórcio")).toBeNull();
  });

  it("descrição sem marcador (ou com #repl) rende null", () => {
    expect(parseLoanMarker("Aluguel")).toBeNull();
    expect(parseLoanMarker("Netflix #repl:ob1:2026-07-01")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// niceChartScale — domínio "nice" (ticks 1–2–5) para o gráfico de comparação.
// Regra do zero CONDICIONAL: o zero entra quando a série o cruza/toca ou quando
// ele cai a até um passo do domínio; longe disso o domínio é truncado (a variação
// entre as linhas é o que o gráfico existe para mostrar).
// ---------------------------------------------------------------------------

describe("niceChartScale", () => {
  it("domínio positivo longe do zero fica truncado (sem tick 0)", () => {
    // Saldos mensais reais do fixture e2e: R$ 25k–34k — zero a ~25k de distância.
    const vals = [2_500_000, 2_700_000, 3_084_059, 3_400_000];
    const s = niceChartScale(vals);
    expect(s.min).toBeGreaterThan(0);
    expect(s.min).toBeLessThanOrEqual(2_500_000);
    expect(s.max).toBeGreaterThanOrEqual(3_400_000);
    expect(s.ticks).not.toContain(0);
    expect(s.ticks.length).toBeGreaterThanOrEqual(2);
    expect(s.ticks.length).toBeLessThanOrEqual(6);
  });

  it("série cruzando o zero inclui o tick 0", () => {
    const vals = [-845_213, 1_200_000, 3_084_059];
    const s = niceChartScale(vals);
    expect(s.min).toBeLessThanOrEqual(-845_213);
    expect(s.max).toBeGreaterThanOrEqual(3_084_059);
    expect(s.ticks).toContain(0);
  });

  it("série toda negativa perto do zero estende até 0 (zero a ≤1 passo)", () => {
    const vals = [-50_000, -30_000, -14_059];
    const s = niceChartScale(vals);
    expect(s.max).toBe(0);
    expect(s.ticks).toContain(0);
  });

  it("série toda negativa LONGE do zero fica truncada (sem tick 0)", () => {
    const vals = [-3_400_000, -3_000_000, -2_500_000];
    const s = niceChartScale(vals);
    expect(s.max).toBeLessThan(0);
    expect(s.ticks).not.toContain(0);
  });

  it("ticks são múltiplos 1–2–5 de potência de 10 e igualmente espaçados", () => {
    const vals = [2_500_000, 3_400_000];
    const s = niceChartScale(vals);
    const step = s.ticks[1]! - s.ticks[0]!;
    const mantissa = step / 10 ** Math.floor(Math.log10(step));
    expect([1, 2, 2.5, 5]).toContainEqual(mantissa);
    for (let i = 1; i < s.ticks.length; i++) {
      expect(s.ticks[i]! - s.ticks[i - 1]!).toBe(step);
    }
    // Cada tick é múltiplo exato do passo (âncora em 0, não no min dos dados).
    for (const t of s.ticks) expect(t % step).toBe(0);
  });

  it("série constante rende faixa mínima em volta do valor (nunca range 0)", () => {
    const vals = [1_000_000, 1_000_000, 1_000_000];
    const s = niceChartScale(vals);
    expect(s.min).toBeLessThan(1_000_000);
    expect(s.max).toBeGreaterThan(1_000_000);
    expect(s.ticks.length).toBeGreaterThanOrEqual(2);
  });

  it("série constante em 0 ainda rende faixa válida com o zero dentro", () => {
    const s = niceChartScale([0, 0]);
    expect(s.min).toBeLessThanOrEqual(0);
    expect(s.max).toBeGreaterThanOrEqual(0);
    expect(s.ticks).toContain(0);
  });

  it("um único ponto funciona", () => {
    const s = niceChartScale([3_084_059]);
    expect(s.min).toBeLessThan(3_084_059);
    expect(s.max).toBeGreaterThan(3_084_059);
  });

  it("o domínio sempre contém todos os dados (nunca clipa uma linha)", () => {
    const cases = [
      [-845_213, 3_084_059],
      [2_500_000, 3_400_000],
      [-3_400_000, -2_500_000],
      [999_999, 1_000_001],
    ];
    for (const vals of cases) {
      const s = niceChartScale(vals);
      expect(s.min).toBeLessThanOrEqual(Math.min(...vals));
      expect(s.max).toBeGreaterThanOrEqual(Math.max(...vals));
    }
  });
});

// ---------------------------------------------------------------------------
// placeChartEndLabels — já existia sem teste dedicado neste módulo; cobre o
// contrato usado pelo DualLineChart (direction-aware + clamp do PAR).
// ---------------------------------------------------------------------------

describe("placeChartEndLabels", () => {
  it("linha mais alta ganha o rótulo acima; a outra, abaixo", () => {
    const p = placeChartEndLabels(40, 120, 10, 200);
    expect(p.realLabelY).toBeLessThan(p.scenarioLabelY);
    expect(p.scenarioLabelY - p.realLabelY).toBeGreaterThanOrEqual(CHART_LABEL_MIN_GAP);
  });

  it("linhas convergentes preservam o vão mínimo", () => {
    const p = placeChartEndLabels(100, 101, 10, 200);
    expect(Math.abs(p.scenarioLabelY - p.realLabelY)).toBeGreaterThanOrEqual(
      CHART_LABEL_MIN_GAP,
    );
  });

  it("perto do topo o PAR desce junto sem comprimir o vão", () => {
    const p = placeChartEndLabels(12, 10, 10, 200);
    expect(Math.min(p.realLabelY, p.scenarioLabelY)).toBeGreaterThanOrEqual(10);
    expect(Math.abs(p.scenarioLabelY - p.realLabelY)).toBeGreaterThanOrEqual(
      CHART_LABEL_MIN_GAP,
    );
  });
});
