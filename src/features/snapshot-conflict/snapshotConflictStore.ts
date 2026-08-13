/**
 * Store externo module-level (useSyncExternalStore) para abrir a tela de conflito do snapshot no
 * Drive a partir de qualquer lugar que reconheça a recusa `CHECKIN_REFUSED_CONFLICT` — hoje só
 * `DriveCheckinLine` (Configurações), mas o gatilho não precisa saber onde a tela é montada.
 * Mesmo padrão de `shell/crumbStore.ts`: identidade fixa das funções, sem depender de contexto
 * React nem de memoização.
 */

let open = false;
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

export function openSnapshotConflict(): void {
  if (open) return;
  open = true;
  notify();
}

export function closeSnapshotConflict(): void {
  if (!open) return;
  open = false;
  notify();
}

export function subscribeSnapshotConflict(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

export function snapshotConflictOpenSnapshot(): boolean {
  return open;
}
