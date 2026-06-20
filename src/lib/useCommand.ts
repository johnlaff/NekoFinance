import { useEffect, useState } from "react";
import { isTauri } from "./api";
import { safeErrorMessage } from "./errors";

const cache = new Map<string, unknown>();

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

/** Drops every cached response. Call after any write/import so finance numbers refresh. */
export function invalidateCommands() {
  cache.clear();
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
  const visible = state.cmd === cmd ? state : stateFor<T>(cmd);

  useEffect(() => {
    if (!isTauri) return;
    let alive = true;
    const cached = cache.get(cmd) as T | undefined;
    fetcher()
      .then((fresh) => {
        cache.set(cmd, fresh);
        if (alive) {
          setState({
            cmd,
            data: fresh,
            error: null,
            loading: false,
          });
        }
      })
      .catch((e: unknown) => {
        if (alive) {
          setState({
            cmd,
            data: cached,
            error: safeErrorMessage(e),
            loading: false,
          });
        }
      });
    return () => {
      alive = false;
    };
    // INVARIANT: fetcher must be referentially stable (module-level arrow or a
    // stable function ref). Adding fetcher to the deps would re-run the effect on
    // every render for callers that inline their arrow, breaking the "no skeleton
    // flash on remount" contract. See the JSDoc on useCommand for the requirement.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cmd]);

  return {
    data: visible.data,
    error: visible.error,
    loading: visible.loading,
  };
}
