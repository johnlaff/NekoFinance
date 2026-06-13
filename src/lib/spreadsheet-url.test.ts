import { describe, expect, it } from "vitest";
import { extractSpreadsheetId } from "./spreadsheet-url";

describe("extractSpreadsheetId", () => {
  const id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";

  it("extrai o id da URL completa de edição", () => {
    expect(
      extractSpreadsheetId(`https://docs.google.com/spreadsheets/d/${id}/edit#gid=0`),
    ).toBe(id);
  });

  it("extrai de URLs com /u/<n>/ (multi-conta) e sem sufixo", () => {
    expect(
      extractSpreadsheetId(`https://docs.google.com/spreadsheets/u/1/d/${id}`),
    ).toBe(id);
  });

  it("aceita o id puro colado", () => {
    expect(extractSpreadsheetId(`  ${id}  `)).toBe(id);
  });

  it("rejeita texto que não é URL de Sheets nem id", () => {
    expect(extractSpreadsheetId("")).toBeNull();
    expect(extractSpreadsheetId("minha planilha")).toBeNull();
    expect(
      extractSpreadsheetId("https://docs.google.com/document/d/abc123"),
    ).toBeNull();
    expect(extractSpreadsheetId("abc")).toBeNull();
  });
});
