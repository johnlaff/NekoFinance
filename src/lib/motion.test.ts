import { afterEach, describe, expect, it } from "vitest";
import {
  applyMotionPreference,
  motionEnabled,
  motionPreference,
  setMotionPreference,
} from "./motion";

// jsdom não tem matchMedia → systemPrefersReducedMotion() é false; o estado
// "system" comporta-se como movimento normal nestes testes.
describe("motion — preferência de animações em 3 estados", () => {
  afterEach(() => {
    localStorage.removeItem("neko-motion");
    document.documentElement.removeAttribute("data-motion");
  });

  it("default (nunca tocado) = system: sem atributo e animações seguem o SO", () => {
    applyMotionPreference();
    expect(motionPreference()).toBe("system");
    expect(document.documentElement.hasAttribute("data-motion")).toBe(false);
    expect(motionEnabled()).toBe(true);
  });

  it('"on" força: persiste, marca data-motion="on" e motionEnabled é true', () => {
    setMotionPreference("on");
    expect(localStorage.getItem("neko-motion")).toBe("on");
    expect(document.documentElement.getAttribute("data-motion")).toBe("on");
    expect(motionEnabled()).toBe(true);
  });

  it('"off" desliga: persiste, marca data-motion="off" e motionEnabled é false', () => {
    setMotionPreference("off");
    expect(localStorage.getItem("neko-motion")).toBe("off");
    expect(document.documentElement.getAttribute("data-motion")).toBe("off");
    expect(motionEnabled()).toBe(false);
  });

  it('voltar a "system" remove a chave e o atributo', () => {
    setMotionPreference("on");
    setMotionPreference("system");
    expect(localStorage.getItem("neko-motion")).toBeNull();
    expect(document.documentElement.hasAttribute("data-motion")).toBe(false);
  });

  it("valor legado/corrompido no storage cai em system (nunca quebra o boot)", () => {
    localStorage.setItem("neko-motion", "banana");
    applyMotionPreference();
    expect(motionPreference()).toBe("system");
    expect(document.documentElement.hasAttribute("data-motion")).toBe(false);
  });
});
