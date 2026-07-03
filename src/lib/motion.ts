/**
 * Preferência de animações do app (toggle "Animações" em Configurações).
 *
 * Três estados persistidos em localStorage (`neko-motion`):
 * - `"on"`  — FORÇA animações, mesmo quando o SO pede movimento reduzido. É uma
 *   escolha explícita do usuário, mais específica que o default do sistema.
 * - `"off"` — desliga animações decorativas.
 * - ausente (`"system"`) — segue o `prefers-reduced-motion` do SO.
 *
 * O estado é refletido em `<html data-motion="on|off">` (ausente = system).
 * O CSS decide por tokens: `--dur-*` colapsam para 0ms sob reduce/off e são
 * restaurados sob `[data-motion="on"]` (ver design-system/tokens/motion.css).
 * Animações dirigidas por JS (WAAPI/View Transitions) consultam `motionEnabled()`.
 */

const MOTION_KEY = "neko-motion";

export type MotionPreference = "on" | "off" | "system";

export function motionPreference(): MotionPreference {
  if (typeof window === "undefined") return "system";
  const stored = localStorage.getItem(MOTION_KEY);
  return stored === "on" || stored === "off" ? stored : "system";
}

/** Reflete a preferência persistida no atributo de <html>. Chamar no boot e a cada mudança. */
export function applyMotionPreference(): void {
  if (typeof document === "undefined") return;
  const pref = motionPreference();
  if (pref === "system") {
    document.documentElement.removeAttribute("data-motion");
  } else {
    document.documentElement.setAttribute("data-motion", pref);
  }
}

export function setMotionPreference(pref: MotionPreference): void {
  if (pref === "system") {
    localStorage.removeItem(MOTION_KEY);
  } else {
    localStorage.setItem(MOTION_KEY, pref);
  }
  applyMotionPreference();
}

/** O que o SISTEMA pede — exposto para a linha de diagnóstico das Configurações. */
export function systemPrefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Animações decorativas (JS) podem rodar? "on" força; "off" nega; "system" segue o SO. */
export function motionEnabled(): boolean {
  const pref = motionPreference();
  if (pref === "on") return true;
  if (pref === "off") return false;
  return !systemPrefersReducedMotion();
}
