import { expect, test } from "@playwright/test";

// Smoke visual da tela Cartões no fallback web (dados demo): veredito + gate +
// card-face na lista, e o drill da fatura (herói, barras, séries, recorte).
// No desktop o drill é a coluna da direita; no mobile a lista drilla por
// estado ao tocar o card-face. A tela vive no menu "Mais" do dock.
test.describe("Cartões — fallback web", () => {
  test("desktop", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto("/");
    await page.getByRole("button", { name: "Cartões", exact: false }).first().click();
    await expect(page).toHaveScreenshot("cartoes-desktop.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
    // Um ciclo fechado troca o herói: status Fechada, sem linha de reconciliação.
    await page.getByRole("radio", { name: "Jul · Fechada" }).click();
    await expect(page).toHaveScreenshot("cartoes-drill-desktop.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });

  test("mobile", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");
    await page.getByRole("button", { name: "Mais telas" }).click();
    await page.getByRole("button", { name: "Cartões", exact: false }).first().click();
    await expect(page).toHaveScreenshot("cartoes-mobile.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
    await page.getByRole("button", { name: /cartão selecionado/ }).click();
    await expect(page.getByRole("button", { name: "Voltar" })).toBeVisible();
    await expect(page).toHaveScreenshot("cartoes-drill-mobile.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
