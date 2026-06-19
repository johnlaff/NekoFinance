import { describe, expect, it } from "vitest";
import { safeErrorMessage } from "./errors";

describe("safeErrorMessage", () => {
  it("maps a locked database to a friendly retry message", () => {
    expect(safeErrorMessage("database is locked")).toMatch(/ocupado/i);
    expect(safeErrorMessage(new Error("db locked: busy"))).toMatch(/ocupado/i);
  });

  it("maps auth/token errors to the connection-review message", () => {
    expect(safeErrorMessage("oauth token expired")).toMatch(/Configurações/i);
    expect(safeErrorMessage("unauthorized")).toMatch(/Configurações/i);
  });

  it("maps network errors to the connectivity message", () => {
    expect(safeErrorMessage("network request failed")).toMatch(/conexão/i);
    expect(safeErrorMessage("fetch timeout")).toMatch(/conexão/i);
  });

  it("falls back for unknown and empty errors", () => {
    expect(safeErrorMessage("boom")).toBe(
      "Não foi possível concluir a ação. Tente novamente.",
    );
    expect(safeErrorMessage(new Error(""))).toBe(
      "Não foi possível concluir a ação. Tente novamente.",
    );
    expect(safeErrorMessage(null, "custom fallback")).toBe("custom fallback");
  });

  it("reads Error, string, and primitive shapes", () => {
    expect(safeErrorMessage(42)).toBe(
      "Não foi possível concluir a ação. Tente novamente.",
    );
    expect(safeErrorMessage({ weird: true })).toBe(
      "Não foi possível concluir a ação. Tente novamente.",
    );
  });
});
