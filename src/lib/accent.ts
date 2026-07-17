/**
 * Paleta de acento do app (cor de marca configurável do DS "Midnight Purr").
 *
 * O acento é COR DE MARCA: pinta chrome, CTAs e seleção. Cores de status do
 * método (paz/atenção/dinheiro) são fixas por tema e nunca seguem a paleta —
 * a separação vive em tokens/colors.css; aqui só a persistência e a aplicação.
 *
 * `jade` é o default de fábrica (a cor consolidada da marca) e corresponde à
 * AUSÊNCIA do atributo `data-accent` no <html> — mesmo contrato do tema escuro.
 */

export type Accent = "jade" | "lima" | "violeta" | "ambar" | "ceu" | "rosa";

export const ACCENT_KEY = "neko-accent";

/** `swatch` é o valor escuro da paleta — identidade da cor na amostra do seletor. */
export const ACCENTS: readonly { key: Accent; label: string; swatch: string }[] = [
  { key: "jade", label: "Jade", swatch: "#3fbf8f" },
  { key: "lima", label: "Lima", swatch: "#95ff48" },
  { key: "violeta", label: "Violeta", swatch: "#c084fc" },
  { key: "ambar", label: "Âmbar", swatch: "#fbbf24" },
  { key: "ceu", label: "Céu", swatch: "#38bdf8" },
  { key: "rosa", label: "Rosa", swatch: "#fb7185" },
];

const VALID = new Set<string>(ACCENTS.map((a) => a.key));

export function getStoredAccent(): Accent {
  if (typeof window === "undefined") return "jade";
  const stored = localStorage.getItem(ACCENT_KEY);
  return stored && VALID.has(stored) ? (stored as Accent) : "jade";
}

export function applyAccent(accent: Accent) {
  if (accent === "jade") {
    document.documentElement.removeAttribute("data-accent");
  } else {
    document.documentElement.setAttribute("data-accent", accent);
  }
  localStorage.setItem(ACCENT_KEY, accent);
}
