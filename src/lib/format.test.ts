import { describe, expect, it } from "vitest";
import { fmtBRL, fmtDate } from "./format";

describe("fmtBRL", () => {
  it("formats cents as BRL with pt-BR grouping", () => {
    expect(fmtBRL(123456)).toBe("R$ 1.234,56");
  });

  it("formats zero", () => {
    expect(fmtBRL(0)).toBe("R$ 0,00");
  });

  it("formats negative amounts", () => {
    expect(fmtBRL(-5000)).toBe("-R$ 50,00");
  });

  it("keeps two decimal places for sub-real cents", () => {
    expect(fmtBRL(7)).toBe("R$ 0,07");
  });
});

describe("fmtDate", () => {
  it("converts ISO 8601 to DD/MM/YYYY", () => {
    expect(fmtDate("2026-03-15")).toBe("15/03/2026");
  });

  it("returns empty string for empty input", () => {
    expect(fmtDate("")).toBe("");
  });
});
