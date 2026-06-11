import { useCallback, useEffect, useState } from "react";
import { flushSync } from "react-dom";
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

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const toggle = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      const next: Theme = theme === "dark" ? "light" : "dark";

      // Fallback path: no View Transitions (WebKitGTK dev shell, jsdom) or
      // reduced motion → instant, correct swap.
      if (
        typeof document.startViewTransition !== "function" ||
        prefersReducedMotion()
      ) {
        setTheme(next);
        return;
      }

      // Circular reveal from the interaction point (button center on
      // keyboard activation, where clientX/Y are 0).
      const rect = event.currentTarget.getBoundingClientRect();
      const x = event.clientX || rect.left + rect.width / 2;
      const y = event.clientY || rect.top + rect.height / 2;
      const radius = Math.hypot(
        Math.max(x, window.innerWidth - x),
        Math.max(y, window.innerHeight - y),
      );

      const transition = document.startViewTransition(() => {
        flushSync(() => setTheme(next));
      });

      void transition.ready.then(() => {
        document.documentElement.animate(
          {
            clipPath: [
              `circle(0px at ${x}px ${y}px)`,
              `circle(${radius}px at ${x}px ${y}px)`,
            ],
          },
          {
            duration: 480, // --dur-deliberate
            easing: "cubic-bezier(0.16, 1, 0.3, 1)", // --ease-entrance
            pseudoElement: "::view-transition-new(root)",
          },
        );
      });
    },
    [theme],
  );

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
