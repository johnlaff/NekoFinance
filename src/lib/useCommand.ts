import { useEffect, useState } from "react";
import { isTauri } from "./api";
import { safeErrorMessage } from "./errors";

/**
 * SWR-lite for Tauri commands: returns the last successful response
 * synchronously (no skeleton flash on re-navigation) and revalidates in the
 * background on every mount. Scope is deliberately tiny — one cache entry per
 * command name; argumentful commands can join when a screen needs them.
 */
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetcher is a stable module-level wrapper
  }, [cmd]);

  return {
    data: visible.data,
    error: visible.error,
    loading: visible.loading,
  };
}
