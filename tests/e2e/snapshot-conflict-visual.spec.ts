import { expect, test, type Page } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Tela de conflito do snapshot no Drive (ADR-0015): só monta de verdade quando o check-in normal
// recusa por `CHECKIN_REFUSED_CONFLICT` — um cenário de DOIS aparelhos publicando a partir da
// mesma base, fora do alcance de um smoke de aparelho único, e atrás de um `GOOGLE_CLIENT_ID`
// pinado vazio neste smoke (`playwright.config.ts`, de propósito: o baseline precisa renderizar
// igual com e sem `.env` local). A store (`snapshotConflictStore.ts`) é module-level e singleton
// no grafo do Vite — o import dinâmico abaixo pega a MESMA instância que `App.tsx` já montou (o
// dev server serve módulos fonte por caminho, o mesmo caminho que o bundle da página usa), então
// `openSnapshotConflict()` chamado daqui flui para o React já renderizado via
// `useSyncExternalStore`, sem precisar de nenhum hook de teste em produção.
async function openConflictScreen(page: Page) {
  await page.evaluate(async () => {
    // Caminho por VARIÁVEL, não literal: um literal levaria o `tsc` do smoke a tentar resolver
    // o módulo estaticamente (`tsconfig.playwright.json` não conhece o grafo do app) e falhar o
    // typecheck — só o browser, em runtime contra o dev server real, precisa resolver isto.
    const storePath = "/src/features/snapshot-conflict/snapshotConflictStore.ts";
    const mod = (await import(storePath)) as { openSnapshotConflict: () => void };
    mod.openSnapshotConflict();
  });
}

for (const theme of ["dark", "light"] as const) {
  test(`Conflito de sincronização entre aparelhos — ${theme}`, async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.addInitScript((t: string) => {
      localStorage.setItem("neko-theme", t);
    }, theme);
    await mockTauri(page);
    await page.goto("/");
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    await openConflictScreen(page);

    const dialog = page.getByRole("dialog", {
      name: "Conflito de sincronização entre aparelhos",
    });
    await expect(dialog).toBeVisible();
    // Os dois lados povoados (regra 7 do ui-standards: a lista mostra só import/write-back, a
    // copy declara o recorte) — a asserção de texto trava a copy; o screenshot trava o layout.
    await expect(
      dialog.getByText(/Escrita de volta na planilha \(aba Saídas\)/),
    ).toBeVisible();
    await expect(
      dialog.getByText(/Importação da planilha \(aba Cartão\)/),
    ).toBeVisible();
    await expect(
      dialog.getByRole("button", { name: "Manter este aparelho" }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("button", { name: "Usar o outro aparelho" }),
    ).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Decidir depois" })).toBeVisible();
    await page.waitForTimeout(200);

    if (theme === "dark") {
      await expect(dialog).toMatchAriaSnapshot({
        name: "snapshot-conflict.aria.yml",
      });
    }
    await expect(page).toHaveScreenshot(`snapshot-conflict-${theme}.png`, {
      fullPage: true,
    });
  });
}
