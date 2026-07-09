import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Superfície de comparação (real × cenário) do Horizonte — plano 074, fatia A (P0 mecânico).
// Guarda os defeitos MEDIDOS em dogfooding: o card de KPI estourava (13px real + seta + 17px
// cenário nunca cabiam nos ~156px úteis de um card de 190px em magnitudes reais de 5 dígitos de
// reais), e os rótulos "Real"/"Simulação" do gráfico caíam em cima do próprio traço quando as
// duas linhas convergem no fim do horizonte.
//
// Sem tipo importado de `src/lib/api` de propósito: esse módulo usa `import.meta.env` (Vite),
// que o `tsconfig.playwright.json` não tipa (só tem `types: ["node"]") — importar de lá quebraria
// `npm run e2e:typecheck` puxando esse arquivo pro grafo de compilação. O shape aqui é só a
// estrutura que `ScenarioCompareDto` exige, verificada manualmente contra `src/lib/api.ts`.
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
  // Trajetória mensal: real e cenário DIVERGEM no meio do horizonte e CONVERGEM no fim — o
  // cenário exato que colidia os rótulos de fim de linha antes da fatia A.
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
  changes: [],
  loan: null,
};

test.describe("Superfície de comparação (real × cenário)", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1280, height: 900 });
    await mockTauri(page, {
      list_scenarios_cmd: [{ id: "s1", name: COMPARE.scenario_name, person_id: "p1" }],
      list_scenario_overrides_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      get_scenario_forecast_cmd: COMPARE,
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Horizonte" }).click();
    await page.getByRole("button", { name: "Simular cenário" }).first().click();
    await page.getByRole("button", { name: COMPARE.scenario_name }).first().click();
    await page.getByText(`Cenário: ${COMPARE.scenario_name}`).waitFor();
  });

  test("nenhum valor de KPI estoura o card em magnitudes reais de 5 dígitos, com o sheet aberto", async ({
    page,
  }) => {
    const cards = page.locator(".scn-kpi");
    const count = await cards.count();
    expect(count).toBe(5);

    for (let i = 0; i < count; i++) {
      const card = cards.nth(i);
      const cardBox = await card.boundingBox();
      expect(cardBox).not.toBeNull();

      // scrollWidth <= clientWidth: o motor de layout confirma que nada dentro do card força
      // uma largura maior que a da própria caixa (a assinatura exata do estouro medido).
      const noHorizontalOverflow = await card.evaluate(
        (el) => el.scrollWidth <= el.clientWidth + 1,
      );
      expect(noHorizontalOverflow).toBe(true);

      for (const selector of [
        ".scn-kpi__headline",
        ".scn-kpi__evidence",
        ".scn-kpi__delta",
      ]) {
        const child = card.locator(selector);
        if ((await child.count()) === 0) continue;
        const childBox = await child.boundingBox();
        expect(childBox).not.toBeNull();
        // Contenção horizontal: a borda direita do valor nunca passa da borda direita do
        // card (com 1px de folga para arredondamento de subpixel).
        expect(childBox!.x + childBox!.width).toBeLessThanOrEqual(
          cardBox!.x + cardBox!.width + 1,
        );
        expect(childBox!.x).toBeGreaterThanOrEqual(cardBox!.x - 1);
      }
    }
  });

  test("rótulos de fim de linha do gráfico ficam à direita do último ponto (sem colisão com o traço)", async ({
    page,
  }) => {
    const chart = page.locator("svg.scn-dualchart");
    await expect(chart).toBeVisible();

    // Lê os atributos SVG crus (unidades do viewBox) em vez de pixels de tela — a asserção
    // fica independente de escala/zoom do navegador.
    const lastPointX = await chart.evaluate((svg) => {
      const real = svg.querySelector(".scn-dualchart__real");
      const pts = real?.getAttribute("points")?.trim().split(" ") ?? [];
      const last = pts[pts.length - 1];
      return last ? Number(last.split(",")[0]) : NaN;
    });
    expect(Number.isFinite(lastPointX)).toBe(true);

    const labelXs = await chart.evaluate((svg) =>
      Array.from(svg.querySelectorAll(".scn-dualchart__label")).map((el) =>
        Number(el.getAttribute("x")),
      ),
    );
    expect(labelXs.length).toBe(2); // "Real" + "Simulação"
    for (const labelX of labelXs) {
      expect(labelX).toBeGreaterThan(lastPointX);
    }
  });

  // Plano 074, fatia B: o veredito (Nível 1) precisa aparecer ACIMA da grade de KPI, no
  // browser real, com ícone + palavra + cor — nunca só cor (o cenário mockado fura o caixa
  // em agosto, então o veredito é o ramo de risco).
  test("veredito (Nível 1) aparece acima da grade de KPI com ícone + palavra + cor", async ({
    page,
  }) => {
    const banner = page.locator(".scn-verdict");
    await expect(banner).toBeVisible();
    await expect(banner).toHaveClass(/scn-verdict--risk/);
    await expect(banner).toContainText("Fura o caixa");
    // Nunca só cor: um ícone (svg) acompanha a palavra dentro do banner.
    await expect(banner.locator("svg")).toBeVisible();

    const kpis = page.locator(".scn-kpis");
    const bannerBox = await banner.boundingBox();
    const kpisBox = await kpis.boundingBox();
    expect(bannerBox!.y).toBeLessThan(kpisBox!.y);
  });
});
