import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Regressão visual da FUNDAÇÃO do DS: chrome mobile (appbar + dock flutuante)
// e paleta de acento — o que app-visual.spec (desktop, acento default) não vê.
// Para atualizar deliberadamente: `npx playwright test foundation-visual --update-snapshots`.

test.describe("fundação — chrome mobile", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
  });

  test("Hoje no mobile: appbar + dock com FAB, sidebar ausente", async ({ page }) => {
    await expect(
      page.getByRole("navigation", { name: "Navegação do app" }),
    ).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "Navegação principal" }),
    ).toBeHidden();
    await page.waitForTimeout(350);
    await expect(page.locator("body")).toMatchAriaSnapshot({
      name: "mobile-hoje.aria.yml",
    });
    await expect(page).toHaveScreenshot("mobile-hoje-dark.png");
  });

  // Regra 18 medida no breakpoint real, com a captura acima como evidência: o veredito e o
  // gasto do dia cabem na primeira tela. `toBeVisible` não serve de prova — ele não enxerga
  // rolagem nem oclusão; a geometria, sim.
  test("primeira tela mobile é do conteúdo primário (regra 18)", async ({ page }) => {
    await page.waitForTimeout(350);
    for (const name of ["Veredito de hoje", "Gasto variável de hoje"]) {
      const box = await page.getByRole("region", { name }).boundingBox();
      expect(box, name).not.toBeNull();
      expect(box!.y + box!.height, name).toBeLessThanOrEqual(844);
    }
  });

  test("dock encolhe ao rolar para baixo e volta ao subir (scroll vive no .sh-body)", async ({
    page,
  }) => {
    // Regressão: uma regra global legada devolvia o scroll ao documento em
    // <=680px, e o listener do dock (no .sh-body) nunca disparava.
    const contract = await page.evaluate(() => ({
      htmlOverflow: getComputedStyle(document.documentElement).overflow,
      bodyScrollable: (() => {
        const b = document.querySelector(".sh-body");
        return b ? b.scrollHeight > b.clientHeight : null;
      })(),
    }));
    expect(contract.htmlOverflow).toBe("hidden");
    expect(contract.bodyScrollable).toBe(true);

    const dock = page.locator(".sh-dock");
    await page.evaluate(() => {
      document.querySelector(".sh-body")?.scrollTo({ top: 300 });
    });
    await expect(dock).toHaveClass(/sh-dock--min/);
    await page.evaluate(() => {
      document.querySelector(".sh-body")?.scrollTo({ top: 0 });
    });
    await expect(dock).not.toHaveClass(/sh-dock--min/);
  });

  test("menu “mais” abre com os destinos fora do dock", async ({ page }) => {
    await page.getByRole("button", { name: "Mais telas" }).click();
    const menu = page.getByRole("group", { name: "Mais telas" });
    await expect(menu.getByRole("button", { name: "O ano" })).toBeVisible();
    await expect(menu.getByRole("button", { name: "Configurações" })).toBeVisible();
    await page.waitForTimeout(200);
    await expect(page.locator("body")).toMatchAriaSnapshot({
      name: "mobile-menu-mais.aria.yml",
    });
    await expect(page).toHaveScreenshot("mobile-menu-mais-dark.png");
  });
});

test.describe("fundação — paleta de acento", () => {
  test("acento lima pinta o chrome; status do método não muda", async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.addInitScript(() => {
      localStorage.setItem("neko-accent", "lima");
    });
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("data-accent", "lima");
    await page.waitForTimeout(350);
    await expect(page.locator("body")).toMatchAriaSnapshot({
      name: "desktop-hoje-lima.aria.yml",
    });
    await expect(page).toHaveScreenshot("desktop-hoje-lima.png", {
      fullPage: true,
    });
  });
});
