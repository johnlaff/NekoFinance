/// <reference types="node" />

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Caixa alta apaga a silhueta da palavra e cobra leitura letra a letra. Num rótulo, olho ou
// título de seção — que são contexto, não conteúdo — o custo não se paga, e micro-label
// uppercase é o idioma dos dashboards corporativos, anti-referência declarada. Abreviação
// convencionalmente maiúscula (dia da semana, mês) é o caso legítimo.
const SRC_DIR = join(process.cwd(), "src");
const SCREENS_DIR = join(SRC_DIR, "screens");

/** Seletores cujo conteúdo é abreviação, não rótulo. */
const ABBREVIATION_SELECTORS = [".calendario__dow span"];

/** As telas mais o chrome compartilhado: rótulo em caixa alta também vaza por fora delas. */
function stylesheets() {
  return [
    ...readdirSync(SCREENS_DIR)
      .filter((name) => name.endsWith(".css"))
      .map((name) => join(SCREENS_DIR, name)),
    join(SRC_DIR, "redesign.css"),
    join(SRC_DIR, "App.css"),
  ];
}

function upperCaseSelectors() {
  return stylesheets().flatMap((path) => {
    const css = readFileSync(path, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    return [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)]
      .filter((match) => /text-transform:\s*uppercase/.test(match[2] ?? ""))
      .map((match) =>
        (match[1] ?? "")
          .trim()
          .split(/\s*\n\s*/)
          .join(" "),
      );
  });
}

describe("contrato de caixa das telas", () => {
  it("uppercase sobrevive só onde o conteúdo é abreviação", () => {
    expect(upperCaseSelectors()).toEqual(ABBREVIATION_SELECTORS);
  });
});
