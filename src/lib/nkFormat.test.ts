import { describe, expect, it } from "vitest";
import { fmtCompactBRL, saldoBand } from "./nkFormat";

describe("fmtCompactBRL — manchete compacta pt-BR (mil/mi, nunca k/M)", () => {
  it.each([
    [120_000_000, "R$ 1,2 mi"], // R$1.200.000,00
    [100_000_000, "R$ 1,0 mi"], // fronteira exata do milhão
    [-120_000_000, "−R$ 1,2 mi"],
  ] as const)("%d centavos → %s (faixa milhão)", (cents, expected) => {
    expect(fmtCompactBRL(cents)).toBe(expected);
  });

  it.each([
    [3_080_000, "R$ 30,8 mil"], // R$30.800,00
    [1_000_000, "R$ 10,0 mil"], // fronteira exata dos R$10.000
    [-3_080_000, "−R$ 30,8 mil"],
  ] as const)("%d centavos → %s (faixa mil)", (cents, expected) => {
    expect(fmtCompactBRL(cents)).toBe(expected);
  });

  it.each([
    [763_200, "R$ 7.632"], // R$7.632,00 — reais inteiros, sem centavos
    [100_000, "R$ 1.000"], // fronteira exata dos R$1.000
    [-763_200, "−R$ 7.632"],
  ] as const)("%d centavos → %s (faixa reais inteiros)", (cents, expected) => {
    expect(fmtCompactBRL(cents)).toBe(expected);
  });

  it.each([
    [19_322, "R$ 193,22"], // tem centavo real → mantém
    [18_000, "R$ 180"], // centavo zero → sem ",00"
    [100, "R$ 1"],
    [0, "R$ 0"],
    [-18_000, "−R$ 180"],
    [-19_322, "−R$ 193,22"],
  ] as const)("%d centavos → %s (abaixo de R$1.000)", (cents, expected) => {
    expect(fmtCompactBRL(cents)).toBe(expected);
  });

  it("arredonda meio-para-cima (nunca bancário) nas faixas mil/mi", () => {
    // 1_225_000 / 10_000 = 122,5 exato — meio-para-cima vai para 123 (12,3), não 122 (par).
    expect(fmtCompactBRL(1_225_000)).toBe("R$ 12,3 mil");
    // 125_000_000 / 10_000_000 = 12,5 exato — meio-para-cima vai para 13 (1,3), não 12 (par).
    expect(fmtCompactBRL(125_000_000)).toBe("R$ 1,3 mi");
  });
});

describe("nkFormat.saldoBand — delega para o termômetro canônico (saldoHeatmap)", () => {
  // Fronteiras inclusivas da planilha (espelha saldoHeatmap.test.ts): R$ 1.000,00 exato
  // é "apertado" e R$ 2.000,00 exato é "ok" — prioridade da regra mais baixa.
  it.each([
    [250_000, "comfortable"],
    [200_001, "comfortable"],
    [200_000, "ok"], // R$ 2.000,00 exato → ok (≤ inclusivo)
    [199_999, "ok"],
    [100_001, "ok"],
    [100_000, "tight"], // R$ 1.000,00 exato → apertado (≤ inclusivo)
    [99_999, "tight"],
    [0, "tight"],
    [-1, "negative"],
    [-50_000, "negative"], // −R$ 500,00 exato → negativo (strict <)
    [-50_001, "critical"],
  ] as const)("saldo %d centavos → %s", (cents, key) => {
    expect(saldoBand(cents).key).toBe(key);
  });

  it("saldo nulo/indefinido → faixa 'none' sem cor de fundo", () => {
    expect(saldoBand(null).key).toBe("none");
    expect(saldoBand(undefined).key).toBe("none");
    expect(saldoBand(null).fill).toBe("transparent");
  });

  it("carrega fill (token do heatmap), cor de texto e rótulo pt-BR por faixa", () => {
    const tight = saldoBand(100_000);
    expect(tight.fill).toBe("var(--saldo-band-tight-fill)");
    expect(tight.text).toBe("var(--warning-400)");
    expect(tight.label).toBe("Apertado");

    const ok = saldoBand(200_000);
    expect(ok.fill).toBe("var(--saldo-band-ok-fill)");
    expect(ok.label).toBe("OK");
  });
});
