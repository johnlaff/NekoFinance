import { useEffect, useState, useSyncExternalStore } from "react";
import { isTauri } from "./env";
import { safeErrorMessage } from "./errors";

const cache = new Map<string, unknown>();

// Versão global do cache: cada invalidate incrementa e notifica os hooks montados, que
// re-executam o fetch. Sem isso, invalidateCommands() só limpava o Map e o refetch ficava
// para o PRÓXIMO mount — todo botão "Tentar novamente" e todo refresh pós-escrita em tela
// já montada era um no-op silencioso.
let cacheVersion = 0;
const listeners = new Set<() => void>();

function subscribeToInvalidations(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getCacheVersion() {
  return cacheVersion;
}

/**
 * Teto por tentativa de `invoke`. No cold start Android a primeira chamada pode não
 * assentar — nem resolve nem rejeita, sem erro nos dois lados (a corrida do bridge de
 * IPC ainda não pronto quando o primeiro `useEffect` dispara). Sem este teto a tela
 * ficava presa no esqueleto para sempre; só uma NOVA montagem (trocar de aba e voltar)
 * disparava um `invoke` novo que funcionava. O teto reproduz esse "invoke novo" sozinho.
 */
export const COMMAND_TIMEOUT_MS = 6_000;

/** Tentativas antes de desistir e virar o estado de erro — a tela já tem o "Tentar
 *  novamente" manual (`invalidateCommands`) para esse desfecho residual. */
export const COMMAND_MAX_ATTEMPTS = 3;

interface CommandState<T> {
  cmd: string;
  data: T | undefined;
  error: string | null;
  loading: boolean;
}

function stateFor<T>(cmd: string): CommandState<T> {
  const cached = cache.get(cmd) as T | undefined;
  return {
    cmd,
    data: cached,
    error: null,
    loading: cached === undefined && isTauri,
  };
}

/**
 * Roda `fetcher()` com um teto por tentativa (`COMMAND_TIMEOUT_MS`), reproduzindo o
 * "invoke novo" de uma remontagem sozinho quando a promessa não assenta — ver o
 * porquê no JSDoc de `COMMAND_TIMEOUT_MS`. Cada tentativa corre isolada: se `fetcher()`
 * assenta primeiro, `settled` cancela o teto; se o teto vence primeiro, a promessa da
 * tentativa perdida vira no-op (o `.then`/`.catch` dela também passa por `settled`) e
 * uma tentativa nova nasce. Devolve a função de cancelamento que o efeito chama no
 * cleanup — vive FORA do efeito de propósito: mantém `useEffect` livre de temporizador
 * próprio, então o cleanup que ele devolve (o retorno desta função) já é o único dono.
 */
function retryCommand<T>(
  cmd: string,
  fetcher: () => Promise<T>,
  onSettle: (result: { data: T | undefined; error: string | null }) => void,
): () => void {
  let cancelled = false;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const cached = cache.get(cmd) as T | undefined;

  const attempt = (n: number) => {
    let settled = false;
    timeoutId = setTimeout(() => {
      if (settled || cancelled) return;
      settled = true;
      if (n < COMMAND_MAX_ATTEMPTS) {
        attempt(n + 1);
      } else {
        onSettle({
          data: cached,
          error: safeErrorMessage(new Error("timeout: comando sem resposta")),
        });
      }
    }, COMMAND_TIMEOUT_MS);

    fetcher()
      .then((fresh) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeoutId);
        cache.set(cmd, fresh);
        if (!cancelled) onSettle({ data: fresh, error: null });
      })
      .catch((e: unknown) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeoutId);
        if (!cancelled) onSettle({ data: cached, error: safeErrorMessage(e) });
      });
  };

  attempt(1);
  return () => {
    cancelled = true;
    if (timeoutId) clearTimeout(timeoutId);
  };
}

/** Drops every cached response and refetches every mounted hook. Call after any
 *  write/import so finance numbers refresh. */
export function invalidateCommands() {
  cache.clear();
  cacheVersion += 1;
  for (const listener of listeners) listener();
}

/**
 * SWR-lite for Tauri commands: returns the last successful response
 * synchronously (no skeleton flash on re-navigation) and revalidates in the
 * background on every mount. Scope is deliberately tiny — one cache entry per
 * command name; argumentful commands can join when a screen needs them.
 *
 * @param cmd   - Stable string key identifying the Tauri command. Changing
 *                `cmd` discards cached data and triggers a fresh load.
 * @param fetcher - MUST be referentially stable across renders (a module-level
 *                  arrow or a function defined outside the component). The first
 *                  fetcher reference is captured by the effect and kept; passing
 *                  an inline arrow `() => invoke(…)` will NOT re-run the effect
 *                  when its captured values change — you would silently fetch
 *                  with a stale closure. If you need per-render arguments, encode
 *                  them into `cmd` (e.g. `"month:2026-07"`) and read the key
 *                  inside a stable fetcher, or call `invalidateCommands()` after
 *                  writes. Do NOT wrap the fetcher in `useCallback`/`useMemo` —
 *                  the React Compiler (enabled in this repo) handles render
 *                  stability; manual memoization conflicts with it.
 */
export function useCommand<T>(cmd: string, fetcher: () => Promise<T>) {
  const [state, setState] = useState<CommandState<T>>(() => stateFor<T>(cmd));
  const version = useSyncExternalStore(subscribeToInvalidations, getCacheVersion);
  const visible = state.cmd === cmd ? state : stateFor<T>(cmd);

  useEffect(() => {
    if (!isTauri) return;
    // `retryCommand` devolve o cancelamento pronto — o efeito não possui temporizador
    // próprio, só entrega o resultado assentado (ou o timeout final) ao estado do hook.
    return retryCommand(cmd, fetcher, ({ data, error }) => {
      setState({ cmd, data, error, loading: false });
    });
    // INVARIANT: fetcher must be referentially stable (module-level arrow or a
    // stable function ref). Adding fetcher to the deps would re-run the effect on
    // every render for callers that inline their arrow, breaking the "no skeleton
    // flash on remount" contract. See the JSDoc on useCommand for the requirement.
    // `version` re-executa o fetch quando invalidateCommands() roda (retry/pós-escrita).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cmd, version]);

  return {
    data: visible.data,
    error: visible.error,
    loading: visible.loading,
  };
}
