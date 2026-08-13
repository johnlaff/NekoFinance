import { useEffect, useSyncExternalStore } from "react";
import { Moon, Sun } from "lucide-react";
import { motionEnabled } from "../lib/motion";
import { logMotion, playThemeReveal, type Theme } from "./themeReveal";

const THEME_KEY = "neko-theme";

/** Preferência de tema do SO — só decide quando o usuário nunca escolheu no app. */
function systemPrefersLightTheme(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: light)").matches
  );
}

function getStoredTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === "light") return "light";
  if (stored === "dark") return "dark";
  return systemPrefersLightTheme() ? "light" : "dark";
}

/* O tema vive num store de módulo, não em useState por instância: o shell monta
   DOIS ThemeToggle simultâneos (sidebar e appbar, alternados só por CSS) — estado
   local por instância dessincroniza o ícone da instância oculta ao trocar de
   breakpoint, e o primeiro clique nela reaplicaria o tema já ativo. */
let currentTheme: Theme = getStoredTheme();
const themeListeners = new Set<() => void>();

function subscribeTheme(cb: () => void) {
  themeListeners.add(cb);
  return () => themeListeners.delete(cb);
}

function applyTheme(theme: Theme) {
  if (theme === "light") {
    document.documentElement.setAttribute("data-theme", "light");
  } else {
    document.documentElement.removeAttribute("data-theme");
  }
  localStorage.setItem(THEME_KEY, theme);
  currentTheme = theme;
  themeListeners.forEach((cb) => cb());
}

/** Estado do tema + troca com o reveal circular, para qualquer superfície (shell,
 *  Configurações). O store de módulo continua a fonte única; o reveal parte do
 *  controle que disparou o evento (centro do rect na ativação por teclado). */
export function useThemeSwitch(): {
  theme: Theme;
  toggleTheme: (event: React.MouseEvent<HTMLElement>) => void;
} {
  const theme = useSyncExternalStore(
    subscribeTheme,
    () => currentTheme,
    (): Theme => "dark",
  );

  // Reconcilia store ↔ storage no mount: aplica o tema salvo no <html> ao abrir
  // o app e realinha o store quando o storage foi mexido por fora (testes).
  // Idempotente — múltiplos consumidores montados não disputam.
  useEffect(() => {
    const stored = getStoredTheme();
    if (
      stored !== currentTheme ||
      (stored === "light") !==
        (document.documentElement.getAttribute("data-theme") === "light")
    ) {
      applyTheme(stored);
    }
  }, []);

  // React Compiler memoizes; no manual useCallback needed.
  const toggleTheme = (event: React.MouseEvent<HTMLElement>) => {
    const next: Theme = theme === "dark" ? "light" : "dark";

    // SO em reduced motion ou toggle "Animações" desligado → troca instantânea.
    if (!motionEnabled()) {
      logMotion(`reveal→${next}: pulado (motionEnabled=false)`);
      applyTheme(next);
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

    // O tema (DOM + store) só troca quando o overlay já cobre a tela.
    playThemeReveal(x, y, radius, next, () => {
      applyTheme(next);
    });
  };

  return { theme, toggleTheme };
}

export function ThemeToggle({ variant = "icon" }: { variant?: "icon" | "row" }) {
  const { theme, toggleTheme: toggle } = useThemeSwitch();

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
