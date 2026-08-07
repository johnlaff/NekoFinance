//! Porta do domínio Conciliação (gate de conflito de import): `ConflictGate.tsx` importa só
//! daqui — nunca de `lib/api`.

import {
  getImportConflicts,
  listenEvent,
  resolveImportConflict,
  SYNC_DONE_EVENT,
  type ImportConflict,
  type SyncDonePayload,
} from "../../lib/api";

export type { ImportConflict, SyncDonePayload };

// --- Leitura -----------------------------------------------------------------------------

export function fetchImportConflicts(): Promise<ImportConflict[]> {
  return getImportConflicts();
}

/** Assina o evento de sync em segundo plano; devolve a função de `unlisten`. */
export function listenSyncDone(
  onDone: (payload: SyncDonePayload) => void,
): Promise<() => void> {
  return listenEvent<SyncDonePayload>(SYNC_DONE_EVENT, onDone);
}

// --- Escrita -------------------------------------------------------------------------------

export function resolveImportConflictCmd(
  id: string,
  choice: "sheet" | "local",
): Promise<void> {
  return resolveImportConflict(id, choice);
}
