import { expect, test } from "@playwright/test";
import { mockTauri } from "./tauri-mock";

// Regressão visual: no mobile, o dock flutuante (`.sh-dock`) encolhe ao rolar (issue do
// composer da Mia órfão) — a reserva de espaço que o mantém longe do conteúdo
// (`.sh-body` padding-bottom) tem que encolher junto, senão sobra uma faixa vazia do
// tamanho exato do dock onde ele estava. Cenário: conversa longa o bastante para rolar,
// rolada até o fim — o composer ancorado (`position: sticky`, regra 21) tem que acompanhar
// o dock, não ficar paralisado na posição calculada para ele visível.
// Para regenerar deliberadamente (regra 38): `rm -rf tests/e2e/mia-conversation-visual.spec.ts-snapshots`
// e rodar a suíte duas vezes (`--update-snapshots`, depois sem a flag).

/** Uma pergunta+resposta curta o bastante para não distorcer a leitura, repetida até a
 *  conversa passar da altura do viewport mobile (390×844) e precisar rolar de verdade. */
function longConversation() {
  const rows: {
    author: "voce" | "mia";
    question: string | null;
    answer: unknown;
    at_iso: string;
  }[] = [];
  const topics = [
    "Quanto posso gastar hoje?",
    "Como está a reserva?",
    "Fechei o mês no azul?",
    "Quanto sobrou este mês?",
    "Como está a economia do ano?",
  ];
  for (let i = 0; i < topics.length; i++) {
    const hh = String(9 + i).padStart(2, "0");
    rows.push({
      author: "voce",
      question: topics[i]!,
      answer: null,
      at_iso: `2026-06-10T${hh}:00`,
    });
    rows.push({
      author: "mia",
      question: null,
      answer: {
        text: [
          { t: "text", s: "Hoje o retrato é " },
          { t: "strong", s: "R$ 4.300,00" },
          { t: "text", s: " por dia, dentro do combinado." },
        ],
        provenance: "runtime",
        transparency: "Provedor: openrouter · Modelo: demo · Custo: US$ 0,0010",
      },
      at_iso: `2026-06-10T${hh}:02`,
    });
  }
  // A última resposta traz markdown cru — a mesma sintaxe do evidence report da #481 — para
  // a captura provar as duas correções juntas: negrito/lista formatados E o composer sem
  // vazio abaixo dele.
  rows.push({
    author: "voce",
    question: "Qual a margem para economizar este mês?",
    answer: null,
    at_iso: "2026-06-10T09:40",
  });
  rows.push({
    author: "mia",
    question: null,
    answer: {
      text: [
        {
          t: "text",
          s: "**R$ 180,00**\n\n- **Margem para economia:** ainda não há registro este mês.",
        },
      ],
      provenance: "runtime",
      transparency: "Provedor: openrouter · Modelo: demo · Custo: US$ 0,0012",
    },
    at_iso: "2026-06-10T09:42",
  });
  return rows;
}

test.describe("Mia — conversa longa no mobile (o composer acompanha o dock)", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      load_mia_conversation: longConversation(),
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Mia" }).click();
    await expect(page.getByRole("log", { name: "Conversa com a Mia" })).toBeVisible();
  });

  test("negrito e lista do modelo renderizam formatados, não crus", async ({
    page,
  }) => {
    const log = page.getByRole("log");
    await expect(log.locator("b", { hasText: "R$ 180,00" })).toBeVisible();
    await expect(log.locator("b", { hasText: "Margem para economia:" })).toBeVisible();

    // A prova de que não sobrou sintaxe crua: o texto corrido da última bolha (innerText,
    // não textContent — CSS não entra na conta) não carrega `**`, e o traço da lista virou
    // marcador visível na própria linha.
    const lastSay = log.locator(".mia__say").last();
    const sayText = await lastSay.innerText();
    expect(sayText).not.toContain("**");
    expect(sayText).toMatch(/•\s*Margem para economia/);
  });

  test("dock some ao rolar; o composer acompanha, sem faixa vazia no rodapé", async ({
    page,
  }) => {
    const dock = page.locator(".sh-dock");
    const composer = page.locator(".mia__composer");

    // A conversa já abre rolada até a última resposta (`CopilotScreen` acompanha o fim do
    // log) — o topo é o estado "em repouso" determinístico para provar o dock visível ANTES
    // do gesto que o esconde.
    await page.evaluate(() => document.querySelector(".sh-body")?.scrollTo({ top: 0 }));
    await expect(dock).not.toHaveClass(/sh-dock--min/);
    await expect(composer).toBeVisible();

    // Rola a CONVERSA até o fim — o mesmo alvo (`.sh-body`) que o listener do dock observa
    // (AppShell) e que `CopilotScreen` rola ao chegar a resposta nova.
    await page.evaluate(() => {
      const scroller = document.querySelector(".sh-body");
      scroller?.scrollTo({ top: scroller.scrollHeight });
    });
    await expect(dock).toHaveClass(/sh-dock--min/);
    // A troca de padding-bottom em `.sh-body` (o encolher da reserva do dock) recomputa a
    // posição sticky do `.mia__dock` num reflow SEGUINTE ao commit da classe — mesmo com
    // `--dur-slow` colapsado a 0ms sob `reducedMotion`, uma leitura de bounding box no
    // exatíssimo instante em que a classe aparece corre na frente desse reflow. Espera pela
    // GEOMETRIA já assentada (o composer perto do fim real da tela), não por um tempo fixo —
    // é a mesma condição que a asserção abaixo confirma, só que tolerante a polling.
    await page.waitForFunction(
      () => {
        const composerEl = document.querySelector(".mia__composer");
        if (!composerEl) return false;
        const rect = composerEl.getBoundingClientRect();
        return window.innerHeight - rect.bottom < 40;
      },
      { timeout: 5000 },
    );

    // Prova geométrica (não só visual, e nas DUAS pontas — o relato original só media a de
    // baixo e deixava passar a faixa vazia ACIMA do composer, entre a última bolha e ele).
    const lastBubble = page.locator(".mia__log > *").last();
    const lastBubbleBox = await lastBubble.boundingBox();
    const miaDockBox = await page.locator(".mia__dock").boundingBox();
    const composerBox = await composer.boundingBox();
    expect(lastBubbleBox).not.toBeNull();
    expect(miaDockBox).not.toBeNull();
    expect(composerBox).not.toBeNull();

    // Acima: a última mensagem termina colada no topo do dock da Mia — nenhuma faixa morta
    // sobra entre a conversa e as sugestões/composer.
    const gapAboveDock = miaDockBox!.y - (lastBubbleBox!.y + lastBubbleBox!.height);
    expect(gapAboveDock).toBeLessThan(40);

    // Abaixo: o composer termina perto do fim real da tela — não paralisado na posição
    // calculada para o dock (do shell) ainda visível.
    const gapBelowComposer = 844 - (composerBox!.y + composerBox!.height);
    expect(gapBelowComposer).toBeLessThan(40);

    await expect(page).toHaveScreenshot("mia-scrolled-dock-hidden.png");
  });
});

// Cenário separado: uma conversa CURTA (poucas trocas, `.mia` perto do piso do próprio
// `min-height`, não esticada pelo conteúdo) — o relato original veio de uma conversa deste
// tamanho. A altura de `.mia` não muda quando o dock some (só a reserva de `.sh-body`), então
// o gap tem que fechar aqui também, não só quando o conteúdo já é alto o bastante para
// dominar por si.
test.describe("Mia — conversa curta no mobile (o composer acompanha o dock)", () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: new Date("2026-06-10T12:00:00-03:00") });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setViewportSize({ width: 390, height: 844 });
    const rows = [
      "Quanto posso gastar hoje?",
      "Como está a reserva?",
      "Fechei no azul?",
    ].flatMap((question, i) => {
      const hh = String(9 + i).padStart(2, "0");
      return [
        {
          author: "voce" as const,
          question,
          answer: null,
          at_iso: `2026-06-10T${hh}:00`,
        },
        {
          author: "mia" as const,
          question: null,
          answer: {
            text: [
              { t: "text", s: "Hoje o retrato é " },
              { t: "strong", s: "R$ 4.300,00" },
              { t: "text", s: " por dia, dentro do combinado." },
            ],
            provenance: "runtime",
            transparency: "Provedor: openrouter · Modelo: demo · Custo: US$ 0,0010",
          },
          at_iso: `2026-06-10T${hh}:02`,
        },
      ];
    });
    await mockTauri(page, {
      list_scenarios_cmd: [],
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      load_mia_conversation: rows,
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Mia" }).click();
    await expect(page.getByRole("log", { name: "Conversa com a Mia" })).toBeVisible();
  });

  test("mesmo com conversa curta, o dock some e a última bolha cola no composer", async ({
    page,
  }) => {
    const dock = page.locator(".sh-dock");
    const composer = page.locator(".mia__composer");

    // Uma troca só não enche a tela — `.mia` fica no piso do próprio `min-height`, não no
    // conteúdo. Ainda assim o `.sh-body` real (a app inteira, não só a conversa) tem mais
    // altura que o viewport por causa do chrome — força uma rolagem real.
    await page.evaluate(() => {
      const scroller = document.querySelector(".sh-body");
      scroller?.scrollTo({ top: scroller.scrollHeight });
    });
    await expect(dock).toHaveClass(/sh-dock--min/);
    // Ver o comentário equivalente no cenário "conversa longa": espera a GEOMETRIA assentar
    // (o reflow do sticky após o encolher da reserva), não um tempo fixo.
    await page.waitForFunction(
      () => {
        const composerEl = document.querySelector(".mia__composer");
        if (!composerEl) return false;
        const rect = composerEl.getBoundingClientRect();
        return window.innerHeight - rect.bottom < 40;
      },
      { timeout: 5000 },
    );

    const lastBubble = page.locator(".mia__log > *").last();
    const lastBubbleBox = await lastBubble.boundingBox();
    const miaDockBox = await page.locator(".mia__dock").boundingBox();
    const composerBox = await composer.boundingBox();
    expect(lastBubbleBox).not.toBeNull();
    expect(miaDockBox).not.toBeNull();
    expect(composerBox).not.toBeNull();

    const gapAboveDock = miaDockBox!.y - (lastBubbleBox!.y + lastBubbleBox!.height);
    expect(gapAboveDock).toBeLessThan(40);
    const gapBelowComposer = 844 - (composerBox!.y + composerBox!.height);
    expect(gapBelowComposer).toBeLessThan(40);
  });
});
