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
 * Client id da credencial Google de tipo Android — a política de OAuth do Google só aceita
 * redirect de esquema customizado (o client id reverso, `oauth::redirect::ANDROID_OAUTH_SCHEME`
 * no lado Rust) para esse tipo de credencial, nunca para a Desktop; ela valida o app pelo par
 * (pacote, SHA-1) registrado no Console (`docs/building-android.md`), sem client secret. É
 * identificador PÚBLICO, não um segredo — por isso entra fixo aqui em vez de em `.env`: uma
 * variável assada em build já produziu builds Android quebrados por ausência/erro de digitação.
 */
export const GOOGLE_ANDROID_CLIENT_ID =
  "50282483752-h53glgfl0laqe0t3rtqsj5a9sgc6b60g.apps.googleusercontent.com";

/**
 * Client id efetivamente usado no consentimento e em toda chamada que o repassa ao backend para
 * renovar o token. O Android tem que enviar o MESMO client id que emitiu o `code`/`refresh_token`
 * — misturar com o da credencial Desktop derruba a troca (`invalid_client`) ou o refresh
 * (`invalid_grant`) em segundo plano.
 */
export const GOOGLE_OAUTH_CLIENT_ID = isAndroid
  ? GOOGLE_ANDROID_CLIENT_ID
  : GOOGLE_CLIENT_ID;

/**
 * Chave da preferência de exibição do recibo, válida em todo o app. O nome persistido guarda
 * o prefixo da conversa, onde o recibo nasceu: renomeá-lo descartaria a escolha já gravada.
 */
export const SHOW_RECEIPT = "mia_show_receipt";
