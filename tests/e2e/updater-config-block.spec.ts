import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Bloco de Configurações (issue #383): mesma máquina de estados do convite calmo de
// launch (updater-invitation.spec.ts), sempre visível — versão, estado do update e a
// ação de checagem manual. Copy travada por asserção de texto — nunca por screenshot
// (docs/ui-standards.md, regra 38).
test.describe("Configurações — bloco de atualizações", () => {
  test("sem update: estado neutro, e Verificar agora reflete o resultado", async ({
    page,
  }) => {
    await mockTauri(page);
    await page.goto("/");
    await page.getByRole("button", { name: "Configurações", exact: false }).click();

    const card = page.locator('section[aria-labelledby="config-atualizacoes"]');
    await card.scrollIntoViewIfNeeded();
    await expect(card.getByText("Nenhuma atualização pendente")).toBeVisible();

    const checkButton = card.getByRole("button", { name: "Verificar agora" });
    await expect(checkButton).toBeEnabled();
    await checkButton.click();
    await expect(card.getByText("Nenhuma atualização pendente")).toBeVisible();
  });

  test("update disponível: o bloco reflete a mesma máquina do convite de launch", async ({
    page,
  }) => {
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
    // O convite de launch checa em background — espera ele aparecer antes de navegar,
    // confirmando que a máquina já resolveu "disponível" antes do bloco ser lido.
    await expect(page.getByText("Atualização disponível")).toBeVisible();

    await page.getByRole("button", { name: "Configurações", exact: false }).click();
    const card = page.locator('section[aria-labelledby="config-atualizacoes"]');
    await card.scrollIntoViewIfNeeded();

    await expect(card.getByText(/Atualização disponível/)).toBeVisible();
    await expect(card.getByText(/v0\.2\.0/)).toBeVisible();
    // A ação some do convite (dispensável) mas continua alcançável aqui — mesma
    // frase do convite calmo (regra 4 do ui-standards: uma invitation por estado).
    await expect(card.getByRole("button", { name: "Baixar e instalar" })).toBeEnabled();
  });
});
