import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Esc não existe em toque (issue #434): a instrução errada ensina o gesto errado. As duas
// variantes do hint vivem no DOM (regra 8 do ui-standards — nunca texto divergente por JS);
// quem decide qual aparece é a media query hover/pointer. jsdom não avalia essa media query
// de verdade, então só um Chromium real prova que o ambiente certo ganha a visibilidade.
test.describe("hint de fechar do InfoPopover por ambiente", () => {
  test("mouse + teclado (hover:hover, pointer:fine): mostra Esc, esconde o toque", async ({
    page,
  }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page);
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await page
      .getByRole("button", { name: "Como funciona? — veredito de hoje" })
      .click();
    const tip = page.getByRole("tooltip");
    await expect(tip.getByText("Esc para fechar")).toBeVisible();
    await expect(tip.getByText("Toque fora para fechar")).toBeHidden();

    // Não regride: Esc segue fechando o popover onde há teclado.
    await page.keyboard.press("Escape");
    await expect(page.getByRole("tooltip")).toHaveCount(0);
  });

  test("touch (hover:none, pointer:coarse): mostra toque fora, esconde Esc", async ({
    browser,
  }) => {
    const context = await browser.newContext({
      hasTouch: true,
      isMobile: true,
      viewport: { width: 390, height: 844 },
    });
    const page = await context.newPage();
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page);
    await page.goto("/");

    await page
      .getByRole("button", { name: "Como funciona? — veredito de hoje" })
      .tap();
    const tip = page.getByRole("tooltip");
    await expect(tip.getByText("Toque fora para fechar")).toBeVisible();
    await expect(tip.getByText("Esc para fechar")).toBeHidden();

    await context.close();
  });
});
