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
import { errorText, safeErrorMessage } from "../../lib/errors";
import { syncRecencyLabel } from "../../lib/syncRecency";

export type { DriveConflictChoice, DriveConflictDetails, DriveConflictGesture };

/** Chave estável por conteúdo para as linhas da lista de gestos: o gesto não tem id próprio e
 *  dois gestos idênticos no mesmo segundo são possíveis — o sufixo de ocorrência desambigua sem
 *  depender do índice de render. `JSON.stringify` do array (nunca `join("|")`): `source_sheet` é
 *  dado do usuário e pode conter "|", o que colidiria duas linhas distintas na mesma chave. */
export function gestureKeys(gestures: DriveConflictGesture[]): string[] {
  const seen = new Map<string, number>();
  return gestures.map((gesture) => {
    const base = JSON.stringify([
      gesture.at,
      gesture.event_type,
      gesture.entity_type,
      gesture.source_sheet ?? "",
    ]);
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    return n === 0 ? base : `${base}|${n}`;
  });
}

export function fetchSnapshotConflictDetails(
  clientId: string,
  clientSecret?: string,
): Promise<DriveConflictDetails> {
  return driveConflictDetails(clientId, clientSecret);
}

export function resolveSnapshotConflictCmd(
  clientId: string,
  choice: DriveConflictChoice,
  seenRemoteSequence: number,
  clientSecret?: string,
): Promise<DriveConflictResolution> {
  return resolveDriveConflict(clientId, choice, seenRemoteSequence, clientSecret);
}

/** Prefixo estável do contrato de recusa do check-in (espelha `CHECKIN_REFUSED_PREFIX` em
 *  `src/screens/configView.ts` e `snapshot_cmds::CHECKIN_REFUSED_PULL/CONFLICT/STALE_CONFLICT`
 *  no Rust) — reconhecido por igualdade de PREFIXO, nunca por regex sobre a frase descritiva. */
export const CHECKIN_REFUSED_PREFIX = "Check-in recusado: ";

/** Mensagem exata do veredito de consentimento obsoleto (`snapshot_cmds::CHECKIN_REFUSED_STALE_CONFLICT`,
 *  Rust): o remoto avançou de novo entre a tela mostrar o conflito e o dono escolher — publicar
 *  (`keep_local`) ou restaurar (`use_remote`) por cima do que ele nunca viu seria a mesma
 *  sobrescrita silenciosa que o lease impede no check-in normal. A tela reconhece por igualdade
 *  exata e recarrega os detalhes em vez de travar num erro parado. */
export const CHECKIN_REFUSED_STALE_CONFLICT =
  "Check-in recusado: a disputa mudou de novo desde que você abriu esta tela — veja os " +
  "detalhes atualizados antes de escolher.";

/** Prefixo estável da recusa de restauração por schema mais novo (espelha o literal usado em
 *  `resolve_conflict_use_remote_core`, Rust) — verbatim atrás dele, nunca a frase inteira casada
 *  por igualdade (as versões de schema variam a cada par de aparelhos). */
export const RESTORE_REFUSED_PREFIX = "Restauração recusada: ";

/** Sufixo compartilhado por todo erro que `resolve_conflict_use_remote_core` (Rust) devolve
 *  DEPOIS de fechar o pool do banco ativo para trocar o arquivo — a partir desse ponto não há
 *  mais pool utilizável para uma nova tentativa, então a tela nunca oferece "tentar de novo"
 *  quando reconhece este sufixo, mesmo que a resolução tenha falhado (ver
 *  `AFTER_POOL_CLOSED_SUFFIX`, Rust). */
export const AFTER_POOL_CLOSED_SUFFIX = "; reinicie o app para continuar";

/** Um erro do gesto de resolução veio do backend depois do ponto de não-retorno (pool já
 *  fechado) — a única saída honesta é reiniciar o app, nunca reoferecer os botões de escolha. */
export function isAfterPoolClosedError(error: unknown): boolean {
  return errorText(error).trim().endsWith(AFTER_POOL_CLOSED_SUFFIX);
}

/** Mensagem verbatim só atrás de um prefixo de contrato conhecido (mesmo padrão de
 *  `driveCheckinErrorMessage`, `src/screens/configView.ts`, PR #439); qualquer outro erro (rede,
 *  banco ocupado, um `sqlx`/IO cru) cai no fallback calmo em vez de vazar a mensagem técnica. */
export function resolveConflictErrorMessage(error: unknown): string {
  const raw = errorText(error).trim();
  if (
    raw.startsWith(CHECKIN_REFUSED_PREFIX) ||
    raw.startsWith(RESTORE_REFUSED_PREFIX)
  ) {
    return raw;
  }
  return safeErrorMessage(error, "Não foi possível concluir a resolução do conflito.");
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
