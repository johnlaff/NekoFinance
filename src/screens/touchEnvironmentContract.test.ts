/// <reference types="node" />

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// O WebView do Android pinta um halo azul de sistema em todo toque (default de plataforma,
// nunca aparece no desktop porque não há toque) e não conhece Esc — sem um contrato de
// fundação essas duas regressões voltam tela a tela, cada uma no seu jeito próprio de
// quebrar o Midnight Purr.
const APP_CSS = readFileSync(join(process.cwd(), "src", "App.css"), "utf8");

describe("contrato de toque da fundação (Android WebView)", () => {
  it("a raiz neutraliza o realce de toque do WebView", () => {
    expect(
      /html\s*{[^}]*-webkit-tap-highlight-color:\s*transparent/.test(APP_CSS),
    ).toBe(true);
  });

  it("o foco de teclado segue visível via :focus-visible (não regride)", () => {
    expect(
      /:focus-visible\s*{[^}]*box-shadow:\s*var\(--shadow-focus\)/.test(APP_CSS),
    ).toBe(true);
  });

  it("o hint de fechar do InfoPopover só promete Esc quando há teclado/mouse real", () => {
    const hasKeysHiddenByDefault = /\.nk-pop__hint--keys\s*{[^}]*display:\s*none/.test(
      APP_CSS,
    );
    const hasHoverFineOverride =
      /@media \(hover: hover\) and \(pointer: fine\)\s*{[^]*?\.nk-pop__hint--touch\s*{[^}]*display:\s*none[^]*?\.nk-pop__hint--keys\s*{[^}]*display:\s*inline/.test(
        APP_CSS,
      );
    expect(hasKeysHiddenByDefault).toBe(true);
    expect(hasHoverFineOverride).toBe(true);
  });
});
