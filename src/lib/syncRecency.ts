/** Rótulo de recência pt-BR ("há 18 min") a partir do timestamp do sync_log.
 *  `datetime('now')` do SQLite é UTC sem sufixo de fuso — parseia como UTC.
 *  Calculado no render; atualiza no próximo invalidateCommands (sem setInterval). */
export function syncRecencyLabel(
  ts: string | null | undefined,
  now: number = Date.now(),
): string | null {
  if (!ts) return null;
  const then = new Date(ts.replace(" ", "T") + "Z").getTime();
  if (Number.isNaN(then)) return null;
  const min = Math.max(0, Math.floor((now - then) / 60000));
  if (min < 1) return "agora mesmo";
  if (min < 60) return `há ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return `há ${h} h`;
  const d = Math.floor(h / 24);
  return `há ${d} ${d === 1 ? "dia" : "dias"}`;
}
