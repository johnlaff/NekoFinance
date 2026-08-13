//! Porta do domínio Conflito do snapshot no Drive (ADR-0006/0015): `SnapshotConflictScreen.tsx`
//! importa só daqui, nunca de `lib/api` direto.

import {
  driveConflictDetails,
  resolveDriveConflict,
  type DriveConflictChoice,
  type DriveConflictDetails,
  type DriveConflictGesture,
  type DriveConflictResolution,
} from "../../lib/api";
import { syncRecencyLabel } from "../../lib/syncRecency";

export type { DriveConflictChoice, DriveConflictDetails, DriveConflictGesture };

export function fetchSnapshotConflictDetails(
  clientId: string,
  clientSecret?: string,
): Promise<DriveConflictDetails> {
  return driveConflictDetails(clientId, clientSecret);
}

export function resolveSnapshotConflictCmd(
  clientId: string,
  choice: DriveConflictChoice,
  clientSecret?: string,
): Promise<DriveConflictResolution> {
  return resolveDriveConflict(clientId, choice, clientSecret);
}

/** `event_type` conhecidos hoje (`sync_log` só é escrito pelo import/write-back da planilha) —
 *  qualquer outro valor cai no rótulo genérico em vez de travar a tela. */
const EVENT_TYPE_LABEL: Record<string, string> = {
  import: "Importação da planilha",
  write_back: "Escrita de volta na planilha",
};

/** Rótulo do TIPO do gesto, sem a data — "Importação da planilha (aba Diário)". */
export function conflictGestureTypeLabel(gesture: DriveConflictGesture): string {
  const base = EVENT_TYPE_LABEL[gesture.event_type] ?? `Gesto (${gesture.event_type})`;
  return gesture.source_sheet ? `${base} (aba ${gesture.source_sheet})` : base;
}

/** Rótulo legível e datado de um gesto: recência quando disponível, senão a data crua — nunca
 *  omite o "quando" só porque o formato do timestamp não bateu. */
export function conflictGestureDatedLabel(
  gesture: DriveConflictGesture,
  now?: number,
): string {
  const recency = syncRecencyLabel(gesture.at, now);
  const type = conflictGestureTypeLabel(gesture);
  return recency ? `${type} — ${recency}` : `${type} — ${gesture.at}`;
}

/** Identifica o outro aparelho pelos 8 primeiros caracteres do id — o suficiente para
 *  diferenciar sem expor o UUID inteiro numa linha de estado (mesmo padrão de
 *  `configView.driveCheckinLabel`). */
export function conflictRemoteDeviceLabel(
  manifest: DriveConflictDetails["remote_manifest"],
): string {
  return `outro aparelho (${manifest.device_id.slice(0, 8)})`;
}
