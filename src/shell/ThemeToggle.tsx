import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";
import { motionEnabled } from "../lib/motion";
import { logMotion, playThemeReveal, type Theme } from "./themeReveal";

const THEME_KEY = "neko-theme";

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

export function ThemeToggle({ variant = "icon" }: { variant?: "icon" | "row" }) {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // React Compiler memoizes; no manual useCallback needed.
  const toggle = (event: React.MouseEvent<HTMLButtonElement>) => {
    const next: Theme = theme === "dark" ? "light" : "dark";

    // SO em reduced motion ou toggle "Animações" desligado → troca instantânea.
    if (!motionEnabled()) {
      logMotion(`reveal→${next}: pulado (motionEnabled=false)`);
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
    playThemeReveal(x, y, radius, next, () => {
      applyTheme(next);
      setTheme(next);
    });
  };

  // O ícone mostra para ONDE o toque leva (sol no escuro, lua no claro).
  const icon =
    theme === "dark" ? (
      <Sun size={17} strokeWidth={1.75} />
    ) : (
      <Moon size={17} strokeWidth={1.75} />
    );
  const target = theme === "dark" ? "Tema claro" : "Tema escuro";

  if (variant === "row") {
    return (
      // aria-label mantém o nome acessível no trilho tablet (rótulo oculto);
      // contém o texto visível ("Tema claro") para não quebrar voice control.
      <button
        type="button"
        onClick={toggle}
        className="sh-theme"
        aria-label={
          theme === "dark" ? "Alternar para tema claro" : "Alternar para tema escuro"
        }
        title={target}
      >
        {icon}
        <span>{target}</span>
      </button>
    );
  }

  return (
    <button
      type="button"
      aria-label={
        theme === "dark" ? "Alternar para tema claro" : "Alternar para tema escuro"
      }
      title={target}
      onClick={toggle}
      className="ak-iconbtn"
    >
      {icon}
    </button>
  );
}
