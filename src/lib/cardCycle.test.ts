import { describe, expect, it } from "vitest";
import cardCycleParity from "../../fixtures/card-cycle-parity.json";
import { shiftCycleMonth, validateCardCycle } from "./cardCycle";

describe("shiftCycleMonth", () => {
  it("atravessa a virada do ano nos dois sentidos", () => {
    expect(shiftCycleMonth("2026-12", 1)).toBe("2027-01");
    expect(shiftCycleMonth("2026-01", -1)).toBe("2025-12");
    expect(shiftCycleMonth("2026-06", 0)).toBe("2026-06");
  });

  it("devolve a entrada quando ela não é uma identidade mensal", () => {
    expect(shiftCycleMonth("junho", 1)).toBe("junho");
  });
});

describe("validateCardCycle", () => {
  it("aceita um cartão que fecha depois do dia 28", () => {
    // Fechar 29, 30 ou 31 é comum; o mês curto é problema de derivar a data, não do cadastro.
    expect(validateCardCycle("29", "12")).toBeNull();
    expect(validateCardCycle("31", "10")).toBeNull();
  });

  it("recusa dia fora do calendário", () => {
    expect(validateCardCycle("0", "10")).toMatch(/entre 1 e 31/);
    expect(validateCardCycle("32", "10")).toMatch(/entre 1 e 31/);
    expect(validateCardCycle("20", "32")).toMatch(/Vencimento/);
    expect(validateCardCycle("20", "0")).toMatch(/Vencimento/);
  });

  it("recusa o par que colidiria em fevereiro", () => {
    // No mesmo mês e acima do dia 28, fechamento e vencimento encurtam para o mesmo dia — um
    // ciclo que fecha no dia em que vence não existe.
    expect(validateCardCycle("28", "29")).toMatch(/fevereiro/);
    expect(validateCardCycle("29", "30")).toMatch(/fevereiro/);
  });

  it("aceita o fechamento alto quando o vencimento é do mês seguinte", () => {
    // Fecha 29, vence 12: o fechamento é do mês anterior ao vencimento, sem colisão possível.
    expect(validateCardCycle("29", "12")).toBeNull();
    expect(validateCardCycle("28", "28")).toBeNull();
  });
});

describe("validateCardCycle — paridade com o backend (fixtures/card-cycle-parity.json)", () => {
  // A mesma tabela é lida pelo teste Rust (card_cmds.rs); mudar um veredito aqui sem ajustar
  // validateCardCycle e validate_cycle quebra os dois lados.
  for (const { closing, due, verdict } of cardCycleParity.cases) {
    it(`fechamento=${closing} vencimento=${due} → ${verdict}`, () => {
      const result = validateCardCycle(String(closing), String(due));
      if (verdict === "ok") {
        expect(result).toBeNull();
      } else {
        expect(result).not.toBeNull();
      }
    });
  }
});
