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
    await expect(page.getByText("Saldo projetado", { exact: true })).toBeVisible();
    // Pockets card (spec 007): grouped balances + net worth
    await expect(page.getByText("Bolsos & patrimônio")).toBeVisible();
    await expect(page.getByText("R$ 35.420,00")).toBeVisible();
    await expect(page.getByText("Pode gastar até")).toBeVisible();
    await expect(page.getByText(/Previsão diária — junho/)).toBeVisible();
    // Stats do herói: reserva + nº de lançamentos.
    await expect(page.getByText("Lançamentos")).toBeVisible();
    // Chained daily table: today marked, salary day visible
    await expect(page.getByRole("table").getByText("hoje").first()).toBeVisible();
    await expect(page.getByText("R$ 12.340,00").first()).toBeVisible();

    // Check-in diário: card com o disponível do dia e registro rápido.
    await expect(page.getByText("Check-in de hoje")).toBeVisible();
    await expect(page.getByText(/disponível/)).toBeVisible();
    await page.getByLabel("Gasto de hoje").fill("9,90");
    await page.getByRole("button", { name: "Registrar" }).click();
    // Campo limpa após registrar (o dashboard refaz a busca).
    await expect(page.getByLabel("Gasto de hoje")).toHaveValue("");

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
    // Multi-titular: o lançamento dividido mostra os OwnerChips dos titulares.
    const splitRow = page.getByRole("row", { name: /Café \+ mercado/ });
    await expect(splitRow.getByText("Gio")).toBeVisible();
    await expect(splitRow.getByText("João")).toBeVisible();
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

  test("método screens render: Totais, Horizonte, Tags", async ({ page }, testInfo) => {
    const nav = (name: string | RegExp) => page.getByRole("button", { name });

    // Totais — 4 métricas-herói + status do método.
    await nav("Totais").click();
    await expect(page.getByText("Performance", { exact: true })).toBeVisible();
    await expect(page.getByText("Custo de vida", { exact: true })).toBeVisible();
    await expect(page.getByText("Diário médio", { exact: true })).toBeVisible();
    await expect(page.getByText("Sobrou dinheiro")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("totais.png"),
    });

    // Horizonte — trajetória do saldo + detalhe diário.
    await nav("Horizonte").click();
    await expect(page.getByText(/Verde é folga, vermelho é aperto/)).toBeVisible();
    await expect(page.getByText("Detalhe diário")).toBeVisible();
    await expect(page.getByText("Junho")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("horizonte.png"),
    });

    // Visão anual — tabela das 4 métricas por mês.
    await nav("Anual").click();
    await expect(page.getByRole("heading", { name: "Visão anual" })).toBeVisible();
    await expect(page.getByText("Jun", { exact: true })).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("anual.png"),
    });

    // Tags — lista colorida com "! Pagar" no topo.
    await nav("Tags").click();
    await expect(page.getByText("! Pagar", { exact: true })).toBeVisible();
    await expect(page.getByText("Viagem", { exact: true })).toBeVisible();
    await expect(page.getByText("Delivery", { exact: true })).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("tags.png"),
    });
  });

  test("novo lançamento: abre o form, preenche e lança", async ({ page }) => {
    await page.getByRole("button", { name: "Transações" }).click();
    await page.getByRole("button", { name: "Novo lançamento" }).click();

    // Form visível com o seletor de tipo e os campos.
    await expect(page.getByText("Tipo de movimento")).toBeVisible();
    await page.getByLabel("Valor").fill("42,50");
    await page.getByLabel("Descrição", { exact: true }).fill("Almoço");
    // Anexa uma tag.
    await page.getByRole("button", { name: /Viagem/ }).click();
    await page.getByRole("button", { name: "Lançar" }).click();

    // Após lançar, o form fecha (botão volta a "Novo lançamento").
    await expect(page.getByRole("button", { name: "Novo lançamento" })).toBeVisible();
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

test.describe("onboarding de primeiro uso", () => {
  test("mostra os 5 passos e fecha ao concluir", async ({ page }, testInfo) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    // Onboarding ainda não feito → overlay aparece.
    await mockTauri(page, { get_app_setting: null });
    await page.goto("/");

    await expect(page.getByRole("dialog", { name: /Boas-vindas/ })).toBeVisible();
    await expect(page.getByText("Bem-vindo ao Neko")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("onboarding.png"),
    });

    // Avança pelos passos até "Começar".
    for (let i = 0; i < 4; i++) {
      await page.getByRole("button", { name: /Avançar/ }).click();
    }
    await expect(page.getByText("Sua meta de poupança")).toBeVisible();
    await page.getByRole("button", { name: /Começar/ }).click();

    // Overlay fecha; o app fica acessível.
    await expect(page.getByRole("dialog", { name: /Boas-vindas/ })).not.toBeVisible();
    await expect(page.getByText("Saldo projetado", { exact: true })).toBeVisible();
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
