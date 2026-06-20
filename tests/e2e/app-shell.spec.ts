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
    // O saldo projetado vive no cabeçalho do herói (o metric tile redundante foi removido).
    await expect(page.getByText(/Saldo no fim de/)).toBeVisible();
    // Pockets card (spec 007): grouped balances + net worth (patrimônio is a quiet footer)
    await expect(page.getByText("Bolsos", { exact: true })).toBeVisible();
    await expect(page.getByText("R$ 35.420,00")).toBeVisible();
    await expect(page.getByText("Pode gastar até")).toBeVisible();
    await expect(page.getByText(/Junho de 2026/)).toBeVisible();
    // Stats do herói: reserva + nº de lançamentos (escopado ao herói; "Lançamentos"
    // também é item de navegação).
    await expect(
      page.locator(".dash-hero__stats").getByText("Lançamentos"),
    ).toBeVisible();
    // Chained daily table: today marked, salary day visible
    await expect(page.getByRole("table").getByText("hoje").first()).toBeVisible();
    await expect(page.getByText("R$ 12.340,00").first()).toBeVisible();

    // Diário de hoje: card com o disponível do dia e registro rápido.
    await expect(page.locator("#dash-checkin-title")).toHaveText("Diário de hoje");
    await expect(page.getByText(/disponível/)).toBeVisible();
    await page.getByLabel("Gasto de hoje").fill("9,90");
    await page.getByRole("button", { name: "Registrar" }).click();
    // Campo limpa após registrar (o dashboard refaz a busca).
    await expect(page.getByLabel("Gasto de hoje")).toHaveValue("");

    // Rodapé do MonthLedgerCard fiel à planilha: linhas Saída Total e Resultado do mês.
    await expect(page.getByRole("row", { name: /Saída Total/ })).toBeVisible();
    await expect(
      page.getByRole("row", { name: /^Resultado do mês/ }).first(),
    ).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("dashboard.png"),
    });
  });

  test("sidebar navigation switches screens and marks the current item", async ({
    page,
  }, testInfo) => {
    const nav = (name: string | RegExp) => page.getByRole("button", { name });

    await nav("Lançamentos").click();
    await expect(nav("Lançamentos")).toHaveAttribute("aria-current", "page");
    await expect(page.getByText("Despesa demo variável")).toBeVisible();
    await expect(page.getByText("5 exibidas")).toBeVisible();
    // Multi-titular: o lançamento dividido mostra os OwnerChips dos titulares.
    const splitRow = page.getByRole("row", { name: /Despesa demo variável/ });
    await expect(splitRow.getByText("Pessoa A")).toBeVisible();
    await expect(splitRow.getByText("Pessoa B")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("transactions.png"),
    });

    // Metodologia foi rebaixada: chega-se a ela pelo item "Ajuda" em Sistema.
    await nav("Ajuda").click();
    await expect(page.getByText(/Previsibilidade primeiro/)).toBeVisible();
    await expect(page.getByText("Débito e crédito: dois ritmos")).toBeVisible();
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

    // Totais — 4 métricas-herói + status do método. Os rótulos são botões (InfoPopover).
    await nav("Totais").click();
    await expect(nav("Performance")).toBeVisible();
    await expect(nav("Custo de vida")).toBeVisible();
    await expect(nav("Diário médio")).toBeVisible();
    await expect(page.getByText("Sobrou dinheiro")).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("totais.png"),
    });

    // Horizonte — trajetória do saldo + detalhe diário.
    await nav("Horizonte").click();
    await expect(page.getByText(/quanto mais verde, mais folga/)).toBeVisible();
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
    await expect(page.getByText("Categoria demo A", { exact: true })).toBeVisible();
    await expect(page.getByText("Categoria demo B", { exact: true })).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("tags.png"),
    });
  });

  test("novo lançamento: abre o form, preenche e lança", async ({ page }) => {
    await page.getByRole("button", { name: "Lançamentos" }).click();
    await page.getByRole("button", { name: "Novo lançamento" }).click();

    // Form visível com o seletor de tipo e os campos.
    await expect(page.getByText("Tipo de movimento")).toBeVisible();
    await page.getByLabel("Valor").fill("42,50");
    await page.getByLabel("Descrição", { exact: true }).fill("Despesa demo");
    // Anexa uma tag.
    await page.getByRole("button", { name: /Categoria demo A/ }).click();
    await page.getByRole("button", { name: "Lançar" }).click();

    // Após lançar, o form fecha (botão volta a "Novo lançamento").
    await expect(page.getByRole("button", { name: "Novo lançamento" })).toBeVisible();
  });

  test("dashboard hero button reaches the honest Mia placeholder", async ({ page }) => {
    await page
      .locator(".dash-hero")
      .getByRole("button", { name: "Conhecer a Mia" })
      .click();
    await expect(page.getByText("O que a Mia vai fazer")).toBeVisible();
    await expect(page.getByText("Em desenvolvimento")).toBeVisible();
  });

  test("transactions filter narrows by scope", async ({ page }) => {
    await page.getByRole("button", { name: "Lançamentos" }).click();
    await page.getByRole("radio", { name: "Crédito" }).click();
    await expect(page.getByText("1 exibida")).toBeVisible();
    await expect(page.getByText("Compromisso demo no crédito")).toBeVisible();
    await expect(page.getByText("Despesa demo variável")).not.toBeVisible();
  });

  test("ctrl/cmd+k focuses the header search", async ({ page }) => {
    await page.keyboard.press("ControlOrMeta+k");
    await expect(page.getByLabel("Buscar lançamentos")).toBeFocused();
  });

  test("page has a main landmark, a labelled nav and the hero forecast region", async ({
    page,
  }) => {
    await expect(page.locator("main.ak-main")).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "Navegação principal" }),
    ).toBeVisible();
    // O metric tile foi removido; a região complementar do herói (saldo projetado) permanece nomeada.
    await expect(
      page.getByRole("complementary", { name: "Saldo projetado do mês" }),
    ).toBeVisible();
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
    await expect(page.getByText(/Saldo no fim de/)).toBeVisible();
  });
});

test.describe("write-back para a planilha (Tauri mockado, sem escrita real)", () => {
  // Comandos do fluxo de import (conectado → escolher → prévia → mapeamento) + do write-back. O mock
  // do Tauri NÃO faz IO real: `apply_write_back` aqui é um stub que conta como chamado, mas o teste
  // só chega ao diálogo de 2ª confirmação e CANCELA — provando que um clique não escreve sozinho.
  const WB_OVERRIDES = {
    check_auth_status: "connected",
    list_user_spreadsheets: [
      { id: "ss1", name: "Planilha demo", modified_time: "2026-06-20T10:00:00.000Z" },
    ],
    list_sheet_names: [{ title: "2026", sheet_id: 0 }],
    fetch_sheet_preview: {
      headers: ["Data", "Entrada", "Saída", "Diário", "Saldo"],
      rows: [["1", "0", "0", "50,00", "100,00"]],
      total_rows: 1,
    },
    detect_sheet_layout: {
      id: "lay1",
      sheet_name: "2026",
      year: 2026,
      month_names_row: 0,
      header_row: 1,
      data_start_row: 2,
      day_column: 0,
      block_size: 5,
      date_direction: "down",
    },
    get_sheet_mappings: [
      {
        id: "m1",
        sheet_name: "2026",
        column_letter: "D",
        column_header: "Diário",
        target_table: "transaction",
        target_field: "amount_daily",
        date_direction: "down",
        layout_id: "lay1",
        block_offset: 3,
        is_active: 1,
      },
    ],
    write_back_enabled: true,
    get_import_conflicts: [],
    preview_write_back_status: {
      cells: [
        {
          a1: "E3",
          row: 2,
          col: 4,
          date: "2026-01-01",
          kind: "diario",
          current: "50,00",
          proposed: "75,00",
          value_cents: 7500,
          changed: true,
        },
      ],
      preview_revision: "2026-06-20T10:00:00.000Z",
      conflicts_pending: false,
      multi_card_warning: false,
    },
    preview_economia_write_back_status: {
      cells: [],
      preview_revision: "2026-06-20T10:00:00.000Z",
      conflicts_pending: false,
      multi_card_warning: false,
    },
    apply_write_back: 1,
  };

  test("renderiza o diff e o diálogo de confirmação gera o envio (cancelar não escreve)", async ({
    page,
  }, testInfo) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockTauri(page, WB_OVERRIDES);
    await page.goto("/");

    await page.getByRole("button", { name: "Configurações e privacidade" }).click();

    // Conectado → escolhe a planilha → a aba-ano → detecta layout → chega ao mapeamento.
    await page.getByLabel("Planilha", { exact: true }).selectOption("ss1");
    await page.getByRole("button", { name: "2026" }).click();
    await page.getByRole("button", { name: "Detectar layout" }).click();

    // O painel de write-back aparece (flag habilitada).
    await expect(page.getByText("Write-back para a planilha")).toBeVisible();
    await expect(page.getByText("habilitado")).toBeVisible();

    // Gera a prévia → mostra a célula divergente.
    await page.getByRole("button", { name: "Gerar prévia do diff" }).click();
    await expect(page.getByText(/1 célula\(s\) divergente\(s\)/)).toBeVisible();
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("writeback-preview.png"),
    });

    // Aprovar abre o diálogo de 2ª confirmação — nada foi escrito ainda.
    await page.getByRole("button", { name: /Aprovar e enviar/ }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/Enviar 1 célula\(s\)/)).toBeVisible();

    // Cancelar fecha o diálogo sem enviar.
    await dialog.getByRole("button", { name: "Cancelar" }).click();
    await expect(page.getByRole("dialog")).not.toBeVisible();
    await expect(page.getByText(/Enviado:/)).not.toBeVisible();

    // Reabrir e confirmar mostra o resultado do envio (mock — sem rede real).
    await page.getByRole("button", { name: /Aprovar e enviar/ }).click();
    await page.getByRole("button", { name: "Confirmar envio" }).click();
    await expect(page.getByText(/Enviado: 1/)).toBeVisible();
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
