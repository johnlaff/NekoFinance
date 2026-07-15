import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Regressão VISUAL da superfície de cenários (dark e light): compara screenshots com os
// baselines versionados em *-snapshots/. Cobre o que asserções de DOM não veem — colisão
// de rótulos nos SVGs, barras que não renderizam, ícone nativo invisível, tema quebrado.
// Para atualizar deliberadamente: `npx playwright test scenario-visual --update-snapshots`
// e revise o diff das imagens no PR como qualquer outra mudança.
//
// Mesmo shape de fixture do scenario-compare.spec.ts (sem importar de src/lib/api — ver a
// nota lá sobre import.meta.env × tsconfig.playwright.json).
const COMPARE = {
  scenario_id: "s1",
  scenario_name: "E se eu financiar um carro",
  real_today: "2026-06-10",
  real_horizon_end: "2026-12-31",
  real_month_end: [{ year: 2026, month: 12, balance_cents: 3_084_059 }],
  real_deepest_deficit: { date: "2026-08-01", balance_cents: 1_845_213 },
  real_performance_cents: 1_234_567,
  real_safe_to_spend_today_cents: 456_789,
  real_binding_guardrail: "cash",
  real_cost_of_living_cents: 3_084_059,
  real_income_cents: 4_000_000,
  scenario_month_end: [{ year: 2026, month: 12, balance_cents: 3_070_000 }],
  scenario_deepest_deficit: { date: "2026-08-01", balance_cents: -845_213 },
  scenario_performance_cents: 987_654,
  scenario_safe_to_spend_today_cents: 234_567,
  scenario_binding_guardrail: "cash",
  scenario_cost_of_living_cents: 2_845_213,
  scenario_income_cents: 3_800_000,
  month_end: [
    {
      year: 2026,
      month: 7,
      real_balance_cents: 3_000_000,
      scenario_balance_cents: 2_950_000,
      delta_cents: -50_000,
    },
    {
      year: 2026,
      month: 8,
      real_balance_cents: 3_100_000,
      scenario_balance_cents: 2_700_000,
      delta_cents: -400_000,
    },
    {
      year: 2026,
      month: 9,
      real_balance_cents: 3_200_000,
      scenario_balance_cents: 2_500_000,
      delta_cents: -700_000,
    },
    {
      year: 2026,
      month: 10,
      real_balance_cents: 3_300_000,
      scenario_balance_cents: 2_900_000,
      delta_cents: -400_000,
    },
    {
      year: 2026,
      month: 11,
      real_balance_cents: 3_400_000,
      scenario_balance_cents: 3_200_000,
      delta_cents: -200_000,
    },
    {
      year: 2026,
      month: 12,
      real_balance_cents: 3_084_059,
      scenario_balance_cents: 3_070_000,
      delta_cents: -14_059,
    },
  ],
  deepest_deficit_delta_cents: -2_690_426,
  performance_delta_cents: -246_913,
  safe_to_spend_delta_cents: -222_222,
  cost_of_living_delta_cents: -238_846,
  changes: [
    {
      op: "add",
      description: "Aluguel Carro",
      from_date: "2026-07-01",
      old_amount_cents: null,
      new_amount_cents: -250_000,
    },
  ],
  loan: {
    loan_total_cost_cents: 45_384,
    loan_total_paid_cents: 445_384,
    loan_installment_cents: 60_000,
    reserve_months_before_financing: 9.6,
    reserve_months_after_financing: 7.4,
    // Parcela (60_000) consome mais da metade da economia típica (100_000): a 2ª perna
    // sai amarela ("Mais da metade da sobra") mesmo com o pós-parcela acima do piso.
    savings_rate_before_bps: 3000,
    savings_rate_after_bps: 2400,
    economia_median_cents: 100_000,
  },
};

// Empréstimo como entidade: as linhas apontam para `loan_id` e os parâmetros vivem em
// `list_scenario_loans_cmd` (fonte do cabeçalho do grupo e do formulário de edição).
const LOAN_ENTITY = {
  id: "loan-abc",
  scenario_id: "s1",
  principal_cents: 40_000,
  rate_bps: 200,
  term_months: 12,
  disbursement_date: "2026-06-10",
  first_installment_date: "2026-07-09",
  description: "Empréstimo",
};

const LOAN_TXNS = [
  {
    id: "l0",
    type: "income",
    amount: 40_000,
    description: "Empréstimo",
    date: "2026-06-10",
    loan_id: "loan-abc",
  },
  ...Array.from({ length: 12 }, (_, i) => ({
    id: `p${i + 1}`,
    type: "expense" as const,
    amount: 3_782,
    description: `Empréstimo parcela ${i + 1}/12`,
    date: `${2026 + Math.floor((6 + i) / 12)}-${String(((6 + i) % 12) + 1).padStart(2, "0")}-09`,
    loan_id: "loan-abc",
  })),
];

for (const theme of ["dark", "light"] as const) {
  test(`superfície de comparação e sheet — ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page, {
      list_scenarios_cmd: [{ id: "s1", name: COMPARE.scenario_name, person_id: "p1" }],
      list_scenario_transactions_cmd: LOAN_TXNS,
      list_scenario_loans_cmd: [LOAN_ENTITY],
      list_obligations_cmd: [],
      list_recurrence_targets_cmd: [],
      get_scenario_forecast_cmd: COMPARE,
      price_installment_cmd: 94_560,
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Horizonte" }).click();
    await page.getByRole("button", { name: "Simular cenário" }).first().click();
    await page.getByRole("button", { name: COMPARE.scenario_name }).first().click();
    await page.getByText(`Cenário: ${COMPARE.scenario_name}`).waitFor();

    const compare = page.locator(".card", { hasText: "Cenário:" }).first();
    await expect(compare).toHaveScreenshot(`compare-${theme}.png`, {
      maxDiffPixelRatio: 0.02,
    });

    // O card inteiro é mais alto que o viewport (o element-screenshot acima não alcança o
    // resumo do empréstimo); este recorte cobre as duas réguas do gate de financiamento.
    await expect(page.locator(".scn-loan-summary")).toHaveScreenshot(
      `loan-summary-${theme}.png`,
      { maxDiffPixelRatio: 0.02 },
    );

    // Sheet com o grupo do empréstimo expandido (rótulos de parcela + coluna de valores).
    await page
      .getByRole("button", { name: /Empréstimo/ })
      .first()
      .click();
    await expect(page.locator(".scn-sheet")).toHaveScreenshot(`sheet-${theme}.png`, {
      maxDiffPixelRatio: 0.02,
    });

    const loanSection = page.getByRole("region", {
      name: "Dimensionar um empréstimo",
    });
    await loanSection.getByLabel("Valor").fill("10.000,00");
    await expect(loanSection.getByText("Total pago")).toBeVisible();
    await expect(loanSection).toHaveScreenshot(`loan-preview-${theme}.png`, {
      maxDiffPixelRatio: 0.02,
    });
  });
}

// Estados restantes da superfície (dark, o tema primário): o hover do gráfico com
// crosshair+tooltip, o sheet SEM cenário ativo (form inicial) e os outros dois níveis do
// veredito (ok/tight) — o fixture principal acima já cobre o nível de risco.
test("estados: hover do gráfico e sheet sem cenário — dark", async ({ page }) => {
  await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await mockTauri(page, {
    list_scenarios_cmd: [{ id: "s1", name: COMPARE.scenario_name, person_id: "p1" }],
    list_scenario_transactions_cmd: [],
    list_scenario_loans_cmd: [],
    list_obligations_cmd: [],
    list_recurrence_targets_cmd: [],
    get_scenario_forecast_cmd: COMPARE,
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Horizonte" }).click();
  await page.getByRole("button", { name: "Simular cenário" }).first().click();

  // Sheet aberto, nenhum cenário selecionado: o form de criação é o estado inicial.
  await expect(page.locator(".scn-sheet")).toHaveScreenshot("sheet-empty-dark.png", {
    maxDiffPixelRatio: 0.02,
  });

  await page.getByRole("button", { name: COMPARE.scenario_name }).first().click();
  await page.getByText(`Cenário: ${COMPARE.scenario_name}`).waitFor();

  // Hover no meio do plot: crosshair + tooltip (mês · real · simulação · Δ).
  const chart = page.locator("svg.scn-dualchart");
  const box = await chart.boundingBox();
  if (!box) throw new Error("gráfico sem bounding box");
  await page.mouse.move(box.x + box.width * 0.45, box.y + box.height * 0.5);
  await page.waitForTimeout(120);
  await expect(page.locator(".scn-dualchart-wrap").first()).toHaveScreenshot(
    "chart-hover-dark.png",
    { maxDiffPixelRatio: 0.02 },
  );
});

/** Variante do fixture com o menor saldo em outra banda do Termômetro (veredito ok/tight). */
function compareWithDeficit(balanceCents: number) {
  return {
    ...COMPARE,
    scenario_deepest_deficit: { date: "2026-08-01", balance_cents: balanceCents },
    deepest_deficit_delta_cents: balanceCents - 1_845_213,
  };
}

for (const [tier, cents] of [
  ["ok", 750_000],
  ["tight", 80_000],
] as const) {
  test(`veredito ${tier} — dark`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await mockTauri(page, {
      list_scenarios_cmd: [{ id: "s1", name: COMPARE.scenario_name, person_id: "p1" }],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      list_recurrence_targets_cmd: [],
      get_scenario_forecast_cmd: compareWithDeficit(cents),
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Horizonte" }).click();
    await page.getByRole("button", { name: "Simular cenário" }).first().click();
    await page.getByRole("button", { name: COMPARE.scenario_name }).first().click();
    await page.getByText(`Cenário: ${COMPARE.scenario_name}`).waitFor();
    await expect(page.locator(".scn-verdict")).toHaveScreenshot(
      `verdict-${tier}-dark.png`,
      { maxDiffPixelRatio: 0.02 },
    );
  });
}
