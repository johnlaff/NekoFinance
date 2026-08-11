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
  type MiaConsentView,
} from "../lib/api";
import { syncRecencyLabel } from "../lib/syncRecency";

export type { AppInfo, AuthStatus, DriveCheckinInfo, MiaConsentView };

/** Mensagem exata devolvida pelo backend (`oauth::token_store::NEEDS_DRIVE_REAUTH`) quando o
 *  token não tem o escopo `drive.appdata` — casada por igualdade para levar ao fluxo de
 *  reconexão, nunca a um erro cru. Precisa ficar em sincronia com a constante Rust. */
export const NEEDS_DRIVE_REAUTH =
  "Re-autorize para habilitar o snapshot no Drive: sua conexão atual não tem esse escopo.";

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
): Promise<DriveCheckinInfo> {
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
