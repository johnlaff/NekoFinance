/**
 * Reveal circular do tema — módulo compartilhado entre o ThemeToggle (produção) e o
 * diagnóstico de animações das Configurações (botão "Testar reveal", mesmo caminho).
 *
 * Técnica: overlay FULLSCREEN da cor de fundo CONCRETA do tema de destino, animado só
 * por `clip-path: circle()` via WAAPI (grow do clique + retract de volta — ver
 * `playThemeReveal`). clip-path é a única primitiva validada VISUALMENTE no hardware-alvo.
 * Evitados de propósito: View Transitions (o WebView2 não pinta os pseudo-elementos),
 * `transform: scale()` a partir de 0 num elemento gigante (raster inicial vazio) e
 * `opacity` (sem prova de que o compositor a pinta neste WebView2).
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

/**
 * Cor de fundo CONCRETA do tema de destino, resolvida flipando o atributo em <html>
 * dentro do mesmo task síncrono (o browser não pinta no meio de um task — zero flash).
 * Necessário porque custom properties HERDAM: um overlay sem atributo herdaria o --bg
 * do tema ANTIGO ainda ativo no html, deixando o reveal invisível no sentido
 * light→dark. Fallbacks cobrem ambientes sem resolução de estilo (jsdom).
 */
function resolveThemeBg(next: Theme): string {
  const html = document.documentElement;
  const prev = html.getAttribute("data-theme");
  if (next === "light") {
    html.setAttribute("data-theme", "light");
  } else {
    html.removeAttribute("data-theme");
  }
  const bg = getComputedStyle(html).getPropertyValue("--bg").trim();
  if (prev === null) {
    html.removeAttribute("data-theme");
  } else {
    html.setAttribute("data-theme", prev);
  }
  return bg || (next === "light" ? "#f4f4f0" : "#0e1413");
}

/**
 * Reveal em DUAS fases, exclusivamente com `clip-path` (a única primitiva confirmada
 * visualmente no WebView2-alvo — `opacity` NÃO é usada em lugar nenhum, pois não há prova
 * de que o compositor a pinta aqui):
 *
 * 1. GROW — um disco da cor do tema de destino cresce do ponto (x, y) por cima da UI
 *    ainda no tema antigo (fora do disco a UI antiga real permanece).
 * 2. Quando o disco cobre a tela, `apply` troca o tema por baixo e um flush síncrono +
 *    dois frames garantem que a nova paleta PINTOU sob o disco opaco.
 * 3. RETRACT — o disco recolhe de volta ao ponto de clique, revelando a UI nova
 *    (já pintada) das bordas para o centro. Sem cor sólida parada, sem dissolve de
 *    opacity, sem "cor sólida → depois a UI".
 *
 * Um cancelamento em qualquer fase aterrissa o tema via `apply` e remove o overlay.
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
  const bg = resolveThemeBg(next);
  const t0 = performance.now();
  logMotion(
    `reveal→${next}: início dur=${REVEAL_DURATION_MS}ms token=${rawDurationToken()} bg=${bg}`,
  );
  overlay.setAttribute("aria-hidden", "true");
  overlay.style.cssText =
    `position:fixed;inset:0;z-index:9999;pointer-events:none;background:${bg};` +
    `clip-path:circle(0px at ${x}px ${y}px);`;
  document.body.appendChild(overlay);

  const clipAt = (r: number) => `circle(${r}px at ${x}px ${y}px)`;
  let done = false;
  const cleanup = () => {
    if (done) return;
    done = true;
    overlay.remove();
  };

  const grow = overlay.animate(
    [{ clipPath: clipAt(0) }, { clipPath: clipAt(radius) }],
    {
      duration: REVEAL_DURATION_MS,
      easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
      fill: "forwards", // mantém o disco cheio enquanto o tema troca por baixo
    },
  );
  grow.addEventListener("cancel", () => {
    logMotion(
      `reveal→${next}: GROW cancelado em ${Math.round(performance.now() - t0)}ms`,
    );
    apply();
    cleanup();
  });
  grow.addEventListener("finish", () => {
    const tGrow = Math.round(performance.now() - t0);
    // Troca o tema por baixo do disco cheio e força o recálculo de estilo+layout AGORA
    // (a repintura tardia da árvore era o que aparecia como "cor sólida → depois a UI").
    apply();
    void document.documentElement.offsetWidth;
    const tApply = performance.now();
    // Dois frames: o segundo só dispara após um ciclo de paint, garantindo que a nova
    // paleta já está pintada sob o disco antes de o retract revelá-la.
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        logMotion(
          `reveal→${next}: cresceu ${tGrow}ms · paint ${Math.round(performance.now() - tApply)}ms`,
        );
        const retract = overlay.animate(
          [{ clipPath: clipAt(radius) }, { clipPath: clipAt(0) }],
          {
            duration: Math.round(REVEAL_DURATION_MS * 0.8),
            easing: "cubic-bezier(0.4, 0, 0.2, 1)", // --ease-standard
            fill: "forwards",
          },
        );
        retract.addEventListener("finish", cleanup);
        retract.addEventListener("cancel", cleanup);
      }),
    );
  });
}
