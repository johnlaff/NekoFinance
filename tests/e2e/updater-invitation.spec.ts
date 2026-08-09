import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Convite calmo de atualização (App.tsx → UpdateInvitation): checagem em background no
// launch, convite só quando há update real. Copy travada por asserção de texto — nunca
// por screenshot (docs/ui-standards.md, regra 38).
test.describe("Convite de atualização no launch", () => {
  test("sem update disponível: nenhum convite aparece", async ({ page }) => {
    await mockTauri(page);
    await page.goto("/");
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    await expect(
      page.getByRole("status", { name: /Atualização disponível/ }),
    ).toHaveCount(0);
    await expect(page.getByText("Atualização disponível")).toHaveCount(0);
  });

  test("update disponível: convite calmo com versão e ações; recusar dispensa", async ({
    page,
  }, testInfo) => {
    await mockTauri(page, {
      "plugin:updater|check": {
        rid: 1,
        currentVersion: "0.1.0",
        version: "0.2.0",
        date: null,
        body: "Notas de teste.",
        rawJson: {},
      },
    });
    await page.goto("/");
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    const invite = page
      .getByRole("status")
      .filter({ hasText: "Atualização disponível" });
    await expect(invite).toBeVisible();
    await expect(invite.getByText(/v0\.2\.0/)).toBeVisible();
    await expect(
      invite.getByRole("button", { name: "Baixar e instalar" }),
    ).toBeVisible();
    await expect(invite.getByRole("button", { name: "Agora não" })).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("update-invitation.png"),
    });

    await invite.getByRole("button", { name: "Agora não" }).click();
    await expect(page.getByText("Atualização disponível")).toHaveCount(0);
  });

  test("sem espaço em disco: o convite nasce como aviso, sem ação de baixar", async ({
    page,
  }, testInfo) => {
    const MIB = 1024 * 1024;
    await mockTauri(page, {
      "plugin:updater|check": {
        rid: 1,
        currentVersion: "0.1.0",
        version: "0.2.0",
        date: null,
        body: null,
        rawJson: {},
      },
      // Faltam 109 MiB — o aviso arredonda o pedido para cima em degraus de 10 MiB.
      check_update_space: {
        ok: false,
        required_bytes: 112 * MIB,
        free_bytes: 3 * MIB,
        missing_bytes: 109 * MIB,
      },
    });
    await page.goto("/");
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    const invite = page
      .getByRole("alert")
      .filter({ hasText: "Sem espaço em disco para atualizar" });
    await expect(invite).toBeVisible();
    await expect(
      invite.getByText("Libere ~110,0 MB para instalar a v0.2.0."),
    ).toBeVisible();
    await expect(invite.getByRole("button", { name: "Baixar e instalar" })).toHaveCount(
      0,
    );
    await expect(invite.getByRole("button", { name: "Tentar de novo" })).toBeVisible();
    await expect(invite.getByText("Como funciona?")).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("update-blocked-space.png"),
    });
  });
});
