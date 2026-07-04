/**
 * Reveal circular do tema — módulo compartilhado entre o ThemeToggle (produção) e o
 * diagnóstico de animações das Configurações (botão "Testar reveal", mesmo caminho).
 *
 * Técnica: troca o tema JÁ e cobre com um overlay da cor ANTIGA que ENCOLHE em
 * `clip-path: circle()` do raio total até 0 no ponto de clique (drain — ver
 * `playThemeReveal`), revelando a UI nova. clip-path é a única primitiva validada
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
 * Reveal "drain", só com `clip-path` — a única primitiva confirmada visualmente neste
 * WebView2 (`opacity` não pinta aqui; os pseudo-elementos de View Transitions também não;
 * daí este caminho manual):
 *
 * 1. O tema é trocado JÁ (a UI nova pinta em ~0-9ms neste hardware) e imediatamente
 *    coberta por um overlay da cor do tema ANTIGO — o usuário continua vendo a cor de
 *    onde saiu, a repintura acontece escondida.
 * 2. Esse overlay encolhe em `clip-path: circle()` do raio total até 0 no ponto de clique,
 *    revelando a UI nova (já pronta) de forma animada e contínua.
 * 3. No fim o clip já está em 0 (overlay invisível) — removê-lo não pisca, porque não há
 *    mais um layer COBRINDO a tela para destruir (a causa do flick da remoção abrupta).
 *
 * Sem corte seco, sem cor sólida parada, sem retract em "loop", sem opacity. Um
 * cancelamento aterrissa o overlay invisível.
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
    `clip-path:circle(${radius}px at ${x}px ${y}px);`;
  document.body.appendChild(overlay);

  let done = false;
  const cleanup = () => {
    if (done) return;
    done = true;
    overlay.remove();
  };

  const drain = overlay.animate(
    [
      { clipPath: `circle(${radius}px at ${x}px ${y}px)` },
      { clipPath: `circle(0px at ${x}px ${y}px)` },
    ],
    {
      duration: REVEAL_DURATION_MS,
      easing: "cubic-bezier(0.4, 0, 0.2, 1)", // --ease-standard
      fill: "forwards", // fica em clip(0) (invisível) até a remoção — sem flash de volta
    },
  );
  drain.addEventListener("finish", cleanup);
  drain.addEventListener("cancel", cleanup);
}
