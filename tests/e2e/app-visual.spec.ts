import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Regressão VISUAL de TODAS as telas, nos dois temas — baselines versionados.
// Complementa scenario-visual.spec.ts (que cobre os estados da superfície de cenários).
// Para atualizar deliberadamente: `npx playwright test app-visual --update-snapshots`.
const SCREENS = [
  "Hoje",
  "Lançamentos",
  "Este mês",
  "O ano",
  "Calendário",
  "Horizonte",
  "Tags",
  "Mia",
  "Configurações",
];

for (const theme of ["dark", "light"] as const) {
  test.describe(`telas — ${theme}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({ width: 1440, height: 1000 });
      await page.addInitScript((t: string) => {
        localStorage.setItem("neko-theme", t);
      }, theme);
      await mockTauri(page, {
        list_scenarios_cmd: [],
        list_scenario_transactions_cmd: [],
        list_obligations_cmd: [],
      });
      await page.goto("/");
    });

    for (const name of SCREENS) {
      test(`tela ${name}`, async ({ page }) => {
        await page.getByRole("button", { name, exact: false }).first().click();
        // Rede zero (mock in-page): um tick de layout basta para estabilizar.
        await page.waitForTimeout(350);
        const slug = name.normalize("NFD").replace(/[^a-zA-Z]/g, "");
        await expect(page).toHaveScreenshot(`${slug}-${theme}.png`, {
          fullPage: true,
          maxDiffPixelRatio: 0.02,
        });
      });
    }
  });
}
