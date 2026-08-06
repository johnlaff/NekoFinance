import {
  backupDatabase,
  getAppInfo,
  getAppSetting,
  getFlagSetting,
  getMiaConsent,
  grantMiaConsent,
  lastSyncAt,
  registerOsReminder,
  revokeMiaConsent,
  setAppSetting,
  setMiaApiKey,
  unregisterOsReminder,
  type AppInfo,
  type AuthStatus,
  type MiaConsentView,
} from "../lib/api";

export type { AppInfo, AuthStatus, MiaConsentView };

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

// --- Leitura -----------------------------------------------------------------------------

export function fetchAppInfo(): Promise<AppInfo> {
  return getAppInfo();
}

export function fetchLastSyncAt(): Promise<string | null> {
  return lastSyncAt();
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
