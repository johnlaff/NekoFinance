/** Helpers puros do cenário "e se" (plano 072, fatia C) — separados de `screens/scenarios.tsx`
 * (arquivo de componentes) para o Fast Refresh preservar estado (react-doctor
 * `only-export-components`) e para ficarem testáveis isoladamente, no mesmo espírito de
 * `lib/nkFormat.ts`/`lib/movement.ts`. */

/** Remove os sufixos de marca (`#loan:<...>`/`#repl:<...>`) do fim da descrição — a UI nunca
 * mostra o marcador cru (ver convenções em `src-tauri/src/scenarios.rs`). */
export function stripScenarioMarker(description: string): string {
  return description.replace(/\s*#(?:loan|repl):\S+/g, "").trim();
}

/** Soma `n` meses a uma data ISO ("YYYY-MM-DD"), preservando o dia quando possível (satura no
 * último dia do mês de destino — ex.: 31/jan + 1 mês = 28 ou 29/fev). */
export function addMonthsISO(iso: string, n: number): string {
  const [y, m, d] = iso.split("-").map((s) => parseInt(s, 10));
  const base = new Date(Date.UTC(y ?? 1970, (m ?? 1) - 1 + n, 1));
  const daysInMonth = new Date(
    Date.UTC(base.getUTCFullYear(), base.getUTCMonth() + 1, 0),
  ).getUTCDate();
  base.setUTCDate(Math.min(d ?? 1, daysInMonth));
  const yy = base.getUTCFullYear();
  const mm = String(base.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(base.getUTCDate()).padStart(2, "0");
  return `${yy}-${mm}-${dd}`;
}
