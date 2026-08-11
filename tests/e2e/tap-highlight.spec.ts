import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// O WebView do Android pinta um halo azul de sistema em todo toque (issue #432); jsdom não
// tem motor de renderização para provar isso, então é o Chromium real que confirma o valor
// computado. -webkit-tap-highlight-color é inspecionável via getComputedStyle — "rgba(0, 0,
// 0, 0)" é como o Chromium reporta "transparent".
test("a raiz do app neutraliza o realce de toque nativo", async ({ page }) => {
  await mockTauri(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");

  const tapHighlight = await page.evaluate(
    () => getComputedStyle(document.documentElement).webkitTapHighlightColor,
  );
  expect(tapHighlight).toBe("rgba(0, 0, 0, 0)");
});
