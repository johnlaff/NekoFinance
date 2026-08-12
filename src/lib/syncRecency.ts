/** Rótulo de recência pt-BR ("há 18 min") a partir de um timestamp UTC em um de dois formatos:
 *  `datetime('now')` do SQLite (sync_log — "YYYY-MM-DD HH:MM:SS", sem sufixo de fuso, sempre
 *  UTC) ou RFC3339 (`chrono::Utc::now().to_rfc3339()` — snapshot_cmds.rs, já com "T" e offset
 *  explícito). Calculado no render; atualiza no próximo invalidateCommands (sem setInterval). */
export function syncRecencyLabel(
  ts: string | null | undefined,
  now: number = Date.now(),
): string | null {
  if (!ts) return null;
  const iso = ts.includes("T") ? ts : `${ts.replace(" ", "T")}Z`;
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return null;
  const min = Math.max(0, Math.floor((now - then) / 60000));
  if (min < 1) return "agora mesmo";
  if (min < 60) return `há ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return `há ${h} h`;
  const d = Math.floor(h / 24);
  return `há ${d} ${d === 1 ? "dia" : "dias"}`;
}
