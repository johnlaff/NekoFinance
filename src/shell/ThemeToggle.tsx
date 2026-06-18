import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";

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

function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * Floreio "circular reveal" a partir do ponto de interação — um overlay com a cor de fundo do tema
 * de destino que floresce em `clip-path: circle()` e se dissolve. Decorativo: o tema já foi aplicado
 * antes (o overlay nunca é a fonte da verdade), então o swap é correto mesmo sem Web Animations API
 * (WebKitGTK antigo, jsdom). Substitui a View Transitions API (sem `flushSync`/`startViewTransition`).
 */
function playReveal(x: number, y: number, radius: number, next: Theme): void {
  const overlay = document.createElement("div");
  if (typeof overlay.animate !== "function") return; // sem WAAPI → sem floreio
  overlay.setAttribute("aria-hidden", "true");
  // `[data-theme="light"]` casa qualquer elemento, então o overlay resolve o --bg do tema de destino.
  if (next === "light") overlay.setAttribute("data-theme", "light");
  overlay.style.cssText =
    "position:fixed;inset:0;z-index:9999;pointer-events:none;background:var(--bg);" +
    `clip-path:circle(0px at ${x}px ${y}px);`;
  document.body.appendChild(overlay);
  const anim = overlay.animate(
    [
      { clipPath: `circle(0px at ${x}px ${y}px)`, opacity: 0.9 },
      { clipPath: `circle(${radius}px at ${x}px ${y}px)`, opacity: 0 },
    ],
    {
      duration: 480, // --dur-deliberate
      easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
    },
  );
  const cleanup = () => overlay.remove();
  anim.addEventListener("finish", cleanup);
  anim.addEventListener("cancel", cleanup);
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // React Compiler memoizes; no manual useCallback needed.
  const toggle = (event: React.MouseEvent<HTMLButtonElement>) => {
    const next: Theme = theme === "dark" ? "light" : "dark";

    // O tema é aplicado já — o reveal é só decorativo. Em reduced motion, troca instantânea.
    setTheme(next);
    if (prefersReducedMotion()) return;

    // Reveal circular a partir do ponto de interação (centro do botão na ativação por teclado,
    // onde clientX/Y são 0).
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX || rect.left + rect.width / 2;
    const y = event.clientY || rect.top + rect.height / 2;
    const radius = Math.hypot(
      Math.max(x, window.innerWidth - x),
      Math.max(y, window.innerHeight - y),
    );
    playReveal(x, y, radius, next);
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
