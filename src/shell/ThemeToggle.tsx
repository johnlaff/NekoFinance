import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";
import { motionEnabled } from "../lib/motion";

const THEME_KEY = "neko-theme";

type Theme = "dark" | "light";

function getStoredTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === "light") return "light";
  return "dark";
}

function applyTheme(theme: Theme) {
  if (theme === "light") {
    document.documentElement.setAttribute("data-theme", "light");
  } else {
    document.documentElement.removeAttribute("data-theme");
  }
  localStorage.setItem(THEME_KEY, theme);
}

/** Duração do reveal a partir do token do DS (fallback cobre ambientes sem getComputedStyle). */
function revealDurationMs(): number {
  return (
    parseFloat(
      getComputedStyle(document.documentElement).getPropertyValue("--dur-deliberate"),
    ) || 480
  );
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
 * Circular reveal do tema com um CÍRCULO REAL crescendo por `transform: scale()` —
 * deliberadamente sem View Transitions (o WebView2 não pinta os pseudo-elementos da
 * transição) e sem `clip-path` animado (não compõe de forma confiável no mesmo engine).
 * `transform` em elemento comum é a única primitiva comprovada em todos os alvos.
 *
 * Sequência: um disco com a cor de fundo do tema de destino escala do ponto de clique
 * POR CIMA da UI ainda no tema antigo; quando cobre a tela, o tema real troca por
 * baixo (`apply`) e o disco se dissolve revelando a UI nova. A troca fica escondida
 * sob o disco cheio — nunca há swap abrupto visível.
 */
function playOverlayReveal(
  x: number,
  y: number,
  radius: number,
  next: Theme,
  apply: () => void,
): void {
  const overlay = document.createElement("div");
  if (typeof overlay.animate !== "function") {
    // Sem WAAPI (jsdom/engines antigos) → troca instantânea, sem floreio.
    apply();
    return;
  }
  overlay.setAttribute("aria-hidden", "true");
  const diameter = Math.ceil(radius * 2);
  overlay.style.cssText =
    `position:fixed;left:${Math.round(x - radius)}px;top:${Math.round(y - radius)}px;` +
    `width:${diameter}px;height:${diameter}px;border-radius:50%;` +
    `background:${resolveThemeBg(next)};transform:scale(0);` +
    "z-index:9999;pointer-events:none;";
  document.body.appendChild(overlay);

  const swapAndRemove = () => {
    apply();
    overlay.remove();
  };
  const grow = overlay.animate([{ transform: "scale(0)" }, { transform: "scale(1)" }], {
    duration: revealDurationMs(),
    easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
    fill: "forwards", // cobre a tela inteira enquanto o tema troca por baixo
  });
  grow.addEventListener("finish", () => {
    apply();
    const fade = overlay.animate([{ opacity: 1 }, { opacity: 0 }], {
      duration: 160,
      easing: "linear",
    });
    const cleanup = () => overlay.remove();
    fade.addEventListener("finish", cleanup);
    fade.addEventListener("cancel", cleanup);
  });
  grow.addEventListener("cancel", swapAndRemove);
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // React Compiler memoizes; no manual useCallback needed.
  const toggle = (event: React.MouseEvent<HTMLButtonElement>) => {
    const next: Theme = theme === "dark" ? "light" : "dark";

    // SO em reduced motion ou toggle "Animações" desligado → troca instantânea.
    if (!motionEnabled()) {
      setTheme(next);
      return;
    }

    // Reveal circular a partir do ponto de interação (centro do botão na ativação por teclado,
    // onde clientX/Y são 0).
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX || rect.left + rect.width / 2;
    const y = event.clientY || rect.top + rect.height / 2;
    const radius = Math.hypot(
      Math.max(x, window.innerWidth - x),
      Math.max(y, window.innerHeight - y),
    );

    // O tema (DOM + estado React) só troca quando o overlay já cobre a tela.
    // applyTheme direto evita depender do agendamento do effect; o effect que o
    // setTheme dispara reaplica o mesmo tema (idempotente).
    playOverlayReveal(x, y, radius, next, () => {
      applyTheme(next);
      setTheme(next);
    });
  };

  return (
    <button
      type="button"
      aria-label={
        theme === "dark" ? "Alternar para tema claro" : "Alternar para tema escuro"
      }
      title={theme === "dark" ? "Tema claro" : "Tema escuro"}
      onClick={toggle}
      className="ak-iconbtn"
    >
      {theme === "dark" ? (
        <Sun size={17} strokeWidth={1.75} />
      ) : (
        <Moon size={17} strokeWidth={1.75} />
      )}
    </button>
  );
}
