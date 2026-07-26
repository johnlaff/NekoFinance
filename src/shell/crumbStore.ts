import type { Screen } from "./screens";

/**
 * Crumb da appbar sobreposto pela própria tela (ex.: o mês visto no Livro-razão).
 *
 * Store externo module-level (useSyncExternalStore) em vez de estado no App via
 * contexto: as funções têm identidade fixa por construção, então o efeito que
 * publica o crumb nunca re-dispara por troca de identidade — correção que não
 * depende de memoização (Compiler ou manual).
 */

type CrumbOverrides = Partial<Record<Screen, string>>;

let overrides: CrumbOverrides = {};
const listeners = new Set<() => void>();

/** Sobrepõe o crumb de uma tela; `null` devolve o padrão de SCREEN_META. */
export function setCrumb(screen: Screen, label: string | null): void {
  if ((overrides[screen] ?? null) === label) return;
  const next = { ...overrides };
  if (label == null) delete next[screen];
  else next[screen] = label;
  overrides = next;
  for (const notify of listeners) notify();
}

export function subscribeCrumbs(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

export function crumbOverridesSnapshot(): CrumbOverrides {
  return overrides;
}
