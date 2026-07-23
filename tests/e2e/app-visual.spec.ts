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
