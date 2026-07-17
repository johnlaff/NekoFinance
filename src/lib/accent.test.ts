import { beforeEach, describe, expect, it } from "vitest";
import { ACCENT_KEY, ACCENTS, applyAccent, getStoredAccent } from "./accent";

describe("accent — persistência e aplicação no :root", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-accent");
  });

  it("default é jade (sem atributo no <html>)", () => {
    expect(getStoredAccent()).toBe("jade");
    applyAccent("jade");
    expect(document.documentElement.hasAttribute("data-accent")).toBe(false);
    expect(localStorage.getItem(ACCENT_KEY)).toBe("jade");
  });

  it("paleta não-default vira data-accent e persiste", () => {
    applyAccent("lima");
    expect(document.documentElement.getAttribute("data-accent")).toBe("lima");
    expect(getStoredAccent()).toBe("lima");
  });

  it("voltar para jade remove o atributo", () => {
    applyAccent("ceu");
    applyAccent("jade");
    expect(document.documentElement.hasAttribute("data-accent")).toBe(false);
  });

  it("valor desconhecido no storage cai para jade", () => {
    localStorage.setItem(ACCENT_KEY, "magenta");
    expect(getStoredAccent()).toBe("jade");
  });

  it("as 6 paletas do contrato existem, jade primeiro", () => {
    expect(ACCENTS.map((a) => a.key)).toEqual([
      "jade",
      "lima",
      "violeta",
      "ambar",
      "ceu",
      "rosa",
    ]);
  });
});
