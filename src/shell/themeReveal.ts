/**
 * Reveal circular do tema — módulo compartilhado entre o ThemeToggle (produção) e o
 * diagnóstico de animações das Configurações (botão "Testar reveal", mesmo caminho).
 *
 * Técnica: overlay FULLSCREEN (camada pequena para o compositor) com a cor de fundo
 * CONCRETA do tema de destino, revelado por `clip-path: circle()` animado via WAAPI —
 * as duas primitivas validadas visualmente pelo autoteste no hardware-alvo. Evitados
 * de propósito: View Transitions (o WebView2 não pinta os pseudo-elementos) e
 * `transform: scale()` a partir de 0 num elemento gigante (o raster inicial em escala
 * zero produz textura vazia que o compositor reaproveita — disco invisível).
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
 * Um disco da cor do tema de destino cresce do ponto (x, y) POR CIMA da UI ainda no
 * tema antigo; quando cobre a tela, `apply` troca o tema real por baixo e o overlay se
 * dissolve revelando a UI nova. Nunca há swap abrupto visível: até um cancelamento
 * aterrissa o tema via `apply`.
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

  const grow = overlay.animate(
    [
      { clipPath: `circle(0px at ${x}px ${y}px)` },
      { clipPath: `circle(${radius}px at ${x}px ${y}px)` },
    ],
    {
      duration: REVEAL_DURATION_MS,
      easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
      fill: "forwards", // cobre a tela inteira enquanto o tema troca por baixo
    },
  );
  grow.addEventListener("finish", () => {
    logMotion(`reveal→${next}: cresceu em ${Math.round(performance.now() - t0)}ms`);
    // Troca o tema por baixo do overlay AINDA opaco; um rAF garante que a nova
    // paleta já pintou antes de o overlay começar a sumir (senão o dissolve revela
    // um frame do tema em transição).
    apply();
    requestAnimationFrame(() => {
      const fade = overlay.animate([{ opacity: 1 }, { opacity: 0 }], {
        duration: 160,
        easing: "linear",
        // `forwards`: sem isso a animação reverte para opacity:1 no frame final
        // antes da remoção — um flash da cor sólida sobre a UI (o "pisca").
        fill: "forwards",
      });
      const cleanup = () => overlay.remove();
      fade.addEventListener("finish", cleanup);
      fade.addEventListener("cancel", cleanup);
    });
  });
  grow.addEventListener("cancel", () => {
    logMotion(`reveal→${next}: CANCELADO em ${Math.round(performance.now() - t0)}ms`);
    apply();
    overlay.remove();
  });
}
