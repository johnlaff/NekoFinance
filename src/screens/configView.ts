import {
  backupDatabase,
  driveCheckin,
  getAppInfo,
  getAppSetting,
  getFlagSetting,
  getMiaConsent,
  grantMiaConsent,
  lastDriveCheckin,
  lastSyncAt,
  registerOsReminder,
  revokeMiaConsent,
  setAppSetting,
  setMiaApiKey,
  unregisterOsReminder,
  type AppInfo,
  type AuthStatus,
  type DriveCheckinInfo,
  type DriveCheckinResult,
  type MiaConsentView,
} from "../lib/api";
import { errorText, safeErrorMessage } from "../lib/errors";
import { syncRecencyLabel } from "../lib/syncRecency";

export type {
  AppInfo,
  AuthStatus,
  DriveCheckinInfo,
  DriveCheckinResult,
  MiaConsentView,
};

/** Mensagem exata devolvida pelo backend (`oauth::token_store::NEEDS_DRIVE_REAUTH`) quando o
 *  token não tem o escopo `drive.appdata` — casada por igualdade para levar ao fluxo de
 *  reconexão, nunca a um erro cru. Precisa ficar em sincronia com a constante Rust. */
export const NEEDS_DRIVE_REAUTH =
  "Re-autorize para habilitar o snapshot no Drive: sua conexão atual não tem esse escopo.";

/** Copy calma para o veredito "em dia" do check-in — sucesso (ADR-0015), nunca erro: nada mudou
 *  desde a última publicação. */
export const DRIVE_CHECKIN_UP_TO_DATE_NOTE =
  "Já está em dia — nada novo para publicar.";

/** Prefixo estável do contrato de recusa do check-in — `snapshot_cmds::CHECKIN_REFUSED_PULL` e
 *  `CHECKIN_REFUSED_CONFLICT` (Rust) sempre começam por ele. A recusa é reconhecida por este
 *  prefixo ESTRUTURAL, nunca por regex sobre as palavras da frase descritiva que segue (que
 *  muda mais fácil que o contrato). Mudar este literal é mudança de contrato: atualize os dois
 *  lados juntos, no mesmo commit. */
export const CHECKIN_REFUSED_PREFIX = "Check-in recusado: ";

/** Mensagem exata do veredito `Pull` (`snapshot_cmds::CHECKIN_REFUSED_PULL`) — fonte única para
 *  os testes, em vez de um literal hardcoded solto. O app não oferece check-out/pull/restore do
 *  snapshot remoto, então a copy nunca instrui um gesto que a tela não tem como cumprir. */
export const CHECKIN_REFUSED_PULL =
  "Check-in recusado: outro aparelho publicou depois do seu último check-in, e a leitura " +
  "dessa versão ainda não chegou a este app — chega numa atualização futura.";

/** Mensagem exata do veredito `Conflict` (`snapshot_cmds::CHECKIN_REFUSED_CONFLICT`). Nunca diz
 *  "baixe" — aqui significaria descartar trabalho local sem aviso. */
export const CHECKIN_REFUSED_CONFLICT =
  "Check-in recusado: os dois lados mudaram desde o último ponto em comum entre os " +
  "aparelhos.";

export function driveCheckinErrorMessage(error: unknown): string {
  const raw = errorText(error).trim();
  if (raw.startsWith(CHECKIN_REFUSED_PREFIX)) return raw;
  return safeErrorMessage(error, "Não foi possível fazer o check-in.");
}

export type GreetTone = "ok" | "warn" | "muted";

/** Pílula de estado do veredito de Configurações. */
export interface GreetState {
  tone: GreetTone;
  /** Parte forte ("Conectado" / "Desconectado" / "Sessão expirada"). */
  headline: string;
  /** Complemento após o separador " · " (null → só a headline). */
  detail: string | null;
}

function pendingPhrase(n: number): string {
  return n === 1 ? "1 mudança aguardando" : `${n} mudanças aguardando`;
}

function conflictPhrase(n: number): string {
  return n === 1
    ? "Conflito de importação a resolver"
    : `${n} conflitos de importação a resolver`;
}

/**
 * Deriva a pílula de estado do greet a partir da conexão e das pendências.
 * O veredito diz a má notícia com o mesmo peso da boa: o pior fato disponível
 * ganha o complemento (conflito > mudanças aguardando > recência do sync), e o
 * ponto só fica verde quando não há nada bloqueando.
 */
export function greetState(
  auth: AuthStatus,
  pendingCount: number,
  conflictCount: number,
  syncLabel: string | null,
): GreetState {
  if (auth === "loading") {
    return { tone: "muted", headline: "Verificando conexão…", detail: null };
  }

  const headline =
    auth === "connected"
      ? "Conectado"
      : auth === "expired"
        ? "Sessão expirada"
        : "Desconectado";

  if (conflictCount > 0) {
    return { tone: "warn", headline, detail: conflictPhrase(conflictCount) };
  }
  if (pendingCount > 0) {
    return {
      tone: auth === "connected" ? "ok" : "warn",
      headline,
      detail: pendingPhrase(pendingCount),
    };
  }
  if (auth !== "connected") {
    return { tone: "warn", headline, detail: null };
  }
  return {
    tone: "ok",
    headline,
    detail: syncLabel ? `Sincronizado ${syncLabel}` : null,
  };
}

/**
 * Rótulo do último check-in do snapshot no Drive: recência + por qual aparelho.
 * "este aparelho" quando o check-in foi deste mesmo device_id; senão, os 8
 * primeiros caracteres do id do outro aparelho — o suficiente para diferenciar sem expor o UUID
 * inteiro numa linha de estado.
 */
export function driveCheckinLabel(
  info: DriveCheckinInfo | null | undefined,
  now?: number,
): string {
  if (!info?.last_checkin_at) {
    return "Nenhum check-in ainda — publique o primeiro snapshot.";
  }
  const recency = syncRecencyLabel(info.last_checkin_at, now);
  const isThisDevice = info.last_checkin_device_id === info.this_device_id;
  const device = isThisDevice
    ? "este aparelho"
    : `outro aparelho (${(info.last_checkin_device_id ?? "").slice(0, 8)})`;
  return recency
    ? `Último check-in ${recency}, por ${device}.`
    : `Publicado por ${device}.`;
}

// --- Leitura -----------------------------------------------------------------------------

export function fetchAppInfo(): Promise<AppInfo> {
  return getAppInfo();
}

export function fetchLastSyncAt(): Promise<string | null> {
  return lastSyncAt();
}

export function fetchLastDriveCheckin(): Promise<DriveCheckinInfo> {
  return lastDriveCheckin();
}

export function fetchMiaConsent(): Promise<MiaConsentView> {
  return getMiaConsent();
}

export function fetchShowReceiptFlag(key: string, fallback: boolean): Promise<boolean> {
  return getFlagSetting(key, fallback);
}

/** Preferência local do domínio Configurações (repassa o shim genérico sob o vocabulário da tela). */
export function fetchConfigSetting(key: string): Promise<string | null> {
  return getAppSetting(key);
}

// --- Escrita -------------------------------------------------------------------------------

export function backupDatabaseCmd(destPath: string): Promise<string> {
  return backupDatabase(destPath);
}

export function driveCheckinCmd(
  clientId: string,
  clientSecret?: string,
): Promise<DriveCheckinResult> {
  return driveCheckin(clientId, clientSecret);
}

export function grantMiaConsentCmd(): Promise<MiaConsentView> {
  return grantMiaConsent();
}

export function revokeMiaConsentCmd(): Promise<MiaConsentView> {
  return revokeMiaConsent();
}

export function setMiaApiKeyCmd(key: string): Promise<MiaConsentView> {
  return setMiaApiKey(key);
}

export function registerOsReminderCmd(timeHhmm: string): Promise<void> {
  return registerOsReminder(timeHhmm);
}

export function unregisterOsReminderCmd(): Promise<void> {
  return unregisterOsReminder();
}

/** Grava uma preferência local do domínio Configurações. */
export function setConfigSetting(key: string, value: string): Promise<void> {
  return setAppSetting(key, value);
}
