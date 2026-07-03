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
 * Circular reveal do tema via View Transitions API: o frame do tema ANTIGO fica congelado
 * embaixo e o tema novo é revelado por um círculo que cresce a partir do ponto de clique
 * (clip-path animado no `::view-transition-new(root)`; o crossfade default é desligado no
 * CSS). É o efeito completo — o tema velho persiste fora do círculo enquanto ele cresce.
 */
function playViewTransitionReveal(
  x: number,
  y: number,
  radius: number,
  apply: () => void,
  onSettled: () => void,
): void {
  const transition = document.startViewTransition(apply);
  transition.ready
    .then(() => {
      // O estado React só pode atualizar DEPOIS do snapshot do tema velho: um
      // re-render antes da captura aplicaria o tema novo cedo demais (via effect)
      // e o círculo revelaria tema novo sobre tema novo — invisível. Pós-ready,
      // o capture de `root` é vivo e o re-render (ícone) aparece através dele.
      onSettled();
      document.documentElement.animate(
        {
          clipPath: [
            `circle(0px at ${x}px ${y}px)`,
            `circle(${radius}px at ${x}px ${y}px)`,
          ],
        },
        {
          duration: revealDurationMs(),
          easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
          pseudoElement: "::view-transition-new(root)",
        },
      );
    })
    .catch(() => {
      // Transição abortada/sem suporte no caminho: o DOM já tem o tema novo
      // (apply rodou); garante que o estado React acompanhe. Idempotente.
      onSettled();
    });
}

/**
 * Fallback sem View Transitions (WebKitGTK antigo, jsdom): um overlay com a cor de fundo
 * do tema de destino floresce em `clip-path: circle()` e se dissolve. Decorativo — o tema
 * já foi aplicado antes (o overlay nunca é a fonte da verdade), então o swap é correto
 * mesmo sem Web Animations API.
 */
function playOverlayReveal(x: number, y: number, radius: number, next: Theme): void {
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
      duration: revealDurationMs(),
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

    if (typeof document.startViewTransition === "function") {
      // A troca visual é o atributo em <html>: applyTheme SÓ dentro do callback da
      // transição (pós-snapshot do tema velho). setTheme aqui fora iniciaria um
      // re-render cujo effect aplicaria o tema ANTES da captura — o snapshot velho
      // nasceria com o tema novo e o reveal seria invisível. O estado React é
      // atualizado pelo onSettled (pós-ready ou transição abortada).
      try {
        playViewTransitionReveal(
          x,
          y,
          radius,
          () => applyTheme(next),
          () => setTheme(next),
        );
        return;
      } catch {
        // API presente mas quebrada em runtime → cai para o overlay abaixo.
      }
    }

    setTheme(next);
    playOverlayReveal(x, y, radius, next);
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
