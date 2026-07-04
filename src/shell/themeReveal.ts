/**
 * Reveal circular do tema — módulo compartilhado entre o ThemeToggle (produção) e o
 * diagnóstico de animações das Configurações (botão "Testar reveal", mesmo caminho).
 *
 * Técnica: troca o tema JÁ e cobre com um overlay da cor ANTIGA; um FURO circular cresce
 * do ponto de clique (`clip-path: path()` com regra evenodd — ver `playThemeReveal`),
 * revelando a UI nova de dentro para fora. clip-path é a única primitiva validada
 * VISUALMENTE no hardware-alvo. Evitados de propósito: View Transitions (o WebView2 não
 * pinta os pseudo-elementos), `transform: scale()` a partir de 0 num elemento gigante
 * (raster inicial vazio) e `opacity` (o compositor não a pinta neste WebView2).
 *
 * Cada etapa grava um evento em `nk-motion-log` (localStorage, últimos 8) — o
 * diagnóstico exibe o log para depurar o caminho real sem devtools.
 */

export type Theme = "dark" | "light";

const LOG_KEY = "nk-motion-log";

export function logMotion(event: string): void {
  try {
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
 *  fazendo o disco encher em ~11ms ("a tela só piscou"). O valor casa o token
 *  `--dur-deliberate` do DS; a leitura do token vai só para o log de diagnóstico. */
const REVEAL_DURATION_MS = 480;

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
export function playThemeReveal(
  x: number,
  y: number,
  radius: number,
  next: Theme,
  apply: () => void,
): void {
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
  // Troca o tema AGORA e força a repintura da UI nova, escondida sob o overlay.
  apply();
  void document.documentElement.offsetWidth;
  logMotion(
    `reveal→${next}: início dur=${REVEAL_DURATION_MS}ms token=${rawDurationToken()} oldbg=${oldBg} · paint ${Math.round(performance.now() - t0)}ms`,
  );

  overlay.setAttribute("aria-hidden", "true");
  overlay.style.cssText =
    `position:fixed;inset:0;z-index:9999;pointer-events:none;background:${oldBg};` +
    `clip-path:${coverWithHolePath(w, h, x, y, 0)};`;
  document.body.appendChild(overlay);

  let done = false;
  const cleanup = () => {
    if (done) return;
    done = true;
    overlay.remove();
  };

  const grow = overlay.animate(
    [
      { clipPath: coverWithHolePath(w, h, x, y, 0) },
      { clipPath: coverWithHolePath(w, h, x, y, radius) },
    ],
    {
      duration: REVEAL_DURATION_MS,
      easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
      fill: "forwards", // furo cheio (overlay invisível) até a remoção
    },
  );
  grow.addEventListener("finish", cleanup);
  grow.addEventListener("cancel", cleanup);
}
