#!/usr/bin/env node
// Anti-drift check para o client id da credencial Android (docs/building-android.md): o mesmo id
// aparece em 4 lugares (versionado, nunca em .env — ver .env.example) e um valor editado em só um
// deles quebraria o consentimento OU o deep link de retorno sem nenhum erro de compilação.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => readFileSync(join(root, relativePath), "utf8");

let hasError = false;
const fail = (message) => {
  console.error(message);
  hasError = true;
};

// 1. src/lib/env.ts — o client id "para a frente", como o Google emite.
const envTs = read("src/lib/env.ts");
const forwardMatch = envTs.match(/GOOGLE_ANDROID_CLIENT_ID\s*=\s*\n?\s*"([^"]+)"/);
if (!forwardMatch) {
  fail("src/lib/env.ts: GOOGLE_ANDROID_CLIENT_ID não encontrado.");
  process.exit(1);
}
const forwardId = forwardMatch[1];

// O redirect_uri custom-scheme do Google é o client id "de trás para frente": os segmentos
// separados por "." invertidos (ex.: "123-abc.apps.googleusercontent.com" vira
// "com.googleusercontent.apps.123-abc"). Derivar em vez de repetir o literal garante que o
// próprio check não precise ser editado se o client id rotacionar algum dia.
const reversedScheme = forwardId.split(".").reverse().join(".");

// 2. src-tauri/src/oauth/redirect.rs — o esquema reverso, fonte do redirect_uri do deep link.
const redirectRs = read("src-tauri/src/oauth/redirect.rs");
const schemeConstMatch = redirectRs.match(
  /ANDROID_OAUTH_SCHEME:\s*&str\s*=\s*\n?\s*"([^"]+)"/,
);
if (!schemeConstMatch) {
  fail("src-tauri/src/oauth/redirect.rs: ANDROID_OAUTH_SCHEME não encontrado.");
} else if (schemeConstMatch[1] !== reversedScheme) {
  fail(
    `src-tauri/src/oauth/redirect.rs: ANDROID_OAUTH_SCHEME é "${schemeConstMatch[1]}", ` +
      `esperado "${reversedScheme}" (o reverso de GOOGLE_ANDROID_CLIENT_ID em src/lib/env.ts).`,
  );
}

// 3. src-tauri/tauri.conf.json — o mesmo esquema, registrado para o plugin de deep link.
const tauriConf = JSON.parse(read("src-tauri/tauri.conf.json"));
const confScheme = tauriConf?.plugins?.["deep-link"]?.mobile?.[0]?.scheme?.[0];
if (confScheme !== reversedScheme) {
  fail(
    `src-tauri/tauri.conf.json: plugins.deep-link.mobile[0].scheme[0] é "${confScheme}", ` +
      `esperado "${reversedScheme}".`,
  );
}

// 4. AndroidManifest.xml gerado — o intent-filter que o SO usa para rotear o deep link ao app.
// Versionado (não regenerado neste checkout), então pode divergir do JSON por edição manual —
// exatamente o que este script existe para pegar.
const manifest = read("src-tauri/gen/android/app/src/main/AndroidManifest.xml");
const manifestSchemeMatch = manifest.match(/<data android:scheme="([^"]+)"/);
if (!manifestSchemeMatch) {
  fail(
    "src-tauri/gen/android/app/src/main/AndroidManifest.xml: <data android:scheme> não encontrado.",
  );
} else if (manifestSchemeMatch[1] !== reversedScheme) {
  fail(
    `src-tauri/gen/android/app/src/main/AndroidManifest.xml: <data android:scheme> é ` +
      `"${manifestSchemeMatch[1]}", esperado "${reversedScheme}".`,
  );
}

if (hasError) {
  console.error(
    "\nOs 4 lugares do client id Android (docs/building-android.md) divergiram — " +
      "corrija-os juntos, no mesmo commit.",
  );
  process.exit(1);
}

console.log(`client id Android em sincronia nos 4 lugares (${reversedScheme}).`);
