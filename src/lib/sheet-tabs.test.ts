import { describe, expect, it } from "vitest";
import { isMetricTab, isTransactionTab } from "./sheet-tabs";

describe("isMetricTab", () => {
  it("reconhece a aba Economia em qualquer caixa e com acentos", () => {
    expect(isMetricTab("Economia")).toBe(true);
    expect(isMetricTab("ECONOMIA")).toBe(true);
    expect(isMetricTab(" economia ")).toBe(true);
    expect(isMetricTab("Totais")).toBe(true);
  });

  it("não classifica abas-ano como métricas", () => {
    expect(isMetricTab("2025")).toBe(false);
    expect(isMetricTab("2026")).toBe(false);
  });

  it("não casa por prefixo (Economia Doméstica é outra aba)", () => {
    expect(isMetricTab("Economia Doméstica")).toBe(false);
  });
});

describe("isTransactionTab", () => {
  it("abas-ano são importáveis como transações", () => {
    expect(isTransactionTab("2026")).toBe(true);
  });

  it("Economia não é importável como transações", () => {
    expect(isTransactionTab("Economia")).toBe(false);
  });
});
