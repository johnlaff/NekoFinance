import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// ---------------------------------------------------------------------------
// Main suite — new redesign 2026 shell
// ---------------------------------------------------------------------------

test.describe("Neko Finance shell (mocked Tauri IPC)", () => {
  test.beforeEach(async ({ page }) => {
    // Relógio congelado na data das fixtures do mock (today: 2026-06-10): a visão mensal
    // default de Lançamentos filtra pelo mês corrente REAL — sem isto a suíte passava em
    // junho e quebrava em 1º de julho (time-bomb de relógio).
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
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
    // Saudação-veredito: o herói é a frase (relógio congelado às 12h ⇒ "Boa tarde.").
    await expect(page.getByRole("heading", { name: "Boa tarde." })).toBeVisible();
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    // Curadoria da assistente assina a ordem dos blocos.
    await expect(page.getByText(/A Mia separou o que importa hoje/)).toBeVisible();

    // Bloco do dia (modo débito do fixture): check-in do teto, sem registro inline.
    await expect(page.getByText("Gasto variável de hoje")).toBeVisible();
    await expect(page.getByText("Diário de hoje")).toBeVisible();
    await expect(page.getByRole("radiogroup")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Registrar", exact: true }),
    ).toHaveCount(0);

    // Insight do mês na voz da Mia (derivado da corrente de saldo do mock).
    await expect(page.getByLabel("Leitura da Mia")).toBeVisible();

    // Próximos movimentos (do mock UPCOMING_BILLS) e o par saldo + reserva.
    await expect(page.getByText("Próximos movimentos")).toBeVisible();
    await expect(page.getByText("Compromisso fixo demo")).toBeVisible();
    await expect(page.getByText("Saldo hoje", { exact: true })).toBeVisible();
    await expect(page.getByText("Reserva de emergência")).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("dashboard.png"),
    });
  });

  // -------------------------------------------------------------------------
  // (2) Sidebar nav — switches screens + aria-current
  // -------------------------------------------------------------------------

  test("sidebar nav is flat (no group headings) and switches screens with aria-current", async ({
    page,
  }, testInfo) => {
    // Nav plana: sem headers de grupo de admin
    await expect(page.getByText("Finanças", { exact: true })).toHaveCount(0);
    await expect(page.getByText("Sistema", { exact: true })).toHaveCount(0);

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
    await expect(page.getByRole("button", { name: "Nova tag" })).toBeVisible();

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
  // (3) CTA "Registrar lançamento" opens the Compose drawer
  // -------------------------------------------------------------------------

  test("Registrar lançamento opens Compose dialog; Cancelar closes it", async ({
    page,
  }, testInfo) => {
    // CTA primário da sidebar
    await page.getByRole("button", { name: /Registrar lançamento/ }).click();

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

  test("Lançamentos explode a nota em linhas de item, com a célula como autoridade", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Lançamentos", exact: true }).click();

    // Regressão do shell: vindo de uma tela COM large-title (Hoje), a appbar da
    // tela sem herói recupera o título — o `quiet` nunca fica preso.
    await expect(page.locator(".sh-appbar")).not.toHaveClass(/sh-appbar--quiet/);

    // Itens da nota são linhas de primeira classe; o contexto carrega a seção.
    await expect(
      page.getByRole("button", { name: /^Compra no crédito demo/ }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: /^Conta fixa demo/ })).toBeVisible();
    await expect(page.getByText("Saída — Total da célula").first()).toBeVisible();
    // Divergência célula×nota (t3: célula 125,00 × itens 100,00): selo no
    // cabeçalho + linha sintética de reconciliação — nunca um item.
    await expect(page.getByText("Com diferença")).toBeVisible();
    await expect(page.getByText("Diferença no detalhamento")).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("lancamentos-celula-nota.png"),
    });

    await page.setViewportSize({ width: 390, height: 840 });
    await expect(
      page.getByRole("button", { name: /^Compra no crédito demo/ }),
    ).toBeVisible();
    await expect(page.getByText("Com diferença")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("lancamentos-celula-nota-mobile.png"),
    });
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
    // Os títulos dos cards são headings; o termo também aparece no sufixo acessível do
    // gatilho "Como funciona?" e na equação da Performance — o heading é o âncora único.
    await expect(
      page.getByRole("heading", { name: "Performance", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Custo de vida", exact: true }),
    ).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("totais.png"),
    });
  });

  test("Calendário (YearGridScreen) renders the balance grid with the day agenda", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Calendário", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Calendário", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    // Veredito do mês → grade → o dia aberto, com o saldo como herói do painel.
    await expect(page.getByRole("grid", { name: /junho de 2026/i })).toBeVisible();
    await expect(
      page.getByRole("heading", { name: /^Junho afunda no dia 20/ }),
    ).toBeVisible();
    await expect(
      page.getByRole("complementary", { name: "O que marca o mês" }),
    ).toBeVisible();
    await expect(page.getByText("R$ 8.420,00").first()).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("calendario.png"),
    });
  });

  test("O ano (AnnualScreen) renders the verdict and the method ruler", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "O ano", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "O ano", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    // Nova direção: veredito → régua da faixa → os doze meses (sem tabela nem abas).
    // Âncora no heading: o texto também vive no rótulo de leitor de tela do "Como funciona?".
    await expect(
      page.getByRole("heading", { name: "A faixa do método" }),
    ).toBeVisible();
    await expect(page.getByRole("heading", { name: "Os doze meses" })).toBeVisible();
    // Abreviações dos meses nas linhas do ano.
    await expect(page.getByText("Jun", { exact: true }).first()).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("ano.png"),
    });
  });

  test("Tags (TagsScreen) shows the ruler exceptions and third-party money", async ({
    page,
  }, testInfo) => {
    await page.getByRole("button", { name: "Tags" }).click();
    await expect(page.getByText("Exceções", { exact: true })).toBeVisible();
    await expect(page.getByText("Trânsito", { exact: true })).toBeVisible();
    await expect(
      page.getByText("Dinheiro de terceiros", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText("Movimentação por rótulo", { exact: true }),
    ).toBeVisible();
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
    await expect(page.getByText("A estrada até dezembro")).toBeVisible();
    await expect(page.getByText("Os próximos 12 meses")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("horizonte.png"),
    });
  });

  // -------------------------------------------------------------------------
  // (5) Theme toggle
  // -------------------------------------------------------------------------

  test("theme toggle switches between dark and light", async ({ page }) => {
    await page.getByRole("button", { name: "Tema claro" }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await page.getByRole("button", { name: "Tema escuro" }).click();
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

    await page.getByRole("button", { name: "Tema claro" }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await page.getByRole("button", { name: "Tema escuro" }).click();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light");
  });
});
