import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Side-sheet de cenários (plano 072, fatia C): guarda as regressões de layout/UX pegas em
// dogfooding — CTA colado no input (anel de foco por trás do botão), sheet modal cobrindo o
// compare, e campo de grid estourando a coluna.
test.describe("Side-sheet de cenários", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page, {
      list_scenarios_cmd: [
        { id: "s1", name: "Mudança de cidade", created_at: "", updated_at: "" },
      ],
      list_scenario_overrides_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      get_scenario_forecast_cmd: null,
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Horizonte" }).click();
    await page.getByRole("button", { name: "Simular cenário" }).first().click();
  });

  test("abre não-modal: o conteúdo reflui e continua operável ao lado", async ({
    page,
  }) => {
    const sheet = page.locator("dialog.scn-sheet");
    await expect(sheet).toBeVisible();
    // Não-modal: o Horizonte reflui (classe de reflow) e o conteúdo por trás segue clicável.
    await expect(page.locator(".hz.hz--sheet-open")).toBeVisible();
    await expect(page.getByText("Horizonte de saldos")).toBeVisible();
    // O foco entra no sheet ao abrir (show() não move o foco sozinho).
    await expect(page.locator("dialog.scn-sheet *:focus")).toHaveCount(1);
    // Escape fecha (gesto reposto à mão no dialog não-modal).
    await page.keyboard.press("Escape");
    await expect(sheet).toBeHidden();
    await expect(page.locator(".hz--sheet-open")).toHaveCount(0);
  });

  test("o CTA nunca encosta no campo (anel de foco visível)", async ({ page }) => {
    const input = page.locator("#scn-new-name");
    const btn = page.getByRole("button", { name: "Criar cenário" });
    const [ib, bb] = [await input.boundingBox(), await btn.boundingBox()];
    // Respiro mínimo de 8px entre a borda do input e o topo do botão — o anel de foco
    // (outline ~2-3px) precisa caber sem ficar por trás do CTA.
    expect(bb!.y - (ib!.y + ib!.height)).toBeGreaterThanOrEqual(8);
  });

  test("campos em grid não estouram a borda do sheet", async ({ page }) => {
    await page.getByRole("button", { name: "Mudança de cidade" }).first().click();
    const sheet = await page.locator("dialog.scn-sheet").boundingBox();
    const juros = page.locator(".scn-row3 input").last();
    await juros.scrollIntoViewIfNeeded();
    const jb = await juros.boundingBox();
    expect(jb!.x + jb!.width).toBeLessThanOrEqual(sheet!.x + sheet!.width);
  });
});
