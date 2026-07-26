/**
 * Reveal circular do tema — módulo compartilhado entre o ThemeToggle (produção) e o
 * diagnóstico de animações das Configurações (botão "Testar reveal", mesmo caminho).
 *
 * Técnica: quando a View Transitions API existe (Chromium/WebView2), ela tira snapshot do
 * tema antigo e do novo e o novo é revelado por um círculo de clip-path do clique — os
 * elementos da UI nunca somem. Fallback (jsdom/engines sem a API): cobre com um overlay da
 * cor antiga e abre um FURO circular via `clip-path: path()`. A duração é SEMPRE uma
 * constante — o token resolve para "~0" via getComputedStyle neste WebView2. Evitados:
 * `opacity` (o compositor não a pinta aqui) e `transform: scale()` de 0 num elemento
 * gigante (raster inicial vazio).
 *
 * Cada etapa grava um evento em `nk-motion-log:v1` (localStorage, últimos 8) — o
 * diagnóstico exibe o log para depurar o caminho real sem devtools.
 */

export type Theme = "dark" | "light";

const LOG_KEY = "nk-motion-log:v1";
const LEGACY_LOG_KEY = "nk-motion-log";
let legacyLogDiscarded = false;

export function logMotion(event: string): void {
  try {
    if (!legacyLogDiscarded) {
      legacyLogDiscarded = true;
      localStorage.removeItem(LEGACY_LOG_KEY);
    }
    const raw = localStorage.getItem(LOG_KEY);
    const entries: string[] = raw ? (JSON.parse(raw) as string[]) : [];
    entries.push(event);
    localStorage.setItem(LOG_KEY, JSON.stringify(entries.slice(-8)));
  } catch {
    // log é best-effort; nunca pode quebrar o reveal.
  }
}

export function readMotionLog(): string[] {
  try {
    const raw = localStorage.getItem(LOG_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

/** Duração do reveal em ms. CONSTANTE, não lida do token: no WebView2 a leitura
 *  `getComputedStyle(...).getPropertyValue("--dur-deliberate")` resolvia para ~0,
 *  fazendo o disco encher em ~11ms ("a tela só piscou"). A leitura do token vai só para o
 *  log de diagnóstico. */
const REVEAL_DURATION_MS = 560;

/** Easing suave (ease-out contido, sem o start abrupto do --ease-entrance) — o furo
 *  cresce de forma controlada, sem sensação de "salto". */
const REVEAL_EASING = "cubic-bezier(0.33, 0, 0.2, 1)"; // --ease-calm

/** Valor bruto do token (só para o log de diagnóstico — não governa a animação). */
function rawDurationToken(): string {
  try {
    return (
      getComputedStyle(document.documentElement)
        .getPropertyValue("--dur-deliberate")
        .trim() || "vazio"
    );
  } catch {
    return "n/d";
  }
}

/** Cor de fundo concreta do tema ATUAL (o que será coberto), lida antes do swap. */
function currentThemeBg(prevWasLight: boolean): string {
  const bg = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
  return bg || (prevWasLight ? "#f4f4f0" : "#0e1413");
}

/**
 * Retângulo da viewport com um FURO circular de raio `r` no ponto (cx, cy), como
 * `clip-path: path(evenodd, …)`. O retângulo (preenchido) menos o círculo (regra
 * evenodd → vira furo) = a cobertura fica visível em tudo MENOS no círculo. Crescer `r`
 * abre o furo do clique para fora. `H`/`V` fecham o retângulo; dois arcos `A` desenham o
 * círculo. A estrutura de comandos é idêntica em qualquer `r`, então a WAAPI interpola.
 */
function coverWithHolePath(
  w: number,
  h: number,
  cx: number,
  cy: number,
  r: number,
): string {
  return (
    `path(evenodd, "M0 0 H${w} V${h} H0 Z ` +
    `M${cx - r} ${cy} A${r} ${r} 0 1 0 ${cx + r} ${cy} A${r} ${r} 0 1 0 ${cx - r} ${cy} Z")`
  );
}

/**
 * Reveal "buraco crescente", só com `clip-path` — a única primitiva confirmada
 * visualmente neste WebView2 (`opacity` não pinta aqui; os pseudo-elementos de View
 * Transitions também não; daí este caminho manual):
 *
 * 1. O tema é trocado JÁ (a UI nova pinta em ~0-9ms neste hardware) e imediatamente
 *    coberta por um overlay da cor do tema ANTIGO — a repintura acontece escondida.
 * 2. Um FURO circular cresce do ponto de clique (`clip-path: path()` com regra evenodd),
 *    revelando a UI nova de DENTRO PARA FORA — o efeito clássico "cresce do clique".
 * 3. No fim o furo cobre a tela (overlay todo recortado, invisível) — removê-lo não
 *    pisca, porque não há mais um layer COBRINDO a tela para destruir.
 *
 * Sem corte seco, sem cor sólida parada, sem opacity, sem inversão de direção.
 * Um cancelamento aterrissa o overlay.
 */
type DocWithVT = Document & {
  startViewTransition?: (cb: () => void) => { ready: Promise<void> };
};

export function playThemeReveal(
  x: number,
  y: number,
  radius: number,
  next: Theme,
  apply: () => void,
): void {
  // Caminho preferido: View Transitions API. Ela tira um snapshot do tema ANTIGO (a UI
  // real, com todos os elementos) e do NOVO; o antigo fica embaixo enquanto um círculo de
  // clip-path revela o novo a partir do clique — os elementos NUNCA somem (o problema da
  // cobertura chapada). Duração FIXA (o token resolve para "~0" via getComputedStyle neste
  // WebView2, ver REVEAL_DURATION_MS). Fallback abaixo cobre jsdom/engines sem a API.
  const doc = document as DocWithVT;
  if (typeof doc.startViewTransition === "function") {
    logMotion(`reveal→${next}: via View Transitions (${REVEAL_DURATION_MS}ms)`);
    try {
      const transition = doc.startViewTransition(() => apply());
      transition.ready
        .then(() => {
          document.documentElement.animate(
            {
              clipPath: [
                `circle(0px at ${x}px ${y}px)`,
                `circle(${Math.ceil(radius * 1.02)}px at ${x}px ${y}px)`,
              ],
            },
            {
              duration: REVEAL_DURATION_MS,
              easing: REVEAL_EASING,
              pseudoElement: "::view-transition-new(root)",
            },
          );
        })
        .catch(() => {
          // Transição abortada — o tema já foi aplicado no callback.
        });
      return;
    } catch {
      // API presente mas quebrada em runtime → cai no fallback de cobertura.
    }
  }

  const overlay = document.createElement("div");
  if (typeof overlay.animate !== "function") {
    // Sem WAAPI (jsdom/engines antigos) → troca instantânea, sem floreio.
    logMotion("reveal: sem WAAPI, swap instantâneo");
    apply();
    return;
  }
  const w = window.innerWidth;
  const h = window.innerHeight;
  // Cor do tema ATUAL (antes do swap) — é ela que o overlay usa para cobrir.
  const oldBg = currentThemeBg(
    document.documentElement.getAttribute("data-theme") === "light",
  );
  const t0 = performance.now();

  // ORDEM CRÍTICA: cobrir ANTES de trocar o tema. Se o tema trocasse primeiro, a paleta
  // nova pintaria por 1 frame antes da cobertura entrar — um flash da UI inteira no clique
  // (gritante em dark→light). Com a cobertura (cor do tema atual) já no DOM, o swap fica
  // escondido sob ela desde o primeiro frame.
  overlay.setAttribute("aria-hidden", "true");
  overlay.style.cssText =
    `position:fixed;inset:0;z-index:9999;pointer-events:none;background:${oldBg};` +
    `clip-path:${coverWithHolePath(w, h, x, y, 0)};`;
  document.body.appendChild(overlay);
  void document.documentElement.offsetWidth; // cobertura no DOM/pintável antes do swap

  // Agora troca o tema, escondido sob a cobertura, e força o recálculo da nova paleta.
  apply();
  void document.documentElement.offsetWidth;
  logMotion(
    `reveal→${next}: início dur=${REVEAL_DURATION_MS}ms token=${rawDurationToken()} oldbg=${oldBg} · paint ${Math.round(performance.now() - t0)}ms`,
  );

  let done = false;
  const cleanup = () => {
    if (done) return;
    done = true;
    overlay.remove();
  };
  // Furo termina com folga (6%) além do canto mais distante: garante que a tela toda já
  // esteja recortada ANTES do fim, sem o canto tocando a borda do círculo no último frame
  // (o "leve flick"). O overlay fica invisível o resto da animação.
  const endRadius = Math.ceil(radius * 1.06);

  const grow = overlay.animate(
    [
      { clipPath: coverWithHolePath(w, h, x, y, 0) },
      { clipPath: coverWithHolePath(w, h, x, y, endRadius) },
    ],
    {
      duration: REVEAL_DURATION_MS,
      easing: REVEAL_EASING,
      fill: "forwards", // furo cheio (overlay invisível) até a remoção
    },
  );
  // Remove só no frame seguinte ao fim: o overlay já está totalmente recortado (invisível),
  // então a remoção não coincide com nenhum frame visível.
  grow.addEventListener("finish", () => requestAnimationFrame(cleanup));
  grow.addEventListener("cancel", cleanup);
}
