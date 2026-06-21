import { describe, it, expect } from "vitest";
import {
  saldoBand,
  SALDO_BAND_FILL,
  SALDO_BAND_LABEL,
  SALDO_BAND_LEGEND,
} from "./saldoHeatmap";

describe("saldoBand — termômetro com limiares absolutos da planilha", () => {
  it.each([
    // > R$ 2.000 → verde forte (folga)
    [250_000, "comfortable"],
    [200_001, "comfortable"],
    // R$ 1.000–2.000 → verde claro (ok). R$ 2.000 exato cai em "ok" (prioridade da planilha).
    [200_000, "ok"],
    [150_000, "ok"],
    [100_001, "ok"],
    // R$ 0–1.000 → âmbar (apertado). R$ 1.000 exato cai em "apertado".
    [100_000, "tight"],
    [50_000, "tight"],
    [0, "tight"],
    // R$ 0 a −R$ 500,00 → vermelho claro (negativo). −R$500,00 EXATO = negativo (planilha: lessThan, não <=).
    [-1, "negative"],
    [-49_999, "negative"],
    [-50_000, "negative"], // boundary: −500,00 exato → negativo (strict <)
    // abaixo de −R$ 500 (strict) → vermelho forte (crítico)
    [-50_001, "critical"],
    [-60_000, "critical"],
  ] as const)("saldo %d centavos → %s", (cents, band) => {
    expect(saldoBand(cents)).toBe(band);
  });

  it("é absoluto: o mesmo saldo é sempre a mesma faixa, independente de escala", () => {
    // R$ 1.500 é sempre "ok" — não depende de baseline de gasto algum.
    expect(saldoBand(150_000)).toBe("ok");
    expect(saldoBand(150_000)).toBe("ok");
  });

  it("aceita limiares customizados (configurável por usuário no futuro)", () => {
    const t = { critical: -100_000, positive: 0, tight: 50_000, ok: 100_000 };
    expect(saldoBand(40_000, t)).toBe("tight");
    expect(saldoBand(120_000, t)).toBe("comfortable");
    // strict <: o limiar exato cai em "negativo"; só abaixo dele vira "crítico".
    expect(saldoBand(-100_000, t)).toBe("negative");
    expect(saldoBand(-100_001, t)).toBe("critical");
  });
});

describe("tabelas do heatmap", () => {
  it("cobre as cinco faixas em fill e label", () => {
    const bands = ["critical", "negative", "tight", "ok", "comfortable"] as const;
    for (const b of bands) {
      expect(SALDO_BAND_FILL[b]).toMatch(/var\(--saldo-band-/);
      expect(SALDO_BAND_LABEL[b]).toBeTruthy();
    }
  });

  it("a legenda vai de folga (verde) a crítico (vermelho)", () => {
    expect(SALDO_BAND_LEGEND.map((l) => l.band)).toEqual([
      "comfortable",
      "ok",
      "tight",
      "negative",
      "critical",
    ]);
  });
});
