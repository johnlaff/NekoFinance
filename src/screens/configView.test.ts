import { describe, it, expect } from "vitest";
import { greetState } from "./configView";

describe("greetState — pílula de estado do veredito", () => {
  it("verificando conexão", () => {
    expect(greetState("loading", 0, 0, null)).toEqual({
      tone: "muted",
      headline: "Verificando conexão…",
      detail: null,
    });
  });

  it("desconectado sem pendência", () => {
    expect(greetState("disconnected", 0, 0, null)).toEqual({
      tone: "warn",
      headline: "Desconectado",
      detail: null,
    });
  });

  it("desconectado com mudanças aguardando (a má notícia tem o mesmo peso)", () => {
    expect(greetState("disconnected", 3, 0, null)).toEqual({
      tone: "warn",
      headline: "Desconectado",
      detail: "3 mudanças aguardando",
    });
  });

  it("sessão expirada", () => {
    expect(greetState("expired", 0, 0, "há 2 h")).toEqual({
      tone: "warn",
      headline: "Sessão expirada",
      detail: null,
    });
  });

  it("sessão expirada com pendência", () => {
    expect(greetState("expired", 1, 0, null)).toEqual({
      tone: "warn",
      headline: "Sessão expirada",
      detail: "1 mudança aguardando",
    });
  });

  it("conectado com conflito bloqueando (pior estado ganha do pending)", () => {
    expect(greetState("connected", 5, 1, "há 2 min")).toEqual({
      tone: "warn",
      headline: "Conectado",
      detail: "Conflito de importação a resolver",
    });
  });

  it("conectado com conflitos (plural)", () => {
    expect(greetState("connected", 0, 2, null)).toEqual({
      tone: "warn",
      headline: "Conectado",
      detail: "2 conflitos de importação a resolver",
    });
  });

  it("conectado com mudanças aguardando", () => {
    expect(greetState("connected", 2, 0, "há 5 min")).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "2 mudanças aguardando",
    });
  });

  it("conectado com 1 mudança (singular)", () => {
    expect(greetState("connected", 1, 0, null)).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "1 mudança aguardando",
    });
  });

  it("conectado e sincronizado", () => {
    expect(greetState("connected", 0, 0, "há 2 min")).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "Sincronizado há 2 min",
    });
  });

  it("conectado sem timestamp de sync", () => {
    expect(greetState("connected", 0, 0, null)).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: null,
    });
  });
});
