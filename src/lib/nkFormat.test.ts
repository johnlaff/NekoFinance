import { describe, expect, it } from "vitest";
import { saldoBand } from "./nkFormat";

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
