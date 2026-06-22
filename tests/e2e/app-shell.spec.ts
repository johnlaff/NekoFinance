import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// ---------------------------------------------------------------------------
// Main suite — new redesign 2026 shell
// ---------------------------------------------------------------------------

test.describe("Neko Finance shell (mocked Tauri IPC)", () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page);
    await page.goto("/");
  });

  // -------------------------------------------------------------------------
  // (1) Hoje — redesign surface
  // -------------------------------------------------------------------------

  test("Hoje renders the redesign surface with mocked data", async ({
    page,
  }, testInfo) => {
    // Hero: "Pode gastar hoje" is the new headline
    await expect(page.getByText("Pode gastar hoje")).toBeVisible();

    // Forecast aside: "Saldo no fim de" month label
    await expect(page.getByText(/Saldo no fim de/)).toBeVisible();

    // Hero stats dl (Saldo hoje / Reserva / Teto diário)
    await expect(page.getByText("Saldo hoje", { exact: true })).toBeVisible();
    await expect(page.getByText("Reserva", { exact: true })).toBeVisible();
    await expect(page.getByText("Teto diário", { exact: true })).toBeVisible();

    // CheckinCard: "Check-in de hoje" card title
    await expect(page.getByText("Check-in de hoje")).toBeVisible();

    // Type chips: Diário / Cartão / Saída (only 3 in checkin, no Economia)
    const radiogroup = page.getByRole("radiogroup", { name: "Tipo de movimento" });
    await expect(radiogroup).toBeVisible();
    await expect(radiogroup.getByRole("radio", { name: /Diário/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await expect(radiogroup.getByRole("radio", { name: /Cartão/ })).toBeVisible();
    await expect(radiogroup.getByRole("radio", { name: /Saída/ })).toBeVisible();

    // Value input + Registrar button. exact:true so it matches the check-in's
    // "Valor" and not the always-mounted Compose dialog's "Valor único".
    await expect(page.getByLabel("Valor", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Registrar" })).toBeVisible();

    // UpcomingCard: "A pagar em breve" (from mock UPCOMING_BILLS)
    await expect(page.getByText("A pagar em breve")).toBeVisible();
    await expect(page.getByText("Compromisso fixo demo")).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("dashboard.png"),
    });
  });

  // -------------------------------------------------------------------------
  // (2) Sidebar nav — switches screens + aria-current
  // -------------------------------------------------------------------------

  test("sidebar nav groups Finanças / Sistema and switches screens with aria-current", async ({
    page,
  }, testInfo) => {
    // Check sidebar group headings
    await expect(page.getByText("Finanças", { exact: true })).toBeVisible();
    await expect(page.getByText("Sistema", { exact: true })).toBeVisible();

    // Default: Hoje is active
    await expect(page.getByRole("button", { name: "Hoje" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    // Navigate to Lançamentos
    await page.getByRole("button", { name: "Lançamentos", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Lançamentos", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    // The transactions screen renders transaction data from mock
    await expect(page.getByText("Despesa demo variável")).toBeVisible();

    // Navigate to Tags
    await page.getByRole("button", { name: "Tags" }).click();
    await expect(page.getByRole("button", { name: "Tags" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(page.getByText("! Pagar", { exact: true })).toBeVisible();

    // Navigate to Configurações (Sistema group)
    await page.getByRole("button", { name: "Configurações" }).click();
    await expect(page.getByRole("button", { name: "Configurações" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(page.getByText(/app\.neko\.finance/)).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("nav.png"),
    });
  });

  // -------------------------------------------------------------------------
  // (3) Lançar button opens the Compose drawer
  // -------------------------------------------------------------------------

  test("Lançar button opens Compose dialog; Cancelar closes it", async ({
    page,
  }, testInfo) => {
    // Topbar "Lançar" button
    await page.getByRole("button", { name: "Lançar" }).click();

    // Dialog opens with correct role + label
    const dialog = page.getByRole("dialog", { name: "Novo lançamento" });
    await expect(dialog).toBeVisible();

    // Contains "Tipo de movimento" chips
    await expect(dialog.getByText("Tipo de movimento")).toBeVisible();

    // Contains "Salvar lançamento" and "Cancelar"
    await expect(
      dialog.getByRole("button", { name: "Salvar lançamento" }),
    ).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Cancelar" })).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("compose.png"),
    });

    // Cancelar closes the dialog
    await dialog.getByRole("button", { name: "Cancelar" }).click();
    await expect(
      page.getByRole("dialog", { name: "Novo lançamento" }),
    ).not.toBeVisible();
  });

  test("tecla N opens the compose drawer from Hoje screen", async ({ page }) => {
    // Ensure focus is on body (not in an input)
    await page.locator("body").click({ position: { x: 5, y: 5 } });
    await page.keyboard.press("n");
    const dialog = page.getByRole("dialog", { name: "Novo lançamento" });
    await expect(dialog).toBeVisible();
  });

  test("tecla N opens the compose drawer from another screen", async ({ page }) => {
    // Navigate away first
    await page.getByRole("button", { name: "Lançamentos" }).click();
    await expect(page.getByText("Despesa demo variável")).toBeVisible();
    // Press N outside an input
    await page.locator("body").press("n");
    const dialog = page.getByRole("dialog", { name: "Novo lançamento" });
    await expect(dialog).toBeVisible();
  });

  // -------------------------------------------------------------------------
  // (4) Individual screens render correctly
  // -------------------------------------------------------------------------

  test("Este mês (TotaisScreen) renders performance metric", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Este mês" }).click();
    await expect(page.getByRole("button", { name: "Este mês" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // TotaisScreen renders "Performance" and "Custo de vida" tiles
    await expect(page.getByText("Performance")).toBeVisible();
    await expect(page.getByText("Custo de vida")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("totais.png"),
    });
  });

  test("Calendário (YearGridScreen) renders the month calendar", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Calendário" }).click();
    await expect(page.getByRole("button", { name: "Calendário" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // Calendar has segmented control with "Mês" / "Ano inteiro"
    await expect(page.getByText("Mês", { exact: true })).toBeVisible();
    await expect(page.getByText("Ano inteiro", { exact: true })).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("calendario.png"),
    });
  });

  test("O ano (AnnualScreen) renders the year view with month abbreviations", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "O ano", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "O ano", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    // Annual screen has "Este ano" / "Comparar anos" segmented tabs
    await expect(page.getByText("Este ano")).toBeVisible();
    await expect(page.getByText("Comparar anos")).toBeVisible();
    // Shows month abbreviations in the chart
    await expect(page.getByText("Jun", { exact: true }).first()).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("ano.png"),
    });
  });

  test("Tags (TagsScreen) renders tag list with demo tags", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Tags" }).click();
    await expect(page.getByText("! Pagar", { exact: true })).toBeVisible();
    await expect(page.getByText("Categoria demo A", { exact: true })).toBeVisible();
    await expect(page.getByText("Categoria demo B", { exact: true })).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("tags.png"),
    });
  });

  test("Horizonte (HorizonteScreen) renders the trajectory card", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Horizonte" }).click();
    await expect(page.getByRole("button", { name: "Horizonte" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(page.getByText("Horizonte de saldos")).toBeVisible();
    await expect(page.getByText("Trajetória até dezembro")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("horizonte.png"),
    });
  });

  // -------------------------------------------------------------------------
  // (5) Theme toggle
  // -------------------------------------------------------------------------

  test("theme toggle switches between dark and light", async ({ page }) => {
    await page.getByRole("button", { name: "Alternar para tema claro" }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await page.getByRole("button", { name: "Alternar para tema escuro" }).click();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light");
  });

  // -------------------------------------------------------------------------
  // (6) Structural landmarks
  // -------------------------------------------------------------------------

  test("page has a main landmark and a labelled nav", async ({ page }) => {
    await expect(page.locator("main.sh-main")).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "Navegação principal" }),
    ).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Onboarding — first-use flow
// ---------------------------------------------------------------------------

test.describe("onboarding de primeiro uso", () => {
  test("mostra os 5 passos e fecha ao concluir", async ({ page }, testInfo) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    // Onboarding not yet done → overlay appears (get_app_setting returns null)
    await mockTauri(page, { get_app_setting: null });
    await page.goto("/");

    await expect(
      page.getByRole("dialog", { name: /Boas-vindas ao Neko Finance/ }),
    ).toBeVisible();
    await expect(page.getByText("Bem-vindo ao Neko")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("onboarding.png"),
    });

    // Advance through 4 steps (total 5)
    for (let i = 0; i < 4; i++) {
      await page.getByRole("button", { name: /Avançar/ }).click();
    }
    await expect(page.getByText("Sua meta de poupança")).toBeVisible();
    await page.getByRole("button", { name: /Começar/ }).click();

    // Overlay closes; the app is accessible
    await expect(
      page.getByRole("dialog", { name: /Boas-vindas ao Neko Finance/ }),
    ).not.toBeVisible();
    // Dashboard visible
    await expect(page.getByText("Pode gastar hoje")).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Theme switch — View Transitions path, motion enabled
// ---------------------------------------------------------------------------

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
