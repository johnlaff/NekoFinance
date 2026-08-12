import { platform } from "@tauri-apps/plugin-os";

/**
 * Flags de ambiente resolvidas em tempo de build/carregamento. Não são contrato do backend
 * (não há `invoke` aqui) — por isso moram fora do funil de `lib/api` e são importáveis de
 * qualquer zona, incluindo as views (docs/adr/0006-lib-api-funnel-gate.md).
 */

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * True no Android — a única leitura de plataforma que o front precisa: as superfícies sem
 * adapter Android (updater, lembrete no nível do SO) mostram estado honesto de
 * indisponibilidade em vez de silenciar como se a checagem tivesse rodado. `platform()`
 * (`@tauri-apps/plugin-os`) lê um valor injetado em `window` na inicialização do plugin — só
 * existe dentro do shell Tauri, daí o guard de `isTauri`. O plugin injeta o próprio global
 * (`__TAURI_OS_PLUGIN_INTERNALS__`) antes de chamar `platform()`: um shell parcial (Tauri sem
 * esse plugin pronto ainda) não deve derrubar o módulo inteiro no import.
 */
export const isAndroid =
  isTauri && "__TAURI_OS_PLUGIN_INTERNALS__" in window && platform() === "android";

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

/**
 * Chave da preferência de exibição do recibo, válida em todo o app. O nome persistido guarda
 * o prefixo da conversa, onde o recibo nasceu: renomeá-lo descartaria a escolha já gravada.
 */
export const SHOW_RECEIPT = "mia_show_receipt";
