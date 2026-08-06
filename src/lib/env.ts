/**
 * Flags de ambiente resolvidas em tempo de build/carregamento. Não são contrato do backend
 * (não há `invoke` aqui) — por isso moram fora do funil de `lib/api` e são importáveis de
 * qualquer zona, incluindo as views (docs/adr/0006-lib-api-funnel-gate.md).
 */

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

/**
 * Chave da preferência de exibição do recibo, válida em todo o app. O nome persistido guarda
 * o prefixo da conversa, onde o recibo nasceu: renomeá-lo descartaria a escolha já gravada.
 */
export const SHOW_RECEIPT = "mia_show_receipt";
