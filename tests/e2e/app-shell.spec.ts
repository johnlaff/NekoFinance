import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

test.describe("Neko Finance shell (mocked Tauri IPC)", () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page);
    await page.goto("/");
  });

  test("dashboard renders the forecast-first reading surface", async ({
    page,
  }, testInfo) => {
    await expect(page.getByText("Saldo projetado")).toBeVisible();
    await expect(page.getByText(/Pode gastar até/)).toBeVisible();
    await expect(page.getByText(/Previsão diária — junho/)).toBeVisible();
    await expect(page.getByText("42 transações")).toBeVisible();
    // Chained daily table: today marked, salary day visible
    await expect(page.getByText("hoje", { exact: true })).toBeVisible();
    await expect(page.getByText("R$ 12.340,00").first()).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("dashboard.png"),
    });
  });

  test("sidebar navigation switches screens and marks the current item", async ({
    page,
  }, testInfo) => {
    const nav = (name: string | RegExp) => page.getByRole("button", { name });

    await nav("Transações").click();
    await expect(nav("Transações")).toHaveAttribute("aria-current", "page");
    await expect(page.getByText("Café + mercado")).toBeVisible();
    await expect(page.getByText("5 exibidas")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("transactions.png"),
    });

    await nav("Metodologia").click();
    await expect(page.getByText(/Previsibilidade primeiro/)).toBeVisible();
    await expect(page.getByText("Régua 1 e Régua 2")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("methodology.png"),
    });

    await nav("Configurações e privacidade").click();
    await expect(page.getByText(/app\.neko\.finance/)).toBeVisible();
    await expect(page.getByText("Importar arquivo local")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("settings.png"),
    });
  });

  test("dashboard hero button reaches the honest Mia placeholder", async ({ page }) => {
    await page
      .locator(".dash-hero")
      .getByRole("button", { name: "Perguntar à Mia" })
      .click();
    await expect(page.getByText("O que a Mia vai fazer")).toBeVisible();
    await expect(page.getByText("Em desenvolvimento")).toBeVisible();
  });

  test("transactions filter narrows by scope", async ({ page }) => {
    await page.getByRole("button", { name: "Transações" }).click();
    await page.getByRole("tab", { name: "Crédito" }).click();
    await expect(page.getByText("1 exibida")).toBeVisible();
    await expect(page.getByText("Assinatura streaming")).toBeVisible();
    await expect(page.getByText("Café + mercado")).not.toBeVisible();
  });

  test("ctrl/cmd+k focuses the header search", async ({ page }) => {
    await page.keyboard.press("ControlOrMeta+k");
    await expect(page.getByLabel("Buscar transações")).toBeFocused();
  });
});

test.describe("theme switch (View Transitions path, motion enabled)", () => {
  test("circular reveal lands on the light theme and back", async ({ page }) => {
    await mockTauri(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Alternar para tema claro" }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await page.getByRole("button", { name: "Alternar para tema escuro" }).click();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light");
  });
});
