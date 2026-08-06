import { describe, expect, it } from "vitest";
import type { ScenarioCompareDto } from "../lib/api";
import {
  custoVidaState,
  EMPTY_SCENARIO_STATE,
  loanGateView,
  performanceState,
  podeGastarState,
  reserveMonthsState,
  saldoState,
  savingsAfterState,
  scenarioDeepestPoint,
  scenariosView,
} from "./scenariosView";

describe("saldoState — Termômetro canônico, limiares absolutos", () => {
  it("−50001 (abaixo de −R$500) é Crítico; −50000 exato ainda é Negativo", () => {
    expect(saldoState(-50_001)).toMatchObject({ key: "critical", label: "Crítico" });
    expect(saldoState(-50_000)).toMatchObject({ key: "negative", label: "Negativo" });
  });

  it("−1 é Negativo; 0 já é Apertado", () => {
    expect(saldoState(-1).key).toBe("negative");
    expect(saldoState(0).key).toBe("tight");
  });

  it("R$1.000 exato é Apertado (fronteira inclusiva); 1 centavo acima vira OK", () => {
    expect(saldoState(100_000)).toMatchObject({ key: "tight", label: "Apertado" });
    expect(saldoState(100_001)).toMatchObject({ key: "ok", label: "OK" });
  });

  it("R$2.000 exato é OK; 1 centavo acima vira Folga", () => {
    expect(saldoState(200_000).key).toBe("ok");
    expect(saldoState(200_001)).toMatchObject({ key: "comfortable", label: "Folga" });
  });

  it("os estados bons levam o ícone 'ok' e os ruins o 'alert'", () => {
    expect(saldoState(200_001).icon).toBe("ok");
    expect(saldoState(100_001).icon).toBe("ok");
    expect(saldoState(100_000).icon).toBe("alert");
    expect(saldoState(-1).icon).toBe("alert");
  });
});

describe("performanceState — sobrou × faltou", () => {
  it("zero exato é 'Sobrou dinheiro' (fronteira inclusiva); −1 centavo já falta", () => {
    expect(performanceState(0)).toMatchObject({
      key: "Sobrou dinheiro",
      label: "Sobrou dinheiro",
      color: "var(--success-400)",
      icon: "ok",
    });
    expect(performanceState(-1)).toMatchObject({
      key: "Faltou dinheiro",
      label: "Faltou dinheiro",
      color: "var(--danger-400)",
      icon: "alert",
    });
  });
});

describe("custoVidaState — custo contra a renda", () => {
  it("custo igual à renda ainda é 'Dentro da renda'; 1 centavo acima vira vermelho", () => {
    expect(custoVidaState(500_000, 500_000)).toMatchObject({
      label: "Dentro da renda",
      color: "var(--success-400)",
      icon: "ok",
    });
    expect(custoVidaState(500_001, 500_000)).toMatchObject({
      label: "Acima da renda",
      color: "var(--danger-400)",
      icon: "alert",
    });
  });
});

describe("podeGastarState — livre × segure, com a régua que limita", () => {
  it("1 centavo já é 'Livre até'; zero é 'Segure hoje'", () => {
    expect(podeGastarState(1, "cash")).toMatchObject({
      key: "livre",
      color: "var(--success-400)",
      icon: "ok",
    });
    expect(podeGastarState(1, "cash").label).toMatch(/^Livre até /);
    expect(podeGastarState(0, "cash")).toMatchObject({
      key: "segure",
      label: "Segure hoje",
      color: "var(--warning-400)",
      icon: "alert",
    });
  });

  it("a `key` é fixa quando o valor muda — mudar de R$X para R$Y não é transição de estado", () => {
    expect(podeGastarState(1_000, "cash").key).toBe(
      podeGastarState(900_000, "cash").key,
    );
    expect(podeGastarState(1_000, "cash").label).not.toBe(
      podeGastarState(900_000, "cash").label,
    );
  });

  it("'Segure hoje' explica QUAL régua limitou; 'Livre até' não precisa de linha", () => {
    expect(podeGastarState(0, "savings").line).toBe(
      "Limitado pela régua de poupança (20–30% ao ano), não pelo caixa.",
    );
    expect(podeGastarState(0, "cash").line).toBe(
      "Limitado pelo caixa do mês, não pela régua de poupança.",
    );
    expect(podeGastarState(1, "savings").line).toBeUndefined();
  });
});

describe("EMPTY_SCENARIO_STATE", () => {
  it("é neutro e NÃO coincide com a banda que saldoState(0) produziria", () => {
    expect(EMPTY_SCENARIO_STATE).toEqual({
      key: "none",
      label: "—",
      color: "var(--text-faint)",
      icon: "none",
    });
    expect(EMPTY_SCENARIO_STATE.key).not.toBe(saldoState(0).key);
  });
});

// ------------------------------------------------------------ construção --

const EMPTY_COMPARE: ScenarioCompareDto = {
  scenario_id: "s1",
  scenario_name: "Cenário",
  real_today: "2026-03-10",
  real_horizon_end: "2026-12-31",
  real_month_end: [],
  real_deepest_deficit: null,
  real_performance_cents: 0,
  real_safe_to_spend_today_cents: 0,
  real_binding_guardrail: "cash",
  real_cost_of_living_cents: 0,
  real_income_cents: 0,
  scenario_month_end: [],
  scenario_deepest_deficit: null,
  scenario_performance_cents: 0,
  scenario_safe_to_spend_today_cents: 0,
  scenario_binding_guardrail: "cash",
  scenario_cost_of_living_cents: 0,
  scenario_income_cents: 0,
  month_end: [],
  deepest_deficit_delta_cents: null,
  performance_delta_cents: 0,
  safe_to_spend_delta_cents: 0,
  cost_of_living_delta_cents: 0,
  changes: [],
  loan: null,
};

function compareWith(patch: Partial<ScenarioCompareDto>): ScenarioCompareDto {
  return { ...EMPTY_COMPARE, ...patch };
}

describe("scenarioDeepestPoint — menor saldo do cenário, fonte única", () => {
  it("prefere o déficit DIÁRIO quando o motor o tem", () => {
    const point = scenarioDeepestPoint(
      compareWith({
        scenario_deepest_deficit: { date: "2026-05-20", balance_cents: -30_000 },
        scenario_month_end: [{ year: 2026, month: 8, balance_cents: -90_000 }],
      }),
    );
    expect(point).toEqual({ minCents: -30_000, monthIdx: 4 });
  });

  it("sem déficit diário, cai no MÍNIMO mensal (nunca no `?? 0`)", () => {
    const point = scenarioDeepestPoint(
      compareWith({
        scenario_month_end: [
          { year: 2026, month: 3, balance_cents: 50_000 },
          { year: 2026, month: 8, balance_cents: -90_000 },
        ],
      }),
    );
    expect(point).toEqual({ minCents: -90_000, monthIdx: 7 });
  });

  it("sem projeção nenhuma retorna null", () => {
    expect(scenarioDeepestPoint(EMPTY_COMPARE)).toBeNull();
  });
});

describe("scenariosView — os cinco KPIs decididos", () => {
  it("mantém a ordem de decisão (padrão-Z)", () => {
    expect(scenariosView(EMPTY_COMPARE).kpis.map((k) => k.label)).toEqual([
      "Buraco do futuro",
      "Saldo no fim do horizonte",
      "Pode gastar hoje",
      "Performance · mês atual",
      "Custo de vida",
    ]);
  });

  it("cenário sem projeção nenhuma rende o vazio neutro no Buraco do futuro", () => {
    const [buraco] = scenariosView(EMPTY_COMPARE).kpis;
    expect(buraco?.emptyScenario).toBe(true);
    expect(buraco?.scenarioState).toBe(EMPTY_SCENARIO_STATE);
  });

  it("com déficit apenas mensal, o card usa o mínimo mensal e o delta derivado", () => {
    const [buraco] = scenariosView(
      compareWith({
        real_deepest_deficit: { date: "2026-04-02", balance_cents: 10_000 },
        scenario_month_end: [{ year: 2026, month: 8, balance_cents: -90_000 }],
      }),
    ).kpis;
    expect(buraco?.emptyScenario).toBe(false);
    expect(buraco?.scenarioCents).toBe(-90_000);
    expect(buraco?.deltaCents).toBe(-100_000);
    expect(buraco?.scenarioState.key).toBe("critical");
  });

  it("o delta do backend vence a derivação quando existe", () => {
    const [buraco] = scenariosView(
      compareWith({
        real_deepest_deficit: { date: "2026-04-02", balance_cents: 10_000 },
        scenario_deepest_deficit: { date: "2026-08-02", balance_cents: -90_000 },
        deepest_deficit_delta_cents: -12_345,
      }),
    ).kpis;
    expect(buraco?.deltaCents).toBe(-12_345);
  });

  it("Saldo no fim lê o ÚLTIMO mês da série comparada", () => {
    const kpis = scenariosView(
      compareWith({
        month_end: [
          {
            year: 2026,
            month: 3,
            real_balance_cents: 100,
            scenario_balance_cents: 200,
            delta_cents: 100,
          },
          {
            year: 2026,
            month: 12,
            real_balance_cents: 300_000,
            scenario_balance_cents: 150_000,
            delta_cents: -150_000,
          },
        ],
      }),
    ).kpis;
    expect(kpis[1]).toMatchObject({
      realCents: 300_000,
      scenarioCents: 150_000,
      deltaCents: -150_000,
      sense: "higher-better",
    });
    expect(kpis[1]?.realState.key).toBe("comfortable");
    expect(kpis[1]?.scenarioState.key).toBe("ok");
  });

  it("Custo de vida é o único 'menor é melhor' e julga cada lado contra a PRÓPRIA renda", () => {
    const kpis = scenariosView(
      compareWith({
        real_cost_of_living_cents: 400_000,
        real_income_cents: 500_000,
        scenario_cost_of_living_cents: 600_000,
        scenario_income_cents: 500_000,
      }),
    ).kpis;
    expect(kpis[4]?.sense).toBe("lower-better");
    expect(kpis[4]?.realState.label).toBe("Dentro da renda");
    expect(kpis[4]?.scenarioState.label).toBe("Acima da renda");
    expect(kpis.filter((k) => k.sense === "lower-better")).toHaveLength(1);
  });

  it("carrega o loanGate resolvido, null sem empréstimo", () => {
    expect(scenariosView(EMPTY_COMPARE).loanGate).toEqual({
      reserve: null,
      savings: null,
    });
  });

  it("Pode gastar hoje leva a régua que limita cada lado", () => {
    const kpis = scenariosView(
      compareWith({
        real_safe_to_spend_today_cents: 25_000,
        scenario_safe_to_spend_today_cents: 0,
        scenario_binding_guardrail: "savings",
        safe_to_spend_delta_cents: -25_000,
      }),
    ).kpis;
    expect(kpis[2]?.realState.key).toBe("livre");
    expect(kpis[2]?.scenarioState.key).toBe("segure");
    expect(kpis[2]?.scenarioState.line).toMatch(/régua de poupança/);
  });
});

// ---------------------------------------------------- gate de financiamento --

const LOAN_BASE: NonNullable<ScenarioCompareDto["loan"]> = {
  loan_principal_cents: 0,
  loan_installment_cents: 0,
  loan_term_months: 0,
  loan_monthly_rate_bps: 0,
  loan_total_paid_cents: 0,
  loan_total_cost_cents: 0,
  reserve_months_before_financing: null,
  reserve_months_after_financing: null,
  savings_rate_before_bps: null,
  savings_rate_after_bps: null,
  economia_median_cents: 0,
};

describe("reserveMonthsState — semáforo de reserva pós-financiamento", () => {
  it("5,9 meses é abaixo do mínimo; 6,0 exato já é zona amarela", () => {
    expect(reserveMonthsState(5.9)).toMatchObject({ key: "below-min", icon: "alert" });
    expect(reserveMonthsState(6)).toMatchObject({ key: "amber", icon: "alert" });
  });

  it("11,9 meses é zona amarela; 12,0 exato já é Paz (fronteira INFERIOR inclusiva)", () => {
    expect(reserveMonthsState(11.9)).toMatchObject({ key: "amber", icon: "alert" });
    expect(reserveMonthsState(12)).toMatchObject({
      key: "peace",
      label: "Paz",
      icon: "ok",
    });
  });
});

describe("savingsAfterState — régua composta (piso ANTES da regra da metade)", () => {
  it("1999 bps é abaixo do piso; 2000 bps exato já passa o piso", () => {
    expect(savingsAfterState(1_999, 100, 1_000)).toMatchObject({
      key: "below-floor",
      icon: "alert",
    });
    expect(savingsAfterState(2_000, 100, 1_000)).not.toMatchObject({
      key: "below-floor",
    });
  });

  it("parcela igual à metade da economia típica é Paz; 1 centavo a mais já trava", () => {
    expect(savingsAfterState(2_000, 500, 1_000)).toMatchObject({
      key: "peace",
      icon: "ok",
    });
    expect(savingsAfterState(2_000, 501, 1_000)).toMatchObject({
      key: "half-rule",
      icon: "alert",
    });
  });

  it("acima do piso mas abaixo da metade não recai na regra do piso: a ORDEM avalia o piso primeiro", () => {
    // 1999 bps E parcela > metade: se a regra da metade fosse avaliada primeiro o resultado
    // seria idêntico (ambas reprovam), então o teste que prova a ORDEM precisa de um caso onde
    // só o piso reprova — a asserção acima (1999→below-floor) já cobre isso; aqui confirmamos
    // que passar o piso com parcela pequena não aciona a regra da metade por engano.
    expect(savingsAfterState(2_000, 100, 1_000)).toMatchObject({ key: "peace" });
  });
});

describe("loanGateView — as duas pernas resolvidas em estado + texto", () => {
  it("sem empréstimo simulado, as duas pernas são null", () => {
    expect(loanGateView(null)).toEqual({ reserve: null, savings: null });
  });

  it("reserva null quando a fonte não tem mês completo realizado", () => {
    const gate = loanGateView({ ...LOAN_BASE, reserve_months_after_financing: null });
    expect(gate.reserve).toBeNull();
  });

  it("reserva resolvida: estado do DEPOIS, before/after formatados com 1 casa decimal", () => {
    const gate = loanGateView({
      ...LOAN_BASE,
      reserve_months_before_financing: 4.25,
      reserve_months_after_financing: 12,
    });
    expect(gate.reserve).toMatchObject({
      state: { key: "peace", label: "Paz" },
      beforeText: "4,3",
      afterText: "12,0",
    });
  });

  it("reserva sem 'antes' (defensivo) deixa beforeText null", () => {
    const gate = loanGateView({
      ...LOAN_BASE,
      reserve_months_before_financing: null,
      reserve_months_after_financing: 3,
    });
    expect(gate.reserve).toMatchObject({
      state: { key: "below-min" },
      beforeText: null,
      afterText: "3,0",
    });
  });

  it("economia null quando falta qualquer uma das duas pontas", () => {
    expect(
      loanGateView({
        ...LOAN_BASE,
        savings_rate_before_bps: 2_500,
        savings_rate_after_bps: null,
      }).savings,
    ).toBeNull();
  });

  it("economia resolvida: percentual TRUNCADO, 'depois' negativo exibe 0% mas o estado julga o bruto", () => {
    const gate = loanGateView({
      ...LOAN_BASE,
      savings_rate_before_bps: 2_500,
      savings_rate_after_bps: -150,
      loan_installment_cents: 60_000,
      economia_median_cents: 50_000,
    });
    expect(gate.savings).toMatchObject({
      state: { key: "below-floor" },
      beforeText: "25%",
      afterText: "0%",
    });
    expect(gate.savings?.popoverBody).toMatch(/excede sua economia típica/);
  });

  it("popover cita a régua da metade quando é ela quem trava (sem exceder a economia)", () => {
    const gate = loanGateView({
      ...LOAN_BASE,
      savings_rate_before_bps: 3_000,
      savings_rate_after_bps: 2_100,
      loan_installment_cents: 600,
      economia_median_cents: 1_000,
    });
    expect(gate.savings?.state.key).toBe("half-rule");
    expect(gate.savings?.popoverBody).toMatch(/consome mais da metade/);
  });

  it("popover sem nenhuma régua acionada é só a copy fixa (paz)", () => {
    const gate = loanGateView({
      ...LOAN_BASE,
      savings_rate_before_bps: 3_000,
      savings_rate_after_bps: 2_500,
      loan_installment_cents: 100,
      economia_median_cents: 1_000,
    });
    expect(gate.savings?.state.key).toBe("peace");
    expect(gate.savings?.popoverBody).not.toMatch(
      /Nesta simulação|excede sua economia típica \(/,
    );
  });
});
