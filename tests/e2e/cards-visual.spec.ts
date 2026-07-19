import { expect, test } from "@playwright/test";

// Smoke visual da tela Cartões no fallback web (dados demo): lista com gate e
// proposta, e o drill da fatura aberto. No mobile a tela vive no menu "Mais".
test.describe("Cartões — fallback web", () => {
  for (const viewport of [
    { name: "desktop", width: 1440, height: 1000 },
    { name: "mobile", width: 390, height: 844 },
  ]) {
    test(`${viewport.name}`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({
        width: viewport.width,
        height: viewport.height,
      });
      await page.goto("/");
      if (viewport.name === "mobile") {
        await page.getByRole("button", { name: "Mais telas" }).click();
      }
      await page.getByRole("button", { name: "Cartões", exact: false }).first().click();
      await expect(page).toHaveScreenshot(`cartoes-${viewport.name}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      });
      await page
        .getByRole("button", { name: /Faturas/ })
        .first()
        .click();
      await expect(page).toHaveScreenshot(`cartoes-drill-${viewport.name}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      });
    });
  }
});
