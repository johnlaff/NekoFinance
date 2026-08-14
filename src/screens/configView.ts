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
 *  os testes, em vez de um literal hardcoded solto. O check-out roda sozinho na PRÓXIMA abertura
 *  do app (`snapshot::checkout`, Rust) — a copy pede esse gesto em vez de prometer um botão de
 *  "baixar agora" que esta tela não tem. */
export const CHECKIN_REFUSED_PULL =
  "Check-in recusado: outro aparelho publicou depois do seu último check-in — feche e abra " +
  "o app de novo para receber a versão dele antes de publicar.";

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

/**
 * Rótulo do último check-out (a leitura do snapshot remoto ao abrir o app): recência + de qual
 * aparelho veio. Espelha `driveCheckinLabel` no mesmo padrão ("deste aparelho" / "de outro
 * aparelho (nome)") comparando `last_checkout_device_id` a `this_device_id` — o árbitro puro
 * (`lease::decide`, Rust) ignora `device_id` por completo, então nada no veredito `Pull` garante
 * que o manifest seja de OUTRO aparelho; é a guarda de `checkout_on_open` (comparação contra
 * `pending_publish_sequence`, a sequência PRETENDIDA gravada antes do upload — ADR-0015, issue
 * #446) que evita registrar um check-out do PRÓPRIO snapshot. A tela não pode assumir essa
 * invariante silenciosamente e cravar "outro aparelho" quando os dois ids batem.
 */
export function driveCheckoutLabel(
  info: DriveCheckinInfo | null | undefined,
  now?: number,
): string {
  if (!info?.last_checkout_at) {
    return "Nenhuma leitura do Drive ainda.";
  }
  const recency = syncRecencyLabel(info.last_checkout_at, now);
  const isThisDevice = info.last_checkout_device_id === info.this_device_id;
  const device = isThisDevice
    ? "deste aparelho"
    : `de outro aparelho (${(info.last_checkout_device_id ?? "").slice(0, 8)})`;
  return recency ? `Última leitura ${recency}, ${device}.` : `Recebido ${device}.`;
}

/** Rótulo fechado que `snapshot_state.last_checkout_outcome` grava — os dois de uma tentativa
 *  real de restauração (`checkout::outcome_warning_fields`) e o da sonda leve de foco
 *  (`checkout::probe_newer_snapshot_on_focus`, ADR-0015). */
type CheckoutOutcomeTag = "refused_newer_schema" | "error" | "newer_available";

/**
 * Aviso calmo do desfecho do último check-out que merece a atenção do dono: a
 * recusa por schema remoto mais novo orienta a atualizar o app; a falha de rede/integridade diz
 * que a leitura não aconteceu e que o app tenta de novo sozinho na próxima abertura; e uma versão
 * mais nova detectada em foco (sem baixar/trocar arquivo mid-session) pede reabrir o app — nunca
 * um botão de "tentar agora"/"baixar agora" que esta tela não tem. `null` quando não há nada a
 * avisar (check-out em dia, restaurado com sucesso, ou nunca rodou).
 */
export function driveCheckoutOutcomeWarning(
  info: DriveCheckinInfo | null | undefined,
): string | null {
  const outcome = info?.last_checkout_outcome as CheckoutOutcomeTag | null | undefined;
  switch (outcome) {
    case "refused_newer_schema":
      return (
        "O snapshot mais recente foi publicado por uma versão mais nova do Neko Finance — " +
        "atualize o app para receber essa leitura."
      );
    case "error":
      return (
        "A última leitura do Drive não aconteceu — o app tenta de novo sozinho na próxima " +
        "abertura."
      );
    case "newer_available":
      return (
        "Uma versão mais nova está disponível no Drive — feche e abra o app de novo para " +
        "recebê-la."
      );
    default:
      return null;
  }
}

/**
 * Nota calma de "há mudanças locais ainda não publicadas" (ADR-0015): o app sobe
 * sozinho ao fechar ou depois de um gesto material, então isto nunca é um pedido de ação — é só o
 * estado honesto para o dono saber que precisa de rede antes de trocar de aparelho.
 */
export function driveUnpublishedChangesNote(
  info: DriveCheckinInfo | null | undefined,
): string | null {
  if (!info?.pending_local_changes) return null;
  return (
    "Há mudanças locais ainda não publicadas — o app publica sozinho ao fechar ou você " +
    "pode fazer o check-in agora."
  );
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
