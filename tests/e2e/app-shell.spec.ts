import { expect, test } from "@playwright/test";

test.describe("Neko Finance shell", () => {
  test("renders the desktop dashboard and captures a visual smoke screenshot", async ({
    page,
  }, testInfo) => {
    await page.goto("/");

    await expect(page.getByText("Neko Finance").first()).toBeVisible();
    await expect(page.getByText(/Abra o app desktop/)).toBeVisible();

    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("desktop-home.png"),
    });
  });

  test("renders on mobile width without horizontal overflow", async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await expect(page.getByText("Neko Finance").first()).toBeVisible();
    await expect(page.getByText(/Abra o app desktop/)).toBeVisible();

    const hasHorizontalOverflow = await page.evaluate(
      () =>
        document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
    );

    expect(hasHorizontalOverflow).toBe(false);
    await page.screenshot({
      fullPage: true,
      path: testInfo.outputPath("mobile-home.png"),
    });
  });
});
