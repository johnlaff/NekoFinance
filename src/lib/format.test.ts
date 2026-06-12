import { describe, expect, it } from "vitest";
import { fmtBRL, fmtDate, fmtDayMonth, monthNamePtBR, parseBRLToCents } from "./format";

describe("parseBRLToCents", () => {
  it("parses pt-BR formatted amounts into cents", () => {
    expect(parseBRLToCents("1.234,56")).toBe(123456);
    expect(parseBRLToCents("R$ 950")).toBe(95000);
    expect(parseBRLToCents("42,5")).toBe(4250);
    expect(parseBRLToCents("0")).toBe(0);
    expect(parseBRLToCents("-12,30")).toBe(-1230);
  });

  it("rejects garbage", () => {
    expect(parseBRLToCents("")).toBeNull();
    expect(parseBRLToCents("abc")).toBeNull();
    expect(parseBRLToCents("1,2,3")).toBeNull();
  });
});

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

describe("fmtDayMonth", () => {
  it("converts ISO 8601 to DD/MM", () => {
    expect(fmtDayMonth("2026-06-28")).toBe("28/06");
  });

  it("returns empty string for empty input", () => {
    expect(fmtDayMonth("")).toBe("");
  });
});

describe("monthNamePtBR", () => {
  it("returns the lower-case pt-BR month name", () => {
    expect(monthNamePtBR("2026-06-10")).toBe("junho");
    expect(monthNamePtBR("2026-03-01")).toBe("março");
  });

  it("returns empty string for malformed input", () => {
    expect(monthNamePtBR("")).toBe("");
  });
});
