/// <reference types="node" />

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// O toggle "Animações" das Configurações vive num atributo de <html>, e media query não
// enxerga atributo: coreografia com duração hardcoded sob `prefers-reduced-motion` fica
// fora do alcance dele — desligar não desliga, ligar não restaura. O contrato é governar
// por token, que colapsa nos dois gatilhos. Sem este teste a regressão é invisível: a tela
// anima igual em todos os cenários que um teste de DOM sabe montar.
const SRC_DIR = join(process.cwd(), "src");
const SCREENS_DIR = join(SRC_DIR, "screens");

/** Passo do stagger que ainda cabe no orçamento: 5 × 40ms + 200ms de duração = 400ms. */
const MAX_STAGGER_STEPS = 5;

/** As telas mais o chrome compartilhado: coreografia também mora fora de `screens/`. */
function screenStylesheets() {
  const paths = [
    ...readdirSync(SCREENS_DIR)
      .filter((name) => name.endsWith(".css"))
      .map((name) => join(SCREENS_DIR, name)),
    join(SRC_DIR, "redesign.css"),
    join(SRC_DIR, "App.css"),
  ];
  return paths.map((path) => ({
    name: path.slice(SRC_DIR.length + 1),
    css: readFileSync(path, "utf8"),
  }));
}

/**
 * Declarações de animação/transição, achatadas — a busca ignora aninhamento porque a
 * coreografia costuma morar dentro de um bloco, e é justamente ela que interessa.
 */
function timingDeclarations(css: string) {
  const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
  return [
    ...withoutComments.matchAll(
      /\b(animation|animation-delay|animation-duration|transition|transition-delay|transition-duration)\s*:([^;}]*)/g,
    ),
  ].map((match) => `${match[1]}:${match[2]}`.replace(/\s+/g, " ").trim());
}

/**
 * Zero não precisa de token (0s dura o mesmo em qualquer modo), e indicador em laço também
 * não pode usá-lo: um giro `infinite` com duração 0ms não para, trava. Esses vivem pela regra
 * inversa — `animation: none` explícito, que precisa cobrir os DOIS gatilhos.
 */
function hasLiteralDuration(declaration: string) {
  if (/\binfinite\b/.test(declaration)) return false;
  return /(?<![\d.])(?!0(?:\.0+)?m?s\b)\d+(?:\.\d+)?m?s\b/.test(declaration);
}

describe("contrato de motion das telas", () => {
  // `no-preference` é o idioma que quebra o toggle: ele LIGA o movimento por media query, e
  // media query não enxerga atributo. `reduce` é legítimo para o que não colapsa por token
  // (giro infinito, transform de afordância) — desde que o kill por atributo venha junto.
  it("ninguém liga coreografia por media query", () => {
    const offenders = screenStylesheets()
      .filter(({ css }) => css.includes("prefers-reduced-motion: no-preference"))
      .map(({ name }) => name);

    expect(offenders).toEqual([]);
  });

  it("todo kill por media query tem o par que o toggle alcança", () => {
    const offenders = screenStylesheets()
      .filter(({ css }) => css.includes("prefers-reduced-motion"))
      .filter(({ css }) => !css.includes('[data-motion="off"]'))
      .map(({ name }) => name);

    expect(offenders).toEqual([]);
  });

  it("toda duração e todo atraso saem de um token --dur-*", () => {
    const offenders = screenStylesheets().flatMap(({ name, css }) =>
      timingDeclarations(css)
        .filter(hasLiteralDuration)
        .map((declaration) => `${name}: ${declaration}`),
    );

    expect(offenders).toEqual([]);
  });

  it("nenhum stagger passa do orçamento de entrada", () => {
    const offenders = screenStylesheets().flatMap(({ name, css }) =>
      [...css.matchAll(/var\(--dur-stagger-step\)\s*\*\s*(\d+)/g)]
        .filter((match) => Number(match[1]) > MAX_STAGGER_STEPS)
        .map((match) => `${name}: passo ${match[1]}`),
    );

    expect(offenders).toEqual([]);
  });
});
