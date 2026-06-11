import { useEffect, useState } from "react";
import { isTauri } from "./api";

/**
 * SWR-lite for Tauri commands: returns the last successful response
 * synchronously (no skeleton flash on re-navigation) and revalidates in the
 * background on every mount. Scope is deliberately tiny — one cache entry per
 * command name; argumentful commands can join when a screen needs them.
 */
const cache = new Map<string, unknown>();

/** Drops every cached response. Call after any write/import so finance numbers refresh. */
export function invalidateCommands() {
  cache.clear();
}

export function useCommand<T>(cmd: string, fetcher: () => Promise<T>) {
  const cached = cache.get(cmd) as T | undefined;
  const [data, setData] = useState<T | undefined>(cached);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(cached === undefined && isTauri);

  useEffect(() => {
    if (!isTauri) return;
    let alive = true;
    fetcher()
      .then((fresh) => {
        cache.set(cmd, fresh);
        if (alive) {
          setData(fresh);
          setError(null);
        }
      })
      .catch((e: unknown) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetcher is a stable module-level wrapper
  }, [cmd]);

  return { data, error, loading };
}
