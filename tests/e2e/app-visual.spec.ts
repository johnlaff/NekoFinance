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
        // O ano dispara 3 buscas anuais paralelas (renda ao longo dos anos); o rodapé só
        // aparece com ≥ 2 anos carregados — espera determinística contra o flash parcial.
        if (name === "O ano") {
          await page.getByText(/Ganhar mais não vira economia/).waitFor();
        }
        const slug = name.normalize("NFD").replace(/[^a-zA-Z]/g, "");
        await expect(page).toHaveScreenshot(`${slug}-${theme}.png`, {
          fullPage: true,
          maxDiffPixelRatio: 0.02,
        });
      });
    }
  });
}

// Estados epistêmicos + modo de gasto: as variantes da Hoje que os fixtures default não
// exercitam, e a tela do teto (fora da nav — alcançada pelo link de Configurações).
test.describe("teto do diário + estados de dado", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
  });

  test("tela Teto do diário (editor da cerimônia)", async ({ page }) => {
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    await page
      .getByRole("button", { name: "Configurações", exact: false })
      .first()
      .click();
    await page.getByRole("button", { name: "Abrir teto do diário" }).click();
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot("Teto-dark.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });

  test("Hoje no modo cartão: as faturas por vencimento são o corpo do bloco do dia", async ({
    page,
  }) => {
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      get_dashboard_summary: {
        balance: 842000,
        daily_budget: 4033,
        daily_ceiling_source: "chosen",
        ceiling_proposal_pending: false,
        daily_spend_today: 0,
        card_spend_today_cents: 15990,
        reserve_months: 4.5,
        reserve_state: "estimate",
        reserve_basis_months: 4,
        reserve_trend: "flat",
        spending_mode: "card",
        card_gate: "below",
        card_gate_economy: "below",
        card_gate_economy_bps: 1400,
        card_gate_reserve: "alive",
        cartao_month_cents: 260000,
        next_fatura_date: "2026-06-20",
        next_fatura_amount_cents: 140000,
        upcoming_invoices: [
          {
            account_id: "itau",
            card_name: "Itaú",
            due_date: "2026-06-20",
            amount_cents: 174739,
            status: "aberta",
            owner_name: "Eu",
            has_refund_expectation: false,
          },
          {
            account_id: "amazon",
            card_name: "Amazon",
            due_date: "2026-06-20",
            amount_cents: 19562,
            status: "fechada",
            owner_name: "Eu",
            has_refund_expectation: false,
          },
          {
            account_id: "gio",
            card_name: "Bradesco Gio",
            due_date: "2026-06-22",
            amount_cents: 98770,
            status: "aberta",
            owner_name: "Gio",
            has_refund_expectation: true,
          },
        ],
        transaction_count: 42,
        last_real_tx_date: "2026-06-09",
      },
      list_cards: [
        { id: "itau", name: "Itaú" },
        { id: "amazon", name: "Amazon" },
        { id: "gio", name: "Bradesco Gio" },
        { id: "inter", name: "Inter" },
        { id: "bb", name: "BB" },
      ],
    });
    await page.goto("/");
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot("Hoje-modo-cartao-dark.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });

  test("Hoje sem teto: travessão + proposta pendente (nunca R$ 0,00 fabricado)", async ({
    page,
  }) => {
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      get_dashboard_summary: {
        balance: 842000,
        daily_budget: 0,
        daily_ceiling_source: "none",
        ceiling_proposal_pending: true,
        daily_spend_today: 0,
        card_spend_today_cents: 0,
        reserve_months: 0,
        reserve_state: "no_record",
        reserve_basis_months: 0,
        reserve_trend: "flat",
        spending_mode: "debit",
        card_gate: "unknown",
        card_gate_economy: "unknown",
        card_gate_economy_bps: null,
        card_gate_reserve: "unknown",
        cartao_month_cents: 0,
        next_fatura_date: null,
        next_fatura_amount_cents: 0,
        upcoming_invoices: [],
        transaction_count: 2,
        last_real_tx_date: "2026-06-09",
      },
      get_ceiling_proposal_cmd: {
        id: "cp-1",
        per_day_cents: 4033,
        divisor_days: 31,
        source_month: "2026-05",
        items: [
          { name: "Alimentação", amount_cents: 100000 },
          { name: "Transporte", amount_cents: 25000 },
        ],
      },
    });
    await page.goto("/");
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot("Hoje-sem-teto-dark.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});

// Este mês no mobile: o bento colapsa em coluna única na ordem do DOM
// (régua → custo → performance → diário médio → série histórica).
for (const theme of ["dark", "light"] as const) {
  test(`Este mês — mobile ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Este mês", exact: false }).first().click();
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot(`mobile-mes-${theme}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
}

// O ano no mobile: os doze meses refluem (texto inteiro, barra decorativa em largura
// cheia embaixo) e a régua da faixa segue como instrumento herói.
for (const theme of ["dark", "light"] as const) {
  test(`O ano — mobile ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    // No mobile "O ano" vive no menu "Mais telas" (o dock tem 5 destinos fixos).
    await page.getByRole("button", { name: "Mais telas" }).click();
    await page.getByRole("button", { name: "O ano", exact: false }).first().click();
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot(`mobile-ano-${theme}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
}

// Lançamentos no mobile: a ergonomia própria do viewport — busca na zona do
// polegar (rodapé da lista) e filtro por tipo em bottom sheet.
for (const theme of ["dark", "light"] as const) {
  test.describe(`Lançamentos — mobile ${theme}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({ width: 390, height: 844 });
      await page.addInitScript((t: string) => {
        localStorage.setItem("neko-theme", t);
      }, theme);
      await mockTauri(page, {
        list_scenarios_cmd: [],
        list_scenario_transactions_cmd: [],
        list_obligations_cmd: [],
      });
      await page.goto("/");
      await page
        .getByRole("button", { name: "Lançamentos", exact: false })
        .first()
        .click();
      await page.waitForTimeout(350);
    });

    test("lista célula×nota com daymarks", async ({ page }) => {
      await expect(page).toHaveScreenshot(`mobile-lancamentos-${theme}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      });
    });

    test("filtro por tipo abre em bottom sheet", async ({ page }) => {
      await page.getByRole("button", { name: /Tipo:/ }).click();
      await expect(
        page.getByRole("dialog", { name: "Filtrar por tipo" }),
      ).toBeVisible();
      await page.waitForTimeout(200);
      await expect(page).toHaveScreenshot(`mobile-lancamentos-sheet-${theme}.png`, {
        maxDiffPixelRatio: 0.02,
      });
    });
  });
}

// Configurações: a porta "Gerenciar" guarda o painel denso da conexão (planilha,
// import) — o estado aberto é uma superfície própria que o baseline padrão não vê.
test("Configurações com a porta Gerenciar aberta", async ({ page }) => {
  await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await mockTauri(page, {
    list_scenarios_cmd: [],
    list_scenario_transactions_cmd: [],
    list_obligations_cmd: [],
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Configurações", exact: false })
    .first()
    .click();
  await page.getByRole("button", { name: "Gerenciar" }).click();
  await page.waitForTimeout(350);
  await expect(page).toHaveScreenshot("Configuracoes-gerenciar-dark.png", {
    fullPage: true,
    maxDiffPixelRatio: 0.02,
  });
});

// A rolagem do app vive na .sh-body (o fullPage não a alcança) — sem estes
// baselines, Aparência e Rotina nunca apareceriam em captura nenhuma.
for (const [label, width, height] of [
  ["", 1440, 1000],
  ["mobile-", 390, 844],
] as const) {
  test(`Configurações — ${label ? "mobile " : ""}seções finais (Aparência + Rotina)`, async ({
    page,
  }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width, height });
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    if (label) {
      await page.getByRole("button", { name: "Mais telas" }).click();
      await page.getByRole("button", { name: "Configurações", exact: false }).click();
    } else {
      await page
        .getByRole("button", { name: "Configurações", exact: false })
        .first()
        .click();
    }
    await page.waitForTimeout(350);
    // Screenshot de ELEMENTO: a posição da janela de rolagem não é
    // determinística entre máquinas (o scrollHeight varia por poucos px),
    // então o alvo é o card — nunca a viewport rolada até o fim.
    for (const section of ["aparencia", "rotina"] as const) {
      const card = page.locator(`section[aria-labelledby="config-${section}"]`);
      await card.scrollIntoViewIfNeeded();
      await page.waitForTimeout(200);
      await expect(card).toHaveScreenshot(`${label}config-${section}-dark.png`, {
        maxDiffPixelRatio: 0.02,
      });
    }
  });
}

// Tags no mobile: veredito como large-title, terceiros e exceções em coluna única,
// interruptores com alvo expandido — a tela inteira na ergonomia de polegar.
for (const theme of ["dark", "light"] as const) {
  test(`Tags — mobile ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    // No mobile a tela vive no menu "Mais telas" do dock.
    await page.getByRole("button", { name: "Mais telas" }).click();
    await page.getByRole("button", { name: "Tags", exact: false }).click();
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot(`mobile-tags-${theme}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
}

// Configurações no mobile: o segundo shell do desenho — appbar com blur, dock
// flutuante com FAB e o greet como large-title que silencia a appbar.
for (const theme of ["dark", "light"] as const) {
  test(`Configurações — mobile ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    await page.goto("/");
    // No mobile a tela vive no menu "Mais telas" do dock.
    await page.getByRole("button", { name: "Mais telas" }).click();
    await page.getByRole("button", { name: "Configurações", exact: false }).click();
    await page.waitForTimeout(350);
    await expect(page).toHaveScreenshot(`mobile-config-${theme}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
}

// Calendário no mobile: fallback deliberado — a grade é navegação (dia + saúde
// pelo termômetro) e os números moram na agenda do dia tocado, abaixo da grade.
for (const theme of ["dark", "light"] as const) {
  test.describe(`Calendário — mobile ${theme}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({ width: 390, height: 844 });
      await page.addInitScript((t: string) => {
        localStorage.setItem("neko-theme", t);
      }, theme);
      await mockTauri(page, {
        list_scenarios_cmd: [],
        list_scenario_transactions_cmd: [],
        list_obligations_cmd: [],
      });
      await page.goto("/");
      await page
        .getByRole("button", { name: "Calendário", exact: true })
        .first()
        .click();
      await page.waitForTimeout(350);
    });

    test("grade como navegação com a agenda de hoje", async ({ page }) => {
      await expect(page).toHaveScreenshot(`mobile-calendario-${theme}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      });
    });

    test("tocar um dia realizado move a agenda", async ({ page }) => {
      await page.getByRole("gridcell", { name: /^2 de junho/ }).click();
      await expect(
        page.getByRole("heading", { name: "Terça-feira, 2 de junho" }),
      ).toBeVisible();
      await page.waitForTimeout(200);
      await expect(page).toHaveScreenshot(`mobile-calendario-dia-${theme}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      });
    });
  });
}
