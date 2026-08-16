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

/** Gera gestos determinísticos: os primeiros `groupRunLength` repetem o MESMO tipo (`import`,
 *  aba "2026") consecutivos — a corrida que a tela colapsa numa linha com contagem (issue #476) —
 *  e o resto varia de aba a cada gesto, então nunca agrupa e mantém a lista genuinamente alta. */
function manyGesturesFixture(count: number, groupRunLength: number) {
  const base = new Date("2026-06-10T06:00:00Z").getTime();
  return Array.from({ length: count }, (_, i) => ({
    at: new Date(base + i * 5 * 60000).toISOString().slice(0, 19).replace("T", " "),
    event_type: i < groupRunLength ? "import" : i % 2 === 0 ? "write_back" : "import",
    entity_type: "transaction",
    source_sheet: i < groupRunLength ? "2026" : `Aba ${i + 1}`,
  }));
}

const MANY_GESTURES_DETAILS = {
  remote_manifest: {
    device_id: "abcdef12-3456-7890-abcd-ef1234567890",
    sequence: 5,
    created_at: "2026-06-10T08:00:00Z",
    app_version: "0.2.1",
    schema_version: 7,
  },
  // 55 gestos deste aparelho (18 formam a corrida que colapsa em "×18"), 30 do outro — o total
  // supera o "50+" da issue #476, o cenário real que estourou a tela no aparelho.
  local_gestures: manyGesturesFixture(55, 18),
  remote_gestures: manyGesturesFixture(30, 12),
  this_device_id: "este-aparelho-99999999",
};

// Prova visual do defeito da issue #476: com MUITOS gestos dos dois lados, o cartão precisa
// conter a rolagem nas listas e manter as ações do rodapé sempre visíveis — nunca empurradas
// para fora da tela nem exigindo rolar até o fim para alcançá-las. Um cenário mobile (o viewport
// onde o defeito apareceu no aparelho) e um desktop, os dois em dark (regra 38: baselines novos
// gravam do zero e a 2ª rodada confere 100% verde).
for (const viewport of [
  { name: "mobile", width: 390, height: 844 },
  { name: "desktop", width: 1440, height: 1000 },
] as const) {
  test(`Conflito com muitos gestos mantém as ações visíveis — ${viewport.name}`, async ({
    page,
  }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.addInitScript(() => {
      localStorage.setItem("neko-theme", "dark");
    });
    await mockTauri(page, { drive_conflict_details: MANY_GESTURES_DETAILS });
    await page.goto("/");
    await expect(page.getByText(/Pode gastar hoje/)).toBeVisible();

    await openConflictScreen(page);

    const dialog = page.getByRole("dialog", {
      name: "Conflito de sincronização entre aparelhos",
    });
    await expect(dialog).toBeVisible();
    // A corrida de 18 gestos idênticos colapsa numa única linha com contagem — nunca 18 linhas
    // repetindo a mesma frase.
    await expect(
      dialog.getByText(/Importação da planilha \(aba 2026\) ×18/),
    ).toBeVisible();
    // As três ações do rodapé continuam alcançáveis mesmo com as duas listas cheias — o defeito
    // original (issue #476) as empurrava para fora da tela sem rolagem que as alcançasse.
    await expect(
      dialog.getByRole("button", { name: "Manter este aparelho" }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("button", { name: "Usar o outro aparelho" }),
    ).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Decidir depois" })).toBeVisible();
    await page.waitForTimeout(200);

    await expect(page).toHaveScreenshot(
      `snapshot-conflict-many-gestures-${viewport.name}.png`,
      {
        fullPage: true,
      },
    );
  });
}
