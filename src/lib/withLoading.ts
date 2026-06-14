/**
 * Executa `fn` com um flag de loading ligado, garantindo o desligamento ao final (sucesso ou erro).
 *
 * Vive em escopo de módulo (fora de qualquer componente) de propósito: o React Compiler não
 * otimiza componentes que contêm `try/finally`, então mantemos o `finally` aqui e os handlers
 * ficam só com `try/catch` (suportado pelo compilador).
 */
export async function withLoading(
  setLoading: (v: boolean) => void,
  fn: () => Promise<void>,
): Promise<void> {
  setLoading(true);
  try {
    await fn();
  } finally {
    setLoading(false);
  }
}
