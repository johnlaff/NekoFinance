/**
 * Preferência de animações do app (toggle "Animações" em Configurações).
 *
 * Duas fontes decidem se animação decorativa roda: o SO (`prefers-reduced-motion`,
 * sempre respeitado) e o toggle do app (persistido em localStorage). O toggle
 * desligado aplica `data-motion="off"` em <html>, que colapsa os tokens `--dur-*`
 * para 0ms (ver design-system/tokens/motion.css) — animações CSS morrem sozinhas.
 * Animações dirigidas por JS (WAAPI/View Transitions) devem consultar
 * `motionEnabled()` antes de rodar.
 */

const MOTION_KEY = "neko-motion";

export function motionUserOff(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(MOTION_KEY) === "off";
}

/** Reflete a preferência persistida no atributo de <html>. Chamar no boot e a cada mudança. */
export function applyMotionPreference(): void {
  if (typeof document === "undefined") return;
  if (motionUserOff()) {
    document.documentElement.setAttribute("data-motion", "off");
  } else {
    document.documentElement.removeAttribute("data-motion");
  }
}

export function setMotionUserOff(off: boolean): void {
  if (off) {
    localStorage.setItem(MOTION_KEY, "off");
  } else {
    localStorage.removeItem(MOTION_KEY);
  }
  applyMotionPreference();
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Animações decorativas (JS) podem rodar? O SO e o toggle do app precisam permitir. */
export function motionEnabled(): boolean {
  return !prefersReducedMotion() && !motionUserOff();
}
